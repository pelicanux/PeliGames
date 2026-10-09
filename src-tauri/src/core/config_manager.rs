use serde::{Deserialize, Serialize};
use std::fs;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub backend: String,
    pub dll_version: String,
    pub shortcut_key: String,
    #[serde(default)]
    pub game_shortcut_keys: HashMap<String, String>,
    #[serde(default)]
    pub custom_game_paths: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steamgriddb_api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_steam_protons: Option<bool>,
    #[serde(default)]
    pub preferred_proton_family: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            backend: "AMDNR".to_string(),
            dll_version: "0.5.1".to_string(),
            shortcut_key: "Insert".to_string(),
            game_shortcut_keys: HashMap::new(),
            custom_game_paths: HashMap::new(),
            steamgriddb_api_key: None,
            scan_steam_protons: None,
            preferred_proton_family: None,
        }
    }
}

// Setup and older clients may omit per-game preferences. Preserve those entries,
// while allowing an explicit incoming choice to replace the previous value.
pub fn preserve_saved_preferences(config: &mut AppConfig, previous: AppConfig) {
    if config.steamgriddb_api_key.is_none() {
        config.steamgriddb_api_key = previous.steamgriddb_api_key;
    }
    if config.scan_steam_protons.is_none() {
        config.scan_steam_protons = previous.scan_steam_protons;
    }
    if config.preferred_proton_family.is_none() { config.preferred_proton_family = previous.preferred_proton_family; }
    let incoming = std::mem::take(&mut config.game_shortcut_keys);
    config.game_shortcut_keys = previous.game_shortcut_keys;
    config.game_shortcut_keys.extend(incoming);
    let incoming = std::mem::take(&mut config.custom_game_paths);
    config.custom_game_paths = previous.custom_game_paths;
    config.custom_game_paths.extend(incoming);
}

// Keep one compatibility view for callers, but never persist launcher preferences
// alongside the mod setup. Serialize read/modify/write operations across commands.
static STORAGE: std::sync::Mutex<()> = std::sync::Mutex::new(());
const MOD_KEYS: [&str; 5] = ["backend", "dll_version", "shortcut_key", "game_shortcut_keys", "custom_game_paths"];

pub fn get_config_path() -> PathBuf {
    super::paths::app_root().unwrap_or_else(|_| dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join(super::paths::APP_DIRECTORY)).join("config.json")
}
pub fn get_mod_config_path() -> PathBuf {
    get_config_path().parent().unwrap().join("Mod/DLSSNR/config.json")
}

fn read_object(path: &std::path::Path) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn write_object(path: &std::path::Path, value: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    use std::io::Write;
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let parent = path.parent().ok_or("Pasta de configuração indisponível.")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("A configuração não pode ser um link: {}", path.display()));
    }
    let temporary = parent.join(format!(".config-{}-{}.tmp", std::process::id(), SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() { let _ = fs::remove_file(temporary); }
    result.map_err(|e| format!("{}: {e}", path.display()))
}

// Save the destination before removing legacy fields. Existing mod choices win
// on conflicts; per-game maps retain nonconflicting legacy entries.
fn migrate_locked() -> Result<(), String> {
    super::paths::initialize()?;
    let launcher_path = get_config_path();
    let mod_path = get_mod_config_path();
    let mut launcher = read_object(&launcher_path)?;
    let mut mod_config = read_object(&mod_path)?;
    let mut changed = false;
    for key in MOD_KEYS {
        if let Some(old) = launcher.remove(key) {
            changed = true;
            if key == "game_shortcut_keys" || key == "custom_game_paths" {
                if let (Some(existing), Some(legacy)) = (mod_config.get_mut(key).and_then(|v| v.as_object_mut()), old.as_object()) {
                    for (name, value) in legacy { existing.entry(name.clone()).or_insert_with(|| value.clone()); }
                    continue;
                }
            }
            mod_config.entry(key.to_string()).or_insert(old);
        }
    }
    // Backend files moved from the standalone application's root in paths.rs.
    if let Some(value) = mod_config.get_mut("dll_version") {
        if let Some(old) = value.as_str() {
            let root = launcher_path.parent().unwrap();
            let old_root = root.parent().unwrap().join("dlssnr-x-amd");
            let relative = std::path::Path::new(old).strip_prefix(&old_root).ok()
                .or_else(|| std::path::Path::new(old).strip_prefix(root).ok().filter(|p| !p.starts_with("Mod")));
            if let Some(relative) = relative {
                let relocated = root.join("Mod/DLSSNR").join(relative);
                if relocated.is_file() { *value = relocated.to_string_lossy().into_owned().into(); changed = true; }
            }
        }
    }
    if changed {
        write_object(&mod_path, &mod_config)?;
        write_object(&launcher_path, &launcher)?;
    }
    Ok(())
}

pub fn load_config_result() -> Result<Option<AppConfig>, String> {
    let _guard = STORAGE.lock().map_err(|e| e.to_string())?;
    migrate_locked()?;
    if !get_config_path().exists() && !get_mod_config_path().exists() { return Ok(None); }
    let mut view = read_object(&get_config_path())?;
    for (key, value) in read_object(&get_mod_config_path())? {
        if MOD_KEYS.contains(&key.as_str()) { view.insert(key, value); }
    }
    serde_json::from_value(serde_json::Value::Object(view)).map(Some).map_err(|e| e.to_string())
}
pub fn load_config() -> Option<AppConfig> { load_config_result().ok().flatten() }

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let _guard = STORAGE.lock().map_err(|e| e.to_string())?;
    migrate_locked()?;
    let mut launcher = read_object(&get_config_path())?;
    let mut mod_config = read_object(&get_mod_config_path())?;
    let serde_json::Value::Object(view) = serde_json::to_value(config).map_err(|e| e.to_string())? else { unreachable!() };
    for (key, value) in view {
        if MOD_KEYS.contains(&key.as_str()) { mod_config.insert(key, value); }
        else { launcher.insert(key, value); }
    }
    write_object(&get_mod_config_path(), &mod_config)?;
    write_object(&get_config_path(), &launcher)
}

// Reconfigure only the mod; API keys and Proton preferences remain available.
pub fn delete_config() -> Result<(), String> {
    let _guard = STORAGE.lock().map_err(|e| e.to_string())?;
    migrate_locked()?;
    match fs::remove_file(get_mod_config_path()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn migrate_neural_preferences(legacy: HashMap<String, bool>) -> Result<(), String> {
    let _guard = STORAGE.lock().map_err(|e| e.to_string())?;
    super::paths::initialize()?;
    let path = get_mod_config_path().parent().unwrap().join("neural-startup.json");
    let mut preferences = read_object(&path)?;
    for (key, value) in legacy { preferences.entry(key).or_insert(value.into()); }
    if !preferences.is_empty() { write_object(&path, &preferences)?; }
    Ok(())
}

pub fn save_neural_preference(game_dir: String, enabled: bool) -> Result<(), String> {
    let _guard = STORAGE.lock().map_err(|e| e.to_string())?;
    super::paths::initialize()?;
    let path = get_mod_config_path().parent().unwrap().join("neural-startup.json");
    let mut preferences = read_object(&path)?;
    preferences.insert(game_dir, enabled.into());
    write_object(&path, &preferences)
}

pub fn load_neural_preferences() -> Result<HashMap<String, bool>, String> {
    let _guard = STORAGE.lock().map_err(|e| e.to_string())?;
    super::paths::initialize()?;
    let path = get_mod_config_path().parent().unwrap().join("neural-startup.json");
    serde_json::from_value(serde_json::Value::Object(read_object(&path)?)).map_err(|e| e.to_string())
}
