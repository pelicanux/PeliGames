use crate::{profiles, versions, Settings};
use serde::Serialize;
use std::path::{Path, PathBuf};
#[derive(Serialize)]
pub struct State {
    pub settings: Settings,
    pub profile: Option<PathBuf>,
    pub installed: bool,
    pub directory: PathBuf,
    pub versions: Vec<versions::Installed>,
}
pub fn inspect(
    storage: &Path,
    key: &str,
    _label: &str,
    profile: Option<&Path>,
) -> Result<State, String> {
    peligames_addon_kit::files::check_directory(storage)?;
    let settings = match profile {
        Some(path) => profiles::read(storage, path)?,
        None => profiles::selected(storage, key)?.unwrap_or_default(),
    };
    let mut settings: Settings = settings;
    if settings.backend != "mako" {
        settings = Settings::default();
    }
    if settings.runtime_version.is_empty() {
        settings.runtime_version = versions::current(storage, "mako")?;
    }
    let directory = versions::location(storage, "mako", &settings.runtime_version)?;
    Ok(State {
        installed: crate::validate_runtime_at(&directory, "mako").is_ok(),
        settings,
        profile: profile.map(Path::to_path_buf),
        directory,
        versions: versions::installed(storage, "mako")?,
    })
}
