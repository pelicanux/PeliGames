#[path = "../../../tools/storage_layout.rs"]
#[allow(dead_code)]
mod layout;
use peligames_addon_kit::{launch, protocol::LaunchContext};
use std::{
    ffi::{OsStr, OsString},
    os::unix::process::CommandExt,
    path::PathBuf,
    process::Command,
};

pub fn run(mut args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let id = args
        .next()
        .and_then(|v| v.into_string().ok())
        .ok_or("Missing launch integration")?;
    let root = PathBuf::from(args.next().ok_or("Missing launcher root")?);
    let host = args
        .next()
        .and_then(|v| v.into_string().ok())
        .ok_or("Missing launcher version")?;
    let text = args
        .next()
        .and_then(|v| v.into_string().ok())
        .filter(|v| v.len() <= 64 * 1024)
        .ok_or("Invalid launch context")?;
    let mut context: LaunchContext = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if args.next().as_deref() != Some(OsStr::new("--")) {
        return Err("Missing launch separator".into());
    }
    let original: Vec<_> = args.collect();
    if original.first().is_none_or(|v| v.is_empty()) {
        return Err("Missing game command".into());
    }
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    if !root.is_absolute() {
        return Err("An absolute launcher root is required".into());
    }
    let store = peligames_addon_kit::registry::Store {
        packages: root.join(layout::ADDON_PACKAGES),
        state_file: root.join(layout::ADDON_STATE),
    };
    let mut command = if let Some(dispatch) = launch::dispatch(&store, &id, &host, &platform)? {
        context.component_directory = dispatch
            .component_directory
            .map(|path| root.join(path))
            .unwrap_or_else(|| root.join(layout::ADDON_COMPONENTS).join(&id));
        let mut command = Command::new(dispatch.executable);
        command
            .arg("--peligames-launch")
            .arg(serde_json::to_string(&context).map_err(|e| e.to_string())?)
            .arg("--")
            .args(original);
        command
    } else {
        let mut command = Command::new(&original[0]);
        command.args(&original[1..]);
        command
    };
    Err(format!(
        "Could not start game integration: {}",
        command.exec()
    ))
}

/// Route version-zero managed wrapper commands through the package registry.
/// Existing exported PeliRun profiles keep working without executing a removed
/// integration's old script. Settings remain owned by the integration service.
pub fn migrate_legacy(
    words: Vec<OsString>,
    helper: &std::path::Path,
    monitoring: bool,
) -> Result<Vec<OsString>, String> {
    for (index, word) in words.iter().enumerate() {
        let path = std::path::Path::new(word);
        let Some(profiles) = path
            .parent()
            .filter(|p| p.file_name().is_some_and(|n| n == "profiles"))
        else {
            continue;
        };
        let Some(component) = profiles
            .parent()
            .filter(|p| p.file_name().is_some_and(|n| n == "FrameGeneration"))
        else {
            continue;
        };
        let Some(parent) = component.parent() else {
            continue;
        };
        let root = if parent.file_name().is_some_and(|n| n == "components") {
            parent.parent().ok_or("Invalid legacy profile root")?
        } else {
            parent
        };
        let Some((id, settings)) = launch::legacy_profile(root, path)? else {
            if launch::legacy_path(root, path) {
                if index + 1 == words.len() {
                    return Err("Missing game command after legacy wrapper".into());
                }
                let mut clean = words[..index].to_vec();
                clean.extend_from_slice(&words[index + 1..]);
                return Ok(clean);
            }
            continue;
        };
        if index + 1 == words.len() {
            return Err("Missing original command after the managed profile".into());
        }
        let context = LaunchContext {
            component_directory: root.to_path_buf(),
            settings,
            gamescope: false,
            monitoring,
            options: vec![],
        };
        let mut next = words[..index].to_vec();
        next.extend([
            helper.as_os_str().to_owned(),
            "--addon".into(),
            id.into(),
            root.as_os_str().to_owned(),
            host_version()?.into(),
            serde_json::to_string(&context)
                .map_err(|e| e.to_string())?
                .into(),
            "--".into(),
        ]);
        next.extend_from_slice(&words[index + 1..]);
        return Ok(next);
    }
    Ok(words)
}

fn host_version() -> Result<String, String> {
    let package: serde_json::Value =
        serde_json::from_str(include_str!("../../../package.json")).map_err(|e| e.to_string())?;
    package["version"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| "Missing launcher version".into())
}
