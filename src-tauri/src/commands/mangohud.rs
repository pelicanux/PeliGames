use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Duration};

#[derive(Serialize)]
pub struct MangoHudStatus {
    installed: bool,
    executable: Option<String>,
}

fn find_executable(path: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join("mangohud"))
        .find(|file| {
            let Ok(metadata) = file.metadata() else {
                return false;
            };
            if !metadata.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        })
}

#[tauri::command]
pub fn mangohud_status() -> MangoHudStatus {
    // Match the native launch wrapper's PATH; do not execute or install anything.
    let executable = find_executable(&std::env::var_os("PATH").unwrap_or_default());
    MangoHudStatus {
        installed: executable.is_some(),
        executable: executable.map(|p| p.to_string_lossy().into_owned()),
    }
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Serialize)]
pub struct MangoHudRelease {
    pub(super) version: String,
    pub(super) release_url: String,
    pub(super) download_url: String,
    pub(super) asset_name: String,
    pub(super) size: u64,
    pub(super) sha256: Option<String>,
}

fn stable_package(release: Release) -> Result<MangoHudRelease, String> {
    // Only plain numeric versions qualify, even if an upstream preview was incorrectly marked stable.
    let version = release.tag_name.trim_start_matches('v');
    let parts: Vec<_> = version.split('.').collect();
    if release.draft
        || release.prerelease
        || parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err("O GitHub não retornou uma versão estável do MangoHUD.".into());
    }
    let release_url = format!(
        "https://github.com/flightlessmango/MangoHud/releases/tag/{}",
        release.tag_name
    );
    if release.html_url != release_url {
        return Err("Endereço da versão do MangoHUD inválido.".into());
    }
    let prefix = format!("MangoHud-{version}.");
    let exact = format!("MangoHud-{version}.tar.gz");
    let url_prefix = format!(
        "https://github.com/flightlessmango/MangoHud/releases/download/{}/",
        release.tag_name
    );
    let asset = release
        .assets
        .into_iter()
        .find(|a| {
            (a.name == exact || a.name.starts_with(&prefix))
                && a.name.ends_with(".tar.gz")
                && !["source", "beta", "alpha", "test", "debug", "experimental"]
                    .iter()
                    .any(|word| a.name.to_lowercase().contains(word))
                && a.name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b))
                && a.browser_download_url == format!("{url_prefix}{}", a.name)
                && a.size > 0
        })
        .ok_or("Não foi encontrado um pacote oficial estável do MangoHUD.")?;
    Ok(MangoHudRelease {
        version: release.tag_name,
        release_url,
        download_url: asset.browser_download_url,
        asset_name: asset.name,
        size: asset.size,
        sha256: asset
            .digest
            .and_then(|d| d.strip_prefix("sha256:").map(str::to_owned))
            .filter(|d| d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(|d| d.to_lowercase()),
    })
}

