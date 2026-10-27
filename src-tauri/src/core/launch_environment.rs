//! Compose per-game mod requirements with the launcher's managed Vulkan layers.
use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    process::Command,
};

pub(crate) fn is_vulkan_list(key: &OsStr) -> bool {
    matches!(
        key.to_str(),
        Some("VK_INSTANCE_LAYERS" | "VK_ADD_LAYER_PATH" | "VK_LAYER_PATH")
    )
}

fn value(command: &Command, key: &str) -> Option<OsString> {
    command
        .get_envs()
        .find(|(name, _)| *name == key)
        .and_then(|(_, value)| value.map(OsString::from))
}

// Add mod entries before the existing managed chain. Repeated requirements must
// not move AFMF, MangoHUD or Steam out of their established relative order.
fn combined(base: &OsStr, additions: &OsStr) -> OsString {
    let mut existing: Vec<&[u8]> = Vec::new();
    for item in base
        .as_bytes()
        .split(|b| *b == b':')
        .filter(|item| !item.is_empty())
    {
        if !existing.contains(&item) {
            existing.push(item);
        }
    }
    let mut items: Vec<&[u8]> = Vec::new();
    for item in additions
        .as_bytes()
        .split(|b| *b == b':')
        .filter(|item| !item.is_empty())
    {
        if !existing.contains(&item) && !items.contains(&item) {
            items.push(item);
        }
    }
    items.extend(existing);
    OsString::from_vec(items.join(&b':'))
}

