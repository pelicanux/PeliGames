use std::{fs, io::Read, path::{Path, PathBuf}};

fn enabled_in_ini(text: &str) -> Option<bool> {
    bool_in_section(text, "DlssNr", "Enabled")
}
fn bool_in_section(text: &str, target: &str, option: &str) -> Option<bool> {
    let mut section = false;
    let mut result = None;
    for line in text.lines() {
        let line = line.split([';', '#']).next().unwrap_or("").trim();
        if line.starts_with('[') { section = line.trim_matches(['[', ']']).trim().eq_ignore_ascii_case(target); }
        else if section {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case(option) {
                    result = match value.trim().to_ascii_lowercase().as_str() { "true" | "1" | "yes" => Some(true), "false" | "0" | "no" => Some(false), _ => None };
                }
            }
        }
    }
    result
}

fn with_enabled(text: &str, enabled: bool) -> String {
    with_section_bool(text, "DlssNr", "Enabled", enabled, false)
}
fn with_section_bool(text: &str, target: &str, option: &str, enabled: bool, numeric: bool) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let setting = format!("{option}={}", if numeric { if enabled { "1" } else { "0" } } else if enabled { "true" } else { "false" });
    let mut lines = Vec::new();
    let mut section = false;
    let mut written = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if section && !written { lines.push(setting.clone()); written = true; }
            section = trimmed.trim_matches(['[', ']']).trim().eq_ignore_ascii_case(target);
        }
        if section && trimmed.split_once('=').map(|(key, _)| key.trim().eq_ignore_ascii_case(option)).unwrap_or(false) {
            if !written { lines.push(setting.clone()); written = true; }
        } else { lines.push(line.to_string()); }
    }
    if !written {
        if !section { lines.push(format!("[{target}]")); }
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
fn lumenite_in_ini(text: &str) -> bool {
    let mut section = false;
    let mut enabled = false;
    for line in text.lines() {
        let line = line.split([';', '#']).next().unwrap_or("").trim();
        if line.starts_with('[') { section = line.trim_matches(['[', ']']).trim().eq_ignore_ascii_case("DlssNr"); }
        else if section {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("MotionVectors") { enabled = value.trim().eq_ignore_ascii_case("lumenite"); }
            }
        }
    }
    enabled
}
pub(super) fn lumenite_selected(game_dir: &Path) -> Result<bool, String> {
    Ok(lumenite_in_ini(&read_file(&game_dir.join("dlssnr-amd.ini"))?))
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

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct NeuralSettings {
    pub enabled: bool,
    pub renodx: bool,
}

fn get_settings(game_dir: &Path) -> Result<NeuralSettings, String> {
    let root = super::installer::installed_mod_directory(game_dir).ok_or("Neural Rendering is not installed in this game")?;
    let backend_text = read_file(&root.join("dlssnr-amd.ini"))?;
    let renodx = bool_in_section(&backend_text, "RenoDX", "Enabled").unwrap_or(false);
    let enabled = if renodx {
        bool_in_section(&backend_text, "RenoDX", "NeuralUplift").unwrap_or(true)
    } else {
        let path = settings_path(game_dir)?;
        let text = read_file(&path)?;
        if path == root.join("OptiScaler.ini") {
            enabled_in_ini(&text).unwrap_or(false)
        } else {
            enabled_in_ini(&backend_text).unwrap_or(true)
                && techniques_in_preset(&text).map(|list| list.split(',').any(is_feed)).unwrap_or(false)
        }
    };
    Ok(NeuralSettings { enabled, renodx })
}

fn set_options(game_dir: &Path, enabled: bool, renodx: Option<bool>) -> Result<(), String> {
    let path = settings_path(game_dir)?;
    let root = super::installer::installed_mod_directory(game_dir).ok_or("Neural Rendering is not installed in this game")?;
    let backend = root.join("dlssnr-amd.ini");
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical_parent = path.parent().ok_or("Invalid configuration path")?.canonicalize().map_err(|e| e.to_string())?;
    if !canonical_parent.starts_with(&canonical_root) { return Err("Redirected configuration directories are not supported".into()); }
    let text = read_file(&path)?;
    let backend_text = read_file(&backend)?;
    let optiscaler = path == root.join("OptiScaler.ini");
    if !optiscaler && !path.is_file() { return Err("ReShade preset was not found".into()); }
    // Keep both routes ready for the next run, including after a RenoDX mode change.
    // In RenoDX mode OptiScaler's NR switch is ignored; NeuralUplift owns NR.
    let mode = renodx.unwrap_or_else(|| bool_in_section(&backend_text, "RenoDX", "Enabled").unwrap_or(false));
    let mut updated_backend = with_section_bool(&backend_text, "RenoDX", "NeuralUplift", enabled, true);
    if renodx.is_some() {
        updated_backend = with_section_bool(&updated_backend, "RenoDX", "Enabled", mode, true);
    }
    // On ReShade, the preset owns classic startup visibility. Disabling the
    // add-on as well would prevent its effect-list checkbox from enabling NR.
    updated_backend = with_enabled(&updated_backend, true);
    let updated = if optiscaler { with_enabled(&text, enabled) } else { with_feed(&text, enabled || mode) };
    // Reject redirected files before writing either configuration.
    for target in [&path, &backend] {
        if fs::symlink_metadata(target).map(|meta| meta.file_type().is_symlink()).unwrap_or(false) {
            return Err("Redirected configuration files are not supported".into());
        }
    }
    write_settings(&path, &updated)?;
    write_settings(&backend, &updated_backend)?;
    let actual = get_settings(game_dir)?;
    if actual != (NeuralSettings { enabled, renodx: mode }) {
        return Err("Neural Rendering configuration could not be confirmed".into());
    }
    Ok(())
}

pub fn set_enabled(game_dir: &Path, enabled: bool) -> Result<(), String> {
    set_options(game_dir, enabled, None)
}
#[tauri::command]
pub async fn get_neural_settings(game_dir: String) -> Result<NeuralSettings, String> {
    tauri::async_runtime::spawn_blocking(move || get_settings(Path::new(&game_dir))).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_neural_startup(app: tauri::AppHandle, game_dir: String, enabled: bool, renodx: Option<bool>) -> Result<(), String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        crate::core::installed_library::ensure_path_writable(Path::new(&game_dir))?;
        set_options(Path::new(&game_dir), enabled, renodx)?;
        crate::core::config_manager::save_neural_preference(game_dir.clone(), enabled)?;
        Ok::<(),String>(())
    }).await.map_err(|e| e.to_string())?;
    crate::core::logger::log_result(&app, "Preferências do Neural Rendering", &result);
    result
}

