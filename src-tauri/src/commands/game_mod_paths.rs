//! Resolve and remember mod destinations independently of any integration.
use peligames_mod_config::write_atomic_bytes as write;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn module_file(key: &str, label: &str) -> Result<PathBuf, String> {
    let folder = crate::core::paths::app_path("GameModules")?;
    let identity = format!("{:x}", Sha256::digest(key.as_bytes()));
    let base = crate::core::paths::names::allocate(&folder, label, &identity)?;
    let path = base.with_extension("json");
    let old = folder.join(identity + ".json");
    if !path.exists() && old.is_file() {
        fs::rename(old, &path).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Paths {
    root: String,
    executable: Option<String>,
    dlss_destination: Option<String>,
}
#[tauri::command]
pub async fn resolve_game_mod_paths(
    directory: String,
    executable: Option<String>,
    game_key: Option<String>,
    legacy_override: Option<String>,
) -> Result<Paths, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&directory)
            .canonicalize()
            .map_err(|_| "Pasta indisponível. Verifique o disco e a montagem.")?;
        let root = super::nexus_discovery::resolve(&root)
            .ok()
            .filter(|(_, module)| module.is_some())
            .map(|(root, _)| root)
            .unwrap_or(root);
        if let Some(key) = game_key {
            let file = module_file(
                &key,
                root.file_name().and_then(|v| v.to_str()).unwrap_or("game"),
            )?;
            if let Ok(bytes) = fs::read(file) {
                if let Ok(saved) = serde_json::from_slice::<Paths>(&bytes) {
                    if Path::new(&saved.root) == root
                        && saved.dlss_destination.as_ref().is_some_and(|p| {
                            Path::new(p)
                                .canonicalize()
                                .is_ok_and(|p| p.starts_with(&root) && p.is_dir())
                        })
                    {
                        return Ok(saved);
                    }
                }
            }
        }
        if let Some(path) = legacy_override
            .and_then(|p| PathBuf::from(p).canonicalize().ok())
            .filter(|p| p.starts_with(&root) && p.is_dir())
        {
            return Ok(Paths {
                root: root.to_string_lossy().into(),
                executable: super::analyzer::scan_directory_for_exe(&path)
                    .map(|p| p.to_string_lossy().into()),
                dlss_destination: Some(path.to_string_lossy().into()),
            });
        }
        let exe = executable
            .map(PathBuf::from)
            .filter(|p| p.is_file() && p.starts_with(&root))
            .or_else(|| {
                crate::core::game_executable::best_executable_dir(&root, 5)
                    .and_then(|p| super::analyzer::scan_directory_for_exe(&p))
            });
        Ok(Paths {
            root: root.to_string_lossy().into(),
            dlss_destination: exe
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_string_lossy().into()),
            executable: exe.map(|p| p.to_string_lossy().into()),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_game_mod_destination(
    game_key: String,
    directory: String,
    destination: String,
) -> Result<Paths, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&directory)
            .canonicalize()
            .map_err(|_| "A pasta do jogo não está disponível.")?;
        let destination = Path::new(&destination)
            .canonicalize()
            .map_err(|_| "O destino não está disponível.")?;
        if !destination.is_dir() || !destination.starts_with(&root) {
            return Err("Selecione uma pasta dentro desta instalação do jogo.".into());
        }
        let paths = Paths {
            root: root.to_string_lossy().into(),
            executable: super::analyzer::scan_directory_for_exe(&destination)
                .map(|p| p.to_string_lossy().into()),
            dlss_destination: Some(destination.to_string_lossy().into()),
        };
        write(
            &module_file(
                &game_key,
                root.file_name().and_then(|v| v.to_str()).unwrap_or("game"),
            )?,
            &serde_json::to_vec(&paths).map_err(|e| e.to_string())?,
            0o600,
        )?;
        Ok(paths)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn resolves_supported_game_root_separately_from_shipping_destination() {
        let root =
            std::env::temp_dir().join(format!("peligames-framegen-paths-{}", uuid::Uuid::new_v4()));
        let binary = root.join("Pal/Binaries/Win64");
        fs::create_dir_all(&binary).unwrap();
        fs::write(root.join("Palworld.exe"), b"MZ").unwrap();
        fs::write(binary.join("Palworld-Win64-Shipping.exe"), b"MZ").unwrap();
        let paths = resolve_game_mod_paths(
            binary.to_string_lossy().into(),
            Some(
                binary
                    .join("Palworld-Win64-Shipping.exe")
                    .to_string_lossy()
                    .into(),
            ),
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(paths.root, root.to_string_lossy());
        assert_eq!(
            paths.dlss_destination.as_deref(),
            Some(binary.to_str().unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }
}
