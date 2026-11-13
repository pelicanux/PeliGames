//! Per-game OptiScaler configuration, independent of neural model weights.
pub mod channel;
pub mod runtime;
mod detection;
mod installation;
mod latency;
pub mod patcher;
pub mod settings;
pub mod software;
mod upgrade;
pub use detection::{nr_route, NrRoute};
pub use installation::{install, managed, remove};
use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
pub use upgrade::upgrade;
static WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[derive(Serialize)]
pub struct State {
    pub software: Vec<software::Component>,
    pub patcher: patcher::State,
    pub installed: bool,
    pub managed: bool,
    pub nr_route: Option<NrRoute>,
    pub component_ready: bool,
    pub framegen_available: bool,
    pub framegen_modern: bool,
    pub proxy: Option<String>,
    pub settings: settings::Settings,
}
pub(crate) fn read_file(path: &Path) -> Result<String, String> {
    match fs::File::open(path) {
        Ok(file) => {
            if file.metadata().map_err(|e| e.to_string())?.len() > 1024 * 1024 {
                return Err("Configuration file is too large".into());
            }
            let mut text = String::new();
            file.take(1024 * 1024)
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            Ok(text)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}
pub(crate) fn config_path(game: &Path) -> Result<PathBuf, String> {
    let path = ["OptiScaler.ini", "optiscaler.ini"]
        .into_iter()
        .map(|name| game.join(name))
        .find(|p| p.is_file())
        .unwrap_or_else(|| game.join("OptiScaler.ini"));
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Redirected configuration files are not supported".into());
    }
    Ok(path)
}
pub(crate) fn game_directory(game: &Path) -> Result<PathBuf, String> {
    if !game.is_absolute() || !game.is_dir() {
        return Err("Choose an absolute game directory".into());
    }
    let root = game.canonicalize().map_err(|e| e.to_string())?;
    if root.parent().is_none() {
        return Err("Invalid game directory".into());
    }
    Ok(root)
}
// Shared with the UI so installation, detection and removal accept the same DLLs.
static PROXY_DLLS: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
    serde_json::from_str(include_str!("../proxies.json"))
    .expect("bundled OptiScaler proxy catalogue")
});
pub(crate) fn supported_proxy(name: &str) -> bool {
    PROXY_DLLS.iter().any(|proxy| proxy == name)
}
pub fn state(game: &Path, component: &Path) -> Result<State, String> {
    let game = game_directory(game)?;
    let path = config_path(&game)?;
    let text = read_file(&path)?;
    let proxy = installation::managed_proxy(&game)
        .filter(|name| game.join(name).is_file())
        .or_else(|| {
            PROXY_DLLS
                .iter()
                .map(String::as_str)
                .chain(["dinput8.dll", "OptiScaler.dll", "OptiScaler.asi"])
                .find(|name| game.join(name).is_file())
                .map(str::to_string)
        });
    let component_ini = if text.is_empty() {
        read_file(&component.join("OptiScaler.ini"))?
    } else {
        text.clone()
    };
    let mut values = settings::read(&text);
    if let Some(supported) = installation::supported_settings(&game) {
        // Old options may remain after a channel switch. They are not capabilities
        // of the newly installed binary just because they remain in its INI.
        values.retain(|key, _| !key.starts_with("FSR.Fsr4") || supported.contains(key));
    }
    let latency_path = installation::safe_target(&game, "fakenvapi.ini")?;
    if latency_path.is_file() {
        latency::read(&read_file(&latency_path)?, &mut values);
    }
    Ok(State {
        software: software::inventory(
            &game,
            proxy.as_deref(),
            installation::package_version(&game),
        ),
        patcher: patcher::state(&game)?,
        installed: !text.is_empty() && proxy.is_some(),
        managed: managed(&game),
        nr_route: nr_route(&game)?,
        component_ready: component.join("OptiScaler.dll").is_file()
            && component.join("OptiScaler.ini").is_file(),
        proxy,
        framegen_modern: peligames_mod_config::ini::get(
            &peligames_mod_config::ini::sections(&component_ini),
            "FrameGen",
            "FGNvngxReplacement",
        )
        .is_some(),
        framegen_available: settings::framegen_supported(&component_ini),
        settings: values,
    })
}
pub fn update(game: &Path, key: &str, value: &str) -> Result<settings::Settings, String> {
    let _lock = WRITE.lock().map_err(|e| e.to_string())?;
    let game = game_directory(game)?;
    let _directory = peligames_mod_config::DirectoryLock::acquire(&game)?;
    let path = config_path(&game)?;
    if !path.is_file() {
        return Err("Install OptiScaler before changing its settings".into());
    }
    let latency_path = installation::safe_target(&game, "fakenvapi.ini")?;
    if key.starts_with("fakenvapi.") && latency_path.is_file() {
        let after = latency::patch(&read_file(&latency_path)?, key, value)?;
        peligames_mod_config::write_atomic(&latency_path, &after)?;
        let mut values = settings::read(&read_file(&path)?);
        latency::read(&after, &mut values);
        return Ok(values);
    }
    let before = read_file(&path)?;
    if key.starts_with("FSR.Fsr4")
        && installation::supported_settings(&game)
            .is_some_and(|supported| !supported.iter().any(|item| item == key))
    {
        return Err("This FSR setting is not supported by the installed OptiScaler package".into());
    }
    let after = settings::patch(&before, key, value)?;
    peligames_mod_config::write_atomic(&path, &after)?;
    Ok(settings::read(&after))
}

#[cfg(test)]
mod tests;
