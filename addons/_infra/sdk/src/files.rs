//! Managed directory boundaries shared by package and component operations.
use crate::Result;
use std::{fs, path::Path};
/// Refuse to unlink native components still mapped by a same-user process.
pub fn ensure_unmapped(paths: &[std::path::PathBuf]) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let uid = fs::metadata("/proc/self").map_err(|e| e.to_string())?.uid();
        for entry in fs::read_dir("/proc").map_err(|e| e.to_string())?.flatten() {
            if !entry
                .file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
                || !entry.metadata().is_ok_and(|meta| meta.uid() == uid)
            {
                continue;
            }
            if let Ok(maps) = fs::read_to_string(entry.path().join("maps")) {
                if paths
                    .iter()
                    .any(|path| maps.contains(&format!("{}/", path.display())))
                {
                    return Err("Close games using this component before removing it".into());
                }
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = paths;
    Ok(())
}
pub fn safe_directory(path: &Path, create: bool) -> Result<()> {
    walk(path, create, false)
}
/// Validate existing ancestors without creating a configuration directory.
pub fn check_directory(path: &Path) -> Result<()> {
    walk(path, false, true)
}
fn walk(path: &Path, create: bool, allow_missing: bool) -> Result<()> {
    if !path.is_absolute() {
        return Err("An absolute add-on directory is required".into());
    }
    for ancestor in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => (),
            Ok(_) => return Err("Redirected or invalid add-on directory".into()),
            Err(e) if create && e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(ancestor).map_err(|e| e.to_string())?;
            }
            Err(e) if allow_missing && e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
