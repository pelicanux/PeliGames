use crate::{
    management,
    releases::{self, Release},
    runtime,
};
use peligames_addon_kit::protocol::TransferProgress;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
pub type Progress = Arc<dyn Fn(TransferProgress) + Send + Sync>;
struct Stage(PathBuf);
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn flatten(path: &Path) -> Result<(), String> {
    let entries = fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if let [entry] = entries.as_slice() {
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            for child in fs::read_dir(entry.path()).map_err(|e| e.to_string())? {
                let child = child.map_err(|e| e.to_string())?;
                fs::rename(child.path(), path.join(child.file_name()))
                    .map_err(|e| e.to_string())?;
            }
            fs::remove_dir(entry.path()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
pub fn install_bytes(root: &Path, release: &Release, bytes: &[u8]) -> Result<(), String> {
    peligames_addon_kit::files::check_directory(root)?;
    releases::verify(release, bytes)?;
    if !releases::compatible(&release.version) {
        return Err("Unsupported AFMF release".into());
    }
    let base = root.join("afmf");
    peligames_addon_kit::files::check_directory(&base)?;
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let _lock = peligames_mod_config::DirectoryLock::acquire(&base)?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stage = Stage(base.join(format!(
        "install-stage-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    fs::create_dir(&stage.0).map_err(|e| e.to_string())?;
    peligames_archive::unpack_tar_xz(bytes, &stage.0)?;
    flatten(&stage.0)?;
    runtime::validate(&stage.0)?;
    let destination = runtime::location(root, &release.version)?;
    if destination.exists() {
        runtime::validate(&destination)
            .map_err(|_| "Remove the damaged AFMF version before reinstalling it")?;
    } else {
        fs::create_dir_all(destination.parent().ok_or("Invalid runtime path")?)
            .map_err(|e| e.to_string())?;
        fs::rename(&stage.0, &destination).map_err(|e| e.to_string())?;
    }
    management::publish(root, release)
}
pub async fn install(
    root: PathBuf,
    version: Option<String>,
    progress: Progress,
) -> Result<management::State, String> {
    peligames_addon_kit::files::check_directory(&root)?;
    let latest = releases::latest(&root);
    let release = if version.as_deref() == Some(runtime::RELEASE.0) {
        releases::builtin()
    } else {
        latest
    };
    if version
        .as_ref()
        .is_some_and(|version| *version != release.version)
    {
        return Err("Refresh the AFMF release catalog before downloading this version".into());
    }
    if !releases::valid(&release) || !releases::compatible(&release.version) {
        return Err("Unsupported AFMF release".into());
    }
    progress(TransferProgress {
        phase: "connecting",
        downloaded: 0,
        total: None,
    });
    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?
        .get(&release.url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let total = response.content_length().filter(|size| *size > 0);
    if total.is_some_and(|size| size > 32 * 1024 * 1024) {
        return Err("AFMF download exceeds limit".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 32 * 1024 * 1024 {
            return Err("AFMF download exceeds limit".into());
        }
        bytes.extend_from_slice(&chunk);
        progress(TransferProgress {
            phase: "downloading",
            downloaded: bytes.len() as u64,
            total,
        });
    }
    progress(TransferProgress {
        phase: "verifying",
        downloaded: bytes.len() as u64,
        total,
    });
    releases::verify(&release, &bytes)?;
    let own = root.clone();
    progress(TransferProgress {
        phase: "extracting",
        downloaded: bytes.len() as u64,
        total,
    });
    tokio::task::spawn_blocking(move || install_bytes(&own, &release, &bytes))
        .await
        .map_err(|e| e.to_string())??;
    management::state(&root)
}
