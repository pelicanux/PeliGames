//! Application storage and migration from the former standalone mod launcher.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[path = "../../../tools/storage_layout.rs"]
pub mod layout;
#[path = "../../../tools/storage_names.rs"]
pub mod names;
#[path = "storage_migration.rs"]
mod storage_migration;

pub fn app_path(relative: impl AsRef<Path>) -> Result<PathBuf, String> {
    Ok(layout::join(&app_root()?, relative))
}
pub fn organize(owns_launcher: bool) -> Result<(), String> {
    if MIGRATIONS_PAUSED.load(std::sync::atomic::Ordering::Acquire) { return Ok(()); }
    storage_migration::organize(&app_root()?, owns_launcher)
}

pub const APP_DIRECTORY: &str = "PeliGames";
pub const APP_IDENTIFIER: &str = "com.pelicano.peligames";
static MIGRATION: OnceLock<Result<(), String>> = OnceLock::new();
static MIGRATIONS_PAUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
// During reset recovery, even a normal migration could merge a folder that was
// not renamed yet into a newly created profile. Keep both sets untouched.
pub fn pause_migrations(paused: bool) {
    MIGRATIONS_PAUSED.store(paused, std::sync::atomic::Ordering::Release);
}

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
    let mods = layout::join(&app,"Mod");
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
            let target=if parent==&app {layout::join(&app,&name)}else{parent.join(&name)};
            merge(&entry.path(), &target)?;
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
    if MIGRATIONS_PAUSED.load(std::sync::atomic::Ordering::Acquire) { return Ok(()); }
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

/// Everything the launcher writes lives below `~/.config/PeliGames`, so deleting
/// that one folder resets it. Older versions kept the webview data, covers and
/// small caches in `~/.local/share/com.pelicano.peligames` and update downloads
/// in `~/.cache/PeliGames`; move them in before any webview opens. Failures keep
/// the old files where they are and never stop the launcher.
pub fn migrate_scattered_data() -> Vec<String> {
    if MIGRATIONS_PAUSED.load(std::sync::atomic::Ordering::Acquire) { return Vec::new(); }
    let Ok(root) = app_root() else { return Vec::new(); };
    migrate_scattered_data_at(&root, dirs::data_local_dir().as_deref(), dirs::cache_dir().as_deref())
}
fn migrate_scattered_data_at(root: &Path, local: Option<&Path>, cache: Option<&Path>) -> Vec<String> {
    let mut errors = Vec::new();
    if let Some(old) = local.map(|base| base.join(APP_IDENTIFIER)) {
        if fs::symlink_metadata(&old).is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink()) {
            match fs::read_dir(&old) {
                Ok(entries) => for entry in entries.flatten() {
                    let name = entry.file_name();
                    let target = match name.to_string_lossy().as_ref() {
                        // Leftovers no current code reads.
                        "analysis_cache.json" | "runtime_api_observations_v1" => {
                            let removed = if entry.file_type().is_ok_and(|kind| kind.is_dir()) { fs::remove_dir_all(entry.path()) } else { fs::remove_file(entry.path()) };
                            if let Err(error) = removed { errors.push(format!("{}: {error}", entry.path().display())); }
                            continue;
                        }
                        "covers" | "nexus-browser" | "analyzer_cache.json" | "release_date_cache.json" => layout::join(root,&name),
                        // Webview storage: localStorage, cookies, IndexedDB and its HTTP cache.
                        _ => root.join("webview").join(&name),
                    };
                    if let Err(error) = merge(&entry.path(), &target) { errors.push(error); }
                },
                Err(error) => errors.push(error.to_string()),
            }
            let _ = fs::remove_dir(&old);
        }
    }
    if let Some(cache) = cache {
        // Downloaded update packages are disposable; the empty webview cache folder too.
        let old = cache.join(APP_DIRECTORY);
        if fs::symlink_metadata(old.join("updates")).is_ok_and(|meta| meta.is_dir() && !meta.file_type().is_symlink()) {
            if let Err(error) = fs::remove_dir_all(old.join("updates")) { errors.push(error.to_string()); }
        }
        let _ = fs::remove_dir(&old);
        let _ = fs::remove_dir(cache.join(APP_IDENTIFIER));
    }
    errors
}
/// Covers folder of older versions, kept for rewriting saved cover addresses.
pub fn legacy_covers() -> Option<PathBuf> {
    dirs::data_local_dir().map(|base| base.join(APP_IDENTIFIER).join("covers"))
}

