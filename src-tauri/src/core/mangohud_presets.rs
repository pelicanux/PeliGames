//! Named local MangoHUD configurations, independent of the active profile.
use super::mangohud_config::{self, Settings};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, sync::Mutex};
static LOCK: Mutex<()> = Mutex::new(());
#[derive(Clone, Deserialize, Serialize)]
pub struct Preset {
    pub name: String,
    pub settings: Settings,
}
fn read(path: &Path) -> Result<Vec<Preset>, String> {
    let text = match fs::read(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.to_string()),
    };
    if text.len() > 4 * 1024 * 1024 {
        return Err("Arquivo de presets excede o limite permitido.".into());
    }
    serde_json::from_slice(&text).map_err(|_| {
        "Não foi possível ler os presets personalizados. O arquivo foi preservado.".into()
    })
}
fn write(path: &Path, presets: &[Preset]) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let result = (|| {
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(presets).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn add(presets: &mut Vec<Preset>, name: String, mut settings: Settings) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) {
        return Err("Use um nome de 1 a 64 caracteres para o preset.".into());
    }
    if presets
        .iter()
        .any(|p| p.name.to_lowercase() == name.to_lowercase())
    {
        return Err("Já existe um preset com esse nome. Escolha outro nome.".into());
    }
    if presets.len() >= 32 {
        return Err("Você pode guardar até 32 presets personalizados.".into());
    }
    mangohud_config::validate(&settings)?;
    // Loading a visual preset must not change global activation or system export.
    settings.remove("launcher_enabled");
    settings.remove("launcher_sync_system");
    presets.push(Preset { name, settings });
    Ok(())
}
pub fn list() -> Result<Vec<Preset>, String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    read(&mangohud_config::directory()?.join("custom-presets.json"))
}
pub fn save(name: String, settings: Settings) -> Result<Vec<Preset>, String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    let path = mangohud_config::directory()?.join("custom-presets.json");
    let mut presets = read(&path)?;
    add(&mut presets, name, settings)?;
    write(&path, &presets)?;
    Ok(presets)
}
pub fn remove(name: String) -> Result<Vec<Preset>, String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    let path = mangohud_config::directory()?.join("custom-presets.json");
    let mut presets = read(&path)?;
    presets.retain(|p| p.name != name);
    write(&path, &presets)?;
    Ok(presets)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn named_snapshots_preserve_order_and_reject_duplicates() {
        let mut items = vec![];
        let mut s = mangohud_config::defaults();
        let blocks: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../../../src/config/mangohudLayout.json")).unwrap();
        let order = blocks
            .iter()
            .rev()
            .map(|b| b["id"].as_str().unwrap())
            .collect::<Vec<_>>()
            .join(",");
        s.insert("launcher_hud_custom_order".into(), serde_json::json!(order));
        add(&mut items, "  Meu HUD  ".into(), s.clone()).unwrap();
        assert_eq!(items[0].name, "Meu HUD");
        assert_eq!(
            items[0].settings["launcher_hud_custom_order"],
            s["launcher_hud_custom_order"]
        );
        assert!(!items[0].settings.contains_key("launcher_enabled"));
        assert!(!items[0].settings.contains_key("launcher_sync_system"));
        assert!(add(&mut items, "meu hud".into(), s.clone()).is_err());
        assert!(add(&mut items, " ".into(), s.clone()).is_err());
        let root = std::env::temp_dir().join(format!("hud-presets-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("custom-presets.json");
        write(&path, &items).unwrap();
        assert_eq!(read(&path).unwrap()[0].settings, items[0].settings);
        fs::write(&path, "invalid").unwrap();
        assert!(read(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "invalid");
        fs::remove_dir_all(root).unwrap();
    }
}
