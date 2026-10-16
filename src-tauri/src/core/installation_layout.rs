//! Visible installation folders and migration of the previous hidden container.
use std::{fs, path::{Path, PathBuf}};
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

// Since 0.3.8 a new destination folder is the Wine prefix itself (like Heroic,
// Lutris or Faugus), with `installation.json` and `logs` next to `drive_c`.
// Installations made before keep their `<destination>/prefix` subfolder.

/// Marks a destination folder this launcher created empty: only such a folder
/// is ever deleted as a whole when the game is uninstalled.
pub const OWNED_MARKER: &str = ".peligames-prefix";

/// The older layout: a `prefix` subfolder inside the destination.
fn legacy_prefix(directory: &Path) -> Option<PathBuf> {
    let prefix = directory.join("prefix");
    let recorded = fs::read(directory.join("installation.json")).ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|record| record["prefix"].as_str().map(PathBuf::from));
    let real_dir = |path: &Path| fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink());
    match recorded {
        Some(recorded) => (recorded == prefix).then_some(prefix),
        None => (real_dir(&prefix) && real_dir(&prefix.join("drive_c"))).then_some(prefix),
    }
}

/// The Wine prefix of a destination folder.
pub fn prefix_for(directory: &Path) -> PathBuf {
    legacy_prefix(directory).unwrap_or_else(|| directory.to_path_buf())
}

/// The destination folder (with `installation.json` and `logs`) of a prefix.
/// Foreign prefixes keep the previous behavior: their parent folder.
pub fn destination_of(prefix: &Path) -> Option<PathBuf> {
    if prefix.join(OWNED_MARKER).is_file() || prefix.join("installation.json").is_file() {
        return Some(prefix.to_path_buf());
    }
    prefix.parent().map(Path::to_path_buf)
}

/// Whether this launcher created `directory` as a game's prefix.
pub fn owned(directory: &Path) -> bool {
    fs::symlink_metadata(directory.join(OWNED_MARKER)).is_ok_and(|meta| meta.is_file())
}

/// A destination the new layout may use: missing, empty, already this
/// launcher's (installation.json, marker or the older layout), or only the
/// leftovers of an earlier uninstall. Never another program's prefix or a
/// folder with other files, which an uninstall would otherwise delete.
pub fn check_new_destination(directory: &Path) -> Result<(), String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("Não foi possível ler o destino: {error}")),
    };
    if owned(directory) || directory.join("installation.json").is_file() || legacy_prefix(directory).is_some() {
        return Ok(());
    }
    let names: Vec<String> = entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
    if names.iter().all(|name| name == "logs") {
        return Ok(());
    }
    if directory.join("drive_c").is_dir() || directory.join("system.reg").is_file() {
        return Err(format!("{} já é um prefixo Wine/Proton de outro programa. Escolha uma pasta vazia ou nova para o jogo.", directory.display()));
    }
    Err(format!("{} não está vazia. Escolha uma pasta vazia ou nova: o jogo usa a pasta inteira como prefixo, e desinstalar apaga essa pasta.", directory.display()))
}

