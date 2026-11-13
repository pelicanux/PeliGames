use crate::runtime::RELEASE;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

#[derive(Clone, Serialize, Deserialize)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub hash: String,
}
#[derive(Serialize, Deserialize)]
struct Cached {
    checked: u64,
    release: Release,
}
pub fn builtin() -> Release {
    Release {
        version: RELEASE.0.into(),
        url: RELEASE.1.into(),
        hash: RELEASE.2.into(),
    }
}
pub fn version(value: &str) -> Option<semver::Version> {
    let version = semver::Version::parse(value).ok()?;
    (version.to_string() == value && version.pre.is_empty() && version.build.is_empty())
        .then_some(version)
}
pub fn compatible(value: &str) -> bool {
    version(value).is_some_and(|v| v.major == 1 && v >= version(RELEASE.0).unwrap())
}
pub fn valid(release: &Release) -> bool {
    version(&release.version).is_some()&&release.url==format!("https://github.com/serialexperimentslainnnn/afmf-linux/releases/download/v{0}/afmf-linux-{0}-x86_64.tar.xz",release.version)
    &&release.hash.len()==64&&release.hash.bytes().all(|v|v.is_ascii_digit()||(b'a'..=b'f').contains(&v))
}
pub fn parse(value: &serde_json::Value) -> Option<Release> {
    value
        .as_array()?
        .iter()
        .filter(|r| r["draft"] == false && r["prerelease"] == false)
        .filter_map(|r| {
            let version = r["tag_name"].as_str()?.strip_prefix('v')?;
            let file = format!("afmf-linux-{version}-x86_64.tar.xz");
            let asset = r["assets"]
                .as_array()?
                .iter()
                .find(|a| a["name"].as_str() == Some(&file))?;
            let release = Release {
                version: version.into(),
                url: asset["browser_download_url"].as_str()?.into(),
                hash: asset["digest"].as_str()?.strip_prefix("sha256:")?.into(),
            };
            valid(&release).then_some(release)
        })
        .max_by_key(|r| version(&r.version))
}
pub fn latest(root: &Path) -> Release {
    if peligames_addon_kit::files::check_directory(root).is_err() {
        return builtin();
    }
    fs::read(root.join("afmf/release.json"))
        .ok()
        .filter(|bytes| bytes.len() <= 2 * 1024 * 1024)
        .and_then(|bytes| serde_json::from_slice::<Cached>(&bytes).ok())
        .map(|cached| cached.release)
        .filter(|release| valid(release) && version(&release.version) >= version(RELEASE.0))
        .unwrap_or_else(builtin)
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub async fn check(root: &Path, force: bool) -> Result<Release, String> {
    peligames_addon_kit::files::check_directory(root)?;
    peligames_addon_kit::files::check_directory(&root.join("afmf"))?;
    let path = root.join("afmf/release.json");
    if !force {
        if let Some(cached) = fs::read(&path)
            .ok()
            .filter(|bytes| bytes.len() <= 2 * 1024 * 1024)
            .and_then(|bytes| serde_json::from_slice::<Cached>(&bytes).ok())
            .filter(|c| valid(&c.release) && now().saturating_sub(c.checked) < 21600)
        {
            return Ok(cached.release);
        }
    }
    let mut response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("PeliGames-AFMF")
        .build()
        .map_err(|e| e.to_string())?
        .get(
            "https://api.github.com/repos/serialexperimentslainnnn/afmf-linux/releases?per_page=30",
        )
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("Release catalog exceeds limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut release = parse(&serde_json::from_slice(&bytes).map_err(|e| e.to_string())?)
        .ok_or("No verified official AFMF release found")?;
    if version(&release.version) < version(RELEASE.0) {
        release = builtin();
    }
    fs::create_dir_all(root.join("afmf")).map_err(|e| e.to_string())?;
    peligames_mod_config::write_atomic_bytes(
        &path,
        &serde_json::to_vec(&Cached {
            checked: now(),
            release: release.clone(),
        })
        .map_err(|e| e.to_string())?,
        0o600,
    )?;
    Ok(release)
}
pub fn verify(release: &Release, bytes: &[u8]) -> Result<(), String> {
    if !valid(release) || format!("{:x}", Sha256::digest(bytes)) != release.hash {
        return Err("AFMF package integrity verification failed".into());
    }
    Ok(())
}
