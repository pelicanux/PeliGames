//! Shared publication switch. The committed release is restricted; local work
//! can restore the JSON switch without changing the published source/artifacts.
use std::{collections::BTreeMap, path::Path, sync::OnceLock};
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| serde_json::from_str::<serde_json::Value>(include_str!("../../../release-features.json"))
        .ok().and_then(|v| v.get("experimental_frame_generation").and_then(|v| v.as_bool())).unwrap_or(false))
}
pub fn require() -> Result<(), String> {
    if enabled() { Ok(()) } else { Err("Geração de quadros e overlay em desenvolvimento; indisponíveis nesta versão.".into()) }
}
pub fn experimental_program(program: &str) -> bool {
    let p = Path::new(program);
    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
    (program.contains("/FrameGeneration/profiles/") && name.ends_with(".sh"))
        || matches!(name, "mako-run" | "mako-launch" | "lsfg" | "lsfg-vk" | "peligames-overlay-run" | "peligames-framegen-host")
}
/// Filter old profiles at execution, without erasing their saved configuration.
pub fn restrict(command: &mut Vec<String>, environment: &mut BTreeMap<String, Option<String>>, allow: bool) {
    if allow { return; }
    let mut skip_separator = false;
    command.retain(|word| {
        if skip_separator { skip_separator = false; if word == "--" { return false; } }
        if experimental_program(word) { skip_separator = true; return false; }
        true
    });
    environment.retain(|key, _| !(key.starts_with("MAKO_") || key.starts_with("LSFG_") || key.starts_with("PELI_OVERLAY_") || matches!(key.as_str(), "ENABLE_MAKO" | "ENABLE_LSFG" | "ENABLE_LSFGVK")));
    for key in ["LD_PRELOAD", "LD_AUDIT", "VK_INSTANCE_LAYERS", "VK_LAYER_PATH", "VK_ADD_LAYER_PATH", "VK_IMPLICIT_LAYER_PATH"] {
        if let Some(Some(value)) = environment.get_mut(key) {
            let kept = value.split(|c: char| c == ':' || (key.starts_with("LD_") && c.is_ascii_whitespace()))
                .filter(|s| !s.is_empty() && !s.contains("/FrameGeneration/") && !s.contains("peligames_game_overlay") && !s.contains("peligames_overlay_audit") && !s.starts_with("VK_LAYER_MAKO") && !s.starts_with("VK_LAYER_LSFG") && *s != "VK_LAYER_PELIGAMES_overlay")
                .collect::<Vec<_>>().join(":");
            if kept.is_empty() { *environment.get_mut(key).unwrap() = None; } else { *value = kept; }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricted_profile_keeps_game_dlss_and_hud_without_experimental_loading() {
        let mut command = vec!["/c/FrameGeneration/profiles/pal.sh".into(), "mangohud".into(), "__PELIGAMES_STEAM_COMMAND__".into()];
        let mut env = BTreeMap::from([
            ("ENABLE_MAKO".into(), Some("1".into())), ("MAKO_CONFIG".into(), Some("/c/a.toml".into())),
            ("VK_INSTANCE_LAYERS".into(), Some("VK_LAYER_MAKO_render:VK_LAYER_DLSSNR_int4:VK_LAYER_PELIGAMES_overlay:VK_LAYER_MANGOHUD_overlay_x86_64".into())),
            ("LD_PRELOAD".into(), Some("/c/libpeligames_game_overlay.so /steam/gameoverlayrenderer.so".into())),
            ("WINEDLLOVERRIDES".into(), Some("dxgi=n,b".into()))]);
        restrict(&mut command, &mut env, false);
        assert_eq!(command, ["mangohud", "__PELIGAMES_STEAM_COMMAND__"]);
        assert!(!env.contains_key("ENABLE_MAKO"));
        assert_eq!(env["VK_INSTANCE_LAYERS"].as_deref(), Some("VK_LAYER_DLSSNR_int4:VK_LAYER_MANGOHUD_overlay_x86_64"));
        assert_eq!(env["LD_PRELOAD"].as_deref(), Some("/steam/gameoverlayrenderer.so"));
        assert_eq!(env["WINEDLLOVERRIDES"].as_deref(), Some("dxgi=n,b"));
    }
    #[test]
    fn local_switch_preserves_experimental_settings() {
        let mut c = vec!["/c/FrameGeneration/profiles/pal.sh".into(), "marker".into()];
        let mut e = BTreeMap::from([("ENABLE_MAKO".into(), Some("1".into()))]);
        let before = (c.clone(), e.clone()); restrict(&mut c, &mut e, true);
        assert_eq!((c, e), before);
    }
}
