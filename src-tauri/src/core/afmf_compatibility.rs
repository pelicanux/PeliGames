//! Per-launch AFMF presentation compatibility; saved profiles stay unchanged.
use std::{ffi::OsString, path::Path, process::Command};

fn value(command: &Command, key: &str) -> Option<OsString> {
    command
        .get_envs()
        .find(|(name, _)| *name == key)
        .and_then(|(_, value)| value.map(OsString::from))
}

/// Discover AFMF as an explicit layer, after existing overlays and mods.
/// The private manifest has no implicit enable/disable rules; DISABLE_AFMF
/// suppresses a separately installed implicit copy without suppressing ours.
pub(crate) fn layer(
    command: &mut Command,
    directory: &Path,
    inherited_paths: Option<OsString>,
    inherited_layers: Option<OsString>,
    inherited_override: Option<OsString>,
) -> Result<(), String> {
    let previous = value(command, "VK_ADD_LAYER_PATH").or(inherited_paths);
    let mut paths = vec![directory.to_path_buf()];
    if let Some(previous) = previous {
        for path in std::env::split_paths(&previous) {
            if !path.as_os_str().is_empty() && !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    // VK_LAYER_PATH suppresses additive discovery. Preserve that boundary,
    // including mod paths, while making AFMF visible without our renderer.
    if let Some(previous) = value(command, "VK_LAYER_PATH").or(inherited_override) {
        let mut overridden = paths.clone();
        for path in std::env::split_paths(&previous) {
            if !path.as_os_str().is_empty() && !overridden.contains(&path) {
                overridden.push(path);
            }
        }
        command.env(
            "VK_LAYER_PATH",
            std::env::join_paths(overridden).map_err(|e| e.to_string())?,
        );
    }
    let paths = std::env::join_paths(paths).map_err(|e| e.to_string())?;
    let previous = value(command, "VK_INSTANCE_LAYERS")
        .or(inherited_layers)
        .unwrap_or_default();
    let previous = previous
        .to_str()
        .ok_or("Lista de camadas Vulkan inválida.")?;
    let mut layers: Vec<_> = previous
        .split(':')
        .filter(|s| {
            !s.is_empty() && !matches!(*s, "VK_LAYER_AFMF" | "VK_LAYER_PELIGAMES_present_counter")
        })
        .collect();
    layers.push("VK_LAYER_AFMF");
    command
        .env("VK_ADD_LAYER_PATH", paths)
        .env("VK_INSTANCE_LAYERS", layers.join(":"))
        .env("DISABLE_AFMF", "1");
    Ok(())
}

/// Count both game and generated presents without moving Steam/GE below AFMF.
/// The manifest is offered explicitly; implicit activation is disabled so that
/// the loader does not place MangoHUD above the frame generator a second time.
pub(crate) fn monitoring(command: &mut Command, directory: &Path) -> Result<(), String> {
    if value(command, "AFMF_ENABLE").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return Ok(());
    }
    let previous = value(command, "VK_INSTANCE_LAYERS").unwrap_or_default();
    let previous = previous
        .to_str()
        .ok_or("Lista de camadas Vulkan inválida.")?;
    let mut layers: Vec<_> = previous
        .split(':')
        .filter(|name| {
            !name.is_empty()
                && !matches!(
                    *name,
                    "VK_LAYER_MANGOHUD_overlay_x86_64" | "VK_LAYER_MANGOHUD_overlay_x86"
                )
        })
        .collect();
    let index = layers
        .iter()
        .position(|name| *name == "VK_LAYER_AFMF")
        .ok_or("Camada explícita do AFMF ausente.")?;
    layers.insert(index + 1, "VK_LAYER_MANGOHUD_overlay_x86_64");
    let mut paths = vec![directory.to_path_buf()];
    if let Some(previous) = value(command, "VK_ADD_LAYER_PATH") {
        for path in std::env::split_paths(&previous) {
            if !path.as_os_str().is_empty() && !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    let additional = std::env::join_paths(&paths).map_err(|error| error.to_string())?;
    let overridden = if let Some(previous) = value(command, "VK_LAYER_PATH") {
        for path in std::env::split_paths(&previous) {
            if !path.as_os_str().is_empty() && !paths.contains(&path) {
                paths.push(path);
            }
        }
        Some(std::env::join_paths(paths).map_err(|error| error.to_string())?)
    } else {
        None
    };
    if let Some(overridden) = overridden {
        command.env("VK_LAYER_PATH", overridden);
    }
    command
        .env("VK_ADD_LAYER_PATH", additional)
        .env("VK_INSTANCE_LAYERS", layers.join(":"))
        .env("DISABLE_MANGOHUD", "1")
        .env("MANGOHUD", "1");
    Ok(())
}

/// Apply after monitoring has composed its FPS cap and visibility overrides.
pub(crate) fn apply(command: &mut Command) {
    if value(command, "AFMF_ENABLE").as_deref() != Some(std::ffi::OsStr::new("1")) {
        return;
    }
    // AFMF is loaded explicitly below the overlays. Its native worker and
    // presenter threads must never execute GE's Wine/Wayland event bridge.
    command.env("AFMF_ASYNC", "1");
    // MangoHUD's present-mode override can leave the borrowed present chain
    // pointing to stack memory. Let the game/AFMF choose the mode for this run.
    if value(command, "MANGOHUD_CONFIGFILE").is_some() {
        let previous = value(command, "MANGOHUD_CONFIG").unwrap_or_else(|| "read_cfg".into());
        let mut options: Vec<_> = previous
            .to_string_lossy()
            .split(',')
            .filter(|option| {
                option
                    .split_once('=')
                    .is_none_or(|(key, _)| key.trim() != "vsync")
            })
            .map(str::to_owned)
            .collect();
        options.push("vsync=-1".into());
        command.env("MANGOHUD_CONFIG", options.join(","));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(wayland: bool, hud: bool) -> Command {
        let mut command = Command::new("true");
        command.env("AFMF_ENABLE", "1");
        command.env("PROTON_ENABLE_WAYLAND", if wayland { "1" } else { "0" });
        if hud {
            // A fixture path: no user configuration is read or written.
            command.env("MANGOHUD_CONFIGFILE", "/fixture/MangoHud.conf");
        }
        command
    }

    #[test]
    fn hud_counts_after_afmf_without_moving_steam_or_renderer() {
        let mut command = command(true, true);
        command.env("VK_INSTANCE_LAYERS", "VK_LAYER_VALVE_steam_overlay_64:VK_LAYER_PELIGAMES_overlay:VK_LAYER_NR:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_AFMF:VK_LAYER_PELIGAMES_present_counter");
        command.env("MANGOHUD_CONFIG", "read_cfg,fps_limit=144,no_display=1");
        command.env("PELI_OVERLAY_BACKGROUND_FPS", "30");
        monitoring(&mut command, Path::new("/private/mango")).unwrap();
        apply(&mut command);
        assert_eq!(value(&command, "VK_INSTANCE_LAYERS"), Some("VK_LAYER_VALVE_steam_overlay_64:VK_LAYER_PELIGAMES_overlay:VK_LAYER_NR:VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_PELIGAMES_present_counter".into()));
        assert_eq!(value(&command, "AFMF_ASYNC"), Some("1".into()));
        assert_eq!(value(&command, "DISABLE_MANGOHUD"), Some("1".into()));
        assert_eq!(value(&command, "MANGOHUD"), Some("1".into()));
        assert_eq!(
            value(&command, "MANGOHUD_CONFIG"),
            Some("read_cfg,fps_limit=144,no_display=1,vsync=-1".into())
        );
        assert_eq!(
            value(&command, "PELI_OVERLAY_BACKGROUND_FPS"),
            Some("30".into())
        );
    }

    #[test]
    fn hud_discovery_keeps_override_and_additive_mod_paths_without_duplicates() {
        let mut command = command(false, true);
        command.env(
            "VK_INSTANCE_LAYERS",
            "VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_MANGOHUD_overlay_x86",
        );
        command.env("VK_ADD_LAYER_PATH", "/private/afmf:/mods/nr");
        command.env("VK_LAYER_PATH", "/mods/control:/private/afmf");
        command.env("VK_ADD_IMPLICIT_LAYER_PATH", "/mods/implicit");
        monitoring(&mut command, Path::new("/private/mango")).unwrap();
        let before: Vec<_> = command
            .get_envs()
            .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
            .collect();
        monitoring(&mut command, Path::new("/private/mango")).unwrap();
        assert_eq!(
            command
                .get_envs()
                .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(
            value(&command, "VK_ADD_LAYER_PATH"),
            Some("/private/mango:/private/afmf:/mods/nr".into())
        );
        assert_eq!(
            value(&command, "VK_LAYER_PATH"),
            Some("/private/mango:/private/afmf:/mods/nr:/mods/control".into())
        );
        assert_eq!(
            value(&command, "VK_ADD_IMPLICIT_LAYER_PATH"),
            Some("/mods/implicit".into())
        );
        assert_eq!(
            value(&command, "VK_INSTANCE_LAYERS"),
            Some("VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64".into())
        );
    }

    #[test]
    fn hud_without_afmf_keeps_its_existing_discovery_and_activation() {
        let mut command = command(true, true);
        command.env("AFMF_ENABLE", "0").env("ENABLE_MAKO", "1");
        command.env(
            "VK_INSTANCE_LAYERS",
            "VK_LAYER_MAKO_render:VK_LAYER_MANGOHUD_overlay_x86_64",
        );
        command.env("MANGOHUD", "1");
        let before: Vec<_> = command
            .get_envs()
            .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
            .collect();
        monitoring(&mut command, Path::new("/private/mango")).unwrap();
        assert_eq!(
            command
                .get_envs()
                .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
                .collect::<Vec<_>>(),
            before
        );
    }

    #[test]
    fn missing_explicit_afmf_does_not_partially_disable_monitoring() {
        let mut command = command(true, true);
        command.env("VK_INSTANCE_LAYERS", "VK_LAYER_PELIGAMES_overlay");
        let before: Vec<_> = command
            .get_envs()
            .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
            .collect();
        assert!(monitoring(&mut command, Path::new("/private/mango")).is_err());
        assert_eq!(
            command
                .get_envs()
                .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
                .collect::<Vec<_>>(),
            before
        );
    }

    #[test]
    fn wayland_with_both_overlays_gets_the_validated_combination() {
        let mut command = command(true, true);
        command.env("PELI_OVERLAY_GENERAL", "1");
        let args: Vec<_> = command.get_args().map(OsString::from).collect();
        apply(&mut command);
        assert_eq!(value(&command, "AFMF_ASYNC"), Some("1".into()));
        assert_eq!(
            value(&command, "MANGOHUD_CONFIG"),
            Some("read_cfg,vsync=-1".into())
        );
        assert_eq!(value(&command, "PELI_OVERLAY_GENERAL"), Some("1".into()));
        assert_eq!(
            command.get_args().map(OsString::from).collect::<Vec<_>>(),
            args
        );
    }

    #[test]
    fn wayland_without_mangohud_keeps_async_generation() {
        let mut command = command(true, false);
        apply(&mut command);
        assert_eq!(value(&command, "AFMF_ASYNC"), Some("1".into()));
        assert_eq!(value(&command, "MANGOHUD_CONFIG"), None);
    }

    #[test]
    fn xwayland_keeps_async_but_does_not_force_mangohud_present_mode() {
        let mut command = command(false, true);
        apply(&mut command);
        assert_eq!(value(&command, "AFMF_ASYNC"), Some("1".into()));
        assert_eq!(
            value(&command, "MANGOHUD_CONFIG"),
            Some("read_cfg,vsync=-1".into())
        );
    }

    #[test]
    fn disabled_afmf_leaves_monitoring_and_other_generators_unchanged() {
        for enabled in [None, Some("0")] {
            let mut command = command(true, true);
            command.env_remove("AFMF_ENABLE");
            if let Some(enabled) = enabled {
                command.env("AFMF_ENABLE", enabled);
            }
            command.env("ENABLE_MAKO", "1");
            command.env(
                "MANGOHUD_CONFIG",
                "read_cfg,vsync=3,fps_limit=144,no_display=1",
            );
            let original: Vec<_> = command
                .get_envs()
                .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
                .collect();
            apply(&mut command);
            assert_eq!(
                command
                    .get_envs()
                    .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
                    .collect::<Vec<_>>(),
                original
            );
        }
    }

    #[test]
    fn fps_limits_visibility_and_config_paths_survive_repeated_application() {
        for config in [
            "read_cfg,fps_limit=144,vsync=3,no_display=1",
            "read_cfg,fps_limit=0,no_display=1",
        ] {
            let mut command = command(true, true);
            command.env("MANGOHUD_CONFIG", config);
            command.env("PELI_OVERLAY_BACKGROUND_FPS", "30");
            apply(&mut command);
            let first = value(&command, "MANGOHUD_CONFIG").unwrap();
            apply(&mut command);
            assert_eq!(value(&command, "MANGOHUD_CONFIG"), Some(first.clone()));
            let first = first.to_string_lossy();
            assert!(first.contains(if config.contains("144") {
                "fps_limit=144"
            } else {
                "fps_limit=0"
            }));
            assert!(first.contains("read_cfg") && first.contains("no_display=1"));
            assert_eq!(first.matches("vsync=").count(), 1);
            assert_eq!(
                value(&command, "MANGOHUD_CONFIGFILE"),
                Some("/fixture/MangoHud.conf".into())
            );
            assert_eq!(
                value(&command, "PELI_OVERLAY_BACKGROUND_FPS"),
                Some("30".into())
            );
        }
    }

    #[test]
    fn hidden_monitoring_does_not_get_reenabled() {
        let mut command = command(true, true);
        command.env("MANGOHUD_CONFIG", "no_display=1");
        apply(&mut command);
        assert_eq!(
            value(&command, "MANGOHUD_CONFIG"),
            Some("no_display=1,vsync=-1".into())
        );
    }

    #[test]
    fn explicit_path_override_can_find_afmf_without_the_peligames_renderer() {
        let mut command = command(true, true);
        layer(
            &mut command,
            Path::new("/private/afmf"),
            Some("/mods/nr".into()),
            None,
            Some("/custom/layers".into()),
        )
        .unwrap();
        assert_eq!(
            value(&command, "VK_LAYER_PATH"),
            Some("/private/afmf:/mods/nr:/custom/layers".into())
        );
        assert_eq!(
            value(&command, "VK_INSTANCE_LAYERS"),
            Some("VK_LAYER_AFMF".into())
        );
    }
    #[test]
    fn explicit_discovery_preserves_other_mods_and_moves_afmf_to_the_end() {
        let mut command = command(true, true);
        command.env("VK_ADD_IMPLICIT_LAYER_PATH", "/mods/implicit");
        layer(
            &mut command,
            Path::new("/private/afmf"),
            Some("/mods/explicit:/private/afmf".into()),
            Some("VK_LAYER_AFMF:VK_LAYER_PELIGAMES_overlay:VK_LAYER_NR".into()),
            None,
        )
        .unwrap();
        assert_eq!(
            value(&command, "VK_INSTANCE_LAYERS"),
            Some("VK_LAYER_PELIGAMES_overlay:VK_LAYER_NR:VK_LAYER_AFMF".into())
        );
        assert_eq!(
            value(&command, "VK_ADD_LAYER_PATH"),
            Some("/private/afmf:/mods/explicit".into())
        );
        assert_eq!(
            value(&command, "VK_ADD_IMPLICIT_LAYER_PATH"),
            Some("/mods/implicit".into())
        );
        assert_eq!(value(&command, "DISABLE_AFMF"), Some("1".into()));
        let before: Vec<_> = command
            .get_envs()
            .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
            .collect();
        layer(&mut command, Path::new("/private/afmf"), None, None, None).unwrap();
        assert_eq!(
            command
                .get_envs()
                .map(|(k, v)| (k.to_owned(), v.map(OsString::from)))
                .collect::<Vec<_>>(),
            before
        );
    }
}
