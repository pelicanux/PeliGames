use serde::Serialize;
use std::path::Path;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NrRoute {
    Optiscaler,
    Reshade,
    Unknown,
}
/// Detect installed files, not runtime activity. A generic proxy DLL proves neither NR nor OptiScaler.
pub fn nr_route(game: &Path) -> Result<Option<NrRoute>, String> {
    let has = |name: &str| game.join(name).is_file();
    if has("OptiScaler/plugins/dlssnr-amd.asi") {
        return Ok(Some(NrRoute::Optiscaler));
    }
    let manifest = super::read_file(&game.join("dlssnr-amd-install.txt"))?;
    let records: Vec<String> = manifest
        .lines()
        .map(|line| line.trim().replace('\\', "/").to_ascii_lowercase())
        .collect();
    let records = |name: &str| records.iter().any(|line| line == name);
    let legacy = has("dlssnr_core.dll");
    if records("optiscaler.ini") || legacy && (has("OptiScaler.ini") || has("optiscaler.ini")) {
        return Ok(Some(NrRoute::Optiscaler));
    }
    if has("dlssnr_amd.addon64")
        || has("dlssnr_amd.addon32")
        || records("reshade.ini")
        || records("dlssnr_amd.addon64")
        || records("dlssnr_amd.addon32")
    {
        return Ok(Some(NrRoute::Reshade));
    }
    if has("dlssnr-amd-install.txt") || legacy {
        return Ok(Some(NrRoute::Unknown));
    }
    Ok(None)
}
