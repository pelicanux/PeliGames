//! Only the generic launch contract is linked into the launcher.
use super::{advanced_settings::AdvancedSettings, paths};
use peligames_addon_kit::{
    manifest::{Capability, Category},
    protocol::LaunchContext,
    registry::Store,
};
use std::{collections::BTreeSet, path::Path, process::Command};

pub(crate) fn wrap(mut command: Command, settings: &AdvancedSettings) -> Result<Command, String> {
    if settings.addon_launch.is_empty() && settings.legacy_extensions.is_empty() {
        return Ok(command);
    }
    let root = paths::app_root()?;
    let store = Store {
        packages: root.join(paths::layout::ADDON_PACKAGES),
        state_file: root.join(paths::layout::ADDON_STATE),
    };
    let state = store.read()?;
    let mut groups = BTreeSet::new();
    let mut selected = Vec::new();
    for (id, installed) in state.addons.iter().filter(|(_, entry)| entry.enabled) {
        let Some(manifest) = installed
            .selected
            .as_ref()
            .and_then(|version| installed.versions.get(version))
            .map(|entry| &entry.manifest)
        else {
            continue;
        };
        if !manifest.capabilities.contains(&Capability::NativeLaunch) {
            continue;
        }
        let own = settings.addon_launch.get(id);
        let data = if let Some(own) = own {
            if !own.enabled {
                continue;
            };
            own.data.clone()
        } else if let Some(legacy) = settings
            .legacy_extensions
            .get(id)
            .filter(|value| value["enabled"] == true)
        {
            legacy.clone()
        } else {
            continue;
        };
        if manifest.category == Some(Category::FrameGeneration) {
            if !groups.insert("frame-generation") {
                return Err("Ative apenas um gerador de quadros por jogo.".into());
            }
            if settings.wrappers.iter().any(|wrapper| {
                wrapper.enabled
                    && wrapper.program.contains("/FrameGeneration/profiles/")
                    && wrapper.program.ends_with(".sh")
            }) {
                return Err(
                    "Desative o outro gerador de quadros antes de iniciar este add-on.".into(),
                );
            }
        }
        selected.push((
            id.clone(),
            LaunchContext {
                component_directory: root.clone(),
                settings: data,
                gamescope: settings.gamescope.enabled(),
                monitoring: ["mangohud", "mangohud_env"]
                    .iter()
                    .any(|key| settings.options.get(*key) == Some(&true)),
                options: settings
                    .options
                    .iter()
                    .filter(|(_, on)| **on)
                    .map(|(key, _)| key.clone())
                    .collect(),
            },
        ));
    }
    if selected.is_empty() {
        return Ok(command);
    }
    let helper = crate::commands::launch_profiles::helper()?;
    for (id, context) in selected.into_iter().rev() {
        command = prepend(
            command,
            &helper,
            &root,
            &id,
            &serde_json::to_string(&context).map_err(|e| e.to_string())?,
        );
    }
    Ok(command)
}
fn prepend(command: Command, helper: &Path, root: &Path, id: &str, context: &str) -> Command {
    let mut wrapped = Command::new(helper);
    wrapped
        .arg("--addon")
        .arg(id)
        .arg(root)
        .arg(env!("CARGO_PKG_VERSION"))
        .arg(context)
        .arg("--")
        .arg(command.get_program())
        .args(command.get_args());
    for (key, value) in command.get_envs() {
        if let Some(value) = value {
            wrapped.env(key, value);
        } else {
            wrapped.env_remove(key);
        }
    }
    if let Some(directory) = command.get_current_dir() {
        wrapped.current_dir(directory);
    }
    wrapped
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_preserves_arguments_environment_and_directory_without_a_shell() {
        let mut original = Command::new("/fixture/game with spaces");
        original
            .arg(";literal")
            .arg("unicode ç")
            .env("VK_INSTANCE_LAYERS", "Other")
            .env_remove("REMOVE_THIS")
            .current_dir("/fixture");
        let wrapped = prepend(
            original,
            Path::new("/fixture/pelirun"),
            Path::new("/fixture/config"),
            "example",
            "{\"enabled\":true}",
        );
        let words = wrapped
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            &words[5..],
            ["--", "/fixture/game with spaces", ";literal", "unicode ç"]
        );
        assert_eq!(wrapped.get_current_dir(), Some(Path::new("/fixture")));
        let env = wrapped
            .get_envs()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(
            env[std::ffi::OsStr::new("VK_INSTANCE_LAYERS")],
            Some(std::ffi::OsStr::new("Other"))
        );
        assert_eq!(env[std::ffi::OsStr::new("REMOVE_THIS")], None);
    }
}

