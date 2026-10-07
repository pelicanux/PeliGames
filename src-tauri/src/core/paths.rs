//! Application storage and migration from the former standalone mod launcher.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub const APP_DIRECTORY: &str = "PeliGames";
pub const APP_IDENTIFIER: &str = "com.pelicano.peligames";
static MIGRATION: OnceLock<Result<(), String>> = OnceLock::new();

fn directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(format!(
            "A pasta de configuração não é um diretório regular: {}",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|e| e.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

// Rename on the same filesystem, merging only missing items. Existing files win;
// conflicting legacy files remain intact for manual recovery. Never follow links.
fn merge(source: &Path, destination: &Path) -> Result<(), String> {
    let source_meta = match fs::symlink_metadata(source) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    if source_meta.file_type().is_symlink() {
        return Err(format!("Migração recusou um link: {}", source.display()));
    }
    if let Some(parent) = destination.parent() {
        directory(parent)?;
    }
    match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::rename(source, destination)
                .map_err(|e| format!("Falha ao migrar {}: {e}", source.display()))
        }
        Err(error) => Err(error.to_string()),
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "Migração recusou um link: {}",
            destination.display()
        )),
        Ok(meta) if source_meta.is_dir() && meta.is_dir() => {
            for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                merge(&entry.path(), &destination.join(entry.file_name()))?;
            }
            if fs::read_dir(source)
                .map_err(|e| e.to_string())?
                .next()
                .is_none()
            {
                fs::remove_dir(source).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        Ok(_) => Ok(()),
    }
}

pub fn migrate_config_root(config: &Path) -> Result<(), String> {
    let app = config.join(APP_DIRECTORY);
    directory(&app)?;
    merge(&config.join("peligames"), &app)?;
    let mods = app.join("Mod");
    directory(&mods)?;
    let dlssnr = mods.join("DLSSNR");
    directory(&dlssnr)?;
    let old = config.join("dlssnr-x-amd");
    if let Ok(meta) = fs::symlink_metadata(&old) {
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err("A configuração antiga não é um diretório regular.".into());
        }
        for entry in fs::read_dir(&old).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name();
            let name_text = name.to_string_lossy();
            let parent = if name_text == "config.json" || name_text.starts_with("launcher.") {
                &app
            } else {
                &dlssnr
            };
            merge(&entry.path(), &parent.join(&name))?;
        }
        if fs::read_dir(&old)
            .map_err(|e| e.to_string())?
            .next()
            .is_none()
        {
            fs::remove_dir(&old).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn initialize() -> Result<(), String> {
    MIGRATION
        .get_or_init(|| {
            let config = dirs::config_dir().ok_or("Pasta de configuração indisponível.")?;
            migrate_config_root(&config)?;
            for root in [Some(config), dirs::data_local_dir(), dirs::cache_dir()]
                .into_iter()
                .flatten()
            {
                merge(
                    &root.join("com.pelicano.dlssnr-installer"),
                    &root.join(APP_IDENTIFIER),
                )?;
            }
            Ok(())
        })
        .clone()
}

pub fn app_root() -> Result<PathBuf, String> {
    initialize()?;
    Ok(dirs::config_dir()
        .ok_or("Pasta de configuração indisponível.")?
        .join(APP_DIRECTORY))
}

pub fn mod_root() -> Result<PathBuf, String> {
    Ok(app_root()?.join("Mod/DLSSNR"))
}

// Installation manifests may still contain absolute paths to managed runners.
// Resolve them after the directory move without altering external Steam/Heroic paths.
pub fn migrated_runner_path(value: &str) -> String {
    let Some(config) = dirs::config_dir() else {
        return value.into();
    };
    if let Ok(relative) = Path::new(value).strip_prefix(config.join("peligames")) {
        if relative.starts_with("runners") {
            let current = config.join(APP_DIRECTORY).join(relative);
            if current.exists() {
                return current.to_string_lossy().into_owned();
            }
        }
    }
    value.into()
}
