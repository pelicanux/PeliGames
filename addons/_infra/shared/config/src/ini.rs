//! Sparse edits that preserve unknown settings, comments and line endings.
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
pub fn get<'a>(sections: &'a Sections, section: &str, key: &str) -> Option<&'a str> {
    sections
        .get(&section.to_ascii_lowercase())?
        .get(&key.to_ascii_lowercase())
        .map(String::as_str)
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

/// Interpret a boolean INI value, preserving a caller-supplied default.
pub fn flag(value:Option<&str>,default:bool)->bool {
 match value.map(str::to_ascii_lowercase).as_deref(){Some("1"|"true"|"on"|"yes")=>true,Some("0"|"false"|"off"|"no")=>false,_=>default}
}
