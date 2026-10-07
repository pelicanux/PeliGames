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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moves_existing_prefix_logs_and_manifest_without_overwrite() {
        let root =
            std::env::temp_dir().join(format!("peligames-layout-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join(".peligames/prefix/drive_c")).unwrap();
        fs::create_dir_all(root.join(".peligames/logs")).unwrap();
        fs::write(root.join(".peligames/prefix/drive_c/saved"), "data").unwrap();
        fs::write(root.join(".peligames/installation.json"), "{}").unwrap();
        assert!(migrate(&root).unwrap());
        assert_eq!(
            fs::read_to_string(root.join("prefix/drive_c/saved")).unwrap(),
            "data"
        );
        assert!(root.join("logs").is_dir());
        assert!(root.join("installation.json").is_file());
        assert!(!root.join(".peligames").exists());
        assert!(!migrate(&root).unwrap());
        fs::create_dir_all(root.join(".peligames/prefix")).unwrap();
        fs::write(root.join(".peligames/prefix/old"), "preserve").unwrap();
        assert!(migrate(&root).is_err());
        assert!(root.join(".peligames/prefix/old").is_file());
        fs::remove_dir_all(root).unwrap();
    }
}
