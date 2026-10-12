//! Mod profiles per game, as in Vortex: installed archives are shared; each
//! profile keeps which mods are enabled, their order and the per-profile game
//! choices (plugins page, BG3 load order, BSA redirection). Switching deploys
//! the chosen profile. (The deployment revision kept in `NexusGame::profile`
//! is a different thing: the built file set of the active profile.)
use super::nexus_local::{NexusGame, Workspace};
use std::{fs, path::PathBuf};

/// Files of the game folder that belong to the active profile.
const PROFILE_FILES: [&str; 4] = ["plugins.json", "bg3-load-order.json", "bsa-redirection", "ba2-order.json"];
pub(super) const DEFAULT_ID: &str = "default";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    /// Mod ids in priority order and which of them are enabled; the active
    /// profile is stored here only when another one is chosen.
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub enabled: Vec<String>,
    /// Own save games (`SLocalSavePath`) and own game INIs, as the Vortex profile options.
    #[serde(default)]
    pub own_saves: bool,
    #[serde(default)]
    pub own_settings: bool,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Profiles {
    pub active: String,
    pub profiles: Vec<Profile>,
}
fn path(game: &NexusGame) -> Result<PathBuf, String> {
    Ok(super::nexus_profile::directory(game)?.join("profiles.json"))
}
fn files_of(game: &NexusGame, id: &str) -> Result<PathBuf, String> {
    uuid::Uuid::parse_str(id).map(|_| ()).or_else(|_| if id == DEFAULT_ID { Ok(()) } else { Err("Perfil inválido.") })?;
    Ok(super::nexus_profile::directory(game)?.join("user-profiles").join(id))
}
pub(super) fn load(game: &NexusGame) -> Result<Profiles, String> {
    match fs::read(path(game)?) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Perfis do jogo inválidos.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Profiles {
            active: DEFAULT_ID.into(),
            profiles: vec![Profile { id: DEFAULT_ID.into(), name: "Padrão".into(), order: Vec::new(), enabled: Vec::new(), own_saves: false, own_settings: false }],
        }),
        Err(e) => Err(e.to_string()),
    }
}
fn save(game: &NexusGame, profiles: &Profiles) -> Result<(), String> {
    let path = path(game)?;
    fs::create_dir_all(path.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temporary, serde_json::to_vec_pretty(profiles).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&temporary, &path).map_err(|e| { let _ = fs::remove_file(&temporary); e.to_string() })
}
/// The live mod state as a profile snapshot.
fn snapshot(game: &NexusGame, profile: &mut Profile) {
    profile.order = game.mods.iter().map(|m| m.id.clone()).collect();
    profile.enabled = game.mods.iter().filter(|m| m.installed && m.enabled).map(|m| m.id.clone()).collect();
}
/// Applies a snapshot: its order first (mods added later keep their place
/// after it) and only its enabled mods; requirements stay before dependents.
fn restore(game: &mut NexusGame, profile: &Profile) {
    let position = |id: &str| profile.order.iter().position(|known| known == id).unwrap_or(usize::MAX);
    let mut mods: Vec<(usize, super::nexus_local::LocalMod)> = std::mem::take(&mut game.mods).into_iter().enumerate().collect();
    mods.sort_by_key(|(index, item)| (position(&item.id), *index));
    game.mods = mods.into_iter().map(|(_, mut item)| { item.enabled = item.installed && profile.enabled.contains(&item.id); item }).collect();
    super::nexus_local::dependency_order(game);
}
/// Moves the per-profile game files between the game folder and a profile folder.
fn store_files(game: &NexusGame, id: &str) -> Result<(), String> {
    let (dir, target) = (super::nexus_profile::directory(game)?, files_of(game, id)?);
    fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    for name in PROFILE_FILES {
        let _ = fs::remove_file(target.join(name));
        if dir.join(name).is_file() { fs::copy(dir.join(name), target.join(name)).map_err(|e| e.to_string())?; }
    }
    Ok(())
}
fn load_files(game: &NexusGame, id: &str) -> Result<(), String> {
    let (dir, source) = (super::nexus_profile::directory(game)?, files_of(game, id)?);
    for name in PROFILE_FILES {
        let _ = fs::remove_file(dir.join(name));
        if source.join(name).is_file() { fs::copy(source.join(name), dir.join(name)).map_err(|e| e.to_string())?; }
    }
    Ok(())
}
#[derive(serde::Serialize)]
pub struct ProfileSummary { id: String, name: String, active: bool, enabled: usize, own_saves: bool, own_settings: bool, saves_supported: bool }
fn summary(game: &NexusGame, profiles: &Profiles) -> Vec<ProfileSummary> {
    let saves_supported = super::nexus_deployment::definition(game).is_some_and(|module| module.saves.is_some());
    profiles.profiles.iter().map(|profile| ProfileSummary {
        id: profile.id.clone(), name: profile.name.clone(), active: profile.id == profiles.active,
        own_saves: profile.own_saves, own_settings: profile.own_settings, saves_supported,
        enabled: if profile.id == profiles.active { game.mods.iter().filter(|m| m.installed && m.enabled).count() }
            else { profile.enabled.iter().filter(|id| game.mods.iter().any(|m| m.id == **id && m.installed)).count() },
    }).collect()
}
fn valid_name(name: &str, profiles: &Profiles, except: Option<&str>) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) { return Err("Nome de perfil inválido.".into()); }
    if profiles.profiles.iter().any(|p| Some(p.id.as_str()) != except && p.name.eq_ignore_ascii_case(&name)) { return Err("Já existe um perfil com esse nome.".into()); }
    Ok(name)
}
fn with_game<T>(game_id: &str, action: impl FnOnce(&NexusGame) -> Result<T, String>) -> Result<T, String> {
    let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
    let data = super::nexus_local::load()?;
    let game = data.games.iter().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    action(game)
}
#[tauri::command]
pub async fn nexus_user_profiles(game_id: String) -> Result<Vec<ProfileSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| Ok(summary(game, &load(game)?)))).await.map_err(|e| e.to_string())?
}
/// New profile: a copy of the current one, or with every mod disabled.
#[tauri::command]
pub async fn create_nexus_user_profile(game_id: String, name: String, copy: bool) -> Result<Vec<ProfileSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| {
        let mut profiles = load(game)?;
        let name = valid_name(&name, &profiles, None)?;
        let mut profile = Profile { id: uuid::Uuid::new_v4().to_string(), name, order: Vec::new(), enabled: Vec::new(), own_saves: false, own_settings: false };
        if copy {
            snapshot(game, &mut profile);
            store_files(game, &profile.id)?;
        } else {
            profile.order = game.mods.iter().map(|m| m.id.clone()).collect();
        }
        profiles.profiles.push(profile);
        save(game, &profiles)?;
        Ok(summary(game, &profiles))
    })).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn rename_nexus_user_profile(game_id: String, id: String, name: String) -> Result<Vec<ProfileSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| {
        let mut profiles = load(game)?;
        let name = valid_name(&name, &profiles, Some(&id))?;
        profiles.profiles.iter_mut().find(|p| p.id == id).ok_or("Perfil não encontrado.")?.name = name;
        save(game, &profiles)?;
        Ok(summary(game, &profiles))
    })).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn delete_nexus_user_profile(game_id: String, id: String) -> Result<Vec<ProfileSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| {
        let mut profiles = load(game)?;
        if profiles.active == id { return Err("Troque para outro perfil antes de apagar este.".into()); }
        let before = profiles.profiles.len();
        profiles.profiles.retain(|p| p.id != id);
        if profiles.profiles.len() == before { return Err("Perfil não encontrado.".into()); }
        save(game, &profiles)?;
        let _ = fs::remove_dir_all(files_of(game, &id)?);
        Ok(summary(game, &profiles))
    })).await.map_err(|e| e.to_string())?
}
/// Saves the active profile, loads the chosen one and deploys it. On any
/// failure the library, the profile files and the game stay as they were.
pub(super) fn switch(data: &mut Workspace, game_id: &str, id: &str) -> Result<(), String> {
    let index = data.games.iter().position(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    let mut game = data.games[index].clone();
    super::nexus_profile::idle(&game)?;
    let mut profiles = load(&game)?;
    if profiles.active == id { return Ok(()); }
    let target = profiles.profiles.iter().find(|p| p.id == id).cloned().ok_or("Perfil não encontrado.")?;
    let current = profiles.active.clone();
    if let Some(active) = profiles.profiles.iter_mut().find(|p| p.id == current) { snapshot(&game, active); }
    let leaving = profiles.profiles.iter().find(|p| p.id == current).cloned();
    let module = super::nexus_deployment::definition(&game);
    // Game INIs and the save folder of the profiles, before the deployment reads the INIs.
    let swap = |game: &NexusGame, from: Option<&Profile>, to: Option<&Profile>| -> Result<(), String> {
        let Some(module) = &module else { return Ok(()); };
        super::nexus_saves::swap_settings(game, module, from.filter(|p| p.own_settings).map(|p| p.id.as_str()), to.filter(|p| p.own_settings).map(|p| p.id.as_str()))?;
        super::nexus_saves::apply_save_path(game, module, to.filter(|p| p.own_saves).map(|p| p.id.as_str()))
    };
    store_files(&game, &current)?;
    swap(&game, leaving.as_ref(), Some(&target))?;
    load_files(&game, &target.id)?;
    restore(&mut game, &target);
    let old = game.profile.clone();
    let deployable = super::nexus_deployment::supports(&game);
    let result = (|| {
        game.profile = Some(if deployable { super::nexus_deployment::build_after_removal(&game)? } else { super::nexus_profile::build(&game)? });
        let tx = if deployable {
            Some(super::nexus_deployment::apply(&game).map_err(|error| { super::nexus_profile::discard(&game, game.profile.as_deref().unwrap()); error })?)
        } else { None };
        Ok::<_, String>(tx)
    })();
    let tx = match result {
        Ok(tx) => tx,
        Err(error) => { let _ = load_files(&game, &current); let _ = swap(&game, Some(&target), leaving.as_ref()); return Err(error); }
    };
    profiles.active = target.id.clone();
    data.games[index] = game.clone();
    if let Err(error) = save(&game, &profiles).and_then(|_| super::nexus_local::save(data)) {
        drop(tx);
        if let Some(revision) = &game.profile { super::nexus_profile::discard(&game, revision); }
        let _ = load_files(&game, &current);
        let _ = swap(&game, Some(&target), leaving.as_ref());
        return Err(error);
    }
    if let Some(tx) = tx { tx.commit(); }
    if let Some(old) = old.filter(|old| game.profile.as_ref() != Some(old)) { super::nexus_profile::discard(&game, &old); }
    Ok(())
}
/// Own saves and own game settings of a profile; on the active profile the
/// change applies at once (INIs kept aside or put back, save folder switched).
#[tauri::command]
pub async fn set_nexus_profile_options(game_id: String, id: String, own_saves: bool, own_settings: bool) -> Result<Vec<ProfileSummary>, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| {
        super::nexus_profile::idle(game)?;
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        if module.saves.is_none() && (own_saves || own_settings) { return Err("Este jogo não tem saves e configurações por perfil.".into()); }
        let mut profiles = load(game)?;
        let active = profiles.active == id;
        let profile = profiles.profiles.iter_mut().find(|p| p.id == id).ok_or("Perfil não encontrado.")?;
        if active {
            if profile.own_settings != own_settings {
                super::nexus_saves::swap_settings(game, &module, (!own_settings).then_some(id.as_str()), own_settings.then_some(id.as_str()))?;
            }
            if profile.own_saves != own_saves { super::nexus_saves::apply_save_path(game, &module, own_saves.then_some(id.as_str()))?; }
        }
        profile.own_saves = own_saves;
        profile.own_settings = own_settings;
        save(game, &profiles)?;
        Ok(summary(game, &profiles))
    })).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn switch_nexus_user_profile(game_id: String, id: String) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = super::nexus_local::load()?;
        switch(&mut data, &game_id, &id)?;
        super::nexus_local::load()
    }).await.map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn game(mods: &[(&str, bool)]) -> NexusGame {
        serde_json::from_value(serde_json::json!({
            "id": "test", "game": {}, "platform": "proton",
            "mods": mods.iter().map(|(id, enabled)| serde_json::json!({"id": id, "name": id, "archive": "a.zip", "size": 1, "installed": true, "enabled": enabled})).collect::<Vec<_>>()
        })).unwrap()
    }
    #[test]
    fn snapshots_keep_order_and_enabled_mods() {
        let mut live = game(&[("a", true), ("b", false), ("c", true)]);
        let mut saved = Profile { id: "p".into(), name: "P".into(), order: Vec::new(), enabled: Vec::new(), own_saves: false, own_settings: false };
        snapshot(&live, &mut saved);
        assert_eq!((saved.order.clone(), saved.enabled.clone()), (vec!["a".into(), "b".into(), "c".into()], vec!["a".to_string(), "c".to_string()]));
        // Another profile: c first, only b enabled; a mod added later keeps its place after the profile's order.
        let other = Profile { id: "o".into(), name: "O".into(), order: vec!["c".into(), "b".into()], enabled: vec!["b".into()], own_saves: false, own_settings: false };
        restore(&mut live, &other);
        assert_eq!(live.mods.iter().map(|m| (m.id.as_str(), m.enabled)).collect::<Vec<_>>(), [("c", false), ("b", true), ("a", false)]);
        restore(&mut live, &saved);
        assert_eq!(live.mods.iter().map(|m| (m.id.as_str(), m.enabled)).collect::<Vec<_>>(), [("a", true), ("b", false), ("c", true)]);
    }
    #[test]
    #[ignore = "Requires an isolated XDG_CONFIG_HOME (/tmp/peligames-*)"]
    fn switching_profiles_deploys_each_profile_and_keeps_its_files() {
        use std::io::Write;
        let config_root = PathBuf::from(std::env::var("XDG_CONFIG_HOME").expect("Isolated config required"));
        assert!(config_root.starts_with("/tmp") && config_root.file_name().unwrap().to_string_lossy().starts_with("peligames-"));
        let root = config_root.join(format!("game-{}", uuid::Uuid::new_v4()));
        let binaries = root.join("SB/Binaries/Win64");
        fs::create_dir_all(&binaries).unwrap();
        fs::write(binaries.join("SB-Win64-Shipping.exe"), b"MZ fixture").unwrap();
        let (first, second) = (uuid::Uuid::new_v4().to_string(), uuid::Uuid::new_v4().to_string());
        let mut game: NexusGame = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "game": {"directory": root, "name": "Stellar Blade", "path": binaries.join("SB-Win64-Shipping.exe")},
            "platform": "proton", "mods": [
                {"id": first, "name": "One", "archive": "one.zip", "size": 1, "installed": true, "enabled": true},
                {"id": second, "name": "Two", "archive": "two.zip", "size": 1, "installed": true, "enabled": true}]
        })).unwrap();
        let archives = super::super::nexus_profile::directory(&game).unwrap().join("archives");
        fs::create_dir_all(&archives).unwrap();
        for (id, name) in [(&first, "One_P.pak"), (&second, "Two_P.pak")] {
            let mut writer = zip::ZipWriter::new(fs::File::create(archives.join(format!("{id}.zip"))).unwrap());
            writer.start_file(name, zip::write::FileOptions::default()).unwrap();
            writer.write_all(name.as_bytes()).unwrap();
            writer.finish().unwrap();
        }
        game.profile = Some(super::super::nexus_profile::build(&game).unwrap());
        super::super::nexus_deployment::apply(&game).unwrap().commit();
        let mut data = Workspace { games: vec![game.clone()] };
        super::super::nexus_local::save(&data).unwrap();
        let paks = root.join("SB/Content/Paks/~mods");
        assert!(paks.join("One_P.pak").is_file() && paks.join("Two_P.pak").is_file());
        // A per-profile file of the active profile.
        let dir = super::super::nexus_profile::directory(&game).unwrap();
        fs::write(dir.join("plugins.json"), b"{\"autosort\":false}").unwrap();

        // New empty profile: switching deploys nothing and keeps the first profile's files apart.
        let mut profiles = load(&game).unwrap();
        let empty = Profile { id: uuid::Uuid::new_v4().to_string(), name: "Vazio".into(), order: vec![second.clone()], enabled: vec![second.clone()], own_saves: false, own_settings: false };
        profiles.profiles.push(empty.clone());
        save(&game, &profiles).unwrap();
        switch(&mut data, &game.id, &empty.id).unwrap();
        assert!(!paks.join("One_P.pak").exists() && paks.join("Two_P.pak").is_file());
        assert!(!dir.join("plugins.json").exists());
        assert_eq!(load(&game).unwrap().active, empty.id);
        let library = super::super::nexus_local::load().unwrap();
        let saved = library.games.iter().find(|g| g.id == game.id).unwrap();
        assert_eq!(saved.mods.iter().map(|m| (m.name.as_str(), m.enabled)).collect::<Vec<_>>(), [("Two", true), ("One", false)]);

        // Back: both mods and the profile's file return.
        let mut data = library;
        switch(&mut data, &game.id, DEFAULT_ID).unwrap();
        assert!(paks.join("One_P.pak").is_file() && paks.join("Two_P.pak").is_file());
        assert_eq!(fs::read(dir.join("plugins.json")).unwrap(), b"{\"autosort\":false}");
        fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn profile_names_are_unique_and_bounded() {
        let profiles = Profiles { active: DEFAULT_ID.into(), profiles: vec![Profile { id: DEFAULT_ID.into(), name: "Padrão".into(), order: Vec::new(), enabled: Vec::new(), own_saves: false, own_settings: false }] };
        assert!(valid_name("padrão", &profiles, None).is_err());
        assert_eq!(valid_name("  Survival  ", &profiles, None).unwrap(), "Survival");
        assert!(valid_name("", &profiles, None).is_err() && valid_name(&"x".repeat(61), &profiles, None).is_err());
        assert_eq!(valid_name("Padrão", &profiles, Some(DEFAULT_ID)).unwrap(), "Padrão");
    }
}
