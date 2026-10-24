//! Whether this launcher is an official build: the SHA-256 of the running
//! executable must be listed in the `PeliGames_<version>_binaries.sha256`
//! file of the GitHub release of the same version. A modified launcher can
//! fake this answer; it identifies plain local builds for support, nothing more.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{io::Read, sync::Mutex, time::Duration};

const RELEASES: &str = "https://api.github.com/repos/pelicanux/PeliGames/releases/tags/v";
const DOWNLOAD_PREFIX: &str = "https://github.com/pelicanux/PeliGames/releases/download/";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildKind {
    /// The executable matches a published release.
    Official,
    /// No release lists it: compiled locally or changed.
    Local,
    /// Not checked yet, or GitHub was unreachable.
    Unknown,
}
static RESULT: Mutex<BuildKind> = Mutex::new(BuildKind::Unknown);

pub fn current() -> BuildKind { RESULT.lock().map(|kind| *kind).unwrap_or(BuildKind::Unknown) }
/// The line reports carry.
pub fn report_label() -> &'static str {
    match current() {
        BuildKind::Official => "oficial (verificada no GitHub)",
        BuildKind::Local => "não oficial (compilada localmente ou modificada)",
        BuildKind::Unknown => "não verificada",
    }
}
fn listed(sums: &str, hash: &str) -> bool {
    sums.lines().any(|line| line.split_whitespace().next().is_some_and(|value| value.eq_ignore_ascii_case(hash)))
}
fn executable_hash() -> Result<String, String> {
    // The running binary, even after an update replaced its file on disk.
    let mut file = std::fs::File::open("/proc/self/exe").map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 { break; }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
async fn check() -> Result<BuildKind, String> {
    let version = env!("CARGO_PKG_VERSION");
    let hash = tauri::async_runtime::spawn_blocking(executable_hash).await.map_err(|e| e.to_string())??;
    let client = reqwest::Client::builder().user_agent("PeliGames").timeout(Duration::from_secs(20)).build().map_err(|e| e.to_string())?;
    let response = client.get(format!("{RELEASES}{version}")).header("Accept", "application/vnd.github+json").send().await.map_err(|e| e.to_string())?;
    // No release with this version: built from the sources.
    if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(BuildKind::Local); }
    if !response.status().is_success() { return Err(format!("GitHub {}", response.status())); }
    let release: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let name = format!("PeliGames_{version}_binaries.sha256");
    let Some(asset) = release["assets"].as_array().into_iter().flatten().find(|asset| asset["name"] == name.as_str()) else {
        // Releases published before the check existed list no binaries.
        return Ok(BuildKind::Unknown);
    };
    let url = asset["browser_download_url"].as_str().filter(|url| url.starts_with(DOWNLOAD_PREFIX)).ok_or("Endereço do arquivo de verificação inválido.")?;
    if asset["size"].as_u64().is_none_or(|size| size > 64 * 1024) { return Err("Arquivo de verificação inválido.".into()); }
    let sums = client.get(url).send().await.map_err(|e| e.to_string())?.error_for_status().map_err(|e| e.to_string())?.text().await.map_err(|e| e.to_string())?;
    // The downloaded list must be the one GitHub registered for the release.
    if let Some(expected) = asset["digest"].as_str().and_then(|digest| digest.strip_prefix("sha256:")) {
        if format!("{:x}", Sha256::digest(sums.as_bytes())) != expected.to_ascii_lowercase() { return Err("Arquivo de verificação alterado.".into()); }
    }
    Ok(if listed(&sums, &hash) { BuildKind::Official } else { BuildKind::Local })
}
/// Runs once per start, in the background; reports read the stored answer.
pub fn verify_in_background(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        match check().await {
            Ok(kind) => {
                if let Ok(mut stored) = RESULT.lock() { *stored = kind; }
                crate::core::logger::log_launcher(&app, "INFO", &format!("Versão do launcher: {}", report_label()));
            }
            Err(error) => crate::core::logger::log_launcher(&app, "WARN", &format!("Não foi possível verificar se a versão é oficial: {error}")),
        }
    });
}
#[tauri::command]
pub fn official_build_status() -> BuildKind { current() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_listed_hashes_count_as_official() {
        let sums = "ABCDEF0123  appimage/peligames\n0011223344  deb/opt/PeliGames/peligames\n";
        assert!(listed(sums, "abcdef0123"));
        assert!(listed(sums, "0011223344"));
        assert!(!listed(sums, "appimage/peligames"), "só a coluna do hash conta");
        assert!(!listed(sums, "deadbeef"));
    }
}
