//! Default library location and exclusive game folder reservation.
use std::{collections::BTreeMap, fs, io::ErrorKind, path::{Path, PathBuf}, sync::Mutex};
// Only folders created by this process can be released by the wizard.
static RESERVATIONS: Mutex<BTreeMap<PathBuf,(u64,u64)>>=Mutex::new(BTreeMap::new());
fn identity(path:&Path)->Result<(u64,u64),String>{
    let metadata=fs::symlink_metadata(path).map_err(|e|e.to_string())?;
    if !metadata.is_dir()||metadata.file_type().is_symlink(){return Err("A pasta foi substituída ou redirecionada.".into());}
    #[cfg(unix)] {use std::os::unix::fs::MetadataExt;Ok((metadata.dev(),metadata.ino()))}
    #[cfg(not(unix))] {Err("Limpeza automática indisponível neste sistema.".into())}
}
pub(crate) fn track_created_directory(path:&Path)->Result<(),String>{
    let id=identity(path)?;RESERVATIONS.lock().map_err(|e|e.to_string())?.insert(path.to_path_buf(),id);Ok(())
}
fn release_directory(path:&Path)->Result<bool,String>{
    let mut reservations=RESERVATIONS.lock().map_err(|e|e.to_string())?;
    let Some(expected)=reservations.remove(path) else{return Ok(false);};
    if identity(path).ok()!=Some(expected){return Ok(false);}
    // remove_dir never removes files or a non-empty prefix, even if content arrived after the check.
    match fs::remove_dir(path){
        Ok(())=>Ok(true),
        Err(error) if matches!(error.kind(),ErrorKind::DirectoryNotEmpty|ErrorKind::NotFound)=>Ok(false),
        Err(error)=>Err(format!("Não foi possível remover a pasta vazia: {error}")),
    }
}
fn reserve_directory(root: &Path, folder: &str) -> Result<PathBuf, String> {
    if folder.is_empty() || folder == "." || folder == ".." || folder.contains('/') || folder.contains('\\') {
        return Err("Nome de pasta inválido.".into());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let original=root.join(folder);
    if original.join("installation.json").is_file() && crate::core::installed_library::removed_destination(&original)?.is_some(){return Ok(original);}
    for number in 1..=10000 {
        let path = root.join(if number == 1 { folder.to_owned() } else { format!("{folder} ({number})") });
        match fs::create_dir(&path) {
            Ok(()) => {
                let result=identity(&path).and_then(|id|{RESERVATIONS.lock().map_err(|e|e.to_string())?.insert(path.clone(),id);Ok(())});
                if let Err(error)=result {let _=fs::remove_dir(&path);return Err(error);}
                return Ok(path);
            },
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
#[tauri::command]
pub fn release_default_installation_directory(path:String)->Result<bool,String>{release_directory(Path::new(&path))}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn cancelled_wizard_removes_only_its_empty_reservation() {
        let root=std::env::temp_dir().join(format!("peligames-cancel-folder-{}",uuid::Uuid::new_v4()));
        let empty=reserve_directory(&root,"Empty").unwrap();assert!(release_directory(&empty).unwrap());assert!(!empty.exists());
        let existing=root.join("Existing");fs::create_dir(&existing).unwrap();assert!(!release_directory(&existing).unwrap());assert!(existing.exists());
        let used=reserve_directory(&root,"Used").unwrap();fs::write(used.join("save.dat"),b"keep").unwrap();assert!(!release_directory(&used).unwrap());assert_eq!(fs::read(used.join("save.dat")).unwrap(),b"keep");
        let replaced=reserve_directory(&root,"Replaced").unwrap();fs::rename(&replaced,root.join("Original")).unwrap();fs::create_dir(&replaced).unwrap();assert!(!release_directory(&replaced).unwrap());assert!(replaced.exists());
        fs::remove_dir_all(root).unwrap();
    }
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