#[tauri::command]
pub async fn mangohud_latest_release() -> Result<MangoHudRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("PeliGames/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Não foi possível consultar o GitHub.".to_string())?;
    let response = client
        .get("https://api.github.com/repos/flightlessmango/MangoHud/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| {
            "Não foi possível consultar o GitHub. Verifique a conexão e tente novamente."
                .to_string()
        })?;
    if response.status().as_u16() == 403 || response.status().as_u16() == 429 {
        return Err(
            "O GitHub limitou as consultas. Aguarde alguns minutos e tente novamente.".into(),
        );
    }
    if !response.status().is_success() {
        return Err("Não foi possível buscar a versão estável do MangoHUD no GitHub.".into());
    }
    // Bound metadata even for chunked responses.
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Falha ao receber os dados do GitHub.".to_string())?
    {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("Resposta do GitHub maior que o esperado.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    stable_package(
        serde_json::from_slice(&bytes).map_err(|_| "Resposta inválida do GitHub.".to_string())?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release() -> Release {
        serde_json::from_value(serde_json::json!({"tag_name":"v0.8.4","html_url":"https://github.com/flightlessmango/MangoHud/releases/tag/v0.8.4","draft":false,"prerelease":false,"assets":[{"name":"MangoHud-v0.8.4-Source.tar.xz","browser_download_url":"https://example.org/source","size":100},{"name":"MangoHud-0.8.4.r0.g992103e.tar.gz","browser_download_url":"https://github.com/flightlessmango/MangoHud/releases/download/v0.8.4/MangoHud-0.8.4.r0.g992103e.tar.gz","size":100}]})).unwrap()
    }
    #[test]
    fn accepts_only_official_binary() {
        assert_eq!(
            stable_package(release()).unwrap().asset_name,
            "MangoHud-0.8.4.r0.g992103e.tar.gz"
        );
        let mut r = release();
        r.assets[1].browser_download_url = "https://example.org/binary".into();
        assert!(stable_package(r).is_err());
    }
    #[test]
    fn rejects_preview_and_source_only() {
        let mut r = release();
        r.prerelease = true;
        assert!(stable_package(r).is_err());
        let mut r = release();
        r.draft = true;
        assert!(stable_package(r).is_err());
        let mut r = release();
        r.tag_name = "v0.9.0-beta".into();
        assert!(stable_package(r).is_err());
        let mut r = release();
        r.assets.pop();
        assert!(stable_package(r).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn requires_executable_file() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("peligames-mangohud-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("mangohud");
        std::fs::write(&file, b"test").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(find_executable(dir.as_os_str()).is_none());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(find_executable(dir.as_os_str()), Some(file));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

use crate::core::mangohud_config::{self, Profile, Settings};
use std::{
    process::{Child, Command, Stdio},
    sync::Mutex,
};
static PREVIEW: Mutex<Option<Child>> = Mutex::new(None);
#[derive(Serialize)]
pub struct PreviewStatus {
    running: bool,
    program: String,
}
#[tauri::command]
pub fn load_mangohud_profile() -> Result<Profile, String> {
    mangohud_config::load()
}
#[tauri::command]
pub fn save_mangohud_profile(settings: Settings) -> Result<String, String> {
    mangohud_config::write(&settings, false)
}
#[tauri::command]
pub fn update_mangohud_preview(settings: Settings) -> Result<(), String> {
    mangohud_config::write(&settings, true).map(|_| ())
}
fn tool(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|d| d.is_absolute())
        .map(|d| d.join(name))
        .find(|p| {
            use std::os::unix::fs::PermissionsExt;
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}
pub fn stop_preview() {
    if let Ok(mut guard) = PREVIEW.lock() {
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
#[tauri::command]
pub async fn mangohud_preview(
    action: String,
    settings: Option<Settings>,
) -> Result<PreviewStatus, String> {
    tauri::async_runtime::spawn_blocking(move||{
        if action=="stop"{stop_preview();return Ok(PreviewStatus{running:false,program:"vkcube".into()});}
        let mut guard=PREVIEW.lock().map_err(|_|"Preview ocupada.")?;
        if let Some(child)=guard.as_mut(){if child.try_wait().map_err(|e|e.to_string())?.is_some(){*guard=None;}}
        match action.as_str(){
            "status"=>Ok(PreviewStatus{running:guard.is_some(),program:"vkcube".into()}),
            "start"=>{
                let settings=settings.ok_or("Configuração da preview ausente.")?;
                let config=mangohud_config::write(&settings,true)?;
                if guard.is_none(){
                    let hud=tool("mangohud").ok_or("MangoHUD não foi detectado. Instale-o antes de abrir a preview.")?;
                    let demo=tool("vkcube").ok_or("A preview precisa do vkcube. Instale o pacote de ferramentas Vulkan da sua distribuição (geralmente vulkan-tools).")?;
                    let log=std::fs::File::create(mangohud_config::directory()?.join("preview.log")).map_err(|e|e.to_string())?;
                    let mut child=Command::new(hud).arg(demo).env_remove("MANGOHUD_CONFIG").env_remove("MANGOHUD_PRESETSFILE")
                        .env("MANGOHUD_CONFIGFILE",config).stdin(Stdio::null()).stderr(log.try_clone().map_err(|e|e.to_string())?).stdout(log).spawn().map_err(|_|"Não foi possível abrir a preview Vulkan.")?;
                    std::thread::sleep(Duration::from_millis(300));
                    if let Some(status)=child.try_wait().map_err(|e|e.to_string())?{return Err(format!("A preview Vulkan encerrou ({status}). Verifique o driver Vulkan e o MangoHUD. O diagnóstico está em MangoHUD/preview.log."));}
                    *guard=Some(child);
                }
                Ok(PreviewStatus{running:true,program:"vkcube".into()})
            },
            _=>Err("Ação de preview inválida.".into())
        }
    }).await.map_err(|e|e.to_string())?
}
