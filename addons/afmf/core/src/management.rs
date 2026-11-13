use crate::{
    releases::{self, Release},
    runtime,
};
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Serialize)]
pub struct Version {
    pub version: String,
    pub current: bool,
    pub directory: String,
    pub compatible: bool,
}
#[derive(Serialize)]
pub struct State {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub compatible: bool,
    pub ready: bool,
    pub directory: String,
    pub versions: Vec<Version>,
}
pub fn current(root: &Path) -> String {
    if peligames_addon_kit::files::check_directory(root).is_err() {
        return runtime::RELEASE.0.into();
    }
    fs::read(root.join("afmf/current.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<String>(&bytes).ok())
        .filter(|version| {
            releases::compatible(version)
                && runtime::location(root, version)
                    .is_ok_and(|path| runtime::validate(&path).is_ok())
        })
        .unwrap_or_else(|| runtime::RELEASE.0.into())
}
pub fn state(root: &Path) -> Result<State, String> {
    peligames_addon_kit::files::check_directory(root)?;
    let current = current(root);
    let latest = releases::latest(root);
    let mut names = std::collections::BTreeSet::new();
    if let Ok(entries) = fs::read_dir(root.join("afmf/versions")) {
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if releases::version(&name).is_some() {
                names.insert(name);
            }
        }
    }
    names.insert(runtime::RELEASE.0.into());
    let mut versions = Vec::new();
    for version in names {
        let path = runtime::location(root, &version)?;
        if runtime::validate(&path).is_ok() {
            versions.push(Version {
                compatible: releases::compatible(&version),
                current: version == current,
                directory: path.to_string_lossy().into(),
                version,
            });
        }
    }
    versions.sort_by_key(|v| std::cmp::Reverse(releases::version(&v.version)));
    let ready = versions.iter().any(|v| v.current);
    let directory = runtime::location(root, if ready { &current } else { &latest.version })?
        .to_string_lossy()
        .into();
    Ok(State {
        available: releases::version(&latest.version) > releases::version(&current),
        compatible: releases::compatible(&latest.version),
        latest: latest.version,
        current,
        ready,
        directory,
        versions,
    })
}
pub fn publish(root: &Path, release: &Release) -> Result<(), String> {
    if !releases::valid(release) || !releases::compatible(&release.version) {
        return Err("Unsupported AFMF release".into());
    }
    runtime::validate(&runtime::location(root, &release.version)?)?;
    peligames_mod_config::write_atomic_bytes(
        &root.join("afmf/current.json"),
        &serde_json::to_vec(&release.version).map_err(|e| e.to_string())?,
        0o600,
    )?;
    peligames_mod_config::write_atomic_bytes(
        &root
            .join("afmf")
            .join(format!("{}.release.json", release.version)),
        &serde_json::to_vec(release).map_err(|e| e.to_string())?,
        0o600,
    )
}
pub fn select(root: &Path, version: &str) -> Result<State, String> {
    let _lock = lock(root)?;
    select_locked(root, version)
}
fn select_locked(root: &Path, version: &str) -> Result<State, String> {
    if !releases::compatible(version) {
        return Err("Unsupported AFMF runtime version".into());
    }
    runtime::validate(&runtime::location(root, version)?)?;
    peligames_mod_config::write_atomic_bytes(
        &root.join("afmf/current.json"),
        &serde_json::to_vec(version).map_err(|e| e.to_string())?,
        0o600,
    )?;
    state(root)
}
pub fn remove(root: &Path, version: &str) -> Result<State, String> {
    let _lock = lock(root)?;
    peligames_addon_kit::files::check_directory(root)?;
    let was_current = current(root) == version;
    let path = runtime::location(root, version)?;
    if path.is_symlink() {
        return Err("Redirected AFMF runtime".into());
    }
    peligames_addon_kit::files::ensure_unmapped(std::slice::from_ref(&path))?;
    if path.exists() {
        fs::remove_dir_all(&path).map_err(|e| e.to_string())?;
    }
    let metadata = root.join("afmf").join(format!("{version}.release.json"));
    if metadata.exists() {
        fs::remove_file(metadata).map_err(|e| e.to_string())?;
    }
    let remaining = state(root)?;
    if was_current {
        if let Some(replacement) = remaining
            .versions
            .iter()
            .find(|v| releases::compatible(&v.version))
        {
            return select_locked(root, &replacement.version);
        }
        let pointer = root.join("afmf/current.json");
        if pointer.exists() {
            fs::remove_file(pointer).map_err(|error| error.to_string())?;
        }
    }
    state(root)
}
fn lock(root: &Path) -> Result<peligames_mod_config::DirectoryLock, String> {
    let directory = root.join("afmf");
    peligames_addon_kit::files::check_directory(&directory)?;
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    peligames_mod_config::DirectoryLock::acquire(&directory)
}
