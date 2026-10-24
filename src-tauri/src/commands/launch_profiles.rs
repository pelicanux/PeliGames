//! Launcher-neutral profiles. A launcher is a caller, never the profile's identity.
use super::steam_runtime::{self, contract::{Profile, UNIVERSAL_VERSION}, profile_io};
use crate::core::advanced_settings::AdvancedSettings;
use serde::Serialize;
use std::{fs, os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt}, os::fd::AsRawFd, path::{Path, PathBuf}, sync::Mutex};

static LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize)]
pub struct Export {
    pub id: String,
    pub executable: String,
    pub arguments: String,
    pub line: String,
    pub profiles: String,
}

fn root() -> Result<PathBuf, String> {
    Ok(dirs::data_dir().ok_or("Pasta de dados indisponível.")?.join("pelirun"))
}

pub(crate) fn profiles_directory() -> Result<PathBuf, String> { Ok(root()?.join("profiles")) }

pub(super) fn prefix(path: &Path) -> Result<Option<String>, String> {
    let root = root()?;
    if !root.join("profiles").is_dir() { return Ok(None); }
    let key = installation(path)?;
    Ok(find(&root.join("profiles"), key.to_str().ok_or("Instalação não usa UTF-8.")?)?
        .and_then(|p| p.id).map(|id| format!("{} --profile {id} -- ", quote(&root.join("pelirun").to_string_lossy()))))
}

pub(super) fn installation(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() { return Err("Escolha uma instalação com caminho absoluto.".into()); }
    let canonical = path.canonicalize().map_err(|e| format!("Instalação indisponível: {e}"))?;
    Ok(if canonical.is_file() { canonical.parent().ok_or("Instalação inválida.")?.into() } else { canonical })
}

fn find(directory: &Path, installation: &str) -> Result<Option<Profile>, String> {
    if !directory.exists() { return Ok(None); }
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|v| v.to_str()) != Some("json") { continue; }
        let Some(id) = path.file_stem().and_then(|v| v.to_str()).filter(|v| steam_runtime::contract::profile_id(v).is_ok()) else { continue };
        if let Some(profile) = profile_io::universal(directory, id)? {
            if profile.installation.as_deref() == Some(installation) { return Ok(Some(profile)); }
        }
    }
    Ok(None)
}

fn quote(value: &str) -> String { format!("'{}'", value.replace('\'', "'\"'\"'")) }

fn save_at(root: &Path, installation: &str, mut profile: Profile) -> Result<Export, String> {
    steam_runtime::private_directory(root)?;
    let lock = fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .mode(0o600).custom_flags(libc::O_NOFOLLOW).open(root.join(".lock")).map_err(|e| e.to_string())?;
    let meta = lock.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o022 != 0 {
        return Err("Bloqueio do perfil PeliRun inválido.".into());
    }
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 { return Err(std::io::Error::last_os_error().to_string()); }
    let directory = root.join("profiles");
    steam_runtime::private_directory(&directory)?;
    let id = match find(&directory, installation)? {
        Some(previous) => previous.id.ok_or("Perfil sem identificador.")?,
        None => loop {
            let id = format!("pg-{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
            // Never reuse a short ID which already belongs to another installation.
            match fs::symlink_metadata(directory.join(format!("{id}.json"))) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break id,
                Err(e) => return Err(e.to_string()),
                Ok(_) => {},
            }
        },
    };
    profile.version = UNIVERSAL_VERSION;
    profile.app_id = 0;
    profile.id = Some(id.clone());
    profile.installation = Some(installation.into());
    profile.validate_id(&id)?;
    let bytes = serde_json::to_vec_pretty(&profile).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > steam_runtime::contract::MAX_BYTES { return Err("Perfil PeliRun muito grande.".into()); }
    let binary = root.join("pelirun");
    if steam_runtime::HELPER.len() < 64 { return Err("PeliRun indisponível neste pacote.".into()); }
    // Atomic replacement also supports a helper which is currently executing.
    let valid_binary = fs::symlink_metadata(&binary).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink()
        && m.uid() == unsafe { libc::geteuid() } && m.permissions().mode() & 0o111 != 0 && m.mode() & 0o022 == 0);
    if !valid_binary || !fs::read(&binary).is_ok_and(|bytes| bytes == steam_runtime::HELPER) {
        steam_runtime::atomic_write(&binary, steam_runtime::HELPER, 0o700)?;
    }
    steam_runtime::atomic_write(&directory.join(format!("{id}.json")), &bytes, 0o600)?;
    let executable = binary.to_str().ok_or("Caminho PeliRun não usa UTF-8.")?.to_string();
    let arguments = format!("--profile {id} --");
    Ok(Export { id, line: format!("{} {arguments}", quote(&executable)), executable,
        arguments, profiles: directory.to_string_lossy().into_owned() })
}

pub(super) fn save(path: &Path, profile: Profile) -> Result<Export, String> {
    let _lock = LOCK.lock().map_err(|_| "Perfil PeliRun ocupado.")?;
    // Existing profiles can be refreshed even while the game's drive is unmounted.
    let key = if path.exists() { installation(path)? } else if path.is_absolute() { path.into() } else { return Err("Instalação inválida.".into()); };
    save_at(&root()?, key.to_str().ok_or("Instalação não usa UTF-8.")?, profile)
}

pub(super) fn refresh(path: &Path, name: &str, advanced: &AdvancedSettings) -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|_| "Perfil PeliRun ocupado.")?;
    let root = root()?;
    if !root.join("profiles").is_dir() { return Ok(()); }
    let key = installation(path)?;
    let key = key.to_str().ok_or("Instalação não usa UTF-8.")?;
    if let Some(previous) = find(&root.join("profiles"), key)? {
        let mods = previous.source.get("mod_options").and_then(|v| v.as_str()).map(str::to_string);
        save_at(&root, key, steam_runtime::build_for(name, advanced, mods)?)?;
    }
    Ok(())
}

