use peligames_addon_kit::protocol::LaunchContext;
use peligames_afmf::{management, Settings};
use std::{ffi::OsString, os::unix::process::CommandExt, process::Command};

pub fn command(context: LaunchContext, original: Vec<OsString>) -> Result<Command, String> {
    if original.first().is_none_or(|word| word.is_empty()) {
        return Err("Missing game command".into());
    }
    if !context.component_directory.is_absolute() {
        return Err("Invalid component directory".into());
    }
    let mut settings: Settings =
        serde_json::from_value(context.settings).map_err(|error| error.to_string())?;
    settings.validate()?;
    if settings.enabled && !management::state(&context.component_directory)?.ready {
        eprintln!("AFMF: runtime unavailable; starting the game without frame generation");
        settings.enabled = false;
    }
    if settings.enabled
        && context
            .options
            .iter()
            .any(|option| ["mlfg", "optiscaler"].contains(&option.as_str()))
    {
        return Err("Disable the conflicting advanced frame generator first".into());
    }
    let mut environment = std::collections::BTreeMap::new();
    for key in super::PRESENTATION_INPUTS {
        if let Some(value) = std::env::var_os(key) {
            environment.insert(
                key.to_string(),
                Some(
                    value
                        .into_string()
                        .map_err(|_| "Invalid presentation environment")?,
                ),
            );
        }
    }
    let monitoring = if settings.enabled && context.monitoring {
        let path = context.component_directory.join("layers/afmf-mangohud");
        peligames_mangohud_layers::stage(&peligames_mangohud_layers::host_directories(), &path)?;
        if !path.join("MangoHud.x86_64.json").is_file() {
            return Err("The 64-bit MangoHUD layer is unavailable".into());
        }
        Some(path)
    } else {
        None
    };
    let version = management::current(&context.component_directory);
    let patch = super::prepare(
        context.component_directory,
        super::Launch {
            settings,
            runtime_version: version,
            gamescope: context.gamescope,
            environment,
            mangohud_directory: monitoring,
        },
    )?;
    let mut command = Command::new(&original[0]);
    command.args(&original[1..]);
    let patch: std::collections::BTreeMap<String, Option<String>> =
        serde_json::from_value(patch).map_err(|e| e.to_string())?;
    for (key, value) in patch {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    Ok(command)
}
pub fn run(mut args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let text = args
        .next()
        .and_then(|value| value.into_string().ok())
        .filter(|text| text.len() <= 64 * 1024)
        .ok_or("Invalid launch context")?;
    let context: LaunchContext = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        return Err("Missing launch separator".into());
    }
    let mut command = command(context, args.collect())?;
    Err(format!("Could not start game: {}", command.exec()))
}
