use crate::core::config_manager::{preserve_saved_preferences, save_config, AppConfig};

#[tauri::command]
pub fn load_app_config(app: tauri::AppHandle) -> Result<Option<AppConfig>, String> {
    let mut config = crate::core::config_manager::load_config_result()?;
    if let Some(ref mut current) = config {
        let root = crate::core::paths::mod_root()?;
        if std::path::Path::new(&current.dll_version).parent() == Some(root.as_path()) && current.dll_version.ends_with(".bin") {
            let gpu = if current.backend == "AMDNR" { "rdna4" } else { "rdna3" };
            let canonical = ["64", "32"].into_iter().map(|bits| root.join("backend-files").join(gpu).join(bits).join("dlssnr-amd/dlssnr.bin")).find(|file| file.is_file());
            if let Some(file) = canonical { current.dll_version = file.to_string_lossy().to_string(); save_config(current)?; }
        }
    }
    crate::core::logger::log_launcher(&app, "INFO", if config.is_some() { "Configuração carregada" } else { "Sem configuração válida; abrindo wizard inicial" });
    Ok(config)
}

#[tauri::command]
pub fn save_app_config(mut config: AppConfig) -> Result<(), String> {
    if let Some(previous) = crate::core::config_manager::load_config_result()? {
        preserve_saved_preferences(&mut config, previous);
    }
    if let Some(key) = &mut config.steamgriddb_api_key { *key = key.trim().to_string(); }
    save_config(&config)?;
    Ok(())
}

#[tauri::command]
pub fn delete_app_config() -> Result<(), String> {
    crate::core::config_manager::delete_config()
}

#[tauri::command]
pub fn save_custom_game_path(game_name: String, new_path: String) -> Result<(), String> {
    let mut config = crate::core::config_manager::load_config_result()?.unwrap_or_default();
    config.custom_game_paths.insert(game_name, new_path);
    crate::core::config_manager::save_config(&config)
}

#[tauri::command]
pub async fn clear_game_caches(app: tauri::AppHandle) -> Result<(), String> {
    let cache_dir = crate::core::paths::app_root()?;
    let result = tauri::async_runtime::spawn_blocking(move || crate::core::game_info_cache::clear_files(&cache_dir))
        .await.map_err(|error| error.to_string())?;
    crate::core::logger::log_result(&app, "Limpeza das informações em cache", &result);
    result
}

#[tauri::command]
pub fn log_cached_library(app: tauri::AppHandle, count: usize) {
    crate::core::logger::log_launcher(&app, "INFO", &format!("Biblioteca carregada do cache: {count} jogos; sem nova varredura"));
}

#[tauri::command]
pub fn migrate_neural_preferences(preferences: std::collections::HashMap<String, bool>) -> Result<(), String> {
    crate::core::config_manager::migrate_neural_preferences(preferences)
}

#[tauri::command]
pub fn load_neural_preferences() -> Result<std::collections::HashMap<String, bool>, String> {
    crate::core::config_manager::load_neural_preferences()
}

#[tauri::command]
pub async fn get_launcher_log() -> Result<crate::core::logger::LauncherLog, String> {
    tauri::async_runtime::spawn_blocking(crate::core::logger::read_launcher_log)
        .await.map_err(|error| error.to_string())?
}
