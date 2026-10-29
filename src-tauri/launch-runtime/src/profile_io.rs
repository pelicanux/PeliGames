//! Private profile reads shared by the GUI and the standalone runner.
use super::profile::{profile_id, Profile, MAX_BYTES};
use serde::{Deserialize, Serialize};
use std::{fs, io::Read, os::unix::fs::{MetadataExt, OpenOptionsExt}, path::{Path, PathBuf}};

#[derive(Serialize, Deserialize)]
pub struct Reference {
    pub version: u32,
    pub app_id: u32,
    pub pelirun_reference: String,
    pub directory: PathBuf,
}

fn document(path: &Path) -> Result<Option<serde_json::Value>, String> {
    let file = match fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("Não foi possível abrir o perfil PeliRun: {e}")),
    };
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o022 != 0 || meta.len() > MAX_BYTES {
        return Err("Perfil PeliRun sem acesso exclusivo ou inválido.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Perfil PeliRun muito grande.".into()); }
    serde_json::from_slice(&bytes).map(Some).map_err(|e| format!("Perfil PeliRun inválido: {e}"))
}

/// Former id → current id of renamed profiles, so launch options saved with the
/// former id (in Steam or pasted in another launcher) keep finding them.
pub const ALIASES: &str = ".aliases.json";
pub fn aliases(directory: &Path) -> Result<std::collections::BTreeMap<String, String>, String> {
    match document(&directory.join(ALIASES))? {
        Some(value) => serde_json::from_value(value).map_err(|e| format!("Apelidos PeliRun inválidos: {e}")),
        None => Ok(Default::default()),
    }
}
pub fn universal(directory: &Path, id: &str) -> Result<Option<Profile>, String> {
    profile_id(id)?;
    let Some(value) = document(&directory.join(format!("{id}.json")))? else {
        return match aliases(directory)?.get(id) {
            Some(current) if current != id && profile_id(current).is_ok() && directory.join(format!("{current}.json")).exists() => universal(directory, current),
            _ => Ok(None),
        };
    };
    let profile: Profile = serde_json::from_value(value).map_err(|e| e.to_string())?;
    profile.validate_id(id)?;
    Ok(Some(profile))
}

pub fn legacy(path: &Path, app: u32) -> Result<Option<Profile>, String> {
    let Some(value) = document(path)? else { return Ok(None) };
    if value.get("pelirun_reference").is_some() {
        let reference: Reference = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if reference.version != 1 || reference.app_id != app || !reference.directory.is_absolute() {
            return Err("Vínculo Steam/PeliRun inválido.".into());
        }
        // A missing target after reset falls back to the original launcher command.
        universal(&reference.directory, &reference.pelirun_reference)
    } else {
        let profile: Profile = serde_json::from_value(value).map_err(|e| e.to_string())?;
        profile.validate(app)?;
        Ok(Some(profile))
    }
}
