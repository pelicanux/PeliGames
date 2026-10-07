//! Official portable UMU release installation, shared by desktop and preview.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(unix)]
use std::os::unix::{fs::PermissionsExt, io::AsRawFd};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const REPO: &str = "Open-Wine-Components/umu-launcher";
const MAX_PACKAGE: u64 = 16 * 1024 * 1024;
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Installed {
    version: String,
    source: String,
    sha256: String,
}
fn safe_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() < 80
        && ![".", ".."].contains(&value)
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}
pub fn runners_dir() -> Result<PathBuf, String> {
    Ok(super::paths::app_root()?.join("runners/umu"))
}
fn cached(base: &Path) -> Option<PathBuf> {
    let installed: Installed =
        serde_json::from_slice(&fs::read(base.join("current.json")).ok()?).ok()?;
    if !safe_version(&installed.version) {
        return None;
    }
    let path = base.join(&installed.version).join("umu/umu-run");
    let bytes = fs::read(&path).ok()?;
    if format!("{:x}", Sha256::digest(&bytes)) != installed.sha256 {
        return None;
    }
    #[cfg(unix)]
    if fs::metadata(&path).ok()?.permissions().mode() & 0o111 == 0 {
        return None;
    }
    Some(path)
}
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
// Preserve the zipapp unchanged (including its bundled license notices). The upstream
// umu_run.py symlink is only an alias and is not needed by our direct invocation.
fn extract(bytes: &[u8], dest: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut total = 0;
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if path == Path::new("umu/umu_run.py")
            && entry.header().entry_type().is_symlink()
            && entry.link_name().map_err(|e| e.to_string())?.as_deref()
                == Some(Path::new("umu-run"))
        {
            continue;
        }
        if path != Path::new("umu") && path != Path::new("umu/umu-run") {
            return Err("Caminho inesperado no pacote UMU.".into());
        }
        if !(entry.header().entry_type().is_file() || entry.header().entry_type().is_dir()) {
            return Err("Tipo de arquivo inválido no pacote UMU.".into());
        }
        total += entry.size();
        if total > MAX_PACKAGE {
            return Err("Pacote UMU excede o limite de tamanho.".into());
        }
        if !entry.unpack_in(dest).map_err(|e| e.to_string())? {
            return Err("Caminho inválido no pacote UMU.".into());
        }
    }
    let run = dest.join("umu/umu-run");
    let mut signature = [0; 26];
    let count = fs::File::open(&run)
        .map_err(|_| "Pacote UMU não contém umu-run.")?
        .read(&mut signature)
        .map_err(|e| e.to_string())?;
    if !signature[..count].starts_with(b"#!/usr/bin/env python3\nPK") {
        return Err("Executável portátil UMU inválido.".into());
    }
    #[cfg(unix)]
    fs::set_permissions(&run, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    Ok(())
}
async fn download(base: &Path) -> Result<PathBuf, String> {
    let client = reqwest::Client::builder()
        .https_only(true)
        .user_agent("PeliGames/0.0.1")
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let release: Release = client
        .get(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        ))
        .send()
        .await
        .map_err(|e| format!("Não foi possível consultar o UMU: {e}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if !safe_version(&release.tag_name) {
        return Err("Versão UMU inválida.".into());
    }
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == format!("umu-launcher-{}-zipapp.tar", release.tag_name))
        .ok_or("A versão oficial não oferece o pacote portátil UMU esperado.")?;
    if asset.size == 0
        || asset.size > MAX_PACKAGE
        || !asset
            .browser_download_url
            .starts_with(&format!("https://github.com/{REPO}/releases/download/"))
    {
        return Err("Download UMU inválido.".into());
    }
    let expected = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
        .filter(|d| d.len() == 64 && d.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or("O pacote UMU não fornece checksum SHA-256.")?;
    let mut response = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|e| format!("Falha ao baixar UMU: {e}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > MAX_PACKAGE as usize {
            return Err("Download UMU excede o limite.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.len() as u64 != asset.size
        || format!("{:x}", Sha256::digest(&bytes)) != expected.to_lowercase()
    {
        return Err("Verificação do download UMU falhou.".into());
    }
    let staging = Staging(base.join(format!(".download-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&staging.0).map_err(|e| e.to_string())?;
    extract(&bytes, &staging.0)?;
    let executable_hash = format!(
        "{:x}",
        Sha256::digest(fs::read(staging.0.join("umu/umu-run")).map_err(|e| e.to_string())?)
    );
    let dest = base.join(&release.tag_name);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    fs::rename(&staging.0, &dest).map_err(|e| e.to_string())?;
    let manifest = base.join(format!(".current-{}.json", uuid::Uuid::new_v4()));
    fs::write(
        &manifest,
        serde_json::to_vec_pretty(&Installed {
            version: release.tag_name,
            source: asset.browser_download_url.clone(),
            sha256: executable_hash,
        })
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(&manifest, base.join("current.json")).map_err(|e| e.to_string())?;
    cached(base).ok_or("Instalação UMU não pôde ser validada.".into())
}
pub fn ensure_at(base: &Path) -> Result<PathBuf, String> {
    let python = std::process::Command::new("python3")
        .args([
            "-c",
            "import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)",
        ])
        .status()
        .map_err(|_| {
            "O UMU portátil precisa do Python 3.10 ou mais recente instalado no sistema."
        })?;
    if !python.success() {
        return Err(
            "O UMU portátil precisa do Python 3.10 ou mais recente instalado no sistema.".into(),
        );
    }
    if let Some(path) = cached(base) {
        return Ok(path);
    }
    fs::create_dir_all(base).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(base.join(".install.lock"))
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        let start = Instant::now();
        loop {
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                break;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::WouldBlock {
                return Err(format!(
                    "Não foi possível bloquear a instalação UMU: {error}"
                ));
            }
            if start.elapsed() > Duration::from_secs(180) {
                return Err("Outra instalação do UMU ainda está em andamento.".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    if let Some(path) = cached(base) {
        return Ok(path);
    }
    let owned = base.to_owned();
    // Network runtime runs on a separate thread so synchronous and async callers
    // (Tauri, Wine tools, browser service) share this resolver safely.
    let result = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?
            .block_on(download(&owned))
    })
    .join()
    .map_err(|_| "Falha ao preparar download do UMU.")?;
    drop(lock);
    result
}
pub fn ensure() -> Result<PathBuf, String> {
    ensure_at(&runners_dir()?)
}
