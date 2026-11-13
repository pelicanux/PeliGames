use peligames_mod_config::ini::{get, put, sections};
use std::collections::BTreeMap;
pub type Settings = BTreeMap<String, String>;
// Only documented OptiScaler keys are editable. Unknown/custom values are preserved on unrelated edits.
pub const FIELDS: &[(&str, &str, &[&str])] = &[
    (
        "Upscalers",
        "Dx12Upscaler",
        &["auto", "ffx", "fsr31", "xess", "fsr21", "fsr22", "dlss"],
    ),
    (
        "Upscalers",
        "Dx11Upscaler",
        &[
            "auto", "fsr22", "fsr31", "xess", "xess_12", "ffx_12", "fsr31_12", "dlss",
        ],
    ),
    (
        "Upscalers",
        "VulkanUpscaler",
        &[
            "auto", "fsr21", "fsr22", "ffx", "fsr31", "xess", "ffx_12", "fsr31_12", "dlss",
        ],
    ),
    ("FSR", "UpscalerIndex", &["auto", "0", "1", "2"]),
    ("FSR", "Fsr4Update", &["auto", "true", "false"]),
    ("FSR", "Fsr4ForceEnableInt8", &["auto", "true", "false"]),
    ("FSR", "Fsr4GpuSupport", &["auto", "0", "1", "2", "3", "4"]),
    ("FSR", "Fsr4Model", &["auto", "0", "1", "2", "3"]),
    ("FrameGen", "Enabled", &["auto", "true", "false"]),
    (
        "FrameGen",
        "FGInput",
        &[
            "auto", "nofg", "dlssg", "nvngxfg", "nukems", "fsrfg", "upscaler", "fsrfg30", "xefg",
        ],
    ),
    (
        "FrameGen",
        "FGOutput",
        &["auto", "nofg", "fsrfg", "xefg", "dlssg", "nukems"],
    ),
    (
        "FrameGen",
        "FGNvngxReplacement",
        &["auto", "None", "Nukems", "Arturs", "FFX", "Combo"],
    ),
    (
        "Menu",
        "Scale",
        &["auto", "0.75", "1", "1.25", "1.5", "1.75", "2"],
    ),
    ("Menu", "ShortcutKey", &["auto"]),
    ("Spoofing", "Dxgi", &["auto", "true", "false"]),
    ("Inputs", "EnableDlssInputs", &["auto", "true", "false"]),
    ("fakenvapi", "ForceReflex", &["auto", "0", "1", "2"]),
    ("fakenvapi", "ForceLatencyFlex", &["auto", "true", "false"]),
    ("fakenvapi", "LatencyFlexMode", &["auto", "0", "1", "2"]),
];
pub fn framegen_supported(text: &str) -> bool {
    let parsed = sections(text);
    ["Enabled", "FGInput", "FGOutput"]
        .iter()
        .all(|key| get(&parsed, "FrameGen", key).is_some())
}
pub fn read(text: &str) -> Settings {
    let parsed = sections(text);
    FIELDS
        .iter()
        .filter(|(section, key, _)| {
            !(*section == "FSR" && key.starts_with("Fsr4")) || get(&parsed, section, key).is_some()
        })
        .map(|(section, key, _)| {
            (
                format!("{section}.{key}"),
                get(&parsed, section, key).unwrap_or("auto").to_string(),
            )
        })
        .collect()
}
pub fn patch(text: &str, key: &str, value: &str) -> Result<String, String> {
    let (section, name, values) = FIELDS
        .iter()
        .find(|(section, name, _)| format!("{section}.{name}") == key)
        .ok_or("Unknown OptiScaler setting")?;
    if *section == "FrameGen" && !framegen_supported(text) {
        return Err("This OptiScaler version does not expose these frame generation settings. Update the component first.".into());
    }
    let parsed = sections(text);
    if *section == "FSR" && name.starts_with("Fsr4") && get(&parsed, section, name).is_none() {
        return Err("This FSR setting is not supported by the installed OptiScaler INI. Update OptiScaler in the game first.".into());
    }
    if *section == "FrameGen"
        && *name == "FGNvngxReplacement"
        && get(&parsed, section, name).is_none()
    {
        return Err("This setting requires a newer OptiScaler version".into());
    }
    if *section == "FrameGen"
        && *name == "FGInput"
        && value == "nvngxfg"
        && get(&parsed, "FrameGen", "FGNvngxReplacement").is_none()
    {
        return Err("Use Nukem for this OptiScaler version".into());
    }
    if *section == "FrameGen"
        && *name == "FGOutput"
        && value == "dlssg"
        && get(&parsed, "FrameGen", "FGNvngxReplacement").is_none()
    {
        return Err("Use Nukem for this OptiScaler version".into());
    }
    if *section == "FrameGen"
        && value == "nukems"
        && get(&parsed, "FrameGen", "FGNvngxReplacement").is_some()
    {
        return Err("Use the NGX replacement setting for this OptiScaler version".into());
    }
    let shortcut = *section == "Menu" && *name == "ShortcutKey";
    let valid = if shortcut {
        value == "auto" || value == "-1" || {
            let key = value
                .strip_prefix("0x")
                .or_else(|| value.strip_prefix("0X"))
                .filter(|digits| {
                    !digits.is_empty()
                        && digits.len() <= 2
                        && digits.bytes().all(|b| b.is_ascii_hexdigit())
                })
                .and_then(|digits| u16::from_str_radix(digits, 16).ok())
                .or_else(|| {
                    (!value.is_empty()
                        && value.len() <= 3
                        && value.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| value.parse::<u16>().ok())
                    .flatten()
                });
            key.is_some_and(|key| (1..=254).contains(&key))
        }
    } else {
        values.contains(&value)
    };
    if !valid {
        return Err("Invalid OptiScaler setting value".into());
    }
    Ok(put(text, section, name, Some(value)))
}
pub fn apply(text: &str, settings: &Settings) -> Result<String, String> {
    settings
        .iter()
        .try_fold(text.to_string(), |text, (key, value)| {
            patch(&text, key, value)
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fsr_compatibility_follows_the_installed_schema_and_preserves_other_settings() {
        let stable = "[FSR]\nUpscalerIndex=auto\nFsr4Update=auto\nFsr4ForceEnableInt8=auto\n[Plugins]\nLoadAsiPlugins=true\n";
        let nightly = "[FSR]\nUpscalerIndex=auto\nFsr4GpuSupport=auto\nFsr4Model=auto\n";
        assert!(!read(stable).contains_key("FSR.Fsr4GpuSupport"));
        assert!(!read(nightly).contains_key("FSR.Fsr4ForceEnableInt8"));
        assert!(!read("").contains_key("FSR.Fsr4Model"));
        assert!(patch(stable, "FSR.Fsr4GpuSupport", "2").is_err());
        assert!(patch(nightly, "FSR.Fsr4ForceEnableInt8", "true").is_err());
        let forced = patch(stable, "FSR.Fsr4ForceEnableInt8", "true").unwrap();
        assert_eq!(read(&forced)["FSR.Fsr4ForceEnableInt8"], "true");
        assert!(forced.contains("LoadAsiPlugins=true"));
        assert!(forced.contains("Fsr4Update=auto"));
        assert_eq!(
            read(&patch(&forced, "FSR.Fsr4ForceEnableInt8", "false").unwrap())
                ["FSR.Fsr4ForceEnableInt8"],
            "false"
        );
        for mode in ["auto", "0", "1", "2", "3", "4"] {
            assert_eq!(
                read(&patch(nightly, "FSR.Fsr4GpuSupport", mode).unwrap())["FSR.Fsr4GpuSupport"],
                mode
            );
        }
        assert!(patch(nightly, "FSR.Fsr4GpuSupport", "5").is_err());
        assert!(patch(nightly, "FSR.Fsr4Model", "4").is_err());
        assert_eq!(
            read(&patch(nightly, "FSR.UpscalerIndex", "0").unwrap())["FSR.UpscalerIndex"],
            "0"
        );
        assert_eq!(
            read(&patch(stable, "Upscalers.Dx12Upscaler", "fsr31").unwrap())
                ["Upscalers.Dx12Upscaler"],
            "fsr31"
        );
    }
    #[test]
    fn sparse_edits_preserve_plugins_fg_and_custom_values() {
        let source="; original\r\n[Upscalers]\r\nDx12Upscaler=custom-x\r\n[Menu]\r\nScale=auto\r\n[Plugins]\r\nLoadAsiPlugins=true\r\n[FrameGen]\r\nEnabled=false\r\n[DlssNr]\r\nEnabled=true\r\n";
        assert_eq!(read(source)["Upscalers.Dx12Upscaler"], "custom-x");
        let out = patch(source, "Menu.Scale", "1.25").unwrap();
        assert!(out.contains("Dx12Upscaler=custom-x"));
        assert!(out.contains("LoadAsiPlugins=true"));
        assert!(out.contains("Enabled=false"));
        assert!(out.contains("; original\r\n"));
        assert_eq!(read(&out)["Menu.Scale"], "1.25");
        assert!(patch(source, "FrameGen.Unsupported", "true").is_err());
        assert!(patch(source, "Menu.Scale", "1\nInjected=true").is_err());
    }
}
