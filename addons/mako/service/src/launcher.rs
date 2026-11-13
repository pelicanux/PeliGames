use peligames_addon_kit::protocol::LaunchContext;
use peligames_mako::{self as mako};
use std::{ffi::OsString, os::unix::process::CommandExt, path::PathBuf, process::Command};
pub fn command(context: LaunchContext, original: Vec<OsString>) -> Result<Command, String> {
    if original.first().is_none_or(|value| value.is_empty()) {
        return Err("Missing game command".into());
    }
    if context.settings["enabled"] != true {
        let mut c = Command::new(&original[0]);
        c.args(&original[1..]);
        return Ok(c);
    }
    let root = context.component_directory;
    let profile = context.settings["profile"]
        .as_str()
        .or_else(|| context.settings["legacy_profile"].as_str())
        .map(PathBuf::from)
        .ok_or("Missing MAKO profile")?;
    let mut settings = mako::profiles::read(&root, &profile)?;
    if !settings.enabled {
        let mut c = Command::new(&original[0]);
        c.args(&original[1..]);
        return Ok(c);
    }
    mako::settings::check_settings(&settings)?;
    if let Some(directory) = context.settings["directory"].as_str() {
        mako::support::check_local_fg(std::path::Path::new(directory))?;
    }
    mako::support::constraints(&context.options)?;
    for key in [
        "PROTON_MLFG_UPGRADE",
        "LOW_LATENCY_LAYER",
        "OBS_VKCAPTURE",
        "PROTON_VKD3D_LOWLATENCY",
    ] {
        if std::env::var(key).is_ok_and(|v| v == "1") {
            return Err("Conflicting frame-generation environment".into());
        }
    }
    if !std::path::Path::new(&settings.dll).is_absolute()
        || !mako::dll_valid(std::path::Path::new(&settings.dll))
    {
        return Err("Lossless.dll is unavailable; select it again in MAKO".into());
    }
    settings.overlay_enabled = false;
    let version = if settings.runtime_version.is_empty() {
        mako::release("mako")?.0
    } else {
        &settings.runtime_version
    };
    if !mako::versions::compatible("mako", version) {
        return Err("Unsupported MAKO runtime version".into());
    }
    let runtime = mako::versions::location(&root, "mako", version)?;
    if !settings.flatpak {
        mako::validate_runtime_at(&runtime, "mako")?;
    }
    let config = profile.with_extension("toml");
    let launcher = profile.with_extension("launch.conf");
    if context.monitoring {
        mako::mangohud::prepare(&config)?;
    }
    let script = mako::profiles::wrapper_script(&settings, &config, &runtime, &launcher);
    mako::write(&profile, script.as_bytes(), 0o700)?;
    let mut command = Command::new(&profile);
    command.args(original);
    if context.monitoring {
        command.env(mako::mangohud::ENABLE_ENV, "1");
    } else {
        command.env_remove(mako::mangohud::ENABLE_ENV);
    }
    Ok(command)
}
pub fn run(mut args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let text = args
        .next()
        .and_then(|v| v.into_string().ok())
        .filter(|v| v.len() <= 65536)
        .ok_or("Invalid launch context")?;
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        return Err("Missing launch separator".into());
    }
    let mut command = command(
        serde_json::from_str(&text).map_err(|e| e.to_string())?,
        args.collect(),
    )?;
    Err(format!("Could not start MAKO: {}", command.exec()))
}
