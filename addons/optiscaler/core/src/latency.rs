//! fakenvapi has its own INI and snake_case numeric keys in official packages.
use super::settings::Settings;
use peligames_mod_config::ini::{get, put, sections};
const FIELDS: &[(&str, &str)] = &[
    ("ForceReflex", "force_reflex"),
    ("ForceLatencyFlex", "force_latencyflex"),
    ("LatencyFlexMode", "latencyflex_mode"),
];
pub fn read(text: &str, settings: &mut Settings) {
    let parsed = sections(text);
    for (ui, key) in FIELDS {
        if let Some(value) = get(&parsed, "fakenvapi", key) {
            settings.insert(
                format!("fakenvapi.{ui}"),
                if *ui == "ForceLatencyFlex" {
                    match value {
                        "0" => "false",
                        "1" => "true",
                        _ => value,
                    }
                } else {
                    value
                }
                .into(),
            );
        }
    }
}
pub fn patch(text: &str, key: &str, value: &str) -> Result<String, String> {
    let (ui, native) = FIELDS
        .iter()
        .find(|(ui, _)| format!("fakenvapi.{ui}") == key)
        .ok_or("Invalid latency setting")?;
    let _ = super::settings::patch("", key, value)?; // use the shared validation whitelist
    if value == "auto" {
        return Ok(text.into());
    }
    let value = if *ui == "ForceLatencyFlex" {
        match value {
            "true" => "1",
            "false" => "0",
            _ => return Err("Invalid latency setting".into()),
        }
    } else {
        value
    };
    Ok(put(text, "fakenvapi", native, Some(value)))
}
