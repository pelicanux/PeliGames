use peligames_addon_kit::{manifest::Manifest, package};
use serde::Serialize;
#[derive(Serialize)]
pub struct Plan {
    manifest: Manifest,
    sha256: String,
    bytes: u64,
}
pub fn read(path: &str) -> Result<Vec<u8>, String> {
    let path = std::path::Path::new(path);
    if !path.is_absolute() {
        return Err("Choose an absolute local package path".into());
    }
    package::bounded_read(path, package::MAX_ARCHIVE_BYTES)
}
pub fn inspect(path: &str, host: &str, platform: &str) -> Result<Plan, String> {
    let bytes = read(path)?;
    let contents = package::inspect_local(&bytes)?;
    contents.manifest.compatible(host, platform)?;
    Ok(Plan {
        manifest: contents.manifest,
        sha256: package::digest(&bytes),
        bytes: bytes.len() as u64,
    })
}
