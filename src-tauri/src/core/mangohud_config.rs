//! Launcher-owned MangoHUD profile with optional export to the user's system profile.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};
pub type Settings = BTreeMap<String, Value>;
static FILE_LOCK: Mutex<()> = Mutex::new(());
#[derive(Deserialize)]
struct OptionSpec {
    key: String,
    section: String,
    kind: String,
    default: Value,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    values: Option<Vec<String>>,
}
fn catalog() -> Vec<OptionSpec> {
    serde_json::from_str(include_str!("../../../src/config/mangohudOptions.json"))
        .expect("MangoHUD option catalog")
}
pub fn defaults() -> Settings {
    catalog().into_iter().map(|o| (o.key, o.default)).collect()
}
#[derive(Deserialize)]
struct LayoutBlock { id: String, keys: Vec<String>, hardware_order: i32 }
fn layout_blocks() -> Vec<LayoutBlock> {
    serde_json::from_str(include_str!("../../../src/config/mangohudLayout.json")).expect("MangoHUD layout catalog")
}
fn valid_hud_order(value: &str) -> bool {
    if value.is_empty() { return true; }
    let blocks = layout_blocks();
    let ids: Vec<_> = value.split(',').collect();
    ids.len() == blocks.len() && ids.iter().enumerate().all(|(i, id)|
        blocks.iter().any(|block| block.id == *id) && !ids[..i].contains(id))
}
fn valid_shortcut(value: &str) -> bool {
    let keys: Vec<_> = value.split('+').collect();
    if keys.is_empty() || keys.len() > 5 || value.len() > 100 { return false; }
    let modifiers = ["Control_L", "Control_R", "Alt_L", "Alt_R", "Shift_L", "Shift_R", "Super_L", "Super_R"];
    let (key, prefix) = keys.split_last().unwrap();
    if !prefix.iter().enumerate().all(|(i, modifier)| modifiers.contains(modifier) && !prefix[..i].contains(modifier)) { return false; }
    if key.len() == 1 && key.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) { return true; }
    if let Some(number) = key.strip_prefix('F').and_then(|v| v.parse::<u8>().ok()) {
        return (1..=24).contains(&number) && *key == format!("F{number}");
    }
    matches!(*key, "Return" | "KP_Enter" | "space" | "BackSpace" | "Tab" | "Insert" | "Delete" | "Home" | "End"
        | "Page_Up" | "Page_Down" | "Up" | "Down" | "Left" | "Right" | "Pause" | "Caps_Lock" | "Scroll_Lock" | "Print"
        | "minus" | "equal" | "bracketleft" | "bracketright" | "backslash" | "semicolon" | "apostrophe" | "grave"
        | "comma" | "period" | "slash" | "KP_Add" | "KP_Subtract" | "KP_Multiply" | "KP_Divide" | "KP_Decimal"
        | "KP_0" | "KP_1" | "KP_2" | "KP_3" | "KP_4" | "KP_5" | "KP_6" | "KP_7" | "KP_8" | "KP_9")
}
fn valid_fps_limits(value: &str) -> bool {
    if value.is_empty() { return true; }
    let values: Vec<_> = value.split(',').collect();
    (2..=8).contains(&values.len()) && values.iter().enumerate().all(|(i, part)| {
        !part.is_empty() && part.bytes().all(|c| c.is_ascii_digit())
            && part.parse::<u32>().is_ok_and(|v| v <= 1000 && !values[..i].iter().any(|p| p.parse::<u32>().ok() == Some(v)))
    })
}
pub fn validate(settings: &Settings) -> Result<(), String> {
    let specs = catalog();
    if settings.len() != specs.len() {
        return Err("Perfil MangoHUD incompleto ou com opções desconhecidas.".into());
    }
    for spec in specs {
        let value = settings.get(&spec.key).ok_or("Opção MangoHUD ausente.")?;
        let valid = match spec.kind.as_str() {
            "hudorder" => value.as_str().is_some_and(valid_hud_order),
            "fpslimits" => value.as_str().is_some_and(valid_fps_limits),
            "shortcut" => value.as_str().is_some_and(valid_shortcut),
            "toggle" => value.is_boolean(),
            "number" => value.as_f64().is_some_and(|n| {
                n.is_finite()
                    && n >= spec.min.unwrap_or(0.)
                    && n <= spec.max.unwrap_or(1000.)
                    && (spec.step != Some(1.) || n.fract() == 0.)
            }),
            "select" => value.as_str().is_some_and(|s| {
                spec.values
                    .as_ref()
                    .is_some_and(|v| v.iter().any(|x| x == s))
            }),
            "color" => value
                .as_str()
                .is_some_and(|s| s.len() == 6 && s.bytes().all(|c| c.is_ascii_hexdigit())),
            "text" if spec.key == "pci_dev" => value.as_str().is_some_and(crate::commands::gpu::valid_pci_address),
            "text" => value
                .as_str()
                .is_some_and(|s| s.chars().count() <= 64 && !s.chars().any(|c| c.is_control())),
            _ => false,
        };
        if !valid {
            return Err(format!("Valor inválido para {}.", spec.key));
        }
    }
    Ok(())
}
pub fn directory() -> Result<PathBuf, String> {
    let dir = super::paths::app_path("MangoHUD")?;
    match fs::symlink_metadata(&dir) {
        Ok(m) if !m.is_dir() || m.file_type().is_symlink() => {
            return Err("Pasta MangoHUD inválida.".into())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?
        }
        Err(e) => return Err(e.to_string()),
        _ => {}
    }
    Ok(dir)
}
fn atomic_write(path: &Path, data: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    use std::io::Write;
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        f.write_all(data).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
pub fn render(settings: &Settings, logs: &Path) -> Result<String, String> {
    validate(settings)?;
    let mut text = String::from("# PeliGames · perfil exclusivo do launcher\nlegacy_layout=0\n");
    let mut specs = catalog();
    let mut blocks = layout_blocks();
    let custom = settings.get("launcher_hud_custom_order").and_then(Value::as_str).unwrap_or("");
    if !custom.is_empty() {
        let ids: Vec<_> = custom.split(',').collect();
        blocks.sort_by_key(|b| ids.iter().position(|id| *id == b.id).unwrap_or(ids.len()));
    } else if settings.get("launcher_hud_order").and_then(Value::as_str) == Some("hardware-first") {
        blocks.sort_by_key(|b| b.hardware_order);
    }
    // Display blocks share their order with the UI. Sensor settings stay with
    // their parent row; launcher-only metadata is never written to MangoHUD.
    specs.sort_by_key(|spec| blocks.iter().position(|b| b.keys.contains(&spec.key)).unwrap_or(blocks.len()));
    for spec in specs {
        if spec.key.starts_with("launcher_") {
            continue;
        }
        let value = &settings[&spec.key];
        if spec.key == "fps_limit" {
            if let Some(limits) = settings.get("launcher_fps_limits").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                text.push_str(&format!("fps_limit={limits}\n"));
                continue;
            }
        }
        // Network takes interface names, not a boolean value. An empty selection monitors all.
        if spec.key == "network" {
            if value == &Value::Bool(true) {
                text.push_str("network\n");
            }
            continue;
        }
        let formatted = if let Some(b) = value.as_bool() {
            if b { "1" } else { "0" }.to_string()
        } else if let Some(s) = value.as_str() {
            s.to_string()
        } else {
            value.to_string()
        };
        text.push_str(&format!("{}={}\n", spec.key, formatted));
    }
    let logs = logs.to_str().ok_or("Caminho de logs inválido.")?;
    if logs.chars().any(|c| c.is_control()) {
        return Err("Caminho de logs inválido.".into());
    }
    text.push_str(&format!("output_folder={logs}\npermit_upload=0\n"));
    Ok(text)
}
#[derive(Serialize)]
pub struct Profile {
    pub settings: Settings,
    pub config_path: String,
}
pub fn load() -> Result<Profile, String> {
    let _lock = FILE_LOCK.lock().map_err(|_| "Perfil ocupado.")?;
    let dir = directory()?;
    let path = dir.join("profile.json");
    let mut settings = defaults();
    if path.exists() {
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        if bytes.len() > 64 * 1024 {
            return Err("Perfil MangoHUD inválido.".into());
        }
        let saved: Settings =
            serde_json::from_slice(&bytes).map_err(|_| "Perfil MangoHUD inválido.")?;
        for (k, v) in saved {
            if settings.contains_key(&k) {
                settings.insert(k, v);
            }
        }
    }
    validate(&settings)?;
    Ok(Profile {
        settings,
        config_path: dir.join("MangoHud.conf").to_string_lossy().into_owned(),
    })
}
pub fn write(settings: &Settings, preview: bool) -> Result<String, String> {
    let _lock = FILE_LOCK.lock().map_err(|_| "Perfil ocupado.")?;
    validate(settings)?;
    let dir = directory()?;
    write_in(&dir, settings, preview)
}
/// Called only by the settings editor. Game launches and live previews never export.
pub fn save(settings: &Settings) -> Result<String, String> {
    let _lock = FILE_LOCK.lock().map_err(|_| "Perfil ocupado.")?;
    validate(settings)?;
    let dir = directory()?;
    let config = dirs::config_dir().ok_or("Pasta de configuração indisponível.")?;
    save_in(&dir, &config.join("MangoHud"), settings)
}
fn save_in(dir: &Path, system: &Path, settings: &Settings) -> Result<String, String> {
    validate(settings)?;
    if settings.get("launcher_sync_system").and_then(Value::as_bool) == Some(true) {
        export_system(system, settings).map_err(|e| format!("Não foi possível aplicar ao MangoHUD do sistema: {e}"))?;
    }
    write_in(dir, settings, false)
}
fn export_system(dir: &Path, settings: &Settings) -> Result<(), String> {
    validate(settings)?;
    // Preserve the original once, without following links or modifying other profiles.
    match fs::symlink_metadata(dir) {
        Ok(m) if !m.is_dir() || m.file_type().is_symlink() => return Err("Pasta MangoHud inválida.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => fs::create_dir_all(dir).map_err(|e| e.to_string())?,
        Err(e) => return Err(e.to_string()),
        _ => {}
    }
    let path = dir.join("MangoHud.conf");
    match fs::symlink_metadata(&path) {
        Ok(m) if !m.is_file() || m.file_type().is_symlink() => return Err("MangoHud.conf não é um arquivo regular.".into()),
        Ok(_) => {
            let backup = dir.join("MangoHud.conf.peligames.bak");
            match fs::symlink_metadata(&backup) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => atomic_write(&backup, &fs::read(&path).map_err(|e| e.to_string())?)?,
                Err(e) => return Err(e.to_string()),
                Ok(m) if !m.is_file() || m.file_type().is_symlink() => return Err("Backup MangoHUD inválido.".into()),
                _ => {}
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(e.to_string()),
    }
    let logs = dir.join("logs");
    let rendered = render(settings, &logs)?.replacen("perfil exclusivo do launcher", "configuração do usuário", 1);
    fs::create_dir_all(logs).map_err(|e| e.to_string())?;
    atomic_write(&path, rendered.as_bytes())
}
fn write_in(dir: &Path, settings: &Settings, preview: bool) -> Result<String, String> {
    validate(settings)?;
    let logs = dir.join("logs");
    fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let path = dir.join(if preview {
        "preview.conf"
    } else {
        "MangoHud.conf"
    });
    atomic_write(&path, render(settings, &logs)?.as_bytes())?;
    if !preview {
        atomic_write(
            &dir.join("profile.json"),
            &serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?,
        )?;
    }
    Ok(path.to_string_lossy().into_owned())
}
pub fn global_enabled() -> Result<bool, String> {
    // Tests must not depend on the user's launcher switch (shared home in containers).
    if cfg!(test) { return Ok(false); }
    Ok(load()?
        .settings
        .get("launcher_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}
// Environment overrides are per invocation; the shared profile is never changed by a game limit.
fn apply_environment(command: &mut Command, config_path: &Path, display: bool, limit: Option<u32>) {
    command.env_remove("MANGOHUD_CONFIG").env_remove("MANGOHUD_PRESETSFILE")
        .env("MANGOHUD_CONFIGFILE", config_path);
    if let Some(fps) = limit {
        let hidden = if display { "" } else { ",no_display=1" };
        command.env("MANGOHUD_CONFIG", format!("read_cfg,fps_limit={fps}{hidden}"));
    }
}
pub fn apply(command: &mut Command, enabled: bool, limit: Option<u32>) -> Result<(), String> {
    if limit.is_some_and(|fps| fps > 1000) { return Err("Informe um limite de FPS entre 0 e 1000.".into()); }
    apply_with_override(command, enabled, limit, None)
}
pub fn apply_with_override(command: &mut Command, enabled: bool, limit: Option<u32>, display_override: Option<bool>) -> Result<(), String> {
    apply_with_preset(command, enabled, limit, display_override, None)
}
pub fn apply_with_preset(command: &mut Command, enabled: bool, limit: Option<u32>, display_override: Option<bool>, preset: Option<&super::mangohud_presets::Preset>) -> Result<(), String> {
    if limit.is_some_and(|fps| fps > 1000) { return Err("Informe um limite de FPS entre 0 e 1000.".into()); }
    let display = match display_override { Some(value) => value, None => enabled || global_enabled()? };
    let background=command.get_envs().any(|(k,v)|k=="PELI_OVERLAY_BACKGROUND_FPS" && v.is_some());
    if display || limit.unwrap_or(0) > 0 || background {
        let profile = load()?;
        let config = if let Some(preset) = preset {
            let settings = selected_settings(&profile.settings, Some(preset))?;
            let text = render(&settings, &directory()?.join("logs"))?;
            use sha2::{Digest, Sha256};
            let fingerprint = format!("{:x}", Sha256::digest(text.as_bytes()));
            let folder = directory()?.join("game-presets");
            safe_directory(&folder)?;
            let path=super::paths::names::allocate(&folder,&preset.name,&fingerprint)?.with_extension("conf");
            atomic_write(&path, text.as_bytes())?;
            path
        } else {
            write(&profile.settings, false)?;
            PathBuf::from(&profile.config_path)
        };
        apply_environment(command, &config, display, limit);
        if background && !display {let config=command.get_envs().find(|(k,_)|*k=="MANGOHUD_CONFIG").and_then(|(_,v)|v).map(|v|v.to_string_lossy().into_owned()).unwrap_or_else(||"read_cfg".into());command.env("MANGOHUD_CONFIG",format!("{config},no_display=1"));}
    } else if display_override == Some(false) {
        command.env("MANGOHUD", "0").env("MANGOHUD_CONFIG", "no_display=1");
    }
    Ok(())
}
/// A per-game snapshot changes appearance, not activation, FPS caps or VSync.
pub fn selected_settings(global: &Settings, preset: Option<&super::mangohud_presets::Preset>) -> Result<Settings, String> {
    let Some(preset) = preset else {return Ok(global.clone());};
    if preset.name.trim().is_empty() || preset.name.chars().count() > 64 || preset.name.chars().any(char::is_control) {
        return Err("Nome de preset MangoHUD inválido.".into());
    }
    let mut settings = defaults();
    settings.extend(preset.settings.clone());
    validate(&settings)?;
    for key in ["launcher_enabled", "launcher_sync_system", "fps_limit", "fps_limit_method", "vsync", "gl_vsync", "no_display"] {
        if let Some(value) = global.get(key) {settings.insert(key.into(), value.clone());}
        else {settings.remove(key);}
    }
    Ok(settings)
}
/// Keep global appearance and shortcuts; only the output destination is per game.
pub fn game_folder(name: &str) -> String {
    let clean: String = name.chars().take(100).map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | ',' | '=' ) { '_' } else { c }).collect();
    let mut clean = clean.as_str();
    while clean.len() > 180 { clean = &clean[..clean.char_indices().last().map(|(i,_)|i).unwrap_or(0)]; }
    let clean = clean.trim().trim_matches('.').trim();
    if clean.is_empty() { "Jogo".into() } else { clean.into() }
}
pub fn apply_game_logs(command: &mut Command, name: &str, preset: Option<&super::mangohud_presets::Preset>) -> Result<(), String> {
    if !command.get_envs().any(|(key,value)| key == "MANGOHUD_CONFIGFILE" && value.is_some()) { return Ok(()); }
    apply_game_logs_in(command, &directory()?, name, &selected_settings(&load()?.settings, preset)?)
}
fn apply_game_logs_in(command: &mut Command, root: &Path, name: &str, settings: &Settings) -> Result<(), String> {
    let folder = game_folder(name);
    let logs = root.join("logs");
    safe_directory(&logs)?;
    let destination = logs.join(&folder);
    safe_directory(&destination)?;
    let configs = root.join("game-configs");
    safe_directory(&configs)?;
    let config = configs.join(format!("{folder}.conf"));
    atomic_write(&config, render(settings, &destination)?.as_bytes())?;
    command.env("MANGOHUD_CONFIGFILE", config);
    Ok(())
}
pub(super) fn safe_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path).map_err(|e| e.to_string()),
        _ => Err("Pasta de benchmarks inválida.".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_layout_accepts_title_priority_and_renders_all_orders() {
        let mut blocks = layout_blocks();
        blocks.sort_by_key(|block| block.hardware_order);
        assert_eq!(blocks.first().unwrap().id, "custom_text_center");
        assert!(blocks.first().unwrap().hardware_order < 0);
        for order in ["fps-first", "hardware-first"] {
            let mut settings = defaults();
            settings.insert("launcher_hud_order".into(), serde_json::json!(order));
            settings.insert("custom_text_center".into(), serde_json::json!("PeliGames"));
            let config = render(&settings, Path::new("/tmp/test-hud-logs")).unwrap();
            let title = config.find("\ncustom_text_center=PeliGames\n").unwrap();
            for metric in ["fps", "gpu_stats", "cpu_stats"] {
                assert!(title < config.find(&format!("\n{metric}=")).unwrap());
            }
        }
    }

    #[test]
    fn monitoring_gpu_uses_valid_pci_identity_and_preserves_legacy_settings() {
        let mut settings=defaults();
        settings.insert("gpu_list".into(),Value::String("0,1".into()));
        assert!(validate(&settings).is_ok());
        settings.insert("pci_dev".into(),Value::String("0000:01:00.0".into()));
        settings.insert("gpu_list".into(),Value::String(String::new()));
        let config=render(&settings,Path::new("/tmp/test-monitoring-logs")).unwrap();
        assert!(config.contains("pci_dev=0000:01:00.0"));
        settings.insert("pci_dev".into(),Value::String("0000:01:00.0\nexec=bad".into()));
        assert!(validate(&settings).is_err());
    }
    #[test]
    fn custom_layout_is_validated_and_exported_as_native_config_order() {
        let mut s = defaults();
        let mut blocks = layout_blocks();
        let wanted = ["time", "wine", "cpu", "gpu", "frame_timing", "fps"];
        blocks.sort_by_key(|b| wanted.iter().position(|id| *id == b.id).unwrap_or(wanted.len()));
        let order = blocks.iter().map(|b| b.id.as_str()).collect::<Vec<_>>().join(",");
        s.insert("launcher_hud_custom_order".into(), Value::String(order.clone()));
        let config = render(&s, Path::new("/tmp/logs")).unwrap();
        for pair in ["time", "wine", "cpu_stats", "gpu_stats", "frame_timing", "fps"].windows(2) {
            assert!(config.find(&format!("\n{}=", pair[0])).unwrap() < config.find(&format!("\n{}=", pair[1])).unwrap());
        }
        assert!(config.contains("legacy_layout=0\n"));
        assert!(!config.contains("launcher_hud_custom_order="));
        assert!(valid_hud_order(""));
        assert!(!valid_hud_order("cpu,gpu"));
        assert!(!valid_hud_order(&order.replace("time,", "gpu,")));
        assert!(!valid_hud_order(&order.replace("time,", "exec,")));
        assert!(!valid_hud_order(&format!("{order}\nexec=bad")));
    }
    #[test]
    fn default_monitoring_has_no_fps_cap_or_forced_vsync() {
        let mut settings = defaults();
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        // Vulkan immediate is 1; OpenGL's disabled swap interval is 0.
        for line in ["fps_limit=0", "vsync=1", "gl_vsync=0"] {
            assert!(config.lines().any(|actual| actual == line), "{line}");
        }
        // Explicit user choices still override these defaults.
        settings.insert("vsync".into(), serde_json::json!("3"));
        settings.insert("gl_vsync".into(), serde_json::json!("1"));
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        assert!(config.lines().any(|line| line == "vsync=3"));
        assert!(config.lines().any(|line| line == "gl_vsync=1"));
    }
    #[test]
    fn game_logs_keep_global_style_and_invocation_fps_override() {
        let root = std::env::temp_dir().join(format!("peligames-game-hud-{}",uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let mut command = Command::new("game");
        command.env("MANGOHUD_CONFIG", "read_cfg,fps_limit=60");
        apply_game_logs_in(&mut command, &root, "Cyberpunk 2077", &defaults()).unwrap();
        let config = fs::read_to_string(root.join("game-configs/Cyberpunk 2077.conf")).unwrap();
        assert!(config.contains(&format!("output_folder={}/logs/Cyberpunk 2077\n",root.display())));
        assert!(command.get_envs().any(|(key,value)|key=="MANGOHUD_CONFIG" && value==Some(std::ffi::OsStr::new("read_cfg,fps_limit=60"))));
        assert!(root.join("logs/Cyberpunk 2077").is_dir());
        #[cfg(unix)] {
            std::os::unix::fs::symlink(root.join("logs/Cyberpunk 2077"),root.join("logs/Linked game")).unwrap();
            assert!(apply_game_logs_in(&mut command, &root, "Linked game", &defaults()).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reset_metrics_shortcut_defaults_to_right_shift_and_accepts_left_shift() {
        let mut settings=defaults();
        assert_eq!(settings.get("reset_fps_metrics").and_then(Value::as_str),Some("Shift_R+F9"));
        settings.insert("reset_fps_metrics".into(),serde_json::json!("Shift_L+F9"));
        let config=render(&settings,Path::new("/tmp/logs")).unwrap();
        assert!(config.contains("reset_fps_metrics=Shift_L+F9\n"));
        settings.insert("reset_fps_metrics".into(),serde_json::json!("Shift_L+F9\nexec=bad"));
        assert!(validate(&settings).is_err());
    }
    #[test]
    fn fps_cycle_exports_all_limits_and_its_recorded_shortcut() {
        let mut settings = defaults();
        settings.insert("launcher_fps_limits".into(), serde_json::json!("30,60,120,0"));
        settings.insert("toggle_fps_limit".into(), serde_json::json!("Control_L+F8"));
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        assert_eq!(config.lines().filter(|line| line.starts_with("fps_limit=")).collect::<Vec<_>>(), vec!["fps_limit=30,60,120,0"]);
        assert!(config.contains("toggle_fps_limit=Control_L+F8\n"));
        assert!(!config.contains("launcher_fps_limits="));
        for invalid in ["60", "30,30", "0,00", "60,1001", "60,-1", "60,NaN", "60,120\nexec=bad"] { assert!(!valid_fps_limits(invalid), "{invalid}"); }
        settings.insert("launcher_fps_limits".into(), serde_json::json!(""));
        assert!(render(&settings, Path::new("/tmp/logs")).unwrap().contains("fps_limit=0\n"));
    }
    #[test]
    fn native_offsets_support_inward_spacing_on_right_and_bottom_edges() {
        let mut settings = defaults();
        settings.insert("position".into(), serde_json::json!("bottom-right"));
        settings.insert("offset_x".into(), serde_json::json!(-20));
        settings.insert("offset_y".into(), serde_json::json!(-20));
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        assert!(config.contains("offset_x=-20\n"));
        assert!(config.contains("offset_y=-20\n"));
        settings.insert("offset_x".into(), serde_json::json!(-1001));
        assert!(render(&settings, Path::new("/tmp/logs")).is_err());
    }
    #[test]
    fn recorded_shortcuts_accept_keys_and_combinations_without_config_injection() {
        for shortcut in ["F8", "Control_L+Shift_R+k", "Alt_R+Return", "Super_L+KP_5", "Shift_R+F12"] {
            assert!(valid_shortcut(shortcut), "{shortcut}");
        }
        for shortcut in ["", "Shift_L", "Control_L+", "F25", "F01", "Control_L+Control_L+k", "F8\nexec=bad", "a,b", "Control_L+unknown"] {
            assert!(!valid_shortcut(shortcut), "{shortcut}");
        }
        let mut settings = defaults();
        settings.insert("toggle_hud".into(), serde_json::json!("Control_L+Shift_R+k"));
        settings.insert("toggle_logging".into(), serde_json::json!("F8"));
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        assert!(config.contains("toggle_hud=Control_L+Shift_R+k\n"));
        assert!(config.contains("toggle_logging=F8\n"));
    }
    #[test]
    fn gpu_load_colors_are_exported_with_validated_thresholds() {
        let mut settings = defaults();
        settings.insert("gpu_load_change".into(), serde_json::json!(true));
        settings.insert("gpu_load_value".into(), serde_json::json!("50,80"));
        settings.insert("gpu_load_color".into(), serde_json::json!("64B5F6,CE93D8,F472B6"));
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        assert!(config.contains("gpu_load_change=1\n"));
        assert!(config.contains("gpu_load_value=50,80\n"));
        assert!(config.contains("gpu_load_color=64B5F6,CE93D8,F472B6\n"));
        settings.insert("gpu_load_value".into(), serde_json::json!("90,20"));
        assert!(render(&settings, Path::new("/tmp/logs")).is_err());
    }
    #[test]
    fn hardware_order_keeps_cpu_first_metrics_before_graph_and_clock_last() {
        let mut settings = defaults();
        settings.insert("launcher_hud_order".into(), serde_json::json!("hardware-first"));
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        let keys: Vec<&str> = config.lines().filter_map(|line| line.split_once('=').map(|(key, _)| key)).collect();
        let order = ["cpu_stats", "gpu_stats", "vram", "ram", "engine_version", "fps", "fps_metrics", "frame_timing", "wine", "gamemode", "time"];
        for pair in order.windows(2) {
            assert!(keys.iter().position(|key| key == &pair[0]).unwrap() < keys.iter().position(|key| key == &pair[1]).unwrap(), "{} before {}", pair[0], pair[1]);
        }
        assert!(!config.contains("launcher_hud_order="));
        settings.insert("launcher_hud_order".into(), serde_json::json!("fps-first"));
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        assert!(config.find("\nfps=").unwrap() < config.find("\ngpu_stats=").unwrap());
        settings.insert("launcher_hud_order".into(), serde_json::json!("invalid"));
        assert!(render(&settings, Path::new("/tmp/logs")).is_err());
    }
    #[test]
    fn renders_explicit_metrics_and_isolated_logs() {
        let mut s = defaults();
        s.insert("gpu_stats".into(), Value::Bool(false));
        let r = render(&s, Path::new("/tmp/PeliGames/MangoHUD/logs")).unwrap();
        assert!(r.contains("gpu_stats=0\n"));
        assert!(r.contains("fps=1\n"));
        assert!(!r.contains("launcher_enabled"));
        assert!(!r.contains("network="));
        s.insert("network".into(), Value::Bool(true));
        assert!(render(&s, Path::new("/tmp/logs"))
            .unwrap()
            .contains("network\n"));
        assert!(r.contains("permit_upload=0\n"));
        assert!(r.contains("output_folder=/tmp/PeliGames/MangoHUD/logs\n"));
    }
    #[test]
    fn system_export_is_opt_in_and_preserves_original_backup() {
        let root = std::env::temp_dir().join(format!("peligames-hud-export-{}", uuid::Uuid::new_v4()));
        let own = root.join("launcher");
        let system = root.join("MangoHud");
        fs::create_dir_all(&own).unwrap();
        fs::create_dir_all(&system).unwrap();
        let path = system.join("MangoHud.conf");
        fs::write(&path, "# original\nfps=0\n").unwrap();
        let mut settings = defaults();
        save_in(&own, &system, &settings).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "# original\nfps=0\n");
        assert!(!system.join("MangoHud.conf.peligames.bak").exists());
        settings.insert("launcher_sync_system".into(), Value::Bool(true));
        save_in(&own, &system, &settings).unwrap();
        let exported = fs::read_to_string(&path).unwrap();
        assert!(exported.contains("fps=1\n"));
        assert!(!exported.contains("launcher_"));
        settings.insert("font_size".into(), serde_json::json!(40));
        write_in(&own, &settings, true).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), exported);
        save_in(&own, &system, &settings).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("font_size=40\n"));
        assert_eq!(fs::read_to_string(system.join("MangoHud.conf.peligames.bak")).unwrap(), "# original\nfps=0\n");
        settings.insert("launcher_sync_system".into(), Value::Bool(false));
        let before = fs::read(&path).unwrap();
        save_in(&own, &system, &settings).unwrap();
        assert_eq!(fs::read(path).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_export_does_not_modify_system_profile() {
        let root = std::env::temp_dir().join(format!("peligames-hud-invalid-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("MangoHud.conf");
        fs::write(&path, "original").unwrap();
        let mut settings = defaults();
        settings.insert("font_size".into(), serde_json::json!(-1));
        assert!(export_system(&root, &settings).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "original");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn preview_does_not_replace_saved_game_profile() {
        let dir =
            std::env::temp_dir().join(format!("peligames-hud-profile-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let settings = defaults();
        write_in(&dir, &settings, false).unwrap();
        let original = fs::read(dir.join("MangoHud.conf")).unwrap();
        let profile = fs::read(dir.join("profile.json")).unwrap();
        let mut draft = settings.clone();
        draft.insert("font_size".into(), serde_json::json!(40));
        write_in(&dir, &draft, true).unwrap();
        assert_eq!(fs::read(dir.join("MangoHud.conf")).unwrap(), original);
        assert_eq!(fs::read(dir.join("profile.json")).unwrap(), profile);
        assert!(fs::read_to_string(dir.join("preview.conf"))
            .unwrap()
            .contains("font_size=40\n"));
        write_in(&dir, &draft, false).unwrap();
        assert!(fs::read_to_string(dir.join("MangoHud.conf"))
            .unwrap()
            .contains("font_size=40\n"));
        let reread: Settings =
            serde_json::from_slice(&fs::read(dir.join("profile.json")).unwrap()).unwrap();
        assert_eq!(reread, draft);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn hardware_names_are_written_as_mangohud_text_options() {
        let mut settings = defaults();
        assert_eq!(settings["gpu_text"], "GPU");
        assert_eq!(settings["cpu_text"], "CPU");
        settings.insert("gpu_text".into(), serde_json::json!("Radeon RX 9070"));
        settings.insert("cpu_text".into(), serde_json::json!("Ryzen 7 9800X3D"));
        let config = render(&settings, Path::new("/tmp/logs")).unwrap();
        assert!(config.contains("gpu_text=Radeon RX 9070\n"));
        assert!(config.contains("cpu_text=Ryzen 7 9800X3D\n"));
        settings.insert("gpu_text".into(), serde_json::json!("GPU\nexec=bad"));
        assert!(render(&settings, Path::new("/tmp/logs")).is_err());
    }
    #[test]
    fn rejects_config_injection_and_unknown_options() {
        let mut s = defaults();
        s.insert(
            "custom_text_center".into(),
            Value::String("hello\nexec=rm".into()),
        );
        assert!(validate(&s).is_err());
        let mut s = defaults();
        s.insert("exec".into(), Value::String("command".into()));
        assert!(validate(&s).is_err());
    }
    #[test]
    fn rejects_out_of_range_and_invalid_enum() {
        let mut s = defaults();
        s.insert("font_size".into(), serde_json::json!(1000));
        assert!(validate(&s).is_err());
        s = defaults();
        s.insert("position".into(), serde_json::json!("anywhere"));
        assert!(validate(&s).is_err());
    }
}

#[cfg(test)]
mod individual_fps_tests {
    use super::*;
    fn override_value(command: &Command) -> Option<String> {
        command.get_envs().find(|(key, _)| *key == "MANGOHUD_CONFIG")
            .and_then(|(_, value)| value.map(|v| v.to_string_lossy().into_owned()))
    }
    #[test]
    fn games_get_independent_limits_and_hidden_hud() {
        let path = Path::new("/tmp/PeliGames/MangoHUD/MangoHud.conf");
        let mut first = Command::new("umu"); let mut second = Command::new("umu");
        apply_environment(&mut first, path, false, Some(60));
        apply_environment(&mut second, path, false, Some(144));
        assert_eq!(override_value(&first).as_deref(), Some("read_cfg,fps_limit=60,no_display=1"));
        assert_eq!(override_value(&second).as_deref(), Some("read_cfg,fps_limit=144,no_display=1"));
        assert_eq!(override_value(&first).as_deref(), Some("read_cfg,fps_limit=60,no_display=1"));
    }
    #[test]
    fn visible_hud_and_zero_override_preserve_profile_selection() {
        let mut cmd = Command::new("umu");
        let path = Path::new("/tmp/MangoHud.conf");
        apply_environment(&mut cmd, path, true, Some(0));
        assert_eq!(override_value(&cmd).as_deref(), Some("read_cfg,fps_limit=0"));
        apply_environment(&mut cmd, path, true, None);
        assert_eq!(override_value(&cmd), None);
        assert!(cmd.get_envs().any(|(k,v)| k == "MANGOHUD_CONFIGFILE" && v == Some(path.as_os_str())));
    }
}

#[cfg(test)]
mod per_game_display_tests {
    use super::*;
    #[test]
    fn performance_explicitly_disabled_game_clears_inherited_monitoring() {
        let mut command = Command::new("game");
        command.env("MANGOHUD", "1");
        apply_with_override(&mut command, true, Some(0), Some(false)).unwrap();
        let value = |key: &str| command.get_envs().find(|(k, _)| *k == key).and_then(|(_, v)| v).map(|v| v.to_string_lossy().into_owned());
        assert_eq!(value("MANGOHUD"), Some("0".into()));
        assert_eq!(value("MANGOHUD_CONFIG"), Some("no_display=1".into()));
    }
}

#[cfg(test)]
mod game_preset_tests {
    use super::*;
    #[test]
    fn game_snapshot_changes_appearance_without_changing_global_controls() {
        let global = defaults();
        let mut snapshot = global.clone();
        snapshot.insert("gpu_color".into(), "112233".into());
        snapshot.insert("fps_limit".into(), 30.into());
        snapshot.remove("launcher_enabled");
        snapshot.remove("launcher_sync_system");
        let preset = super::super::mangohud_presets::Preset {name:"Meu jogo".into(),settings:snapshot};
        let selected = selected_settings(&global, Some(&preset)).unwrap();
        assert_eq!(selected["gpu_color"], "112233");
        for key in ["fps_limit", "vsync", "gl_vsync", "launcher_enabled"] {
            assert_eq!(selected.get(key), global.get(key));
        }
        assert_eq!(global, defaults());
        assert_eq!(selected_settings(&global, None).unwrap(), global);
        assert!(render(&selected, Path::new("/tmp/logs")).unwrap().contains("112233"));
    }
    #[test]
    fn saved_snapshot_survives_roundtrip_and_rejects_unknown_options() {
        let mut advanced = crate::core::advanced_settings::AdvancedSettings::default();
        advanced.mangohud_preset=Some(super::super::mangohud_presets::Preset {name:"Cópia independente".into(),settings:defaults()});
        let restored:crate::core::advanced_settings::AdvancedSettings=serde_json::from_value(serde_json::to_value(&advanced).unwrap()).unwrap();
        assert_eq!(restored.mangohud_preset.as_ref().unwrap().name,"Cópia independente");
        restored.validate().unwrap();
        let reset=crate::core::game_reset::settings(restored,crate::core::game_reset::Section::All);
        assert!(reset.mangohud_preset.is_some());
        advanced.mangohud_preset.as_mut().unwrap().settings.insert("unknown_setting".into(),true.into());
        assert!(advanced.validate().is_err());
    }
}