/// Prepares a destination folder of the new layout and marks it as owned.
pub fn claim(directory: &Path) -> Result<(), String> {
    check_new_destination(directory)?;
    fs::create_dir_all(directory).map_err(|e| format!("Não foi possível criar o destino: {e}"))?;
    if prefix_for(directory) == directory && !owned(directory) && !directory.join("installation.json").is_file() {
        fs::write(directory.join(OWNED_MARKER), b"PeliGames\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Where the game profiles of a launcher's prefix live. They must be outside
/// the prefix (an overlay or reflink copy of it must not contain itself): for
/// the new layout a hidden `.<folder>.peligames/profiles` next to it, on the
/// same disk for reflinks.
pub fn profiles_dir(directory: &Path, prefix: &Path) -> Result<PathBuf, String> {
    if prefix != directory {
        return Ok(directory.join("profiles"));
    }
    let name = directory.file_name().ok_or("Destino inválido.")?.to_string_lossy();
    Ok(directory.parent().ok_or("Destino inválido.")?.join(format!(".{name}.peligames")).join("profiles"))
}

/// Removes what an older-layout uninstall leaves behind: the logs, an empty
/// profiles folder and the destination folder once it is empty.
pub fn remove_leftovers(directory: &Path, prefix: &Path) {
    let real_dir = |path: &Path| fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink());
    if prefix != directory {
        let logs = directory.join("logs");
        if real_dir(&logs) { let _ = fs::remove_dir_all(&logs); }
        let _ = fs::remove_dir(directory.join("profiles"));
        let _ = fs::remove_dir(directory);
    } else {
        let logs = directory.join("logs");
        if real_dir(&logs) { let _ = fs::remove_dir_all(&logs); }
        if let Ok(profiles) = profiles_dir(directory, prefix) {
            let _ = fs::remove_dir(&profiles);
            if let Some(parent) = profiles.parent() { let _ = fs::remove_dir(parent); }
        }
    }
}

/// Empties a current-layout prefix folder (the game inside it included) but
/// keeps this launcher's settings, logs and ownership marker.
pub fn clear_prefix_keeping_settings(prefix: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(prefix)? {
        let entry = entry?;
        if ["installation.json", "logs", OWNED_MARKER].contains(&entry.file_name().to_string_lossy().as_ref()) {
            continue;
        }
        let path = entry.path();
        if fs::symlink_metadata(&path)?.is_dir() { fs::remove_dir_all(&path)? } else { fs::remove_file(&path)? }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        std::env::temp_dir().join(format!("peligames-layout-{}", uuid::Uuid::new_v4()))
    }
    #[test]
    fn new_destinations_are_the_prefix_and_older_ones_keep_the_subfolder() {
        let base = temp();
        let new = base.join("Jogo");
        claim(&new).unwrap();
        assert_eq!(prefix_for(&new), new);
        assert!(owned(&new));
        assert_eq!(destination_of(&new), Some(new.clone()));
        let old = base.join("Antigo");
        fs::create_dir_all(old.join("prefix/drive_c")).unwrap();
        fs::write(old.join("installation.json"), serde_json::json!({"prefix": old.join("prefix")}).to_string()).unwrap();
        assert_eq!(prefix_for(&old), old.join("prefix"));
        assert_eq!(destination_of(&old.join("prefix")), Some(old.clone()));
        claim(&old).unwrap();
        assert!(!owned(&old), "o formato antigo não ganha a marca");
        assert_eq!(profiles_dir(&old, &old.join("prefix")).unwrap(), old.join("profiles"));
        assert_eq!(profiles_dir(&new, &new).unwrap(), base.join(".Jogo.peligames/profiles"));
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn foreign_or_busy_folders_are_refused() {
        let base = temp();
        let foreign = base.join("Heroic/FC2026");
        fs::create_dir_all(foreign.join("drive_c")).unwrap();
        fs::write(foreign.join("system.reg"), b"").unwrap();
        assert!(claim(&foreign).unwrap_err().contains("outro programa"));
        assert!(!owned(&foreign));
        let games = base.join("Games");
        fs::create_dir_all(games.join("Outro jogo")).unwrap();
        assert!(claim(&games).unwrap_err().contains("não está vazia"));
        let leftover = base.join("Removido");
        fs::create_dir_all(leftover.join("logs")).unwrap();
        claim(&leftover).unwrap();
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn older_layout_leftovers_are_removed() {
        let base = temp();
        let old = base.join("Antigo");
        fs::create_dir_all(old.join("logs")).unwrap();
        fs::write(old.join("logs/installer.log"), b"x").unwrap();
        fs::create_dir_all(old.join("profiles")).unwrap();
        remove_leftovers(&old, &old.join("prefix"));
        assert!(!old.exists());
        fs::create_dir_all(old.join("Salvo")).unwrap();
        remove_leftovers(&old, &old.join("prefix"));
        assert!(old.join("Salvo").exists(), "arquivos do usuário ficam");
        fs::remove_dir_all(base).unwrap();
    }
}
