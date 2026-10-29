//! Global recording destination; existing videos are never moved or removed.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
#[derive(Debug, Serialize)]
pub struct StorageInfo {
    pub directory: String,
    pub default_directory: String,
}
#[derive(Serialize, Deserialize)]
struct Preferences {
    directory: PathBuf,
}
fn default_directory() -> Result<PathBuf, String> {
    Ok(dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|p| p.join("Videos")))
        .ok_or("Pasta de vídeos indisponível.")?
        .join("PeliGames"))
}
fn config() -> Result<PathBuf, String> {
    Ok(super::super::paths::app_path("capture-storage.json")?)
}
fn read(config: &Path, default: &Path) -> Result<PathBuf, String> {
    match fs::read(config) {
        Ok(bytes) => {
            let saved: Preferences = serde_json::from_slice(&bytes)
                .map_err(|_| "Preferência de pasta de gravações inválida.")?;
            if !saved.directory.is_absolute() {
                return Err("A pasta de gravações deve usar um caminho absoluto.".into());
            }
            Ok(saved.directory)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(default.to_owned()),
        Err(e) => Err(format!("Não foi possível ler a pasta de gravações: {e}")),
    }
}
pub fn directory() -> Result<PathBuf, String> {
    read(&config()?, &default_directory()?)
}
pub fn info() -> Result<StorageInfo, String> {
    Ok(StorageInfo {
        directory: directory()?.to_string_lossy().into_owned(),
        default_directory: default_directory()?.to_string_lossy().into_owned(),
    })
}
fn save(config: &Path, directory: &Path) -> Result<PathBuf, String> {
    if !directory.is_absolute() {
        return Err("Selecione uma pasta com caminho absoluto.".into());
    }
    let directory = directory
        .canonicalize()
        .map_err(|e| format!("Não foi possível acessar a pasta escolhida: {e}"))?;
    if !directory.is_dir() {
        return Err("O destino da gravação precisa ser uma pasta.".into());
    }
    // Test real write access without changing any existing file.
    let probe = directory.join(format!(".peligames-write-{}", uuid::Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&probe)
        .map_err(|e| format!("Não é possível salvar gravações nesta pasta: {e}"))?;
    drop(file);
    fs::remove_file(&probe).map_err(|e| e.to_string())?;
    let parent = config
        .parent()
        .ok_or("Pasta de configuração indisponível.")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(".capture-storage-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(
            &serde_json::to_vec(&Preferences {
                directory: directory.clone(),
            })
            .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, config).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    Ok(directory)
}
pub fn configure(directory: Option<String>) -> Result<StorageInfo, String> {
    let destination = match directory {
        Some(path) => PathBuf::from(path),
        None => {
            let path = default_directory()?;
            fs::create_dir_all(&path).map_err(|e| e.to_string())?;
            path
        }
    };
    save(&config()?, &destination)?;
    info()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destination_persists_and_invalid_choices_preserve_existing_settings_and_videos() {
        let root = std::env::temp_dir().join(format!("peli-storage-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let default = root.join("default");
        let destination = root.join("Clipes e vídeos");
        fs::create_dir(&default).unwrap();
        fs::create_dir(&destination).unwrap();
        let video = default.join("recording.mkv");
        fs::write(&video, b"original").unwrap();
        let config = root.join("config/storage.json");
        assert_eq!(read(&config, &default).unwrap(), default);
        assert_eq!(save(&config, &destination).unwrap(), destination);
        assert_eq!(read(&config, &default).unwrap(), destination);
        let original = fs::read(&config).unwrap();
        for invalid in [
            PathBuf::from("relative"),
            root.join("missing"),
            video.clone(),
        ] {
            assert!(save(&config, &invalid).is_err());
            assert_eq!(fs::read(&config).unwrap(), original);
        }
        assert_eq!(fs::read(&video).unwrap(), b"original");
        assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
        save(&config, &default).unwrap();
        assert_eq!(read(&config, &default).unwrap(), default);
        fs::remove_dir_all(root).unwrap();
    }
}
