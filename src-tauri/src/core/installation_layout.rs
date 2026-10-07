//! Visible installation folders and migration of the previous hidden container.
use std::{fs, path::Path};
pub fn migrate(directory: &Path) -> Result<bool, String> {
    let old = directory.join(".peligames");
    if !old.exists() {
        return Ok(false);
    }
    if fs::symlink_metadata(&old)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("A pasta antiga .peligames é um link; não foi movida.".into());
    }
    let names = ["prefix", "logs", "installation.json"];
    // Check every collision before moving anything. Never replace a user's folder.
    for name in names {
        if old.join(name).exists() && directory.join(name).exists() {
            return Err(format!(
                "Não foi possível migrar: {} já existe.",
                directory.join(name).display()
            ));
        }
    }
    for name in names {
        let source = old.join(name);
        if source.exists() {
            fs::rename(&source, directory.join(name))
                .map_err(|e| format!("Não foi possível mover {}: {e}", source.display()))?;
        }
    }
    // Keep unexpected files rather than deleting them.
    if fs::read_dir(&old)
        .map_err(|e| e.to_string())?
        .next()
        .is_none()
    {
        fs::remove_dir(&old).map_err(|e| e.to_string())?;
    }
    Ok(true)
}
