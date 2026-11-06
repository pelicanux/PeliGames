//! Only locale preferences cross IPC; other environment values stay private.
fn locale_tag(value: &str) -> Option<String> {
    let clean = value.trim().split(['.', '@']).next()?.replace('_', "-");
    if clean.eq_ignore_ascii_case("c") || clean.eq_ignore_ascii_case("posix") { return Some("en".into()); }
    if clean.is_empty() || clean.len() > 80 || !clean.split('-').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric())) { return None; }
    Some(clean)
}

pub fn preferences(mut get: impl FnMut(&str) -> Option<String>) -> Vec<String> {
    let effective = ["LC_ALL", "LC_MESSAGES", "LANG"].into_iter()
        .filter_map(|name| get(name)).find(|value| !value.trim().is_empty());
    if effective.as_deref().is_some_and(|value| matches!(value.split('.').next(), Some("C" | "POSIX"))) { return vec!["en".into()]; }
    let mut result = Vec::new();
    // LANGUAGE is the ordered list of UI languages used by gettext on Linux.
    for value in get("LANGUAGE").into_iter().flat_map(|value| value.split(':').map(str::to_owned).collect::<Vec<_>>()).chain(effective) {
        if let Some(tag) = locale_tag(&value) { if !result.contains(&tag) { result.push(tag); } }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn locales(values: &[(&str, &str)]) -> Vec<String> { preferences(|name| values.iter().find(|(key, _)| *key == name).map(|(_, value)| value.to_string())) }
    #[test] fn message_locale_precedes_numeric_locale() {
        assert_eq!(locales(&[("LANG", "en_US.UTF-8"), ("LC_MESSAGES", "pt_BR.UTF-8")]), ["pt-BR"]);
    }
    #[test] fn gettext_preferences_are_ordered_and_deduplicated() {
        assert_eq!(locales(&[("LANG", "pt_BR.UTF-8"), ("LANGUAGE", "es_MX:pt_BR:es_MX")]), ["es-MX", "pt-BR"]);
    }
    #[test] fn explicit_c_locale_ignores_gettext_language() {
        assert_eq!(locales(&[("LC_ALL", "C.UTF-8"), ("LANGUAGE", "pt_BR")]), ["en"]);
    }
    #[test] fn invalid_values_and_missing_environment_are_safe() {
        assert!(locales(&[]).is_empty());
        assert_eq!(locales(&[("LANGUAGE", "/home/private:pt_BR")]), ["pt-BR"]);
    }
}
