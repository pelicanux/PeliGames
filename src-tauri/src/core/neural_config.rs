//! Sparse edits of the upstream NR configuration. Unknown settings and comments remain intact.
use std::collections::BTreeMap;
pub type Sections = BTreeMap<String, BTreeMap<String, String>>;
pub fn sections(text: &str) -> Sections {
    let mut result = Sections::new();
    let mut section = String::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.split([';', '#']).next().unwrap_or("").trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
            result.entry(section.clone()).or_default();
        } else if !section.is_empty() {
            if let Some((key, value)) = line.split_once('=') {
                result
                    .entry(section.clone())
                    .or_default()
                    .insert(key.trim().to_ascii_lowercase(), value.trim().into());
            }
        }
    }
    result
}
pub fn supported(text: &str) -> bool {
    let s = sections(text);
    s.contains_key("inspect") && s.contains_key("hudfix")
}
pub fn get<'a>(sections: &'a Sections, section: &str, key: &str) -> Option<&'a str> {
    sections
        .get(&section.to_ascii_lowercase())?
        .get(&key.to_ascii_lowercase())
        .map(String::as_str)
}
pub fn flag(value: Option<&str>, default: bool) -> bool {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("1" | "true" | "on" | "yes") => true,
        Some("0" | "false" | "off" | "no") => false,
        _ => default,
    }
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
/// Replace only the chosen key, collapsing duplicates as upstream's last-value-wins parser does.
pub fn put(text: &str, section: &str, key: &str, value: Option<&str>) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines = Vec::new();
    let mut inside = false;
    let mut found = false;
    let mut written = false;
    let setting = value.map(|v| format!("{key} = {v}"));
    for line in text.lines() {
        let clean = line
            .trim_start_matches('\u{feff}')
            .split([';', '#'])
            .next()
            .unwrap_or("")
            .trim();
        if clean.starts_with('[') && clean.ends_with(']') {
            if inside && !written {
                if let Some(v) = &setting {
                    lines.push(v.clone());
                }
                written = true;
            }
            inside = clean[1..clean.len() - 1]
                .trim()
                .eq_ignore_ascii_case(section);
            found |= inside;
        }
        if inside
            && clean
                .split_once('=')
                .is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key))
        {
            if !written {
                if let Some(v) = &setting {
                    lines.push(v.clone());
                }
                written = true;
            }
        } else {
            lines.push(line.into());
        }
    }
    if !written {
        if let Some(v) = setting {
            if !found {
                lines.push(format!("[{section}]"));
            }
            lines.push(v);
        }
    }
    lines.join(newline) + newline
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
