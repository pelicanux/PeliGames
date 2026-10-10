use crate::core::config_manager::{preserve_saved_preferences, save_config, AppConfig};

#[tauri::command]
pub fn load_app_config(app: tauri::AppHandle) -> Result<Option<AppConfig>, String> {
    let config = crate::core::config_manager::load_config_result()?;
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
    use tauri::Manager;
    let cache_dir = app.path().app_local_data_dir().map_err(|error| error.to_string())?;
    let result = tauri::async_runtime::spawn_blocking(move || crate::core::game_info_cache::clear_files(&cache_dir))
        .await.map_err(|error| error.to_string())?;
    crate::core::logger::log_result(&app, "Limpeza das informações em cache", &result);
    result
}

// The frontend cannot provide a deletion path. Never follow a redirected app folder.
fn remove_launcher_directory(config_root: &std::path::Path) -> Result<(), String> {
    let target = config_root.join(crate::core::paths::APP_DIRECTORY);
    match std::fs::symlink_metadata(&target) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() =>
            return Err("The launcher configuration folder is not a regular directory".to_string()),
        Ok(_) => {},
    }
    // Reset settings and mod data; preserve the installed library and runners.
    for file in ["config.json", "launcher.log", "launcher.previous.log"] {
        let path = target.join(file);
        match std::fs::remove_file(path) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error.to_string()),
        }
    }
    let mods = target.join("Mod");
    if let Ok(meta) = std::fs::symlink_metadata(&mods) {
        if meta.file_type().is_symlink() || !meta.is_dir() { return Err("A pasta Mod não é um diretório regular.".into()); }
        let mod_data = mods.join("DLSSNR");
        match std::fs::symlink_metadata(&mod_data) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => std::fs::remove_dir_all(mod_data).map_err(|e| e.to_string())?,
            Ok(_) => return Err("A pasta do mod não é um diretório regular.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn emergency_reset_launcher(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let config_root = dirs::config_dir().ok_or("Configuration directory unavailable")?;
    let cache_dir = app.path().app_local_data_dir().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::game_info_cache::clear_files(&cache_dir)?;
        remove_launcher_directory(&config_root)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn restart_after_emergency_reset(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let executable = tauri::process::current_binary(&app.env()).map_err(|error| error.to_string())?;
    let mut command = std::process::Command::new(executable);
    // AppImage must mount its own fresh runtime instead of reusing the old mount.
    if std::env::var_os("APPIMAGE").is_some() {
        command.env_remove("APPIMAGE").env_remove("APPDIR")
            .env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD");
    }
    command.env("PELIGAMES_RESTART_PARENT_PID", std::process::id().to_string());
    command.spawn().map_err(|error| format!("Failed to restart launcher: {error}"))?;
    crate::core::logger::log_launcher(&app, "INFO", "Redefinição de emergência concluída; reiniciando para o wizard");
    app.exit(0);
    Ok(())
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
