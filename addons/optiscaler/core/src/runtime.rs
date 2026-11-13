//! Official component downloads, isolated from DLSSNR packages.
use crate::channel::{self, Channel};
use crate as optiscaler;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
static DOWNLOAD: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
    size: u64,
}
#[derive(Deserialize)]
struct Release {
    #[serde(default)]
    draft: bool,
    tag_name: String,
    assets: Vec<Asset>,
}
#[derive(Serialize)]
pub struct RuntimeState {
    pub directory: Option<String>,
    pub software: Vec<optiscaler::software::Component>,
    pub channel: Channel,
    pub version: Option<String>,
    pub ready: bool,
}
pub fn component(root: &Path) -> Result<PathBuf, String> {
    Ok(channel::read(&root)?.package(&root))
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("PeliGames")
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())
}
pub use peligames_addon_kit::protocol::TransferProgress as DownloadProgress;
pub type ProgressChannel = Arc<dyn Fn(DownloadProgress) + Send + Sync>;
fn report(
    progress: Option<&ProgressChannel>,
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
) {
    if let Some(progress) = progress {
        progress(DownloadProgress {
            phase,
            downloaded,
            total,
        });
    }
}
async fn fetch(client: &reqwest::Client, url: &str, limit: u64) -> Result<Vec<u8>, String> {
    fetch_progress(client, url, limit, None).await
}
async fn fetch_progress(
    client: &reqwest::Client,
    url: &str,
    limit: u64,
    progress: Option<&ProgressChannel>,
) -> Result<Vec<u8>, String> {
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if response.content_length().is_some_and(|n| n > limit) {
        return Err("Component download exceeds size limit".into());
    }
    let total = response.content_length();
    let mut last_report = Instant::now();
    report(progress, "downloading", 0, total);
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() as u64 + chunk.len() as u64 > limit {
            return Err("Component download exceeds size limit".into());
        }
        bytes.extend_from_slice(&chunk);
        if last_report.elapsed() >= Duration::from_millis(200) {
            report(progress, "downloading", bytes.len() as u64, total);
            last_report = Instant::now();
        }
    }
    report(progress, "downloading", bytes.len() as u64, total);
    Ok(bytes)
}
async fn asset(
    client: &reqwest::Client,
    repo: &str,
    tag: &str,
    extension: &str,
    progress: Option<&ProgressChannel>,
) -> Result<(String, Vec<u8>), String> {
    let suffix = if tag.starts_with('?') {
        tag.to_string()
    } else {
        format!("/{tag}")
    };
    let url = format!("https://api.github.com/repos/optiscaler/{repo}/releases{suffix}");
    let bytes = fetch(client, &url, 4 * 1024 * 1024).await?;
    let release: Release = if tag.starts_with('?') {
        serde_json::from_slice::<Vec<Release>>(&bytes)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|r| !r.draft && r.assets.iter().any(|a| a.name.ends_with(extension)))
            .ok_or("No official nightly package is available")?
    } else {
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
    };
    let a = release
        .assets
        .into_iter()
        .find(|a| a.name.ends_with(extension))
        .ok_or("Official release has no compatible asset")?;
    if !a.browser_download_url.starts_with(&format!(
        "https://github.com/optiscaler/{repo}/releases/download/"
    )) || a.size > 256 * 1024 * 1024
    {
        return Err("Invalid official asset".into());
    }
    let bytes =
        fetch_progress(client, &a.browser_download_url, 256 * 1024 * 1024, progress).await?;
    report(progress, "verifying", bytes.len() as u64, Some(a.size));
    if bytes.len() as u64 != a.size {
        return Err("Incomplete component download".into());
    }
    let expected = a
        .digest
        .as_deref()
        .and_then(|s| s.strip_prefix("sha256:"))
        .ok_or("Official release is missing a SHA-256 digest")?;
    if format!("{:x}", Sha256::digest(&bytes)) != expected {
        return Err("Component checksum mismatch".into());
    }
    Ok((release.tag_name, bytes))
}
struct Stage(PathBuf);
impl Stage {
    fn new(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let p = root.join(format!("download-temporary-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).map_err(|e| e.to_string())?;
        Ok(Self(p))
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn publish(stage: &Path, target: &Path) -> Result<(), String> {
    // Replacing a cached package does not modify copies already installed in games.
    let previous = target.with_extension("previous");
    if previous.exists() {
        return Err("A previous component update needs recovery".into());
    }
    if target.exists() {
        fs::rename(target, &previous).map_err(|e| e.to_string())?;
    }
    if let Err(e) = fs::rename(stage, target) {
        if previous.exists() {
            let _ = fs::rename(&previous, target);
        }
        return Err(e.to_string());
    }
    if previous.exists() {
        let _ = fs::remove_dir_all(previous);
    }
    Ok(())
}
pub async fn state(root: &Path) -> Result<RuntimeState, String> {
    let selected = channel::read(&root)?;
    let p = selected.package(&root);
    let version = fs::read_to_string(p.join("version.txt")).ok();
    Ok(RuntimeState {
        directory: channel::downloaded_directory(&root, selected)?
            .map(|path| path.to_string_lossy().into_owned()),
        software: optiscaler::software::inventory(&p, Some("OptiScaler.dll"), version.clone()),
        channel: selected,
        version,
        ready: p.join("OptiScaler.dll").is_file()
            && p.join("OptiScaler.ini").is_file()
            && p.join("OptiScaler_LICENSE.txt").is_file(),
    })
}
pub async fn set_channel(root: &Path, channel: Channel) -> Result<RuntimeState, String> {
    let _guard = DOWNLOAD.lock().await;
    channel::save(root, channel)?;
    state(root).await
}
pub async fn remove(root: PathBuf, channel: Channel) -> Result<RuntimeState, String> {
    let _guard = DOWNLOAD.lock().await;
    let selected_root = root.clone();
    tokio::task::spawn_blocking(move || channel::remove_download(&selected_root, channel))
        .await
        .map_err(|error| error.to_string())??;
    state(&root).await
}
pub async fn download(
    root: PathBuf,
    channel: Channel,
    on_progress: ProgressChannel,
) -> Result<(), String> {
    let _guard = DOWNLOAD.lock().await;
    let client = client()?;
    if channel::read(&root)? != channel {
        return Err("The OptiScaler channel changed; refresh before downloading".into());
    }
    report(Some(&on_progress), "connecting", 0, None);
    let (version, bytes) = asset(
        &client,
        channel.repository(),
        channel.release_endpoint(),
        ".7z",
        Some(&on_progress),
    )
    .await?;
    let license = fetch(
        &client,
        "https://raw.githubusercontent.com/optiscaler/OptiScaler/master/LICENSE",
        1024 * 1024,
    )
    .await?;
    report(Some(&on_progress), "extracting", 0, None);
    let target = channel.package(&root);
    tokio::task::spawn_blocking(move || {
        let stage = Stage::new(target.parent().unwrap())?;
        let archive = stage.0.join("official.7z");
        fs::write(&archive, bytes).map_err(|e| e.to_string())?;
        let normalized = stage.0.join("official.zip");
        peligames_archive::normalize(&archive, &normalized)?;
        let unpacked = stage.0.join("package");
        fs::create_dir(&unpacked).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(fs::File::open(normalized).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        zip.extract(&unpacked).map_err(|e| e.to_string())?;
        if !unpacked.join("OptiScaler.dll").is_file() || !unpacked.join("OptiScaler.ini").is_file()
        {
            return Err("Invalid official OptiScaler package".into());
        }
        fs::write(unpacked.join("OptiScaler_LICENSE.txt"), license).map_err(|e| e.to_string())?;
        fs::write(unpacked.join("version.txt"), version).map_err(|e| e.to_string())?;
        publish(&unpacked, &target)?;
        report(Some(&on_progress), "complete", 0, None);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
pub async fn patcher_set(root: PathBuf, game: PathBuf, enabled: bool) -> Result<(), String> {
    let _guard = DOWNLOAD.lock().await;
    if !enabled {
        return tokio::task::spawn_blocking(move || optiscaler::patcher::disable(&game))
            .await
            .map_err(|e| e.to_string())?;
    }
    let client = client()?;
    let (_, bytes) = asset(&client, "OptiPatcher", "tags/rolling", ".asi", None).await?;
    let license = fetch(
        &client,
        "https://raw.githubusercontent.com/optiscaler/OptiPatcher/main/LICENSE",
        1024 * 1024,
    )
    .await?;
    tokio::task::spawn_blocking(move || {
        let stage = Stage::new(&root)?;
        fs::write(stage.0.join("OptiPatcher.asi"), bytes).map_err(|e| e.to_string())?;
        fs::write(stage.0.join("OptiPatcher_LICENSE.txt"), license).map_err(|e| e.to_string())?;
        optiscaler::patcher::enable(&game, &stage.0)
    })
    .await
    .map_err(|e| e.to_string())?
}
