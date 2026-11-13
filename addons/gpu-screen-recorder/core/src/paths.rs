pub use peligames_mod_config::{layout, names};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
static ROOT: OnceLock<PathBuf> = OnceLock::new();
pub fn configure(root: PathBuf) -> Result<(), String> {
    peligames_addon_kit::files::safe_directory(&root, true)?;
    if let Some(previous) = ROOT.get() {
        if previous != &root {
            return Err("Recorder storage context changed".into());
        }
        return Ok(());
    }
    ROOT.set(root)
        .map_err(|_| "Recorder storage context already initialized".into())
}
pub fn app_root() -> Result<PathBuf, String> {
    if let Some(root) = ROOT.get() {
        return Ok(root.clone());
    }
    #[cfg(test)]
    {
        let root =
            std::env::temp_dir().join(format!("peligames-recorder-tests-{}", std::process::id()));
        configure(root.clone())?;
        return Ok(root);
    }
    #[cfg(not(test))]
    Err("Recorder storage context was not provided by the host".into())
}
pub fn app_path(relative: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(layout::join(&app_root()?, relative))
}
