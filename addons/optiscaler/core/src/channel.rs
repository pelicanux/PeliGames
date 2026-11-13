//! Shared download channels and their isolated cached packages.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    #[default]
    Stable,
    Nightly,
}
impl Channel {
    pub fn package(self, root: &Path) -> PathBuf {
        root.join(match self {
            Self::Stable => "official",
            Self::Nightly => "nightly",
        })
    }
    pub fn repository(self) -> &'static str {
        match self {
            Self::Stable => "OptiScaler",
            Self::Nightly => "OptiScaler-nightly",
        }
    }
    pub fn release_endpoint(self) -> &'static str {
        match self {
            Self::Stable => "latest",
            Self::Nightly => "?per_page=10",
        }
    }
}
pub fn read(root: &Path) -> Result<Channel, String> {
    match fs::read(root.join("channel.json")) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid OptiScaler channel: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Channel::Stable),
        Err(e) => Err(e.to_string()),
    }
}
pub fn save(root: &Path, channel: Channel) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let _directory = peligames_mod_config::DirectoryLock::acquire(
        &root.canonicalize().map_err(|e| e.to_string())?,
    )?;
    peligames_mod_config::write_atomic(
        &root.join("channel.json"),
        &serde_json::to_string(&channel).map_err(|e| e.to_string())?,
    )
}
pub fn downloaded_directory(root: &Path, channel: Channel) -> Result<Option<PathBuf>, String> {
    for path in [root.to_path_buf(), channel.package(root)] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err("Invalid OptiScaler package directory".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(Some(channel.package(root)))
}
pub fn remove_download(root: &Path, channel: Channel) -> Result<(), String> {
    let _guard = super::WRITE.lock().map_err(|error| error.to_string())?;
    let _directory = if root.exists() {
        Some(peligames_mod_config::DirectoryLock::acquire(
            &root.canonicalize().map_err(|e| e.to_string())?,
        )?)
    } else {
        None
    };
    if read(root)? != channel {
        return Err("The OptiScaler channel changed; refresh before removing the package".into());
    }
    if let Some(directory) = downloaded_directory(root, channel)? {
        fs::remove_dir_all(directory).map_err(|error| error.to_string())?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deleting_a_download_preserves_other_channels_game_files_and_preferences() {
        let root = std::env::temp_dir().join(format!(
            "peligames-optiscaler-cache-{}",
            uuid::Uuid::new_v4()
        ));
        let game = root.join("game-copy");
        for directory in [
            Channel::Stable.package(&root),
            Channel::Nightly.package(&root),
            game.clone(),
        ] {
            fs::create_dir_all(&directory).unwrap();
            fs::write(directory.join("OptiScaler.dll"), "fixture").unwrap();
        }
        save(&root, Channel::Nightly).unwrap();
        assert!(remove_download(&root, Channel::Stable).is_err());
        remove_download(&root, Channel::Nightly).unwrap();
        assert!(downloaded_directory(&root, Channel::Nightly)
            .unwrap()
            .is_none());
        assert!(Channel::Stable
            .package(&root)
            .join("OptiScaler.dll")
            .is_file());
        assert!(game.join("OptiScaler.dll").is_file());
        assert_eq!(read(&root).unwrap(), Channel::Nightly);
        remove_download(&root, Channel::Nightly).unwrap();
        std::os::unix::fs::symlink(&game, Channel::Nightly.package(&root)).unwrap();
        assert!(remove_download(&root, Channel::Nightly).is_err());
        assert!(game.join("OptiScaler.dll").is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn selecting_nightly_preserves_stable_and_survives_reopening() {
        let root = std::env::temp_dir().join(format!(
            "peligames-optiscaler-channels-{}",
            uuid::Uuid::new_v4()
        ));
        assert_eq!(read(&root).unwrap(), Channel::Stable);
        fs::create_dir_all(Channel::Stable.package(&root)).unwrap();
        fs::write(Channel::Stable.package(&root).join("version.txt"), "v0.9.4").unwrap();
        save(&root, Channel::Nightly).unwrap();
        assert_eq!(read(&root).unwrap(), Channel::Nightly);
        assert_ne!(
            Channel::Nightly.package(&root),
            Channel::Stable.package(&root)
        );
        assert_eq!(
            fs::read_to_string(Channel::Stable.package(&root).join("version.txt")).unwrap(),
            "v0.9.4"
        );
        save(&root, Channel::Stable).unwrap();
        assert_eq!(read(&root).unwrap(), Channel::Stable);
        fs::write(root.join("channel.json"), "\"invalid\"").unwrap();
        assert!(read(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn nightly_uses_its_official_prerelease_repository() {
        assert_eq!(Channel::Stable.repository(), "OptiScaler");
        assert_eq!(Channel::Stable.release_endpoint(), "latest");
        assert_eq!(Channel::Nightly.repository(), "OptiScaler-nightly");
        assert!(Channel::Nightly.release_endpoint().starts_with('?'));
    }
}