#[cfg(test)]
mod lumenite_tests {
    use super::*;
    #[test]
    fn installed_motion_vector_choice_is_read_only_from_dlssnr_section() {
        assert!(!lumenite_in_ini(""));
        assert!(lumenite_in_ini("[DlssNr]\n MotionVectors = Lumenite ; selected\n[Network]\nMotionVectors=vort"));
        assert!(!lumenite_in_ini(";MotionVectors=lumenite\n[Network]\nMotionVectors=lumenite"));
        assert!(!lumenite_in_ini("[DlssNr]\nMotionVectors=lumenite\nMotionVectors=vort"));
    }
}

#[cfg(test)]
mod startup_mode_tests {
    use super::*;
    #[test]
    fn renodx_startup_preserves_the_mode_and_other_sections() {
        let original = "[DlssNr]\r\nEnabled=true\r\n[RenoDX]\r\nEnabled = 1\r\nNeuralUplift = 1\r\nNRIntensity=0.8\r\n";
        let disabled = with_section_bool(original, "RenoDX", "NeuralUplift", false, true);
        assert_eq!(bool_in_section(&disabled, "RenoDX", "Enabled"), Some(true));
        assert_eq!(bool_in_section(&disabled, "RenoDX", "NeuralUplift"), Some(false));
        assert_eq!(enabled_in_ini(&disabled), Some(true));
        assert!(disabled.contains("NRIntensity=0.8\r\n"));
        let enabled = with_section_bool(&disabled, "RenoDX", "NeuralUplift", true, true);
        assert_eq!(bool_in_section(&enabled, "RenoDX", "NeuralUplift"), Some(true));
    }
    #[test]
    fn legacy_and_feed_toggles_remain_independent() {
        let ini = "[DlssNr]\nEnabled=true\n[RenoDX]\nEnabled=0\n";
        let disabled = with_enabled(ini, false);
        assert_eq!(enabled_in_ini(&disabled), Some(false));
        assert_eq!(bool_in_section(&disabled, "RenoDX", "Enabled"), Some(false));
        let preset = "Techniques=Lumenite_Kernel@lumenite_Kernel.fx,DLSS5_Feed@DLSS5_Feed.fx\n";
        let disabled = with_feed(preset, false);
        assert!(!techniques_in_preset(&disabled).unwrap().split(',').any(is_feed));
        assert!(disabled.contains("Lumenite_Kernel@lumenite_Kernel.fx"));
        assert!(techniques_in_preset(&with_feed(&disabled, true)).unwrap().split(',').any(is_feed));
    }
}

