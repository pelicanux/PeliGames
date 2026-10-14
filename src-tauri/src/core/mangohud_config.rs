//! Launcher-owned MangoHUD profile. No writes to the global MangoHud directory.
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
pub fn validate(settings: &Settings) -> Result<(), String> {
    let specs = catalog();
    if settings.len() != specs.len() {
        return Err("Perfil MangoHUD incompleto ou com opções desconhecidas.".into());
    }
    for spec in specs {
        let value = settings.get(&spec.key).ok_or("Opção MangoHUD ausente.")?;
        let valid = match spec.kind.as_str() {
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
    let dir = super::paths::app_root()?.join("MangoHUD");
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
    for spec in catalog() {
        if spec.key == "launcher_enabled" {
            continue;
        }
        let value = &settings[&spec.key];
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
    let display = enabled || global_enabled()?;
    if display || limit.unwrap_or(0) > 0 {
        let profile = load()?;
        write(&profile.settings, false)?;
        apply_environment(command, Path::new(&profile.config_path), display, limit);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
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
