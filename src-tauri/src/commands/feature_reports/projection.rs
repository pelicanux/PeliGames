//! Allowlisted, typed diagnostic settings; no arbitrary configuration dumps.
use crate::core::report_privacy::select;
use serde_json::{json, Value};
pub(super) fn same_installation(game: &Value, path: &str, directory: &str) -> bool {
    if game["path"].as_str() == Some(path) && !path.is_empty() {
        return true;
    }
    let Some(saved) = game["directory"].as_str().filter(|s| !s.is_empty()) else {
        return false;
    };
    if directory.is_empty() {
        return false;
    }
    if saved == directory {
        return true;
    }
    match (
        std::path::Path::new(saved).canonicalize(),
        std::path::Path::new(directory).canonicalize(),
    ) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
pub(super) fn advanced(value: &Value) -> Value {
    let mut out = select(
        value,
        &[
            "schema_version",
            "overlay_enabled",
            "background_fps",
            "mangohud_enabled",
            "mangohud_fps_limit",
            "launch_backend",
        ],
    );
    out["afmf"] = select(
        &value["afmf"],
        &[
            "enabled",
            "performance_mode",
            "search_mode",
            "fast_motion",
            "min_fps",
            "hud_detect",
            "pacing",
        ],
    );
    out["gamescope"] = select(
        &value["gamescope"],
        &[
            "enable_upscaling",
            "enable_limiter",
            "force_grab_cursor",
            "hdr",
            "adaptive_sync",
            "width",
            "height",
            "output_width",
            "output_height",
            "fps",
            "fps_unfocused",
            "filter",
            "window_type",
        ],
    );
    let catalog: Value =
        serde_json::from_str(include_str!("../../../../src/config/advancedOptions.json"))
            .unwrap_or_default();
    let mut flags = serde_json::Map::new();
    let mut values = serde_json::Map::new();
    for option in catalog.as_array().into_iter().flatten() {
        let Some(id) = option["id"].as_str() else {
            continue;
        };
        if let Some(flag) = value["options"][id].as_bool() {
            flags.insert(id.into(), flag.into());
        }
        if let Some(text) = value["option_values"][id].as_str() {
            let known = option["choices"]
                .as_array()
                .is_some_and(|choices| choices.iter().any(|c| c["value"].as_str() == Some(text)));
            if known || (option.get("min").is_some() && text.parse::<u32>().is_ok()) {
                values.insert(id.into(), text.into());
            }
        }
    }
    out["options"] = flags.into();
    out["option_values"] = values.into();
    out["custom_arguments_present"] = json!(value["launch_options"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    out["custom_wrappers"] = json!(value["wrappers"].as_array().map_or(0, |a| a.len()));
    out
}
pub(super) fn neural_settings_snapshot(text: &str) -> Value {
    let schema: Value =
        serde_json::from_str(include_str!("../../../resources/neural-controls.json"))
            .unwrap_or_default();
    let mut result = serde_json::Map::new();
    for (group, fields) in crate::core::neural_config::sections(text) {
        let layer = group
            .strip_prefix("layer")
            .and_then(|s| s.parse::<u16>().ok())
            .is_some_and(|n| (1..=9999).contains(&n));
        let specs: Vec<_> = schema
            .as_array()
            .into_iter()
            .flatten()
            .filter(|g| {
                layer
                    || g["id"]
                        .as_str()
                        .is_some_and(|s| s.eq_ignore_ascii_case(&group))
            })
            .flat_map(|g| g["fields"].as_array().into_iter().flatten())
            .collect();
        let mut selected = serde_json::Map::new();
        for (key, value) in fields {
            let numeric = value.parse::<f64>().is_ok_and(|n| n.is_finite());
            let boolean = ["true", "false", "on", "off", "auto"]
                .contains(&value.to_ascii_lowercase().as_str());
            let spec = specs.iter().find(|f| {
                f["key"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(&key))
            });
            let enumeration = spec.is_some_and(|f| {
                f["options"].as_array().is_some_and(|opts| {
                    opts.iter().any(|o| {
                        o[0].as_str()
                            .is_some_and(|s| s.eq_ignore_ascii_case(&value))
                    })
                })
            });
            let layer_field = layer
                && match key.as_str() {
                    "enabled" => numeric || boolean,
                    "processing" => {
                        ["renodx", "wilsjo2"].contains(&value.to_ascii_lowercase().as_str())
                    }
                    "placement" => ["present", "before", "after"]
                        .contains(&value.to_ascii_lowercase().as_str()),
                    "network" => [
                        "CC_SMALL_FP8",
                        "CC_SMALL_FP8_EXPERIMENTAL",
                        "CC_SILVER_AARDWOLD",
                        "CC_SILVER_AARDWOLD_INT4_MIXED",
                    ]
                    .contains(&value.as_str()),
                    _ => false,
                };
            let legacy = matches!(group.as_str(), "dlssnr" | "renodx" | "wilsjo2")
                && matches!(
                    key.as_str(),
                    "enabled"
                        | "neuraluplift"
                        | "intensity"
                        | "nrtogglekey"
                        | "nrhookpoint"
                        | "runbeforesr"
                )
                && (numeric || boolean);
            if (spec.is_some() && (numeric || boolean || enumeration)) || layer_field || legacy {
                selected.insert(key, json!(value));
            }
        }
        if !selected.is_empty() {
            result.insert(group, selected.into());
        }
    }
    result.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_nexus_ids_cannot_collect_another_installations_logs() {
        let game = json!({"path":"/fixture/Diablo/game.exe","directory":"/fixture/Diablo"});
        assert!(!same_installation(
            &game,
            "/fixture/Tomb/game.exe",
            "/fixture/Tomb"
        ));
        assert!(!same_installation(&json!({}), "", ""));
        assert!(same_installation(
            &game,
            "/fixture/Diablo/game.exe",
            "/fixture/Diablo"
        ));
    }
    #[test]
    fn advanced_report_excludes_free_arguments_sources_and_unknown_options() {
        let value = json!({"overlay_enabled":true,"launch_options":"--password hidden", "wrappers":[{"program":"/home/Alice/wrapper", "arguments":"token=hidden"}],"gamescope":{"fps":60,"additional_options":"secret"},"options":{"unknown_secret":true,"wayland":true},"option_values":{"unknown":"hidden"}});
        let out = advanced(&value).to_string();
        assert!(out.contains("overlay_enabled"));
        assert!(out.contains("60"));
        for value in ["Alice", "hidden", "secret", "additional_options", "program"] {
            assert!(!out.contains(value), "{out}");
        }
    }
    #[test]
    fn neural_snapshot_keeps_layers_and_controls_but_excludes_unknown_fields() {
        let output=neural_settings_snapshot("[Layer1]\nEnabled=1\nProcessing=renodx\nPlacement=present\nNetwork=CC_SMALL_FP8\nUsername=Alice\n[RenoDX]\nNeuralUplift=1\nSecret=hidden\n[AccountAlice]\nEnabled=1").to_string();
        assert_eq!(
            neural_settings_snapshot("[DlssNr]\nEnabled=true\nIntensity=0.6")["dlssnr"]["enabled"],
            "true"
        );
        assert!(output.contains("CC_SMALL_FP8"));
        assert!(output.contains("neuraluplift"));
        for secret in ["Alice", "alice", "hidden", "username", "secret"] {
            assert!(!output.contains(secret), "{output}");
        }
    }
}
