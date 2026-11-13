//! Component lifecycle. The existing rdna4 path is retained; no user files are moved.
use crate::{
    backend_package::{self, UpdateStage},
    backend_source,
    neural_models::{self, ORIGINAL},
};
use peligames_addon_kit::{files, protocol::TransferProgress};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
const LIMIT: u64 = 512 * 1024 * 1024;
pub type Progress = Arc<dyn Fn(TransferProgress) + Send + Sync>;
pub fn directory(root: &Path) -> Result<PathBuf, String> {
    files::check_directory(root)?;
    Ok(root.join("rdna4"))
}
pub fn ready(path: &Path) -> bool {
    path.join("install.sh").is_file()
        && path.join("model-tools/extract_model.sh").is_file()
        && path.join("dlssnr-amd").is_dir()
}
pub fn component(root: &Path, bits: &str) -> Result<PathBuf, String> {
    if !matches!(bits, "32" | "64") {
        return Err("Invalid component architecture".into());
    }
    let dir = directory(root)?.join(bits);
    files::check_directory(&dir)?;
    if !ready(&dir) {
        return Err("Download or import the complete Linux backend first".into());
    }
    Ok(dir)
}
#[derive(Serialize)]
pub struct State {
    pub directory: String,
    pub current: Option<String>,
    pub ready: bool,
    pub models: bool,
    pub architectures: Vec<String>,
}
pub fn state(root: &Path) -> Result<State, String> {
    let dir = directory(root)?;
    let architectures: Vec<_> = ["64", "32"]
        .into_iter()
        .filter(|bits| ready(&dir.join(bits)))
        .map(str::to_string)
        .collect();
    let models = architectures
        .iter()
        .any(|bits| neural_models::find(&dir.join(bits).join("dlssnr-amd"), ORIGINAL).is_some());
    Ok(State {
        directory: dir.to_string_lossy().into(),
        current: fs::read_to_string(dir.join("version.txt"))
            .ok()
            .map(|s| s.trim().to_owned()),
        ready: !architectures.is_empty(),
        architectures,
        models,
    })
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("PeliGames-DLSSNR-addon")
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())
}
pub async fn latest() -> Result<String, String> {
    Ok(backend_source::linux_release(&client()?).await?.tag_name)
}
pub async fn download(root: PathBuf, progress: Progress) -> Result<State, String> {
    let directory = directory(&root)?;
    files::safe_directory(&root, true)?;
    let _lock = peligames_mod_config::DirectoryLock::acquire(&root)?;
    let client = client()?;
    let release = backend_source::linux_release(&client).await?;
    let stage = UpdateStage::new(&root, "rdna4")?;
    let mut downloaded = 0;
    for (asset, bits) in release.linux_assets() {
        progress(TransferProgress {
            phase: "connecting",
            downloaded,
            total: None,
        });
        let mut response = client
            .get(&asset.url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let checksum = asset.sha256.clone().or_else(|| {
            response
                .headers()
                .get("x-checksum-sha256")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        });
        let total = response.content_length();
        if total.is_some_and(|n| n > LIMIT) {
            return Err("Backend download exceeds size limit".into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
            if bytes.len() as u64 + chunk.len() as u64 > LIMIT {
                return Err("Backend download exceeds size limit".into());
            }
            bytes.extend_from_slice(&chunk);
            downloaded += chunk.len() as u64;
            progress(TransferProgress {
                phase: "downloading",
                downloaded: bytes.len() as u64,
                total,
            });
        }
        backend_package::verify_download(&bytes, checksum.as_deref())?;
        stage.unpack(&asset.name, bits, &bytes)?;
    }
    progress(TransferProgress {
        phase: "installing",
        downloaded,
        total: None,
    });
    stage.publish(&directory, &format!("{}-rdna4", release.tag_name), || false)?;
    state(&root)
}
pub fn import(root: &Path, source: &Path) -> Result<State, String> {
    if !source.is_absolute() || !source.is_dir() {
        return Err("Choose the folder containing Linux 32/64 packages".into());
    }
    files::check_directory(source)?;
    files::safe_directory(root, true)?;
    let _lock = peligames_mod_config::DirectoryLock::acquire(root)?;
    let stage = UpdateStage::new(root, "rdna4")?;
    // Feed a directory import through the same transaction as downloaded archives.
    stage.import_directory(source)?;
    stage.publish(&directory(root)?, "local-copy-rdna4", || false)?;
    state(root)
}
pub fn remove(root: &Path) -> Result<State, String> {
    let dir = directory(root)?;
    files::ensure_unmapped(std::slice::from_ref(&dir))?;
    if dir.exists() {
        files::safe_directory(&dir, false)?;
        let _lock = peligames_mod_config::DirectoryLock::acquire(root)?;
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    state(root)
}
pub fn import_models(root: &Path, source: &Path) -> Result<State, String> {
    let first = ["64", "32"]
        .into_iter()
        .find_map(|bits| component(root, bits).ok())
        .ok_or("Download the backend before importing models")?;
    let _lock = peligames_mod_config::DirectoryLock::acquire(&directory(root)?)?;
    neural_models::stage(source, &first.join("dlssnr-amd"))?;
    for bits in ["64", "32"] {
        if let Ok(other) = component(root, bits) {
            if other != first {
                neural_models::cache(&first.join("dlssnr-amd"), &other.join("dlssnr-amd"))?;
            }
        }
    }
    state(root)
}
pub fn extract(root: &Path, dll: &Path) -> Result<State, String> {
    if !dll.is_absolute() || !dll.is_file() {
        return Err("Choose the NVIDIA model DLL or ZIP".into());
    }
    let first = ["64", "32"]
        .into_iter()
        .find_map(|bits| component(root, bits).ok())
        .ok_or("Download the backend before extracting models")?;
    let _lock = peligames_mod_config::DirectoryLock::acquire(&directory(root)?)?;
    let game = TemporaryGame::new()?;
    crate::script_runner::execute_standalone_install(
        &first,
        &game.0.to_string_lossy(),
        "reshade-feeder",
        Some(&dll.to_string_lossy().into_owned()),
        None,
        "Insert",
        Default::default(),
    )?;
    for bits in ["64", "32"] {
        if let Ok(other) = component(root, bits) {
            if other != first {
                neural_models::cache(&first.join("dlssnr-amd"), &other.join("dlssnr-amd"))?;
            }
        }
    }
    state(root)
}
struct TemporaryGame(PathBuf);
impl TemporaryGame {
    fn new() -> Result<Self, String> {
        let dir = std::env::temp_dir().join(format!(
            "peligames-nr-model-extraction-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&dir).map_err(|e| e.to_string())?;
        Ok(Self(dir))
    }
}
impl Drop for TemporaryGame {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_import_is_transactional_and_download_removal_keeps_game_copies() {
        let dir =
            std::env::temp_dir().join(format!("peligames-mochi-runtime-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let source = dir.join("source");
        fs::create_dir_all(source.join("64/model-tools")).unwrap();
        fs::create_dir_all(source.join("64/dlssnr-amd")).unwrap();
        fs::write(source.join("64/install.sh"), "fixture").unwrap();
        fs::write(source.join("64/model-tools/extract_model.sh"), "fixture").unwrap();
        let root = dir.join("managed");
        assert!(import(&root, &source).unwrap().ready);
        let broken = dir.join("broken");
        fs::create_dir(&broken).unwrap();
        assert!(import(&root, &broken).is_err());
        assert!(state(&root).unwrap().ready);
        let game = dir.join("game");
        fs::create_dir(&game).unwrap();
        fs::write(game.join("dxgi.dll"), "keep").unwrap();
        assert!(!remove(&root).unwrap().ready);
        assert_eq!(fs::read_to_string(game.join("dxgi.dll")).unwrap(), "keep");
        fs::remove_dir_all(dir).unwrap();
    }
}