/// Claim launcher ownership without migrating files or advancing reset recovery.
pub fn launcher_root() -> Result<PathBuf, String> {
    // Tests share the home folder (container): never touch the real profile.
    #[cfg(test)]
    let root = std::env::temp_dir().join(format!("peligames-test-profile-{}", std::process::id())).join(APP_DIRECTORY);
    #[cfg(not(test))]
    let root = dirs::config_dir()
        .ok_or("Pasta de configuração indisponível.")?
        .join(APP_DIRECTORY);
    directory(&root)?;
    Ok(root)
}
pub fn app_root() -> Result<PathBuf, String> {
    initialize()?;
    launcher_root()
}

pub fn mod_root() -> Result<PathBuf, String> {
    app_path("Mod/DLSSNR")
}

/// Points the symlink `link` at `target` atomically (a new link renamed over it),
/// so readers never see it missing.
pub fn replace_symlink(link: &Path, target: &Path) -> Result<(), String> {
    let temporary = link.with_file_name(format!(".peligames-link-{}", std::process::id()));
    let _ = fs::remove_file(&temporary);
    std::os::unix::fs::symlink(target, &temporary).map_err(|e| e.to_string())?;
    fs::rename(&temporary, link).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        e.to_string()
    })
}

// Installation manifests may still contain absolute paths to managed runners.
// Resolve them after the directory move without altering external Steam/Heroic paths.
pub fn migrated_runner_path(value: &str) -> String {
    let Some(config) = dirs::config_dir() else {
        return value.into();
    };
    if let Ok(relative) = Path::new(value).strip_prefix(config.join(APP_DIRECTORY)) {
        let current = layout::join(&config.join(APP_DIRECTORY), relative);
        if current.exists() { return current.to_string_lossy().into_owned(); }
    }
    if let Ok(relative) = Path::new(value).strip_prefix(config.join("peligames")) {
        if relative.starts_with("runners") {
            let current = layout::join(&config.join(APP_DIRECTORY), relative);
            if current.exists() {
                return current.to_string_lossy().into_owned();
            }
        }
    }
    value.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scattered_data_moves_into_the_launcher_folder() {
        let base = std::env::temp_dir().join(format!("peligames-paths-{}", uuid::Uuid::new_v4()));
        let (root, local, cache) = (base.join("config/PeliGames"), base.join("local"), base.join("cache"));
        let old = local.join(APP_IDENTIFIER);
        for folder in ["covers", "nexus-browser", "localstorage", "WebKitCache", "runtime_api_observations_v1"] { fs::create_dir_all(old.join(folder)).unwrap(); }
        fs::write(old.join("covers/a.png"), b"cover").unwrap();
        fs::write(old.join("localstorage/file.sqlite"), b"storage").unwrap();
        fs::write(old.join("analyzer_cache.json"), b"{}").unwrap();
        fs::write(old.join("analysis_cache.json"), b"{}").unwrap();
        fs::create_dir_all(cache.join("PeliGames/updates")).unwrap();
        fs::write(cache.join("PeliGames/updates/old.AppImage"), b"x").unwrap();
        fs::create_dir_all(cache.join(APP_IDENTIFIER)).unwrap();
        fs::create_dir_all(&root).unwrap();
        // A newer file already in place wins over the old copy.
        fs::create_dir_all(root.join("webview/localstorage")).unwrap();
        fs::write(root.join("webview/localstorage/file.sqlite"), b"new").unwrap();
        assert!(migrate_scattered_data_at(&root, Some(&local), Some(&cache)).is_empty());
        assert_eq!(fs::read(layout::join(&root,"covers/a.png")).unwrap(), b"cover");
        assert!(layout::join(&root,"nexus-browser").is_dir() && layout::join(&root,"analyzer_cache.json").is_file() && root.join("webview/WebKitCache").is_dir());
        assert_eq!(fs::read(root.join("webview/localstorage/file.sqlite")).unwrap(), b"new");
        assert!(!root.join("analysis_cache.json").exists() && !root.join("webview/runtime_api_observations_v1").exists());
        assert!(!cache.join("PeliGames").exists() && !cache.join(APP_IDENTIFIER).exists());
        // The conflicting old file stays for manual recovery; everything else left.
        assert_eq!(fs::read(old.join("localstorage/file.sqlite")).unwrap(), b"storage");
        assert!(!old.join("covers").exists());
        // Running again with nothing left is harmless.
        fs::remove_dir_all(&old).unwrap();
        assert!(migrate_scattered_data_at(&root, Some(&local), Some(&cache)).is_empty());
        fs::remove_dir_all(base).unwrap();
    }
}
