use std::{fs, io::Read, path::{Path, PathBuf}};

fn enabled_in_ini(text: &str) -> Option<bool> {
    let mut section = false;
    let mut result = None;
    for line in text.lines() {
        let line = line.split([';', '#']).next().unwrap_or("").trim();
        if line.starts_with('[') { section = line.trim_matches(['[', ']']).trim().eq_ignore_ascii_case("DlssNr"); }
        else if section {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("Enabled") {
                    result = match value.trim().to_ascii_lowercase().as_str() { "true" | "1" | "yes" => Some(true), "false" | "0" | "no" => Some(false), _ => None };
                }
            }
        }
    }
    result
}

fn with_enabled(text: &str, enabled: bool) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let setting = format!("Enabled={}", if enabled { "true" } else { "false" });
    let mut lines = Vec::new();
    let mut section = false;
    let mut written = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if section && !written { lines.push(setting.clone()); written = true; }
            section = trimmed.trim_matches(['[', ']']).trim().eq_ignore_ascii_case("DlssNr");
        }
        if section && trimmed.split_once('=').map(|(key, _)| key.trim().eq_ignore_ascii_case("Enabled")).unwrap_or(false) {
            if !written { lines.push(setting.clone()); written = true; }
        } else { lines.push(line.to_string()); }
    }
    if !written {
        if !section { lines.push("[DlssNr]".into()); }
        lines.push(setting);
    }
    lines.join(newline) + newline
}

const FEED: &str = "DLSS5_Feed@DLSS5_Feed.fx";

fn is_feed(value: &str) -> bool {
    value.trim().split('@').next().unwrap_or("").eq_ignore_ascii_case("DLSS5_Feed")
}

fn techniques_in_preset(text: &str) -> Option<&str> {
    // Techniques belongs to the global section; shader sections may have unrelated keys.
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') { break; }
        if let Some((key, value)) = line.split_once('=') {
            if key.trim().eq_ignore_ascii_case("Techniques") { return Some(value.trim()); }
        }
    }
    None
}

fn with_feed(text: &str, enabled: bool) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut techniques: Vec<&str> = techniques_in_preset(text).unwrap_or("").split(',')
        .map(str::trim).filter(|name| !name.is_empty() && !is_feed(name)).collect();
    if enabled { techniques.push(FEED); }
    let setting = format!("Techniques={}", techniques.join(","));
    let mut result = Vec::new();
    let mut global = true;
    let mut written = false;
    for line in text.lines() {
        if line.trim().starts_with('[') {
            if !written { result.push(setting.clone()); written = true; }
            global = false;
        }
        if global && line.split_once('=').map(|(key, _)| key.trim().eq_ignore_ascii_case("Techniques")).unwrap_or(false) {
            if !written { result.push(setting.clone()); written = true; }
        } else { result.push(line.to_string()); }
    }
    if !written { result.push(setting); }
    result.join(newline) + newline
}

fn settings_path(game_dir: &Path) -> Result<PathBuf, String> {
    let root = super::installer::installed_mod_directory(game_dir).ok_or("Neural Rendering is not installed in this game")?;
    let optiscaler = root.join("OptiScaler.ini");
    if optiscaler.is_file() { return Ok(optiscaler); }
    let config = read_file(&root.join("ReShade.ini"))?;
    let mut in_general = false;
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('[') { in_general = line.eq_ignore_ascii_case("[GENERAL]"); }
        else if in_general {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("PresetPath") {
                    let value = value.trim().trim_matches('"').replace('\\', "/");
                    let relative = Path::new(&value);
                    if relative.is_absolute() || relative.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                        return Err("ReShade preset must be located inside the game directory".into());
                    }
                    return Ok(root.join(relative));
                }
            }
        }
    }
    Ok(root.join("ReShadePreset.ini"))
}
fn read_file(path: &Path) -> Result<String, String> {
    match fs::File::open(path) {
        Ok(file) => {
            if file.metadata().map_err(|e| e.to_string())?.len() > 1024 * 1024 { return Err("Configuration file is too large".into()); }
            let mut text = String::new();
            file.take(1024 * 1024).read_to_string(&mut text).map_err(|e| e.to_string())?;
            Ok(text)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn write_settings(path: &Path, text: &str) -> Result<(), String> {
    if fs::symlink_metadata(path).map(|meta| meta.file_type().is_symlink()).unwrap_or(false) { return Err("Redirected configuration files are not supported".into()); }
    let temporary = path.with_file_name(format!(".dlssnr-settings-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        fs::write(&temporary, text).map_err(|e| e.to_string())?;
        if let Ok(meta) = fs::metadata(path) { fs::set_permissions(&temporary, meta.permissions()).map_err(|e| e.to_string())?; }
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    let _ = fs::remove_file(temporary);
    result
}

pub fn set_enabled(game_dir: &Path, enabled: bool) -> Result<(), String> {
    let path = settings_path(game_dir)?;
    let root = super::installer::installed_mod_directory(game_dir).ok_or("Neural Rendering is not installed in this game")?;
    let backend = root.join("dlssnr-amd.ini");
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical_parent = path.parent().ok_or("Invalid configuration path")?.canonicalize().map_err(|e| e.to_string())?;
    if !canonical_parent.starts_with(&canonical_root) { return Err("Redirected configuration directories are not supported".into()); }
    let text = read_file(&path)?;
    let updated = if path == root.join("OptiScaler.ini") {
        with_enabled(&text, enabled)
    } else {
        if !path.is_file() { return Err("ReShade preset was not found".into()); }
        with_feed(&text, enabled)
    };
    // Keep the backend available, including installations disabled by the old launcher.
    // The visible checkbox is controlled by OptiScaler or by the ReShade technique list.
    let backend_text = read_file(&backend)?;
    if enabled_in_ini(&backend_text) != Some(true) {
        write_settings(&backend, &with_enabled(&backend_text, true))?;
    }
    write_settings(&path, &updated)
}
#[tauri::command]
pub async fn get_neural_startup(game_dir: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = settings_path(Path::new(&game_dir))?;
        let text = read_file(&path)?;
        if path.file_name().map(|name| name == "OptiScaler.ini").unwrap_or(false) {
            Ok(enabled_in_ini(&text).unwrap_or(true))
        } else {
            Ok(techniques_in_preset(&text).map(|list| list.split(',').any(is_feed)).unwrap_or(false))
        }
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_neural_startup(app: tauri::AppHandle, game_dir: String, enabled: bool) -> Result<(), String> {
    let result = tauri::async_runtime::spawn_blocking(move || set_enabled(Path::new(&game_dir), enabled)).await.map_err(|e| e.to_string())?;
    crate::core::logger::log_result(&app, "Preferência de inicialização do Neural Rendering", &result);
    result
}
