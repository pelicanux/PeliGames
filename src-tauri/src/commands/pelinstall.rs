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
pub fn open_peligames_launcher(path: Option<String>) -> Result<(), String> {
    crate::core::pelinstall::open_launcher(path.as_deref())
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
