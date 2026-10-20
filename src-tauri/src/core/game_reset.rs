//! Reset only the settings owned by the selected details tab.
use super::advanced_settings::{AdvancedSettings, GamescopeSettings};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Mods,
    Dlss,
    FrameGeneration,
    Performance,
    Options,
    Advanced,
    All,
}
pub fn frame_wrapper(program: &str) -> bool {
    program.contains("/FrameGeneration/profiles/") && program.ends_with(".sh")
}
pub fn settings(mut value: AdvancedSettings, section: Section) -> AdvancedSettings {
    if section == Section::All {
        let mut clean = AdvancedSettings::default();
        for enabled in clean.options.values_mut() {
            *enabled = false;
        }
        clean.mangohud_preset = value.mangohud_preset.clone();
        clean.overlay_enabled = Some(false);
        clean.background_fps = None;
        clean.mangohud_enabled = Some(false);
        clean.mangohud_fps_limit = Some(0);
        return clean;
    }

    if matches!(section, Section::All | Section::Options) {
        value.launch_options = None;
        value.dll_overrides.clear();
        value.options.insert("dll_overrides".into(), false);
        value.options.insert("steam_client".into(), false);
    }
    if matches!(section, Section::All | Section::Performance) {
        value.overlay_enabled = Some(false);
        value.background_fps = None;
        value.mangohud_enabled = Some(false);
        value.mangohud_fps_limit = Some(0);
        value.options.insert("mangohud".into(), false);
        value.options.insert("mangohud_env".into(), false);
        value.gamescope.enable_limiter = false;
        value.gamescope.fps = 0;
        value.gamescope.fps_unfocused = 0;
        if !value.gamescope.enabled() {
            value.launch_backend = "system".into();
        }
    }
    if matches!(section, Section::All | Section::FrameGeneration) {
        value.wrappers.retain(|w| !frame_wrapper(&w.program));
    }
    if matches!(section, Section::All | Section::Advanced) {
        for (key, enabled) in value.options.iter_mut() {
            if !matches!(
                key.as_str(),
                "dll_overrides" | "steam_client" | "mangohud" | "mangohud_env"
            ) {
                *enabled = false;
            }
        }
        value.gamescope = GamescopeSettings {
            model_version: 2,
            ..Default::default()
        };
        value.launch_backend = "system".into();
        value
            .wrappers
            .retain(|w| section != Section::All && frame_wrapper(&w.program));
    }
    value
}
#[cfg(test)]
mod tests {
    use super::*;
    fn configured() -> AdvancedSettings {
        let mut s = AdvancedSettings::default();
        for v in s.options.values_mut() {
            *v = true;
        }
        s.launch_options = Some("CUSTOM=1 %command%".into());
        s.dll_overrides = "dxgi=n,b".into();
        s.mangohud_enabled = Some(true);
        s.mangohud_fps_limit = Some(60);
        s.overlay_enabled=Some(true);s.background_fps=Some(30);
        s.wrappers = vec![
            super::super::advanced_settings::LaunchWrapper {
                program: "/config/FrameGeneration/profiles/game-12345678.sh".into(),
                arguments: String::new(),
                enabled: true,
            },
            super::super::advanced_settings::LaunchWrapper {
                program: "/usr/bin/custom".into(),
                arguments: String::new(),
                enabled: true,
            },
        ];
        s
    }
    #[test]
    fn tab_resets_do_not_discard_other_tabs() {
        let original = configured();
        let options = settings(original.clone(), Section::Options);
        assert!(options.launch_options.is_none());
        assert!(options.dll_overrides.is_empty());
        assert_eq!(options.mangohud_fps_limit, Some(60));
        assert_eq!(options.options["wayland"], true);
        assert_eq!(options.wrappers.len(), 2);
        let advanced = settings(original.clone(), Section::Advanced);
        assert_eq!(advanced.launch_options, original.launch_options);
        assert_eq!(advanced.dll_overrides, original.dll_overrides);
        assert_eq!(advanced.mangohud_enabled, Some(true));
        assert!(!advanced.options["wayland"]);assert_eq!(advanced.overlay_enabled,Some(true));assert_eq!(advanced.background_fps,Some(30));
        assert_eq!(advanced.wrappers.len(), 1);
        assert!(frame_wrapper(&advanced.wrappers[0].program));
        let performance = settings(original.clone(), Section::Performance);
        assert_eq!(performance.mangohud_enabled, Some(false));
        assert_eq!(performance.mangohud_fps_limit, Some(0));assert_eq!(performance.overlay_enabled,Some(false));assert_eq!(performance.background_fps,None);
        assert_eq!(performance.launch_options, original.launch_options);
        assert_eq!(performance.options["wayland"], true);
        let fg = settings(original.clone(), Section::FrameGeneration);
        assert_eq!(fg.wrappers.len(), 1);assert_eq!(fg.overlay_enabled,Some(true));assert_eq!(fg.background_fps,Some(30));
        assert_eq!(fg.wrappers[0].program, "/usr/bin/custom");
        let all = settings(original, Section::All);
        assert!(all.wrappers.is_empty());
        assert!(all.options.values().all(|v| !*v));
        assert!(all.launch_options.is_none());
        assert_eq!(all.mangohud_fps_limit, Some(0));
    }
    #[test]
    fn performance_keeps_upscaling_but_removes_its_limiter() {
        let mut original = AdvancedSettings::default();
        original.gamescope.enable_upscaling = true;
        original.gamescope.enable_limiter = true;
        original.gamescope.width = 1280;
        original.gamescope.fps = 144;
        original.gamescope.fps_unfocused = 30;
        original.launch_backend = "gamescope".into();
        let next = settings(original, Section::Performance);
        assert!(next.gamescope.enable_upscaling);
        assert!(!next.gamescope.enable_limiter);
        assert_eq!(next.gamescope.width, 1280);
        assert_eq!(next.gamescope.fps, 0);
        assert_eq!(next.gamescope.fps_unfocused, 0);
        assert_eq!(next.launch_backend, "gamescope");
    }
    #[test]
    fn file_only_resets_preserve_settings_and_full_reset_drops_obsolete_flags() {
        let original = configured();
        for section in [Section::Mods, Section::Dlss] {
            assert_eq!(
                serde_json::to_value(settings(original.clone(), section)).unwrap(),
                serde_json::to_value(&original).unwrap()
            );
        }
        let mut legacy = original;
        legacy.options.insert("obsolete_option".into(), true);
        let next = settings(legacy, Section::All);
        assert!(!next.options.contains_key("obsolete_option"));
    }
}
