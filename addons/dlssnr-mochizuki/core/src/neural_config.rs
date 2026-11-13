//! Sparse edits of the upstream NR configuration. Unknown settings and comments remain intact.
pub use peligames_mod_config::ini::{get, put, sections, Sections};
pub fn supported(text: &str) -> bool {
    let s = sections(text);
    s.contains_key("inspect") && s.contains_key("hudfix")
}
pub use peligames_mod_config::ini::flag;
/// Upstream's four optimized-network levels. Missing keys identify older installs.
pub fn optimized_layers(text: &str) -> Option<u8> {
    get(&sections(text), "Network", "OptimizedLayers")?
        .parse::<u8>()
        .ok()
        .filter(|value| *value <= 3)
}

pub fn with_optimized_layers(text: &str, level: u8) -> Result<String, String> {
    if level > 3 {
        return Err("Invalid optimized-network quality level".into());
    }
    if optimized_layers(text).is_none() {
        return Err("Update and repair the mod to configure optimized layers".into());
    }
    Ok(put(
        text,
        "Network",
        "OptimizedLayers",
        Some(&level.to_string()),
    ))
}

fn layer_number(section: &str) -> Option<u32> {
    let name = section.to_ascii_lowercase();
    let n = name.strip_prefix("layer")?;
    if n.is_empty() || !n.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    n.parse().ok().filter(|n| (1..=9999).contains(n))
}
pub fn layers(s: &Sections) -> Vec<String> {
    let mut names: Vec<_> = s
        .keys()
        .filter(|name| layer_number(name).is_some())
        .cloned()
        .collect();
    names.sort_by_key(|name| layer_number(name));
    names
}
/// Make the implicit first layer explicit without losing its advanced processing settings.
pub fn materialize(text: &str, plugin: bool) -> String {
    let s = sections(text);
    if !layers(&s).is_empty() {
        return text.into();
    }
    let renodx = flag(get(&s, "RenoDX", "Enabled"), plugin);
    let source = if renodx {
        "renodx"
    } else if plugin {
        "wilsjo2"
    } else {
        "dlssnr"
    };
    let mut result = text.to_string();
    if let Some(values) = s.get(source) {
        for (key, value) in values {
            let global = matches!(
                key.as_str(),
                "enabled"
                    | "neuraluplift"
                    | "nrtogglekey"
                    | "nrhookpoint"
                    | "nroutputrect"
                    | "runbeforesr"
            ) || key.starts_with("nrscreenshot");
            if !global {
                result = put(&result, "Layer1", key, Some(value));
            }
        }
    }
    let placement = if !plugin {
        "present"
    } else if renodx {
        match get(&s, "RenoDX", "NRHookPoint") {
            Some("0") => "after",
            Some("1") => "before",
            _ => "present",
        }
    } else {
        match get(&s, "wilsjo2", "RunBeforeSR") {
            Some("true" | "1") => "before",
            Some("false" | "0") => "after",
            _ => "present",
        }
    };
    result = put(
        &result,
        "Layer1",
        "Processing",
        Some(if renodx { "renodx" } else { "wilsjo2" }),
    );
    result = put(&result, "Layer1", "Placement", Some(placement));
    put(&result, "Layer1", "Enabled", Some("1"))
}
/// Startup options of 0.0.8 live in the NR plugin's chain, not in OptiScaler.ini.
pub fn startup(text: &str, plugin: bool, enabled: bool, renodx: Option<bool>) -> String {
    let mut result = materialize(text, plugin);
    let first = layers(&sections(&result)).into_iter().next().unwrap();
    result = put(
        &result,
        "RenoDX",
        "NeuralUplift",
        Some(if enabled { "1" } else { "0" }),
    );
    if let Some(style) = renodx {
        result = put(
            &result,
            &first,
            "Processing",
            Some(if style { "renodx" } else { "wilsjo2" }),
        );
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    const INI: &str = "; keep\r\n[Network]\r\nModel=CC_SMALL_FP8\r\n[Inspect]\r\nCompare=0\r\n[HudFix]\r\nMode=1\r\n[RenoDX]\r\nEnabled=1\r\nNRIntensity=0.7\r\nNRHookPoint=2\r\nCustom=value\r\n";
    #[test]
    fn optimized_quality_edits_only_the_network_key_and_rejects_unsupported_values() {
        let original = format!("{INI}\r\n[Layer4]\r\nOptimizedLayers=2\r\n");
        assert_eq!(optimized_layers(&original), None);
        assert!(with_optimized_layers(&original, 1).is_err());
        let original = original.replace(
            "Model=CC_SMALL_FP8",
            "Model=CC_SILVER_AARDWOLD_OPTIMIZED\r\nOptimizedLayers = 3 ; default",
        );
        for level in 0..=3 {
            let changed = with_optimized_layers(&original, level).unwrap();
            assert_eq!(optimized_layers(&changed), Some(level));
            let values = sections(&changed);
            assert_eq!(
                get(&values, "Network", "Model"),
                Some("CC_SILVER_AARDWOLD_OPTIMIZED")
            );
            assert_eq!(get(&values, "Layer4", "OptimizedLayers"), Some("2"));
            assert_eq!(get(&values, "RenoDX", "NRIntensity"), Some("0.7"));
            assert!(changed.contains("; keep\r\n"));
        }
        assert!(with_optimized_layers(&original, 4).is_err());
        assert_eq!(optimized_layers("[Network]\nOptimizedLayers=999\n"), None);
        assert_eq!(optimized_layers("[Network]\nOptimizedLayers=-1\n"), None);
    }

    #[test]
    fn startup_preserves_processing_comments_and_network() {
        let text = startup(INI, true, false, Some(false));
        let parsed = sections(&text);
        assert_eq!(get(&parsed, "Layer1", "Custom"), Some("value"));
        assert_eq!(get(&parsed, "Layer1", "NRIntensity"), Some("0.7"));
        assert_eq!(get(&parsed, "Layer1", "Placement"), Some("present"));
        assert_eq!(get(&parsed, "Layer1", "Enabled"), Some("1"));
        assert_eq!(get(&parsed, "Layer1", "Processing"), Some("wilsjo2"));
        assert_eq!(get(&parsed, "RenoDX", "NeuralUplift"), Some("0"));
        assert_eq!(get(&parsed, "Network", "Model"), Some("CC_SMALL_FP8"));
        assert!(text.contains("; keep\r\n"));
    }
    #[test]
    fn startup_updates_the_first_existing_layer_without_renumbering_or_adding_layers() {
        let text = format!("{INI}[Layer2]\r\nEnabled=1\r\nProcessing=wilsjo2\r\nCustom=first\r\n[Layer7]\r\nEnabled=0\r\nProcessing=wilsjo2\r\nCustom=other\r\n");
        let changed = startup(&text, true, true, Some(true));
        let parsed = sections(&changed);
        assert_eq!(layers(&parsed), vec!["layer2", "layer7"]);
        assert_eq!(get(&parsed, "Layer2", "Processing"), Some("renodx"));
        assert_eq!(get(&parsed, "Layer7", "Processing"), Some("wilsjo2"));
        assert_eq!(get(&parsed, "Layer7", "Enabled"), Some("0"));
        assert_eq!(get(&parsed, "Layer7", "Custom"), Some("other"));
    }
}
