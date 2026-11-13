use crate::{
    detection, neural_config,
    neural_models::{self, ORIGINAL, SMALL},
    runtime,
    script_runner::{self, InstallExtras, NeuralModel},
    settings,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
#[derive(Serialize)]
pub struct State {
    pub installation: detection::GameInstallationDetails,
    pub settings: Option<settings::NeuralSettings>,
    pub bits: String,
    pub original: bool,
    pub small: bool,
    pub experimental: bool,
    pub optimized: bool,
    pub lumenite: bool,
    pub optimized_layers: Option<u8>,
    pub selected_network: Option<String>,
    pub ready: bool,
    pub route: Option<String>,
    pub shortcut: String,
    pub lumenite_selected: bool,
}
pub fn inspect(root: &Path, directory: &Path, bits: &str) -> Result<State, String> {
    validate_directory(directory)?;
    if !matches!(bits, "32" | "64") {
        return Err("Invalid game architecture".into());
    }
    let component = runtime::component(root, bits).ok();
    let script = component
        .as_ref()
        .and_then(|dir| fs::read_to_string(dir.join("install.sh")).ok())
        .unwrap_or_default();
    let models = component.as_ref().map(|dir| dir.join("dlssnr-amd"));
    let has = |name| {
        models
            .as_ref()
            .and_then(|dir| neural_models::find(dir, name))
            .is_some()
    };
    let installation = detection::details_in_directory(directory);
    let route = if !installation.dlssnr_installed {
        None
    } else if installation.status == "OptiScaler" {
        Some("optiscaler".to_string())
    } else {
        Some(
            match installation.installed_dll.as_deref() {
                Some("vulkan-1.dll" | "winevulkan.dll") => "vulkan",
                Some("d3d9.dll") => "dx9",
                _ => "reshade",
            }
            .to_string(),
        )
    };
    let shortcut =
        script_runner::menu_shortcut(directory, route.as_deref().unwrap_or("optiscaler"))?;
    let ini = settings::read_file(&directory.join("dlssnr-amd.ini"))?;
    Ok(State {
        installation,
        settings: settings::get_settings(directory).ok(),
        bits: bits.into(),
        original: has(ORIGINAL),
        small: has(SMALL) && has(ORIGINAL),
        experimental: script_runner::experimental_network(&script)
            && component.as_ref().is_some_and(|dir| {
                dir.join("experimental/dlssnr-small-experimental.delta")
                    .is_file()
            }),
        optimized: script_runner::optimized_network(&script),
        lumenite: script_runner::supports_lumenite(&script),
        optimized_layers: neural_config::optimized_layers(&ini),
        selected_network: neural_config::get(&neural_config::sections(&ini), "Network", "Model")
            .map(str::to_owned),
        ready: component.is_some(),
        route,
        shortcut,
        lumenite_selected: settings::lumenite_selected(directory)?,
    })
}
fn validate_directory(directory: &Path) -> Result<(), String> {
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("Choose the existing executable directory".into());
    }
    peligames_addon_kit::files::check_directory(directory)
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Action {
    Install {
        route: String,
        #[serde(default)]
        model: Option<NeuralModel>,
        #[serde(default)]
        int4: bool,
        #[serde(default)]
        renodx: bool,
        #[serde(default)]
        lumenite: bool,
        #[serde(default = "enabled")]
        enabled: bool,
        #[serde(default)]
        optimized_layers: Option<u8>,
        shortcut: String,
    },
    Remove,
    Startup {
        enabled: bool,
        renodx: Option<bool>,
    },
    Optimized {
        level: u8,
    },
    Shortcut {
        key: String,
        route: String,
    },
}
fn enabled() -> bool {
    true
}
pub fn mutate(root: &Path, directory: &Path, bits: &str, action: Action) -> Result<String, String> {
    validate_directory(directory)?;
    let _lock = peligames_mod_config::DirectoryLock::acquire(directory)?;
    match action {
        Action::Install {
            route,
            model,
            int4,
            renodx,
            lumenite,
            enabled,
            optimized_layers,
            shortcut,
        } => {
            if !matches!(route.as_str(), "optiscaler" | "reshade" | "vulkan" | "dx9") {
                return Err("Invalid installation route".into());
            }
            if peligames_mod_config::ownership::installed(directory, "optiscaler") {
                return Err(
                    "Remove the standalone OptiScaler before installing DLSSNR in this folder"
                        .into(),
                );
            }
            let component = runtime::component(root, bits)?;
            let script =
                fs::read_to_string(component.join("install.sh")).map_err(|e| e.to_string())?;
            if optimized_layers
                .is_some_and(|level| level > 3 || !script_runner::optimized_network(&script))
            {
                return Err("Unsupported optimized-network quality".into());
            }
            let model_path = neural_models::find(&component.join("dlssnr-amd"), ORIGINAL)
                .ok_or("Import or extract the NVIDIA model before installing in this game")?;
            let output = script_runner::execute_standalone_install(
                &component,
                &directory.to_string_lossy(),
                &route,
                None,
                Some(&model_path.to_string_lossy().into_owned()),
                &shortcut,
                InstallExtras {
                    model,
                    int4: Some(int4),
                    renodx: Some(renodx),
                    lumenite: Some(lumenite),
                },
            )?;
            // Settings edits acquire their own lock. Release the install lock first.
            drop(_lock);
            if let Some(level) = optimized_layers {
                settings::set_optimized_layers(directory, level)?;
            }
            settings::set_enabled(directory, enabled)?;
            Ok(output)
        }
        Action::Remove => {
            if !detection::details_in_directory(directory).dlssnr_installed {
                return Ok(String::new());
            }
            let component = runtime::component(root, bits)?;
            script_runner::execute_standalone_install(
                &component,
                &directory.to_string_lossy(),
                "remove",
                None,
                None,
                "Insert",
                Default::default(),
            )
        }
        Action::Startup { enabled, renodx } => {
            drop(_lock);
            settings::set_options(directory, enabled, renodx)?;
            Ok(String::new())
        }
        Action::Optimized { level } => {
            drop(_lock);
            settings::set_optimized_layers(directory, level)?;
            Ok(String::new())
        }
        Action::Shortcut { key, route } => {
            script_runner::inject_shortcut_key_pub(&directory.to_string_lossy(), &key, &route)?;
            Ok(String::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("peligames-mochi-game-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(root.join("backend/rdna4/64/model-tools")).unwrap();
            fs::create_dir_all(root.join("backend/rdna4/64/dlssnr-amd")).unwrap();
            fs::create_dir(root.join("game")).unwrap();
            fs::write(
                root.join("backend/rdna4/64/model-tools/extract_model.sh"),
                "fixture",
            )
            .unwrap();
            fs::write(root.join("backend/rdna4/64/install.sh"),"#!/bin/bash\nif [ \"$2\" != remove ]; then exit 19; fi\nprintf '%s' \"$2\" > \"$1/action.txt\"\nrm -f \"$1/dlssnr-amd.ini\" \"$1/dlssnr_amd.addon64\"\nprintf removed\n").unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn remove_uses_only_the_module_installer_and_preserves_other_game_files() {
        let f = Fixture::new();
        let game = f.0.join("game");
        fs::write(game.join("dxgi.dll"), "proxy").unwrap();
        fs::write(game.join("dlssnr_amd.addon64"), "addon").unwrap();
        fs::write(game.join("dlssnr-amd.ini"), "[DlssNr]\nEnabled=true").unwrap();
        fs::write(game.join("save.dat"), "save").unwrap();
        assert!(
            inspect(&f.0.join("backend"), &game, "64")
                .unwrap()
                .installation
                .dlssnr_installed
        );
        let result = mutate(&f.0.join("backend"), &game, "64", Action::Remove).unwrap();
        assert!(result.contains("removed"));
        assert_eq!(
            fs::read_to_string(game.join("action.txt")).unwrap(),
            "remove"
        );
        assert_eq!(fs::read_to_string(game.join("save.dat")).unwrap(), "save");
        assert!(
            !inspect(&f.0.join("backend"), &game, "64")
                .unwrap()
                .installation
                .dlssnr_installed
        );
    }
    #[test]
    fn missing_models_and_foreign_optiscaler_fail_before_starting_the_script() {
        let f = Fixture::new();
        let game = f.0.join("game");
        let install = || Action::Install {
            route: "optiscaler".into(),
            model: Some(NeuralModel::Original),
            int4: false,
            renodx: false,
            lumenite: false,
            enabled: true,
            optimized_layers: None,
            shortcut: "Insert".into(),
        };
        assert!(mutate(&f.0.join("backend"), &game, "64", install())
            .unwrap_err()
            .contains("model"));
        assert!(!game.join("action.txt").exists());
        fs::write(game.join("peligames-optiscaler-install.json"), "{}").unwrap();
        assert!(mutate(&f.0.join("backend"), &game, "64", install())
            .unwrap_err()
            .contains("standalone OptiScaler"));
    }
}