/// Merge only explicit child-process variables, never the launcher's environment.
/// Scalar overrides and removals retain their existing behavior; Vulkan lists
/// are additive so an installed mod cannot erase the managed generator chain.
/// Arguments and working directories remain the caller's responsibility.
pub(crate) fn apply(command: &mut Command, overrides: &Command) {
    for (key, next) in overrides.get_envs() {
        match next {
            Some(next) if is_vulkan_list(key) => {
                let previous = command
                    .get_envs()
                    .find(|(name, _)| *name == key)
                    .and_then(|(_, value)| value)
                    .unwrap_or_else(|| OsStr::new(""));
                let merged = combined(previous, next);
                command.env(key, merged);
            }
            Some(next) => {
                command.env(key, next);
            }
            None => {
                command.env_remove(key);
            }
        }
    }
    // VK_LAYER_PATH suppresses additive discovery in the Vulkan loader. When
    // a mod supplies that override, managed manifests still need to be visible.
    if let (Some(explicit), Some(additional)) = (
        value(command, "VK_LAYER_PATH"),
        value(command, "VK_ADD_LAYER_PATH"),
    ) {
        command.env("VK_LAYER_PATH", combined(&explicit, &additional));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn managed() -> Command {
        let mut command = Command::new("game");
        command.env("VK_INSTANCE_LAYERS", "VK_LAYER_VALVE_steam_overlay_64:VK_LAYER_PELIGAMES_overlay:VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64")
            .env("VK_ADD_LAYER_PATH", "/managed/mango:/managed/afmf")
            .env("DISABLE_AFMF", "1").env("DISABLE_MANGOHUD", "1")
            .env("AFMF_ENABLE", "1").env("AFMF_ASYNC", "1")
            .env("MANGOHUD_CONFIG", "read_cfg,fps_limit=144,vsync=-1");
        command
    }
    fn nr() -> Command {
        let mut command = Command::new("options");
        command
            .env(
                "VK_INSTANCE_LAYERS",
                "VK_LAYER_DLSSNR_int4:VK_LAYER_DLSSNR_control",
            )
            .env(
                "VK_ADD_LAYER_PATH",
                "/game with spaces/int4/layer:/game with spaces/control/layer",
            )
            .env("WINEDLLOVERRIDES", "dxgi=n,b")
            .arg("-dx12");
        command
    }
    fn snapshot(command: &Command) -> Vec<(OsString, Option<OsString>)> {
        command
            .get_envs()
            .map(|(k, v)| (k.into(), v.map(OsString::from)))
            .collect()
    }
    #[test]
    fn dlssnr_preserves_afmf_monitoring_order_and_discovery() {
        let mut command = managed();
        command.arg("original");
        apply(&mut command, &nr());
        assert_eq!(value(&command,"VK_INSTANCE_LAYERS").unwrap(), "VK_LAYER_DLSSNR_int4:VK_LAYER_DLSSNR_control:VK_LAYER_VALVE_steam_overlay_64:VK_LAYER_PELIGAMES_overlay:VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64");
        assert_eq!(value(&command,"VK_ADD_LAYER_PATH").unwrap(), "/game with spaces/int4/layer:/game with spaces/control/layer:/managed/mango:/managed/afmf");
        assert_eq!(value(&command, "AFMF_ASYNC").unwrap(), "1");
        assert_eq!(
            value(&command, "MANGOHUD_CONFIG").unwrap(),
            "read_cfg,fps_limit=144,vsync=-1"
        );
        assert_eq!(value(&command, "WINEDLLOVERRIDES").unwrap(), "dxgi=n,b");
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["original"]);
        let before = snapshot(&command);
        apply(&mut command, &nr());
        assert_eq!(snapshot(&command), before);
    }
    #[test]
    fn repeated_managed_names_cannot_reorder_the_generator_chain() {
        let mut command = managed();
        let before = value(&command, "VK_INSTANCE_LAYERS").unwrap();
        let mut overrides = Command::new("options");
        overrides.env(
            "VK_INSTANCE_LAYERS",
            "VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_AFMF:VK_LAYER_AFMF",
        );
        apply(&mut command, &overrides);
        assert_eq!(value(&command, "VK_INSTANCE_LAYERS").unwrap(), before);
    }
    #[test]
    fn explicit_mod_path_keeps_managed_manifests_visible() {
        let mut command = managed();
        let mut overrides = nr();
        overrides.env("VK_LAYER_PATH", "/custom/only");
        apply(&mut command, &overrides);
        assert_eq!(value(&command,"VK_LAYER_PATH").unwrap(), "/game with spaces/int4/layer:/game with spaces/control/layer:/managed/mango:/managed/afmf:/custom/only");
        let before = snapshot(&command);
        apply(&mut command, &overrides);
        assert_eq!(snapshot(&command), before);
    }
    #[test]
    fn scalar_overrides_removals_and_unrelated_variables_keep_their_semantics() {
        let mut command = Command::new("game");
        command
            .env("WINEDLLOVERRIDES", "version=n,b")
            .env("REMOVE", "old")
            .env("KEEP", "yes");
        let mut overrides = nr();
        overrides.env_remove("REMOVE").env("SCALAR", "new");
        apply(&mut command, &overrides);
        assert_eq!(value(&command, "WINEDLLOVERRIDES").unwrap(), "dxgi=n,b");
        assert_eq!(value(&command, "KEEP").unwrap(), "yes");
        assert!(command
            .get_envs()
            .any(|(k, v)| k == "REMOVE" && v.is_none()));
        assert_eq!(value(&command, "SCALAR").unwrap(), "new");
    }
    #[test]
    fn no_generator_does_not_add_managed_layers_and_mako_order_is_preserved() {
        let mut command = Command::new("game");
        apply(&mut command, &nr());
        assert_eq!(
            value(&command, "VK_INSTANCE_LAYERS"),
            value(&nr(), "VK_INSTANCE_LAYERS")
        );
        command.env(
            "VK_INSTANCE_LAYERS",
            "VK_LAYER_MAKO_render:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_VALVE_steam_overlay_64",
        );
        apply(&mut command, &nr());
        assert_eq!(value(&command,"VK_INSTANCE_LAYERS").unwrap(), "VK_LAYER_DLSSNR_int4:VK_LAYER_DLSSNR_control:VK_LAYER_MAKO_render:VK_LAYER_MANGOHUD_overlay_x86_64:VK_LAYER_VALVE_steam_overlay_64");
    }
    #[test]
    fn paths_preserve_non_utf8_bytes() {
        let mut command = Command::new("game");
        command.env(
            "VK_ADD_LAYER_PATH",
            OsString::from_vec(b"/managed/\xff".to_vec()),
        );
        let mut overrides = Command::new("options");
        overrides.env("VK_ADD_LAYER_PATH", "/mod");
        apply(&mut command, &overrides);
        assert_eq!(
            value(&command, "VK_ADD_LAYER_PATH").unwrap().as_bytes(),
            b"/mod:/managed/\xff"
        );
    }
}