/// Normalize the old generic wrapper envelope before assembling a command. Missing
/// or disabled packages then follow the same bypass behavior as current add-ons.
pub(crate) fn migrate(settings: &mut AdvancedSettings) -> Result<(), String> {
    if settings.wrappers.is_empty() {
        return Ok(());
    }
    let root = paths::app_root()?;
    migrate_at(settings, &root)
}
fn migrate_at(settings: &mut AdvancedSettings, root: &Path) -> Result<(), String> {
    let mut keep = Vec::new();
    for wrapper in std::mem::take(&mut settings.wrappers) {
        if wrapper.arguments.is_empty() {
            if let Some((id, mut data)) =
                peligames_addon_kit::launch::legacy_profile(&root, Path::new(&wrapper.program))?
            {
                data["enabled"] = serde_json::json!(wrapper.enabled && data["enabled"] == true);
                if data["enabled"] == true
                    && data["overlay_enabled"] == true
                    && settings.overlay_enabled.is_none()
                {
                    settings.overlay_enabled = Some(true);
                }
                settings
                    .addon_launch
                    .entry(id)
                    .or_insert(peligames_addon_kit::launch::Settings {
                        enabled: data["enabled"] == true,
                        group: Some(Category::FrameGeneration),
                        data,
                    });
                continue;
            }
            if peligames_addon_kit::launch::legacy_path(root, Path::new(&wrapper.program)) {
                continue;
            }
        }
        keep.push(wrapper);
    }
    settings.wrappers = keep;
    Ok(())
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    #[test]
    fn old_wrappers_become_opaque_selections_and_keep_an_explicit_disable() {
        let root = std::env::temp_dir().join(format!(
            "peligames-launch-migration-{}",
            uuid::Uuid::new_v4()
        ));
        let profiles = root.join("components/FrameGeneration/profiles");
        std::fs::create_dir_all(&profiles).unwrap();
        let script = profiles.join("fixture-game.sh");
        std::fs::write(
            script.with_extension("json"),
            br#"{"backend":"fixture","enabled":true,"overlay_enabled":true,"multiplier":4}"#,
        )
        .unwrap();
        let mut settings = AdvancedSettings::default();
        settings.wrappers = vec![
            super::super::advanced_settings::LaunchWrapper {
                program: script.to_string_lossy().into(),
                arguments: String::new(),
                enabled: true,
            },
            super::super::advanced_settings::LaunchWrapper {
                program: "/custom/wrapper".into(),
                arguments: "literal".into(),
                enabled: true,
            },
        ];
        migrate_at(&mut settings, &root).unwrap();
        assert_eq!(settings.wrappers.len(), 1);
        assert_eq!(settings.overlay_enabled, Some(true));
        let own = &settings.addon_launch["fixture"];
        assert!(own.enabled);
        assert_eq!(own.group, Some(Category::FrameGeneration));
        assert_eq!(own.data["legacy_profile"], script.to_str().unwrap());
        assert!(own.data.get("multiplier").is_none());
        settings.addon_launch.get_mut("fixture").unwrap().enabled = false;
        settings
            .wrappers
            .push(super::super::advanced_settings::LaunchWrapper {
                program: script.to_string_lossy().into(),
                arguments: String::new(),
                enabled: true,
            });
        migrate_at(&mut settings, &root).unwrap();
        assert!(!settings.addon_launch["fixture"].enabled);
        assert_eq!(settings.wrappers.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
