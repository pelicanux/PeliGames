//! The in-process layer owns its key. No global desktop grab or permission UI.
use serde::{Deserialize, Serialize};
#[path = "../../../tools/game-overlay/shortcut.rs"]
mod physical_key;
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub shortcut: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            shortcut: "F8".into(),
        }
    }
}
#[derive(Serialize)]
pub struct Status {
    pub preferences: Preferences,
}
fn path() -> Result<std::path::PathBuf, String> {
    Ok(crate::core::paths::app_root()?.join("frame-overlay.json"))
}
fn load() -> Preferences {
    path()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn same_shortcut(a: &str, b: &str) -> bool {
    match (physical_key::parse(a), physical_key::parse(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
pub(super) fn check_mangohud_shortcuts(
    settings: &crate::core::mangohud_config::Settings,
) -> Result<(), String> {
    if !crate::core::release_features::enabled() { return Ok(()); }
    let p = load();
    physical_key::parse(&p.shortcut)?;
    for (key, value) in settings {
        if (key.starts_with("toggle_") || key == "reset_fps_metrics")
            && value
                .as_str()
                .is_some_and(|v| same_shortcut(v, &p.shortcut))
        {
            return Err(
                "Essa combinação está reservada ao mini painel do PeliGames. Escolha outro atalho."
                    .into(),
            );
        }
    }
    Ok(())
}

#[tauri::command]
pub fn frame_overlay_preferences() -> Status {
    Status {
        preferences: load(),
    }
}
#[tauri::command]
pub async fn configure_frame_overlay(preferences: Preferences) -> Result<(), String> {
    crate::core::release_features::require()?;
    physical_key::parse(&preferences.shortcut)?;
    if same_shortcut(&preferences.shortcut, "Control_L+k")
        || same_shortcut(&preferences.shortcut, "Super_L+k")
    {
        return Err("Esse atalho já abre a busca da biblioteca.".into());
    }
    for (name, value) in super::mangohud::load_mangohud_profile()?.settings {
        if (name.starts_with("toggle_") || name == "reset_fps_metrics")
            && value
                .as_str()
                .is_some_and(|v| same_shortcut(v, &preferences.shortcut))
        {
            return Err(format!(
                "Esse atalho já está atribuído ao MangoHUD ({name})."
            ));
        }
    }
    super::frame_generation::write(
        &path()?,
        &serde_json::to_vec(&preferences).map_err(|e| e.to_string())?,
        0o600,
    )?;
    // Flatpak Steam reads its own copy, kept beside its PeliRun.
    if let Some(root) = super::steam_runtime::flatpak_directory().filter(|root| root.join("overlay").is_dir()) {
        super::frame_overlay_runtime::mirror_preferences(&root)?;
    }
    Ok(())
}
/// Migrate only our previous KDE action. Its persistent global grab would
/// otherwise swallow F8 before the game receives it. Never edit system key files
/// or unregister another component/action; X11 and portal grabs end on exit.
pub fn startup() {
    if !crate::core::release_features::enabled() { return; }
    if !std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .any(|s| s.eq_ignore_ascii_case("KDE"))
    {
        return;
    }
    tauri::async_runtime::spawn(async {
        let result = async {
            let connection = zbus::Connection::session().await?;
            let proxy = zbus::Proxy::new(
                &connection,
                "org.kde.kglobalaccel",
                "/kglobalaccel",
                "org.kde.KGlobalAccel",
            )
            .await?;
            let component = crate::core::paths::APP_IDENTIFIER;
            let actions: Vec<Vec<String>> = proxy
                .call("allActionsForComponent", &(vec![component, ""],))
                .await?;
            if actions.iter().any(|action| {
                action.first().is_some_and(|s| s == component)
                    && action.get(1).is_some_and(|s| s == "frame-overlay")
            }) {
                let removed: bool = proxy
                    .call("unregister", &(component, "frame-overlay"))
                    .await?;
                if !removed {
                    eprintln!("PeliGames: o KDE não liberou o atalho do mini painel antigo.");
                }
            }
            Ok::<(), zbus::Error>(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("Migração do atalho do overlay: {error}");
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_sides_and_modifier_order_are_the_same_conflict() {
        assert!(same_shortcut("Control_L+F8", "F8+Control_R"));
        assert!(!same_shortcut("F8", "Shift_L+F8"));
        assert!(!same_shortcut("not-a-key", "not-a-key"));
    }
}
