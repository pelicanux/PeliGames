use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::Emitter;
const MAX_SIZE: u64 = 256 * 1024 * 1024;
static OPERATION: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut op) = OPERATION.lock() {
            *op = None;
        }
    }
}
fn begin() -> Result<(Guard, Arc<AtomicBool>), String> {
    let mut op = OPERATION.lock().map_err(|_| "Operação ocupada.")?;
    if op.is_some() {
        return Err("O MangoHUD já está sendo baixado ou instalado.".into());
    }
    let flag = Arc::new(AtomicBool::new(false));
    *op = Some(flag.clone());
    Ok((Guard, flag))
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Ready {
    version: String,
    sha256: String,
    size: u64,
}
#[derive(Clone, Serialize)]
struct Progress {
    phase: &'static str,
    downloaded: u64,
    total: u64,
    speed_bytes_per_sec: f64,
}
fn progress(app: &tauri::AppHandle, phase: &'static str, downloaded: u64, total: u64, speed: f64) {
    let _ = app.emit(
        "mangohud-download-progress",
        Progress {
            phase,
            downloaded,
            total,
            speed_bytes_per_sec: speed,
        },
    );
}
fn directory() -> Result<PathBuf, String> {
    let dir = crate::core::mangohud_config::directory()?.join("downloads");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if fs::symlink_metadata(&dir)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Pasta de downloads inválida.".into());
    }
    Ok(dir)
}
fn valid_ready(r: &Ready) -> bool {
    r.sha256.len() == 64
        && r.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        && r.size > 0
        && r.size <= MAX_SIZE
        && r.version.len() < 40
}
fn path(r: &Ready) -> Result<PathBuf, String> {
    if !valid_ready(r) {
        return Err("Pacote MangoHUD inválido.".into());
    }
    Ok(directory()?.join(format!("{}.tar.gz", r.sha256)))
}
#[tauri::command]
pub fn mangohud_downloaded() -> Result<Option<Ready>, String> {
    let manifest = directory()?.join("ready.json");
    let Ok(bytes) = fs::read(manifest) else {
        return Ok(None);
    };
    if bytes.len() > 4096 {
        return Ok(None);
    }
    let Ok(r) = serde_json::from_slice::<Ready>(&bytes) else {
        return Ok(None);
    };
    if !valid_ready(&r) {
        return Ok(None);
    }
    let file = path(&r)?;
    Ok(fs::symlink_metadata(file)
        .ok()
        .filter(|m| m.is_file() && !m.file_type().is_symlink() && m.len() == r.size)
        .map(|_| r))
}
fn redirect_allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}
struct Partial(PathBuf);
impl Drop for Partial {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
#[tauri::command]
pub async fn download_mangohud(app: tauri::AppHandle, version: String) -> Result<Ready, String> {
    let (_guard, cancel) = begin()?;
    if std::env::consts::ARCH != "x86_64" {
        return Err("O pacote oficial disponível requer um sistema x86_64.".into());
    }
    progress(&app, "checking", 0, 0, 0.);
    let release = super::mangohud::mangohud_latest_release().await?;
    if release.version != version {
        return Err("A versão estável mudou. Consulte a versão mais recente novamente.".into());
    }
    let digest = release.sha256.ok_or(
        "O GitHub não forneceu o checksum do pacote. Não é possível instalar com segurança.",
    )?;
    let ready = Ready {
        version: release.version,
        sha256: digest,
        size: release.size,
    };
    if !valid_ready(&ready) {
        return Err("Tamanho do pacote inválido.".into());
    }
    let staging = Partial(directory()?.join(format!(".{}.part", uuid::Uuid::new_v4())));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging.0)
        .map_err(|e| e.to_string())?;
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(1800))
        .user_agent(concat!("PeliGames/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && redirect_allowed(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("Redirecionamento não permitido")
            }
        }))
        .build()
        .map_err(|e| e.to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Download cancelado.".into());
    }
    let mut response = client
        .get(release.download_url)
        .send()
        .await
        .map_err(|_| "Falha ao baixar MangoHUD. Verifique a conexão.")?
        .error_for_status()
        .map_err(|_| "O GitHub recusou o download do pacote.")?;
    let mut hash = Sha256::new();
    let mut downloaded = 0;
    let start = Instant::now();
    let mut last = Instant::now();
    progress(&app, "downloading", 0, ready.size, 0.);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "O download foi interrompido. Tente novamente.")?
    {
        if cancel.load(Ordering::Relaxed) {
            return Err("Download cancelado.".into());
        }
        downloaded += chunk.len() as u64;
        if downloaded > ready.size {
            return Err("Tamanho do pacote diferente do esperado.".into());
        }
        hash.update(&chunk);
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        if last.elapsed() >= Duration::from_millis(100) {
            progress(
                &app,
                "downloading",
                downloaded,
                ready.size,
                downloaded as f64 / start.elapsed().as_secs_f64().max(0.001),
            );
            last = Instant::now();
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("Download cancelado.".into());
    }
    progress(&app, "verifying", downloaded, ready.size, 0.);
    if downloaded != ready.size || format!("{:x}", hash.finalize()) != ready.sha256 {
        return Err("A integridade do pacote não pôde ser confirmada. Baixe novamente.".into());
    }
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(&staging.0, path(&ready)?).map_err(|e| e.to_string())?;
    let manifest = directory()?.join(format!(".ready-{}.json", uuid::Uuid::new_v4()));
    fs::write(
        &manifest,
        serde_json::to_vec(&ready).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(manifest, directory()?.join("ready.json")).map_err(|e| e.to_string())?;
    progress(&app, "complete", downloaded, ready.size, 0.);
    Ok(ready)
}
#[tauri::command]
pub fn cancel_mangohud_download() -> Result<(), String> {
    if let Some(flag) = OPERATION.lock().map_err(|_| "Operação ocupada.")?.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
#[tauri::command]
pub async fn install_mangohud(app: tauri::AppHandle) -> Result<(), String> {
    let (_guard, _) = begin()?;
    let ready = mangohud_downloaded()?.ok_or("Baixe o MangoHUD antes de instalar.")?;
    // Never trust a checksum read from a user-writable manifest for root execution.
    let official = super::mangohud::mangohud_latest_release().await?;
    if official.version != ready.version
        || official.sha256.as_deref() != Some(&ready.sha256)
        || official.size != ready.size
    {
        return Err("O pacote baixado não corresponde à versão estável atual. Busque e baixe a versão novamente.".into());
    }
    let file = path(&ready)?;
    progress(&app, "authorizing", 0, 0, 0.);
    tauri::async_runtime::spawn_blocking(move||{
  if !std::path::Path::new("/usr/bin/pkexec").is_file(){return Err("Instale o Polkit para autorizar a instalação do MangoHUD.".into());}
  let log=fs::File::create(crate::core::mangohud_config::directory()?.join("installation.log")).map_err(|e|e.to_string())?;
  progress(&app,"installing",0,0,0.);
  let status=std::process::Command::new("/usr/bin/pkexec").args(["/usr/bin/python3","-I","-c",include_str!("mangohud_install_helper.py")]).arg(file).arg(ready.sha256).arg(ready.size.to_string())
    .stdin(std::process::Stdio::null()).stdout(log.try_clone().map_err(|e|e.to_string())?).stderr(log).status().map_err(|_|"Não foi possível solicitar autorização de administrador.")?;
  match status.code(){Some(0)=>{progress(&app,"installed",0,0,0.);Ok(())},Some(126)=>Err("A autorização foi cancelada. O pacote continua disponível para instalar.".into()),Some(127)=>Err("A autorização de administrador falhou. Verifique o Polkit.".into()),_=>Err("A instalação do MangoHUD falhou. Consulte MangoHUD/installation.log e tente novamente.".into())}
 }).await.map_err(|e|e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_untrusted_redirects() {
        for url in [
            "http://github.com/a",
            "https://evil.example/a",
            "https://github.com:8443/a",
        ] {
            assert!(!redirect_allowed(&url.parse().unwrap()));
        }
        assert!(redirect_allowed(
            &"https://release-assets.githubusercontent.com/a"
                .parse()
                .unwrap()
        ));
    }
    #[test]
    fn rejects_bad_cached_metadata() {
        assert!(!valid_ready(&Ready {
            version: "v1.0.0".into(),
            sha256: "../../x".into(),
            size: 1
        }));
        assert!(!valid_ready(&Ready {
            version: "v1.0.0".into(),
            sha256: "a".repeat(64),
            size: MAX_SIZE + 1
        }));
    }
}
