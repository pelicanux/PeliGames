use crate::core::pelinstall::StartupInfo;
#[tauri::command]
pub fn get_startup_context(info: tauri::State<'_, StartupInfo>) -> StartupInfo {
    info.inner().clone()
}
#[tauri::command]
pub fn validate_pelinstall_file(path: String) -> Result<String, String> {
    let absolute = if std::path::Path::new(&path).is_absolute() {
        std::path::PathBuf::from(path)
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(path)
    };
    crate::core::game_installation::validate_executable(&absolute.to_string_lossy())
        .map(|p| p.to_string_lossy().into_owned())
}
#[tauri::command]
pub async fn create_peligames_shortcut(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::pelinstall::create_shortcut(&path))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn open_peligames_launcher(path: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::pelinstall::open_launcher(path.as_deref()))
        .await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn find_pelinstall_matches(executable: String) -> Result<Vec<crate::core::installed_library::ExecutableMatch>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::find_executable(&executable)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn find_peligames_shortcuts(path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::pelinstall::shortcut_paths(&path)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn remove_peligames_shortcuts(path: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::pelinstall::remove_shortcuts(&path)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn appimage_install_status() -> Result<crate::core::pelinstall::AppImageStatus, String> {
    crate::core::pelinstall::appimage_status()
}
/// Installs the running AppImage extracted in `~/.local/share/PeliGames`,
/// opens the installed copy and closes this one (as an update does).
#[tauri::command]
pub async fn install_peligames_appimage(app: tauri::AppHandle) -> Result<(), String> {
    let target = tauri::async_runtime::spawn_blocking(crate::core::pelinstall::install_appimage).await.map_err(|e| e.to_string())??;
    // Nexus links registered from the AppImage file now open the installed copy.
    if let Err(error) = super::nexus_downloads::refresh_nexus_handler(&target) { crate::core::logger::log_launcher(&app, "WARN", &format!("Links Nexus: {error}")); }
    std::process::Command::new(&target).env("PELIGAMES_RESTART_PARENT_PID", std::process::id().to_string())
        .env_remove("APPIMAGE").env_remove("APPDIR").env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD")
        .spawn().map_err(|e| format!("PeliGames instalado em {}, mas não foi possível abri-lo: {e}", target.display()))?;
    crate::core::logger::log_launcher(&app, "INFO", &format!("AppImage instalado em {}; reiniciando", target.display()));
    app.exit(0);
    Ok(())
}
#[tauri::command]
pub async fn get_pelinstall_icon(path: String) -> Result<Option<Vec<u8>>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::executable_icon::extract_png(std::path::Path::new(&path)).ok()).await.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn set_peligames_cover(path: String, cover_url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::set_cover(&path,&cover_url)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn launch_pelinstall_entry(path: String) -> Result<(), String> {
    crate::core::pelinstall::launch_entry(&path)
}

#[tauri::command]
pub async fn update_peligames_shortcuts(path: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::pelinstall::update_shortcuts(&path)).await.map_err(|e| e.to_string())?
}

#[derive(Default)]
pub struct LauncherOpenRequest(pub std::sync::Mutex<Option<String>>);

#[tauri::command]
pub fn take_launcher_open_request(request: tauri::State<'_, LauncherOpenRequest>) -> Option<String> {
    request.0.lock().unwrap_or_else(|e| e.into_inner()).take()
}

#[derive(serde::Serialize)]
pub struct DroppedExecutables { direct_file: bool, paths: Vec<String>, limited: bool, recommended: Option<String>, excluded: usize }
fn dropped_executables(path: &str, name: &str, adding: bool) -> Result<DroppedExecutables, String> {
    use std::{collections::VecDeque, path::PathBuf};
    let root = std::fs::canonicalize(path).map_err(|e| format!("Não foi possível abrir o caminho: {e}"))?;
    if !root.is_dir() {
        return crate::core::game_installation::validate_executable(&root.to_string_lossy())
            .map(|path| DroppedExecutables { direct_file: true, paths: vec![path.to_string_lossy().into_owned()], limited: false, recommended: None, excluded: 0 });
    }
    if adding {
        let result=crate::core::installed_library::discover_game_executables(&root, 5, 10000, 100)?;
        let files=result.files.into_iter().map(|path|path.to_string_lossy().into_owned()).collect();
        let (paths,recommended)=crate::core::installed_library::rank_dropped_executables(files,name);
        return Ok(DroppedExecutables {direct_file:false, paths, recommended:if result.limited {None} else {recommended}, limited:result.limited, excluded:result.excluded});
    }
    let mut queue = VecDeque::from([(root, 0usize)]);
    let mut paths = Vec::new(); let mut visited = 0; let mut limited = false;
    while let Some((directory, depth)) = queue.pop_front() {
        let entries = std::fs::read_dir(&directory).map_err(|e| format!("Não foi possível ler a pasta: {e}"))?;
        for entry in entries {
            visited += 1;
            if visited > 10000 || paths.len() >= 100 { limited = true; break; }
            let entry = entry.map_err(|e| e.to_string())?;
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() { continue; }
            let path: PathBuf = entry.path();
            if kind.is_dir() {
                if depth < 5 { queue.push_back((path, depth + 1)); } else { limited = true; } }
            else if kind.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("msi")) {
                paths.push(path.to_string_lossy().into_owned());
            }
        }
        if visited > 10000 || paths.len() >= 100 { break; }
    }
    paths.sort();
    Ok(DroppedExecutables {direct_file:false, paths, limited, recommended:None, excluded:0})
}
#[tauri::command]
pub async fn inspect_dropped_executables(path: String, name: String, mode: String) -> Result<DroppedExecutables, String> {
    if !matches!(mode.as_str(), "add" | "install") { return Err("Ação de importação inválida.".into()); }
    tauri::async_runtime::spawn_blocking(move || dropped_executables(&path, &name, mode == "add")).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod drop_tests {
    use super::*;
    #[test]
    fn adding_uses_installation_filters_and_ranking_without_a_prefix() {
        let dir = std::env::temp_dir().join(format!("peligames-shared-drop-{}", uuid::Uuid::new_v4()));
        for file in ["Start.exe", "Game/Binaries/Win64/Game-Win64-Shipping.exe",
            "UEPrereqSetup_x64.exe", "UE4PrereqSetup_x64.exe", "NDP452-KB2901907.exe",
            "Prerequisites/runtime.exe", "package cache/runtime.exe"] {
            let path=dir.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path,b"MZ").unwrap();
        }
        std::fs::create_dir_all(dir.join("Engine")).unwrap();
        let result=dropped_executables(dir.to_str().unwrap(), "Unrelated title", true).unwrap();
        assert_eq!(result.paths.len(),2);
        assert_eq!(result.recommended,Some(dir.join("Start.exe").to_string_lossy().into_owned()));
        let shared=crate::core::installed_library::discover_game_executables(&dir,24,100_000,usize::MAX).unwrap();
        assert_eq!(shared.files.into_iter().map(|path|path.to_string_lossy().into_owned()).collect::<std::collections::BTreeSet<_>>(),result.paths.into_iter().collect());
        // Shipping alone is still a valid game; equal bootstraps require a manual choice.
        std::fs::remove_file(dir.join("Start.exe")).unwrap();
        let shipping=dropped_executables(dir.to_str().unwrap(), "Unrelated title", true).unwrap();
        assert!(shipping.recommended.unwrap().ends_with("Game-Win64-Shipping.exe"));
        for file in ["First.exe","Second.exe"] { std::fs::write(dir.join(file),b"MZ").unwrap(); }
        let tied=dropped_executables(dir.to_str().unwrap(), "Unrelated title", true).unwrap();
        assert!(tied.recommended.is_none());
        assert!(!dir.join("prefix").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn adding_final_fantasy_folder_excludes_nested_redists_and_game_settings() {
        let dir = std::env::temp_dir().join(format!("peligames-ffxii-drop-{}", uuid::Uuid::new_v4()));
        let nested = "Final.Fantasy.XII.The.Zodiac.Age.v1.0.4.0";
        let files = [
            format!("{nested}/_CommonRedist/DotNet/4.5.2/NDP452-KB2901907-x86-x64-AllOS-ENU.exe"),
            format!("{nested}/_CommonRedist/vcredist/2015/vc_redist.x64.exe"),
            format!("{nested}/_CommonRedist/vcredist/2015/vc_redist.x86.exe"),
            format!("{nested}/x64/FFXII_TZA.exe"),
            format!("{nested}/x64/FFXII_TZA_GameSetting.exe"),
            "_Redist/dotNetFx40_Full_setup.exe".into(),
            "_Redist/runtime.exe".into(),
        ];
        for file in &files {
            let path=dir.join(file); std::fs::create_dir_all(path.parent().unwrap()).unwrap(); std::fs::write(path,b"MZ").unwrap();
        }
        let adding=dropped_executables(dir.to_str().unwrap(), "Final Fantasy XII", true).unwrap();
        assert_eq!(adding.paths, vec![dir.join(format!("{nested}/x64/FFXII_TZA.exe")).to_string_lossy().into_owned()]);
        assert_eq!(adding.recommended.as_deref(), adding.paths.first().map(String::as_str));
        assert!(!adding.limited);
        let installing=dropped_executables(dir.to_str().unwrap(), "Final Fantasy XII", false).unwrap();
        assert_eq!(installing.paths.len(), files.len());
        assert!(installing.recommended.is_none());
        // An explicit file remains selectable even if normally hidden as a tool.
        let manual=dropped_executables(dir.join(format!("{nested}/x64/FFXII_TZA_GameSetting.exe")).to_str().unwrap(), "Configuração", true).unwrap();
        assert_eq!(manual.paths.len(), 1);
        assert!(manual.direct_file);
        assert!(!adding.direct_file);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn folder_drop_lists_nested_executables_without_following_symlinks() {
        let dir = std::env::temp_dir().join(format!("peligames-drop-test-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::create_dir(dir.as_path().join("bin")).unwrap();
        std::fs::write(dir.as_path().join("bin/game.EXE"), b"MZ").unwrap();
        std::fs::write(dir.as_path().join("readme.txt"), b"text").unwrap();
        for file in ["setup.exe", "unins000.exe", "UnityCrashHandler64.exe", "vc_redist.x64.exe"] { std::fs::write(dir.join(file), b"MZ").unwrap(); }
        std::fs::create_dir(dir.join("_CommonRedist")).unwrap();
        std::fs::write(dir.join("_CommonRedist/runtime.exe"), b"MZ").unwrap();
        std::os::unix::fs::symlink("/tmp", dir.as_path().join("outside")).unwrap();
        let result = dropped_executables(dir.as_path().to_str().unwrap(), "game", true).unwrap();
        assert_eq!(result.paths.len(), 1); assert!(result.paths[0].ends_with("game.EXE"));
        assert!(!result.limited);
        assert_eq!(result.excluded, 4);
        assert!(result.recommended.as_ref().unwrap().ends_with("game.EXE"));
        let install = dropped_executables(dir.to_str().unwrap(), "game", false).unwrap();
        assert!(install.paths.iter().any(|path| path.ends_with("setup.exe")));
        assert!(install.paths.iter().any(|path| path.ends_with("runtime.exe")));
        assert!(install.recommended.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
