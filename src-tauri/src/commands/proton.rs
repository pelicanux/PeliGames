//! Discovery and Tauri adapters for the independent Proton runner manager.
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

#[derive(Serialize)]
pub struct InstalledProton {
    pub name: String,
    pub path: String,
}

fn scan_roots(roots: impl IntoIterator<Item = PathBuf>) -> Vec<InstalledProton> {
    let mut found = BTreeMap::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() || !path.join("proton").is_file() {
                continue;
            }
            let Ok(canonical) = path.canonicalize() else {
                continue;
            };
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            found
                .entry(canonical.to_string_lossy().into_owned())
                .or_insert(name);
        }
    }
    let mut result: Vec<_> = found
        .into_iter()
        .map(|(path, name)| InstalledProton { name, path })
        .collect();
    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    result
}

fn steam_libraries(home: &Path) -> BTreeSet<PathBuf> {
    let mut libraries = BTreeSet::from([
        dirs::data_dir().unwrap_or_else(|| home.join(".local/share")).join("Steam"),
        home.join(".steam/steam"),
        home.join(".steam/root"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ]);
    for steam in libraries.clone() {
        for relative in ["config/libraryfolders.vdf", "steamapps/libraryfolders.vdf"] {
            if let Ok(vdf) = std::fs::read_to_string(steam.join(relative)) {
                for line in vdf.lines() {
                    let fields: Vec<_> = line.split('"').collect();
                    if fields.get(1) == Some(&"path") {
                        if let Some(path) = fields.get(3) {
                            libraries.insert(PathBuf::from(path.replace("\\\\", "/")));
                        }
                    }
                }
            }
        }
    }
    libraries
}

#[tauri::command]
pub async fn scan_installed_protons() -> Result<Vec<InstalledProton>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let home = dirs::home_dir().ok_or("Não foi possível localizar a pasta pessoal.")?;
        let mut roots = vec![
            crate::core::proton_manager::runners_dir()?.join("ge-proton"),
            crate::core::proton_manager::runners_dir()?.join("cachyos-proton"),
            home.join(".local/share/compatibilitytools.d"),
            home.join(".local/share/lutris/runners/proton"),
            home.join(".config/heroic/tools/proton"),
            home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic/tools/proton"),
            PathBuf::from("/opt/proton"),
        ];
        if let Some(data) = dirs::data_dir() {
            roots.push(data.join("lutris/runners/proton"));
        }
        let scan_steam = crate::core::config_manager::load_config()
            .and_then(|config| config.scan_steam_protons).unwrap_or(false);
        if scan_steam {
            roots.extend([PathBuf::from("/usr/share/steam/compatibilitytools.d"), PathBuf::from("/usr/local/share/steam/compatibilitytools.d")]);
            if let Some(data) = dirs::data_dir() { roots.push(data.join("Steam/compatibilitytools.d")); }
            for library in steam_libraries(&home) {
                roots.push(library.join("compatibilitytools.d"));
                roots.push(library.join("steamapps/common"));
            }
        }
        Ok(scan_roots(roots))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_proton_skips_other_folders_and_deduplicates_roots() {
        let root =
            std::env::temp_dir().join(format!("peligames-proton-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("GE-Proton-Test")).unwrap();
        std::fs::create_dir_all(root.join("OtherGame")).unwrap();
        std::fs::write(root.join("GE-Proton-Test/proton"), "test fixture").unwrap();
        let found = scan_roots([root.clone(), root.clone(), root.join("missing")]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "GE-Proton-Test");
        assert_eq!(
            PathBuf::from(&found[0].path),
            root.join("GE-Proton-Test").canonicalize().unwrap()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

use crate::core::proton_manager::{self, Family, RunnerRelease};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::Emitter;

#[derive(Default)]
pub struct ProtonDownloadState(Mutex<Option<Arc<AtomicBool>>>);
struct DownloadGuard<'a>(&'a ProtonDownloadState);
impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.0 .0.lock() {
            *state = None;
        }
    }
}

#[tauri::command]
pub async fn check_proton_release(family: Family) -> Result<RunnerRelease, String> {
    proton_manager::check_at(family, &proton_manager::runners_dir()?).await
}
#[tauri::command]
pub async fn install_proton(
    app: tauri::AppHandle,
    state: tauri::State<'_, ProtonDownloadState>,
    family: Family,
) -> Result<String, String> {
    let flag = Arc::new(AtomicBool::new(false));
    {
        let mut current = state.inner().0.lock().map_err(|e| e.to_string())?;
        if current.is_some() {
            return Err("Um runner já está sendo instalado.".into());
        }
        *current = Some(flag.clone());
    }
    let _guard = DownloadGuard(state.inner());
    proton_manager::install_at(
        family,
        proton_manager::runners_dir()?,
        flag,
        Arc::new(move |progress| {
            let _ = app.emit("proton-download-progress", progress);
        }),
    )
    .await
}
#[tauri::command]
pub fn cancel_proton_install(state: tauri::State<'_, ProtonDownloadState>) -> Result<(), String> {
    if let Some(flag) = state.inner().0.lock().map_err(|e| e.to_string())?.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}
