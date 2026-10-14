use std::path::Path;

/// Covers folder of older versions and the current one, so the interface can
/// rewrite cover addresses saved before the data moved to `~/.config/PeliGames`.
#[tauri::command]
pub fn cover_directories() -> Result<(Option<String>, String), String> {
    let current = crate::core::paths::app_root()?.join("covers");
    Ok((crate::core::paths::legacy_covers().map(|p| p.to_string_lossy().into_owned()), current.to_string_lossy().into_owned()))
}

// Keep a private copy: moving or deleting the user's source image must not break the cover.
#[tauri::command]
pub async fn import_game_cover(source: String) -> Result<String, String> {
    let directory = crate::core::paths::app_root()?.join("covers");
    tauri::async_runtime::spawn_blocking(move || {
        let input = Path::new(&source);
        let extension = input.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !["png", "jpg", "jpeg", "webp"].contains(&extension.as_str()) {
            return Err("Unsupported cover format".to_string());
        }
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let target = directory.join(format!("{}.{}", uuid::Uuid::new_v4(), extension));
        std::fs::copy(input, &target).map_err(|e| e.to_string())?;
        Ok(target.to_string_lossy().into_owned())
    }).await.map_err(|e| e.to_string())?
}

// Stable copies keep automatic Heroic covers inside the existing asset scope.
// Repeated scans reuse the same file until its source changes.
pub(super) async fn cache_heroic_cover(source: std::path::PathBuf) -> Result<std::path::PathBuf, String> {
    let directory = crate::core::paths::app_root()?.join("covers");
    tauri::async_runtime::spawn_blocking(move || {
        use std::hash::{Hash, Hasher};
        let extension = source.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if !["png", "jpg", "jpeg", "webp"].contains(&extension.as_str()) {
            return Err("Unsupported Heroic cover format".to_string());
        }
        let metadata = std::fs::metadata(&source).map_err(|e| e.to_string())?;
        if !metadata.is_file() { return Err("Heroic cover is not a file".to_string()); }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hash);
        metadata.len().hash(&mut hash);
        metadata.modified().ok().hash(&mut hash);
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let target = directory.join(format!("heroic-{:x}.{}", hash.finish(), extension));
        if !target.is_file() { std::fs::copy(source, &target).map_err(|e| e.to_string())?; }
        Ok(target)
    }).await.map_err(|e| e.to_string())?
}

fn artwork_host_allowed(url: &reqwest::Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some()
        || url.port().is_some_and(|port| port != 443) { return false; }
    let host = url.host_str().unwrap_or("");
    ["steamstatic.com", "steamcdn-a.akamaihd.net", "epicgames.com", "gog-statics.com", "steamgriddb.com"]
        .iter().any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}

fn artwork_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { Some("png") }
    else if bytes.starts_with(b"\xff\xd8\xff") { Some("jpg") }
    else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { Some("webp") }
    else { None }
}

/// Automatic fallback artwork must outlive the browser's HTTP cache and scans.
#[tauri::command]
pub async fn cache_automatic_game_cover(url: String) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let parsed = reqwest::Url::parse(&url).map_err(|_| "Invalid artwork URL")?;
    if !artwork_host_allowed(&parsed) { return Err("Unsupported artwork host".into()); }
    let directory = crate::core::paths::app_root()?.join("covers");
    let key = format!("automatic-{:x}", Sha256::digest(url.as_bytes()));
    for extension in ["png", "jpg", "webp"] {
        let path = directory.join(format!("{key}.{extension}"));
        if path.is_file() {
            return reqwest::Url::from_file_path(path).map(|url| url.to_string()).map_err(|_| "Invalid artwork path".into());
        }
    }
    let client = reqwest::Client::builder()
        .user_agent(format!("PeliGames/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !artwork_host_allowed(attempt.url()) { attempt.stop() }
            else { attempt.follow() }
        }))
        .build().map_err(|error| error.to_string())?;
    let mut response = client.get(parsed).send().await.map_err(|error| error.to_string())?
        .error_for_status().map_err(|error| error.to_string())?;
    const LIMIT: usize = 15 * 1024 * 1024;
    if response.content_length().is_some_and(|length| length > LIMIT as u64) { return Err("Artwork exceeds size limit".into()); }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        if bytes.len() + chunk.len() > LIMIT { return Err("Artwork exceeds size limit".into()); }
        bytes.extend_from_slice(&chunk);
    }
    let extension = artwork_extension(&bytes).ok_or("Unsupported artwork format")?;
    let path = directory.join(format!("{key}.{extension}"));
    let saved = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let temporary = directory.join(format!(".{key}-{}.tmp", uuid::Uuid::new_v4()));
        let result = std::fs::write(&temporary, bytes).and_then(|_| std::fs::rename(&temporary, &saved));
        if result.is_err() { let _ = std::fs::remove_file(&temporary); }
        result.map_err(|error| error.to_string())
    }).await.map_err(|error| error.to_string())??;
    reqwest::Url::from_file_path(path).map(|url| url.to_string()).map_err(|_| "Invalid artwork path".into())
}

#[cfg(test)]
mod automatic_cover_tests {
    use super::*;
    #[test]
    fn restricts_artwork_downloads_and_redirects() {
        for url in ["https://shared.akamai.steamstatic.com/a", "https://images.gog-statics.com/a", "https://cdn2.steamgriddb.com/a", "https://cdn1.epicgames.com/a"] {
            assert!(artwork_host_allowed(&reqwest::Url::parse(url).unwrap()));
        }
        for url in ["http://cdn2.steamgriddb.com/a", "https://127.0.0.1/a", "https://steamstatic.com.attacker.org/a", "https://user@steamstatic.com/a", "https://steamstatic.com:8080/a"] {
            assert!(!artwork_host_allowed(&reqwest::Url::parse(url).unwrap()));
        }
    }
    #[test]
    fn rejects_html_and_identifies_supported_image_bytes() {
        assert_eq!(artwork_extension(b"<html>404 Not Found</html>"), None);
        assert_eq!(artwork_extension(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(artwork_extension(b"\xff\xd8\xffrest"), Some("jpg"));
        assert_eq!(artwork_extension(b"RIFF1234WEBPrest"), Some("webp"));
    }
}
