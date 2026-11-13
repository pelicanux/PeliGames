//! Official catalog updates are independent of launcher releases.
use crate::core::paths::{app_path, layout};
use peligames_addon_kit::{catalog::Catalog, package};
use std::{path::Path, time::Duration};
const LIMIT: u64 = 2 * 1024 * 1024;
const URL: &str =
    "https://raw.githubusercontent.com/pelicanux/PeliGames/main/addons/_infra/config/catalog.json";
static FETCH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn decode(bytes: &[u8]) -> Result<Catalog, String> {
    if bytes.len() as u64 > LIMIT {
        return Err("Add-on catalog exceeds its size limit".into());
    }
    let catalog: Catalog = serde_json::from_slice(bytes).map_err(|_| "Invalid add-on catalog")?;
    catalog.validate()?;
    Ok(catalog)
}
pub(super) fn load() -> Result<(Catalog, bool), String> {
    load_at(&app_path(layout::ADDON_CATALOG)?)
}
fn load_at(path: &Path) -> Result<(Catalog, bool), String> {
    if path.exists() {
        if let Ok(catalog) = package::bounded_read(&path, LIMIT).and_then(|bytes| decode(&bytes)) {
            return Ok((catalog, false));
        }
        return Ok((
            decode(include_bytes!(
                "../../../../addons/_infra/config/catalog.json"
            ))?,
            true,
        ));
    }
    Ok((
        decode(include_bytes!(
            "../../../../addons/_infra/config/catalog.json"
        ))?,
        false,
    ))
}
pub(super) fn publish(bytes: &[u8]) -> Result<(), String> {
    publish_at(&app_path(layout::ADDON_CATALOG)?, bytes)
}
fn publish_at(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let catalog = decode(bytes)?;
    std::fs::create_dir_all(path.parent().ok_or("Invalid add-on catalog path")?)
        .map_err(|e| e.to_string())?;
    peligames_mod_config::write_atomic(
        path,
        &serde_json::to_string_pretty(&catalog).map_err(|e| e.to_string())?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_cache_preserves_last_valid_update_and_recovers_from_local_corruption() {
        let root = std::env::temp_dir().join(format!(
            "peligames-addon-catalog-test-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("cache/addons/catalog.json");
        assert!(load_at(&path).unwrap().0.addons.is_empty());
        assert!(!root.join("cache").exists());
        let original = br#"{"schema_version":1,"addons":[]}"#;
        publish_at(&path, original).unwrap();
        let saved = std::fs::read(&path).unwrap();
        assert!(publish_at(&path, b"not json").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        assert!(!load_at(&path).unwrap().1);
        std::fs::write(&path, "broken cache").unwrap();
        let (fallback, warning) = load_at(&path).unwrap();
        assert!(warning);
        assert!(fallback.addons.is_empty());
        assert!(publish_at(&path, &vec![b'x'; LIMIT as usize + 1]).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
pub(super) async fn fetch() -> Result<Vec<u8>, String> {
    let _guard = FETCH.lock().await;
    let client = reqwest::Client::builder()
        .user_agent("PeliGames")
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(URL)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if response.content_length().is_some_and(|n| n > LIMIT) {
        return Err("Add-on catalog exceeds its size limit".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() as u64 + chunk.len() as u64 > LIMIT {
            return Err("Add-on catalog exceeds its size limit".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    decode(&bytes)?;
    Ok(bytes)
}