pub(super) fn refresh_all() -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|_| "Perfil PeliRun ocupado.")?;
    let root = root()?;
    let directory = root.join("profiles");
    if !directory.is_dir() { return Ok(()); }
    for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        let Some(previous) = profile_io::universal(&directory, id)? else { continue };
        let advanced: AdvancedSettings = serde_json::from_value(previous.source["advanced"].clone()).map_err(|e| e.to_string())?;
        let name = previous.source["name"].as_str().unwrap_or("Jogo");
        let mods = previous.source["mod_options"].as_str().map(str::to_string);
        save_at(&root, previous.installation.as_deref().ok_or("Perfil sem instalação.")?, steam_runtime::build_for(name, &advanced, mods)?)?;
    }
    Ok(())
}

pub(crate) fn enforce_release_policy() -> Result<(), String> {
    if crate::core::release_features::enabled() { return Ok(()); }
    // Rebuilds effective commands through the same release restrictions, while
    // keeping saved source preferences available to development builds.
    refresh_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn profile(name: &str) -> Profile {
        Profile { version: 1, app_id: 42, id: None, installation: None,
            command: vec![steam_runtime::contract::MARKER.into()], environment: BTreeMap::new(), extra_args: vec![],
            source: serde_json::json!({"name":name}) }
    }

    #[test]
    fn short_ids_reuse_one_installation_but_never_merge_equal_titles() {
        let root = std::env::temp_dir().join(format!("pelirun-registry-{}", uuid::Uuid::new_v4()));
        let first = save_at(&root, "/games/one", profile("Tomb Raider")).unwrap();
        assert_eq!(first.id.len(), 11);
        let again = save_at(&root, "/games/one", profile("Renamed game")).unwrap();
        assert_eq!(first.id, again.id);
        let other = save_at(&root, "/other-disk/one", profile("Tomb Raider")).unwrap();
        assert_ne!(first.id, other.id);
        let loaded = profile_io::universal(&root.join("profiles"), &first.id).unwrap().unwrap();
        assert_eq!(loaded.source["name"], "Renamed game");
        assert_eq!(loaded.app_id, 0);
        assert_eq!(loaded.version, 2);
        assert_eq!(fs::metadata(root.join("profiles").join(format!("{}.json", first.id))).unwrap().mode() & 0o777, 0o600);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_writers_keep_one_identifier_and_valid_atomic_json() {
        let root = std::env::temp_dir().join(format!("pelirun-concurrent-{}", uuid::Uuid::new_v4()));
        // Establish the private directories before concurrently acquiring the file lock.
        let first = save_at(&root, "/games/shared", profile("Initial")).unwrap();
        let threads: Vec<_> = (0..8).map(|i| {
            let root = root.clone();
            std::thread::spawn(move || save_at(&root, "/games/shared", profile(&format!("Game {i}"))).unwrap().id)
        }).collect();
        for thread in threads { assert_eq!(thread.join().unwrap(), first.id); }
        let directory = root.join("profiles");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        profile_io::universal(&directory, &first.id).unwrap().unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}

#[tauri::command]
pub async fn export_launch_profile(name: String, path: String, advanced: AdvancedSettings, external_id: Option<String>) -> Result<Export, String> {
    tauri::async_runtime::spawn_blocking(move || {
        advanced.validate()?;
        let key = installation(Path::new(&path))?;
        let mut steam_mods = None;
        if let Some(external_id) = external_id {
            let game = super::external_library::all()?.into_iter().find(|g| g.external_id == external_id).ok_or("Jogo externo não encontrado.")?;
            if installation(Path::new(&game.path))? != key { return Err("O jogo não corresponde à instalação do perfil.".into()); }
            if game.launcher == "Steam" {
                steam_mods = steam_runtime::mod_options(&game)?;
                steam_runtime::prepare(&game, &advanced, steam_mods.clone())?;
            }
        }
        let _lock = LOCK.lock().map_err(|_| "Perfil PeliRun ocupado.")?;
        let root = root()?;
        let previous = find(&root.join("profiles"), key.to_str().ok_or("Instalação não usa UTF-8.")?)?;
        let mods = steam_mods.or_else(|| previous.as_ref().and_then(|p| p.source.get("mod_options")).and_then(|v| v.as_str()).map(str::to_string));
        save_at(&root, key.to_str().ok_or("Instalação não usa UTF-8.")?, steam_runtime::build_for(&name, &advanced, mods)?)
    }).await.map_err(|e| e.to_string())?
}

pub(super) fn remove_installation(installation:&Path)->Result<(),String>{
    let _guard=LOCK.lock().map_err(|e|e.to_string())?;
    let directory=profiles_directory()?;
    if !directory.is_dir(){return Ok(());}
    for entry in fs::read_dir(&directory).map_err(|e|e.to_string())?{
        let path=entry.map_err(|e|e.to_string())?.path();
        let Some(id)=path.file_stem().and_then(|s|s.to_str()).filter(|id|steam_runtime::contract::profile_id(id).is_ok()) else{continue};
        if path.extension().and_then(|s|s.to_str())!=Some("json"){continue;}
        if let Some(profile)=profile_io::universal(&directory,id)?{if profile.installation.as_deref()==installation.to_str(){fs::remove_file(&path).map_err(|e|e.to_string())?;}}
    }
    Ok(())
}