#[cfg(test)]
mod saved_options_tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(opti: bool) -> Self {
            let root = std::env::temp_dir().join(format!("peligames-neural-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            fs::write(root.join("dlssnr-amd.ini"), "[DlssNr]\r\nEnabled=true\r\n[RenoDX]\r\nEnabled=1\r\nNeuralUplift=1\r\nNRIntensity=0.7\r\n[Network]\r\nModel=custom\r\n").unwrap();
            if opti {
                fs::write(root.join("OptiScaler.ini"), "[DlssNr]\nEnabled=true\nIntensity=0.6\n").unwrap();
            } else {
                fs::write(root.join("ReShade.ini"), "[GENERAL]\nPresetPath=ReShadePreset.ini\n").unwrap();
                fs::write(root.join("ReShadePreset.ini"), format!("Techniques=Lumenite_Kernel@lumenite_Kernel.fx,{FEED}\n")).unwrap();
            }
            Self(root)
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    #[test]
    fn switches_persist_across_reload_and_mode_changes_on_both_routes() {
        for opti in [true, false] {
            let f = Fixture::new(opti);
            assert_eq!(get_settings(&f.0).unwrap(), NeuralSettings { enabled: true, renodx: true });
            for (enabled, renodx) in [(false,true),(false,false),(true,false),(true,true),(false,true),(false,false)] {
                set_options(&f.0, enabled, Some(renodx)).unwrap();
                assert_eq!(get_settings(&f.0).unwrap(), NeuralSettings { enabled, renodx });
                let backend = read_file(&f.0.join("dlssnr-amd.ini")).unwrap();
                assert_eq!(bool_in_section(&backend,"RenoDX","Enabled"), Some(renodx));
                assert_eq!(bool_in_section(&backend,"RenoDX","NeuralUplift"), Some(enabled));
                assert!(backend.contains("NRIntensity=0.7\r\n"));
                assert!(backend.contains("Model=custom\r\n"));
                if !opti { assert!(read_file(&f.0.join("ReShadePreset.ini")).unwrap().contains("Lumenite_Kernel@lumenite_Kernel.fx")); }
            }
        }
    }
    #[test]
    fn classic_reshade_can_be_enabled_from_the_effect_list_after_starting_off() {
        let f = Fixture::new(false);
        set_options(&f.0, false, Some(false)).unwrap();
        let backend = read_file(&f.0.join("dlssnr-amd.ini")).unwrap();
        assert_eq!(enabled_in_ini(&backend), Some(true));
        assert!(!get_settings(&f.0).unwrap().enabled);
        // Simulate checking DLSS5_Feed in ReShade, without changing the add-on.
        let preset = f.0.join("ReShadePreset.ini");
        fs::write(&preset, with_feed(&read_file(&preset).unwrap(), true)).unwrap();
        assert!(get_settings(&f.0).unwrap().enabled);
        assert_eq!(read_file(&f.0.join("dlssnr-amd.ini")).unwrap(), backend);
    }
    #[test]
    fn renodx_startup_uses_neural_uplift_and_keeps_the_feeder_available() {
        let f = Fixture::new(false);
        set_options(&f.0, false, Some(true)).unwrap();
        let preset = read_file(&f.0.join("ReShadePreset.ini")).unwrap();
        assert!(techniques_in_preset(&preset).unwrap().split(',').any(is_feed));
        assert!(!get_settings(&f.0).unwrap().enabled);
        assert_eq!(bool_in_section(&read_file(&f.0.join("dlssnr-amd.ini")).unwrap(), "RenoDX", "NeuralUplift"), Some(false));
    }
    #[test]
    fn installer_startup_preserves_the_selected_mode() {
        for opti in [true, false] {
            let f = Fixture::new(opti);
            set_enabled(&f.0, false).unwrap();
            assert_eq!(get_settings(&f.0).unwrap(), NeuralSettings { enabled: false, renodx: true });
            set_options(&f.0, true, Some(false)).unwrap();
            set_enabled(&f.0, false).unwrap();
            assert_eq!(get_settings(&f.0).unwrap(), NeuralSettings { enabled: false, renodx: false });
        }
    }
}
