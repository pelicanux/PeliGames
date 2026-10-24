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
            if entry.file_name().to_string_lossy().starts_with('.') { continue; }
            // Steam aliases delegate to an existing runner; they are not another Proton.
            if path.join(".peligames-proton-path").symlink_metadata().is_ok() { continue; }
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

fn system_wines(paths: impl IntoIterator<Item = PathBuf>) -> Vec<InstalledProton> {
    let mut seen=BTreeSet::new();let mut found=Vec::new();
    for directory in paths {
        if !directory.is_absolute() { continue; }
        for name in ["wine","wine64"] {
            let path=directory.join(name);
            if crate::core::game_installation::native_wine(&path).is_none() { continue; }
            let Ok(real)=path.canonicalize() else { continue };
            if seen.insert(real) {found.push(InstalledProton{name:format!("Wine do sistema ({name})"),path:path.to_string_lossy().into_owned()});}
        }
    }
    found
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
        let mut found=scan_roots(roots);
        { // System Wine is always included when installed.
            let mut paths:Vec<PathBuf>=std::env::var_os("PATH").map(|path|std::env::split_paths(&path).collect()).unwrap_or_default();
            paths.extend([PathBuf::from("/usr/bin"),PathBuf::from("/usr/local/bin")]);
            found.extend(system_wines(paths));
        }
        Ok(found)
    })
    .await
    .map_err(|error| error.to_string())?
}


use crate::core::proton_manager::{self, Family, RunnerRelease};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::Emitter;

#[derive(Default)]
pub struct ProtonDownloadState(Mutex<Option<Arc<AtomicBool>>>);
pub(super) fn download_active(state: &ProtonDownloadState) -> bool { state.0.lock().map(|slot| slot.is_some()).unwrap_or(true) }
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

#[tauri::command]
pub async fn uninstall_proton(
    state: tauri::State<'_, ProtonDownloadState>,
    family: Family,
    version: String,
) -> Result<(), String> {
    {
        let mut current = state.inner().0.lock().map_err(|e| e.to_string())?;
        if current.is_some() { return Err("Uma operação de Proton já está em andamento.".into()); }
        *current = Some(Arc::new(AtomicBool::new(false)));
    }
    let _guard = DownloadGuard(state.inner());
    let root = proton_manager::runners_dir()?;
    tauri::async_runtime::spawn_blocking(move || proton_manager::uninstall_at(family, &root, &version))
        .await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod discovery_tests {
    use super::*;
    #[test]
    fn system_wine_discovery_deduplicates_links_and_preserves_the_command_path() {
        use std::os::unix::fs::{PermissionsExt,symlink};
        let root=std::env::temp_dir().join(format!("peligames-system-wine-{}",uuid::Uuid::new_v4()));
        let bin=root.join("bin");let lib=root.join("lib");std::fs::create_dir_all(&bin).unwrap();std::fs::create_dir_all(&lib).unwrap();
        let real=lib.join("wine-loader");std::fs::write(&real,"#!/bin/sh\nexit 99\n").unwrap();std::fs::set_permissions(&real,std::fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&real,bin.join("wine")).unwrap();symlink(&real,bin.join("wine64")).unwrap();
        assert!(system_wines([bin.clone()]).is_empty(),"no wineserver means an unusable runner");
        std::fs::write(bin.join("wineserver"),"fixture").unwrap();std::fs::set_permissions(bin.join("wineserver"),std::fs::Permissions::from_mode(0o755)).unwrap();
        let found=system_wines([bin.clone(),bin.clone()]);assert_eq!(found.len(),1);
        assert_eq!(found[0].path,bin.join("wine").to_string_lossy());
        let runner=crate::core::game_installation::validate_proton(&found[0].path).unwrap();assert_eq!(runner,bin.join("wine"));
        assert_eq!(crate::core::game_installation::runner_backend(&found[0].path).unwrap(),runner);
        let mut command=std::process::Command::new(&runner);crate::core::game_installation::wine_environment(&mut command,&runner,&root.join("prefix"));
        assert!(command.get_envs().any(|(key,value)|key=="WINESERVER" && value==Some(bin.join("wineserver").as_os_str())));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn steam_aliases_are_hidden_without_removing_or_renaming_real_runners(){
        let root=std::env::temp_dir().join(format!("peligames-proton-discovery-{}",uuid::Uuid::new_v4()));
        let real=root.join("GE-Proton-Test");let alias=root.join("peligames-1234567890abcdef");
        std::fs::create_dir_all(&real).unwrap();std::fs::create_dir(&alias).unwrap();
        std::fs::write(real.join("proton"),"original").unwrap();
        std::os::unix::fs::symlink(real.join("proton"),alias.join("proton")).unwrap();
        std::fs::write(alias.join(".peligames-proton-path"),real.to_str().unwrap()).unwrap();
        let found=scan_roots([root.clone()]);assert_eq!(found.len(),1);assert_eq!(found[0].name,"GE-Proton-Test");assert_eq!(found[0].path,real.canonicalize().unwrap().to_string_lossy());
        assert!(alias.join("proton").is_file());assert_eq!(std::fs::read_to_string(real.join("proton")).unwrap(),"original");
        std::fs::remove_dir_all(root).unwrap();
    }
}
