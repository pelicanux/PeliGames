//! Default library location and exclusive game folder reservation.
use std::{fs, io::ErrorKind, path::{Path, PathBuf}};
fn reserve_directory(root: &Path, folder: &str) -> Result<PathBuf, String> {
    if folder.is_empty() || folder == "." || folder == ".." || folder.contains('/') || folder.contains('\\') {
        return Err("Nome de pasta inválido.".into());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    for number in 1..=10000 {
        let path = root.join(if number == 1 { folder.to_owned() } else { format!("{folder} ({number})") });
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Não foi possível criar a pasta padrão: {error}")),
        }
    }
    Err("Não foi possível reservar uma pasta livre para o jogo.".into())
}
#[tauri::command]
pub fn prepare_default_installation_directory(folder: Option<String>) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Não foi possível localizar a pasta pessoal.")?;
    let root = home.join("Games").join("PeliGames");
    let path = if let Some(folder) = folder { reserve_directory(&root, &folder)? } else {
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        root
    };
    Ok(path.to_string_lossy().into_owned())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn default_installation_reserves_distinct_folders() {
        let root = std::env::temp_dir().join(format!("peligames-reserve-{}", uuid::Uuid::new_v4()));
        let first = reserve_directory(&root, "Among Us").unwrap();
        fs::write(first.join("saves"), b"keep").unwrap();
        fs::write(root.join("Among Us (2)"), b"occupied").unwrap();
        assert_eq!(reserve_directory(&root, "Among Us").unwrap(), root.join("Among Us (3)"));
        assert_eq!(fs::read(first.join("saves")).unwrap(), b"keep");
        assert!(reserve_directory(&root, "../outside").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
