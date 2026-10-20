//! Nexus archive library and transactional, isolated mod profiles.
use crate::core::paths;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub(super) static STORAGE: Mutex<()> = Mutex::new(());
#[derive(Clone, Serialize, Deserialize)]
pub struct LocalMod {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) identification_note: Option<String>,
    pub(super) archive: String,
    pub(super) size: u64,
    #[serde(default)]
    pub(super) installed: bool,
    #[serde(default)]
    pub(super) enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) fomod_selection: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) fomod_files: Option<Vec<(String,String)>>,
    // Keep the approved installation separate from later deployment toggles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) installed_files: Option<Vec<(String,String)>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) nexus_source: Option<NexusSource>,
    /// User-chosen package root for archives without a recognized structure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) manual_root: Option<ManualRoot>,
    /// Approved as a replacer: may replace game files, which are kept and restored.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(super) replace_originals: bool,
}
/// MO2-style "set data directory": `source` is a folder inside the archive
/// (empty = archive root) and `destination` one of the module's destinations
/// (empty = apply the module's own routes from that folder).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManualRoot {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub destination: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct NexusSource {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    pub domain: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub requirements: super::nexus_catalog::Requirements,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct NexusGame {
    #[serde(default)]
    pub(super) launch_options: Option<String>,
    #[serde(default)]
    pub(super) advanced: crate::core::advanced_settings::AdvancedSettings,
    pub(super) id: String,
    pub(super) game: serde_json::Value,
    pub(super) platform: String,
    #[serde(default)]
    pub(super) deploy_method: super::nexus_deploy_methods::Method,
    pub(super) mods: Vec<LocalMod>,
    #[serde(default)]
    pub(super) profile: Option<String>,
    #[serde(default, skip_deserializing)]
    pub(super) running: bool,
    #[serde(default)]
    pub(super) compat_data: String,
    #[serde(default)]
    pub(super) proton: String,
    #[serde(default, skip_deserializing)]
    pub(super) adapter: String,
    #[serde(default)]
    pub(super) nexus_domain: String,
    #[serde(default, skip_deserializing, skip_serializing_if = "Vec::is_empty")]
    pub(super) detected_mods: Vec<DetectedMod>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct DetectedMod {
    name: String,
    enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mod_id: Option<u64>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Workspace {
    pub(super) games: Vec<NexusGame>,
}
pub(super) fn root() -> Result<PathBuf, String> {
    let path = paths::app_root()?.join("Mod/Nexus");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
/// Match requirements by Nexus domain and mod ID, never by a guessed display name.
pub(super) fn dependency_order(game: &mut NexusGame) -> bool {
    let dependencies: Vec<Vec<usize>> = game.mods.iter().map(|item| {
        item.nexus_source.iter().flat_map(|source| source.requirements.items.iter())
            .filter(|requirement| requirement.kind == "nexus")
            .filter_map(|requirement| requirement.url.as_deref().and_then(|value| reqwest::Url::parse(value).ok()))
            .filter(|url| url.scheme() == "https" && matches!(url.host_str(), Some("www.nexusmods.com" | "nexusmods.com")))
            .flat_map(|url| {
                let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
                let target = if parts.len() == 3 && parts[1] == "mods" { parts[2].parse::<u64>().ok().map(|id| (parts[0].to_owned(), id)) } else { None };
                game.mods.iter().enumerate().filter_map(move |(index, candidate)| {
                    candidate.nexus_source.as_ref().filter(|source| target.as_ref().is_some_and(|(domain,id)| source.domain.eq_ignore_ascii_case(domain) && source.mod_id == *id)).map(|_| index)
                })
            }).collect()
    }).collect();
    let order = super::nexus_dependency_order::order(&dependencies);
    if order.iter().enumerate().all(|(index, &other)| index == other) { return false; }
    let mut mods: Vec<_> = std::mem::take(&mut game.mods).into_iter().map(Some).collect();
    game.mods = order.into_iter().map(|index| mods[index].take().unwrap()).collect();
    true
}
pub(super) fn load() -> Result<Workspace, String> {
    let path = root()?.join("library.json");
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Workspace>(&bytes)
            .map_err(|e| format!("Biblioteca Nexus inválida: {e}"))
            .and_then(|mut data| {
                let mut paths_changed = false;
                // One read of the running processes for every game.
                let processes = super::nexus_deployment::ProcessSnapshot::take();
                for game in &mut data.games {
                    let storage = super::nexus_profile::directory(game)?;
                    let legacy = root()?.join("games").join(&game.id);
                    let named_legacy = super::nexus_storage::legacy_directory(game)?;
                    for item in &mut game.mods {
                        if let Ok(relative) = Path::new(&item.archive).strip_prefix(&legacy).or_else(|_| Path::new(&item.archive).strip_prefix(&named_legacy)) {
                            let archive = storage.join(relative).to_string_lossy().to_string();
                            paths_changed |= item.archive != archive;
                            item.archive = archive;
                        }
                    }
                    if let Some(source) = game.game["directory"].as_str() {
                        if let Ok((root, Some(definition))) = super::nexus_discovery::resolve(Path::new(source)) {
                            game.game["directory"] = root.to_string_lossy().to_string().into();
                            game.platform = definition.platform.into();
                        }
                    }
                    super::nexus_deployment::detect(game);
                    game.adapter = super::nexus_profile::adapter(game);
                    if !game.adapter.is_empty() {
                        game.nexus_domain = game.adapter.clone();
                    }
                    if let (Some(layout), Some(root)) = (super::nexus_deployment::definition(game).and_then(|module| module.ue4ss()), game.game["directory"].as_str()) {
                        game.detected_mods = super::nexus_scan::unreal(Path::new(root), &layout)
                            .into_iter().map(|(name, enabled)| DetectedMod { name, enabled, mod_id: None }).collect();
                    }
                    if let Some(module) = game.game["directory"].as_str().and_then(|root| super::nexus_discovery::at(Path::new(root))) {
                        let root = Path::new(game.game["directory"].as_str().unwrap());
                        let active_profile = super::nexus_profile::profile(game).ok();
                        for framework in module.frameworks.iter().filter(|framework| framework.installed(root) || active_profile.as_deref().is_some_and(|profile|framework.installed(profile))) {
                            game.detected_mods.push(DetectedMod {name:framework.name.clone(),enabled:true,mod_id:Some(framework.mod_id)});
                        }
                    }
                    game.running = super::nexus_profile::running_in(&processes, game);
                    paths_changed |= dependency_order(game);
                }
                if paths_changed { save(&data)?; }
                super::nexus_durable::recover(&mut data)?;
                Ok(data)
            }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut data=Workspace::default();
            super::nexus_durable::recover(&mut data)?;
            Ok(data)
        },
        Err(e) => Err(e.to_string()),
    }
}
pub(super) fn save(data: &Workspace) -> Result<(), String> {
    let mut ordered = data.clone();
    for game in &mut ordered.games { dependency_order(game); }
    let data = &ordered;
    let directory = root()?;
    let temporary = directory.join(format!("library-{}.tmp", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    let metadata=super::nexus_durable::stage_metadata(data)?;
    if let Err(error) = fs::rename(&temporary, directory.join("library.json")) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    metadata.commit();
    super::mod_backup::schedule(data);
    Ok(())
}
pub(super) fn workspace() -> Result<Workspace,String> {
    let _lock = STORAGE.lock().map_err(|e|e.to_string())?;
    load()
}
#[tauri::command]
pub async fn load_nexus_workspace() -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(|| {
        workspace()
    }).await.map_err(|e|e.to_string())?
}
/// Refresh discovery separately from the lightweight workspace/status polling.
#[tauri::command]
pub async fn scan_nexus_library(app: tauri::AppHandle) -> Result<Workspace, String> {
    use tauri::Manager;
    let home = app.path().home_dir().map_err(|error| error.to_string())?;
    let mut candidates = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<serde_json::Value>, String> {
        let mut candidates: Vec<serde_json::Value> = super::scanner::nexus_store_games(&home)
            .into_iter().map(|game| serde_json::to_value(game).unwrap()).collect();
        candidates.extend(crate::core::installed_library::list(false)?.into_iter()
            .map(|game| serde_json::to_value(game).unwrap()));
        Ok(candidates)
    }).await.map_err(|error| error.to_string())??;
    for game in &mut candidates {
        if let Some(url) = game["cover_url"].as_str().filter(|url| url.starts_with("file://")) {
            let source = reqwest::Url::parse(url).ok().and_then(|url| url.to_file_path().ok());
            game["cover_url"] = match source {
                Some(source) => super::covers::cache_heroic_cover(source).await.ok()
                    .and_then(|path| reqwest::Url::from_file_path(path).ok()).map(|url| serde_json::Value::String(url.to_string())).unwrap_or_default(),
                None => serde_json::Value::Null,
            };
        }
    }
    let steam_home = app.path().home_dir().map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        // Library files are read here, never on the async command thread.
        let libraries = super::scanner::steam_libraries(&steam_home);
        let _lock = STORAGE.lock().map_err(|error| error.to_string())?;
        let mut data = load()?;
        for id in prune_unsupported(&mut data.games) {
            if let Some(game) = data.games.iter().find(|game| game.id == id) {
                super::nexus_reports::event(game, "WARN", "escanear biblioteca", "jogo sem suporte Vortex mantido: há mods instalados ou o jogo está em execução");
            }
        }
        for mut game in candidates {
            let source = game["directory"].as_str().or(game["path"].as_str()).unwrap_or("");
            let Ok((directory, definition)) = super::nexus_discovery::resolve(Path::new(source)) else { continue };
            // Only games with a module and confirmed Vortex support are added automatically.
            if !definition.as_ref().is_some_and(|found| found.vortex_supported) { continue; }
            // Deduplicate installations, preserving IDs, profiles, manually selected settings and mods.
            if data.games.iter().any(|existing| existing.game["directory"].as_str()
                .is_some_and(|root| fs::canonicalize(root).ok().as_ref() == Some(&directory))) { continue; }
            if super::nexus_durable::recover_game(&directory, &mut data)? || super::mod_backup::recover(&directory, &mut data)? { continue; }
            game["directory"] = directory.to_string_lossy().to_string().into();
            game["library_view"] = "nexus".into();
            // Lutris names come from the config file slug; the module knows the real title.
            if game["launcher"] == "Lutris" { if let Some(found) = &definition { game["name"] = found.name.clone().into(); } }
            let mut compat_data = game["prefix"].as_str()
                .and_then(|prefix| super::nexus_prefix::resolve(Path::new(prefix)).ok())
                .map(|(prefix, _)| prefix.to_string_lossy().into_owned()).unwrap_or_default();
            if compat_data.is_empty() && game["launcher"] == "Steam" {
                let id = game["app_id"].as_str().map(str::to_string).or_else(|| super::nexus_discovery::steam_app_id(&directory)).unwrap_or_default();
                if let Some(prefix) = super::nexus_discovery::steam_prefix(&directory, &id, &libraries) {
                    if let Ok((prefix, _)) = super::nexus_prefix::resolve(&prefix) { compat_data = prefix.to_string_lossy().into_owned(); }
                }
            }
            let platform = definition.as_ref().map(|item| item.platform.as_str()).unwrap_or("proton").to_string();
            let proton = game["proton"].as_str().unwrap_or("").to_string();
            let mut entry = NexusGame {
                id: uuid::Uuid::new_v4().to_string(), launch_options: None, advanced: Default::default(), game, platform, deploy_method: Default::default(), mods: Vec::new(), profile: None,
                running: false, compat_data, proton, adapter: String::new(),
                nexus_domain: definition.map(|item| item.domain.to_string()).unwrap_or_default(), detected_mods: Vec::new(),
            };
            super::nexus_deployment::detect(&mut entry);
            entry.adapter = super::nexus_profile::adapter(&entry);
            data.games.push(entry);
        }
        save(&data)?;
        load()
    }).await.map_err(|error| error.to_string())?
}

/// Launchers whose games enter the library through the automatic scan.
const AUTOMATIC_LAUNCHERS: [&str; 7] = ["Steam", "Heroic", "Epic Games", "Lutris", "Faugus", "Bottles", "PeliGames"];

/// A rescan drops automatically detected games without a module and confirmed
/// Vortex support (or whose folder is gone). Games added by hand stay. A game
/// with installed mods stays too: removing it would orphan files in the game.
/// Returns the IDs of unsupported games kept for that reason.
fn prune_unsupported(games: &mut Vec<NexusGame>) -> Vec<String> {
    let mut kept = Vec::new();
    games.retain(|game| {
        if !game.game["launcher"].as_str().is_some_and(|launcher| AUTOMATIC_LAUNCHERS.contains(&launcher)) { return true; }
        let supported = game.game["directory"].as_str()
            .and_then(|root| super::nexus_discovery::at(Path::new(root)))
            .is_some_and(|module| module.vortex_supported);
        if supported { return true; }
        if game.mods.iter().any(|item| item.installed) || super::nexus_profile::idle(game).is_err() {
            kept.push(game.id.clone());
            return true;
        }
        false
    });
    kept
}

/// Deletes a Nexus library entry (right click → Remove). The game folder and the
/// stored mod packages stay on disk. Refused while mods are installed, which
/// would leave their files in the game, or while the game runs.
#[tauri::command]
pub async fn remove_nexus_game(game_id: String) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = load()?;
        let index = data.games.iter().position(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
        removable(&data.games[index])?;
        let game = data.games.remove(index);
        super::nexus_reports::event(&game, "INFO", "remover da biblioteca Nexus", "entrada removida; jogo e pacotes preservados no disco");
        save(&data)?;
        Ok(data)
    }).await.map_err(|e| e.to_string())?
}
fn removable(game: &NexusGame) -> Result<(), String> {
    if game.mods.iter().any(|item| item.installed) {
        return Err("Remova os mods instalados deste jogo antes de tirá-lo da biblioteca Nexus.".into());
    }
    super::nexus_profile::idle(game)
}

#[tauri::command]
pub async fn set_nexus_game_cover(game_id: String, name: String, cover_url: String, previous_cover: Option<String>) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let url = reqwest::Url::parse(&cover_url).map_err(|_| "Capa inválida.")?;
        if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
            return Err("Capa inválida.".into());
        }
        let _lock = STORAGE.lock().map_err(|error| error.to_string())?;
        let mut data = load()?;
        let game = data.games.iter_mut().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
        // A late search must not replace a renamed game's artwork or a newer result.
        if game.game["name"].as_str() != Some(name.as_str()) || game.game["cover_url"].as_str().filter(|value| !value.is_empty()) != previous_cover.as_deref().filter(|value| !value.is_empty()) {
            return Ok(data);
        }
        game.game["cover_url"] = cover_url.clone().into();
        game.game["automatic_cover_url"] = cover_url.into();
        save(&data)?;
        Ok(data)

    }).await.map_err(|error| error.to_string())?
}
#[derive(Serialize)]
pub struct DiscoveryReport {
    directory: String,
    name: Option<String>,
    domain: Option<String>,
    platform: Option<String>,
    required_files: Vec<String>,
    mod_destinations: Vec<String>,
    isolated_profile: bool,
    frameworks: Vec<super::nexus_deployment::FrameworkStatus>,
    module_version: Option<String>,
    vortex_supported: bool,
    notes: Vec<String>,
}
#[tauri::command]
pub async fn inspect_nexus_game(source: String) -> Result<DiscoveryReport, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<DiscoveryReport, String> {
        let (root, found) = super::nexus_discovery::resolve(Path::new(&source))?;
        Ok(DiscoveryReport {
            directory: root.to_string_lossy().into(),
            name: found.as_ref().map(|definition| definition.name.clone()),
            domain: found.as_ref().map(|definition| definition.domain.clone()),
            platform: found.as_ref().map(|definition| definition.platform.clone()),
            required_files: found.as_ref().map(|definition| definition.required.clone()).unwrap_or_default(),
            mod_destinations: found.as_ref().filter(|definition| definition.engine == super::nexus_modules::Engine::Deployment).map(|definition| definition.destinations.iter().map(|relative| root.join(relative).to_string_lossy().into()).collect()).unwrap_or_default(),
            frameworks: found.as_ref().map(|definition| super::nexus_deployment::framework_status(definition, &root)).unwrap_or_default(),
            vortex_supported: found.as_ref().is_some_and(|m|m.vortex_supported),
            module_version: found.as_ref().map(|definition| definition.version.clone()),
            notes: found.as_ref().map(|definition| definition.notes.clone()).unwrap_or_default(),
            isolated_profile: found.is_some_and(|definition| definition.engine == super::nexus_modules::Engine::ValheimProfile),
        })
    }).await.map_err(|_| "Falha ao identificar a pasta do jogo.".to_string())?
}
#[tauri::command]
pub async fn add_nexus_game(game: serde_json::Value, platform: String) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || add_nexus_game_inner(game,platform)).await.map_err(|e|e.to_string())?
}
fn add_nexus_game_inner(mut game: serde_json::Value, platform: String) -> Result<Workspace, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    if !["native", "proton"].contains(&platform.as_str()) {
        return Err("Plataforma inválida.".into());
    }
    let object = game.as_object_mut().ok_or("Jogo inválido.")?;
    if object
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        return Err("Informe o nome.".into());
    }
    let source = object
        .get("directory")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| object.get("path").and_then(|v| v.as_str()))
        .ok_or("Escolha a pasta do jogo.")?;
    let (directory, identified) = super::nexus_discovery::resolve(Path::new(source))?;
    let platform = identified.map(|definition| definition.platform.to_string()).unwrap_or(platform);
    object.insert(
        "directory".into(),
        directory.to_string_lossy().to_string().into(),
    );
    object.insert("library_view".into(), "nexus".into());
    let mut data = load()?;
    let game_path = game
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Caminho inválido.")?;
    if data.games.iter().any(|g| {
        (g.game.get("path").and_then(|v| v.as_str()) == Some(game_path)
            && g.game.get("launcher") == game.get("launcher")
        ) || g.game.get("directory").and_then(|v| v.as_str()).is_some_and(|existing| {
            fs::canonicalize(existing).ok().as_ref() == Some(&directory)
        })
    }) {
        // Opening a unified game tab may race discovery or use a legacy store
        // identity. Reuse the registration and its mods/profile, never create
        // another entry or treat an already managed installation as a failure.
        return Ok(data);
    }
    if super::nexus_durable::recover_game(&directory, &mut data)? || super::mod_backup::recover(&directory, &mut data)? { save(&data)?; return Ok(data); }
    let mut compat_data = String::new();
    let mut proton = String::new();
    if platform == "proton" {
        if let Some(value) = game["prefix"].as_str().map(str::trim).filter(|value| !value.is_empty()) {
            let (prefix, _) = super::nexus_prefix::resolve(Path::new(value))?;
            compat_data = prefix.to_string_lossy().into();
        }
        if let Some(value) = game["proton"].as_str().map(str::trim).filter(|value| !value.is_empty()) {
            let runner = fs::canonicalize(value).map_err(|error| format!("Proton: {error}"))?;
            if !runner.join("proton").is_file() {
                return Err("Selecione uma instalação do Proton.".into());
            }
            proton = runner.to_string_lossy().into();
        }
    }
    let mut entry = NexusGame {
        launch_options: None, advanced: Default::default(),
        id: uuid::Uuid::new_v4().to_string(),
        game,
        platform,
        deploy_method: Default::default(),
        mods: Vec::new(),
        profile: None,
        running: false,
        compat_data,
        proton,
        adapter: String::new(),
        nexus_domain: String::new(),
        detected_mods: Vec::new(),
    };
    if entry.platform == "proton" && entry.compat_data.is_empty() {
        let executable = entry.game["executable"].as_str().or(entry.game["path"].as_str()).unwrap_or("");
        if let Some(prefix) = super::nexus_prefix::containing(Path::new(executable))
            .or_else(|| super::nexus_prefix::containing(&directory)) {
            entry.compat_data = prefix.to_string_lossy().into();
        } else if let Ok(installed) = crate::core::installed_library::list(false) {
            let matches: Vec<_> = installed.iter().filter(|game| {
                fs::canonicalize(&game.executable).ok() == fs::canonicalize(executable).ok()
                    && fs::canonicalize(executable).is_ok()
            }).collect();
            if matches.len() == 1 {
                if let Ok((prefix, _)) = super::nexus_prefix::resolve(Path::new(&matches[0].prefix)) {
                    entry.compat_data = prefix.to_string_lossy().into();
                    if entry.proton.is_empty() { entry.proton = matches[0].proton.clone(); }
                }
            }
        }
    }
    super::nexus_deployment::detect(&mut entry);
    if entry.platform == "proton" {
        if entry.compat_data.is_empty() {
            return Err("Não foi possível detectar o prefixo deste jogo. Selecione o prefixo existente antes de adicionar.".into());
        }
        let (_, wine) = super::nexus_prefix::resolve(Path::new(&entry.compat_data))?;
        entry.game["prefix"] = wine.to_string_lossy().to_string().into();
        entry.game["proton"] = entry.proton.clone().into();
    }
    entry.adapter = super::nexus_profile::adapter(&entry);
    if !entry.adapter.is_empty() {
        entry.nexus_domain = entry.adapter.clone();
    }
    data.games.push(entry);
    save(&data)?;
    Ok(data)
}
#[tauri::command]
pub async fn import_nexus_archive(game_id: String, source: String) -> Result<Workspace, String> {
    let before = diagnostic_game(&game_id)?;
    let imported = import_archive(game_id.clone(), source.clone(), None).await?;
    let item = imported.games.iter().find(|g| g.id == game_id)
        .and_then(|g| g.mods.last())
        .ok_or("Arquivo importado não encontrado.")?.clone();
    // Identify the managed copy, never the user's file after it has been copied.
    // The original name is kept only for the Nexus filename convention.
    let original_name = Path::new(&source).file_name().and_then(|name| name.to_str()).unwrap_or_default().to_string();
    let identity = identify_local_archive(&before.nexus_domain, &item.archive, &original_name).await;
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let mut data = load()?;
    if let Some(mod_item) = data.games.iter_mut().find(|g| g.id == game_id && g.nexus_domain == before.nexus_domain)
        .and_then(|g| g.mods.iter_mut().find(|m| m.id == item.id && m.archive == item.archive && m.nexus_source.is_none())) {
        match identity {
            Ok(Some(source)) => {
                mod_item.name = source.name.clone(); mod_item.version = source.version.clone();
                mod_item.nexus_source = Some(source); mod_item.identification_note = None;
            }
            Ok(None) => mod_item.identification_note = Some("Arquivo local: sem correspondência única no Nexus. Os requisitos declarados pelo autor não foram identificados.".into()),
            Err(_) => mod_item.identification_note = Some("Arquivo local importado. Não foi possível identificar no Nexus; confira a associação do jogo, a conta conectada e a conexão. Requisitos do autor não verificados.".into()),
        }
        save(&data)?;
    }
    for game in &mut data.games { dependency_order(game); }
    Ok(data)
}

async fn identify_local_archive(domain: &str, archive: &str, original_name: &str) -> Result<Option<NexusSource>, String> {
    let domain = super::nexus_catalog::domain(domain)?.to_string();
    // 1. Nexus filename convention, confirmed by an exact file name (and size) in that mod's file list.
    let size = { let path = archive.to_string(); tauri::async_runtime::spawn_blocking(move || fs::metadata(path).map(|m| m.len()).ok()).await.ok().flatten() };
    for mod_id in filename_mod_ids(original_name) {
        let Ok(files) = super::nexus_account::metadata(&format!("games/{domain}/mods/{mod_id}/files.json")).await else { continue };
        let Some((file_id, version)) = filename_match(&files, original_name, size) else { continue };
        let Ok(info) = super::nexus_account::metadata(&format!("games/{domain}/mods/{mod_id}.json")).await else { continue };
        let Some(name) = mod_title(&info, &domain, mod_id) else { continue };
        let requirements = super::nexus_catalog::requirements_for_file(&domain, mod_id, Some(file_id)).await;
        return Ok(Some(NexusSource { domain, mod_id, file_id, name, version, requirements }));
    }
    // 2. Content hash, independent of the file name.
    let archive = archive.to_string();
    let hash = tauri::async_runtime::spawn_blocking(move || archive_md5(Path::new(&archive))).await.map_err(|e| e.to_string())??;
    let result = super::nexus_account::metadata(&format!("games/{domain}/mods/md5_search/{hash}.json")).await?;
    let Some((mod_id, file_id, name, version)) = unique_archive_match(&result, &domain)? else { return Ok(None); };
    let requirements = super::nexus_catalog::requirements_for_file(&domain, mod_id, Some(file_id)).await;
    Ok(Some(NexusSource { domain, mod_id, file_id, name, version, requirements }))
}

pub(super) fn archive_md5(path: &Path) -> Result<String, String> {
    use md5::{Digest, Md5};
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > super::nexus_archive::TOTAL_LIMIT { return Err("Arquivo inválido para identificação.".into()); }
    let mut hash = Md5::new(); let mut buffer = [0u8; 65536]; let mut total = 0u64;
    loop {
        let size = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if size == 0 { break; }
        total += size as u64;
        if total > super::nexus_archive::TOTAL_LIMIT { return Err("Arquivo excede o limite de identificação.".into()); }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Mod IDs suggested by a Nexus download name: `<title>-<mod_id>-<version...>-<timestamp>.ext`.
/// A version in the title (`Mod v1-2-0-17561-1-2-0`) makes later numbers the
/// likelier ID. These are only candidates: `filename_match` must confirm them.
fn filename_mod_ids(file_name: &str) -> Vec<u64> {
    let lower = file_name.to_ascii_lowercase();
    let stem = [".tar.gz", ".zip", ".7z", ".rar"].iter().find_map(|ext| lower.ends_with(ext).then(|| &file_name[..file_name.len() - ext.len()])).unwrap_or(file_name);
    let segments: Vec<&str> = stem.split('-').collect();
    let digits = segments.iter().rev().take_while(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).count();
    if digits == 0 || digits == segments.len() { return Vec::new(); }
    let title = segments[..segments.len() - digits].join("-");
    let parts = &segments[segments.len() - digits..];
    let version_in_title = {
        let t = title.trim_end();
        let digits_end = t.trim_end_matches(|c: char| c.is_ascii_digit());
        digits_end.len() < t.len() && (digits_end.ends_with(" v") || digits_end.ends_with("_v") || digits_end.eq_ignore_ascii_case("v") || digits_end.ends_with(" V"))
    };
    let later: Vec<usize> = (1..parts.len().saturating_sub(1)).filter(|i| parts[*i].len() >= 3).rev().take(4).collect();
    if version_in_title && later.is_empty() { return Vec::new(); }
    let starts: Vec<usize> = if version_in_title { later.into_iter().chain([0]).collect() } else { [0].into_iter().chain(later).collect() };
    let mut ids = Vec::new();
    for index in starts {
        if let Ok(id) = parts[index].parse::<u64>() {
            if id > 0 && parts[index].len() <= 9 && !ids.contains(&id) { ids.push(id); }
        }
    }
    ids
}

/// The single file of a mod whose download name equals the archive name (and
/// size, when the API reports it). Ambiguous or missing matches are refused.
fn filename_match(files: &serde_json::Value, file_name: &str, size: Option<u64>) -> Option<(u64, String)> {
    let matches: Vec<_> = files["files"].as_array()?.iter()
        .filter(|f| f["file_name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(file_name)))
        .filter(|f| match (f["size_in_bytes"].as_u64(), size) { (Some(expected), Some(actual)) => expected == actual, _ => true })
        .collect();
    let [file] = matches.as_slice() else { return None };
    let file_id = file["file_id"].as_u64().filter(|id| *id > 0)?;
    Some((file_id, file["version"].as_str().unwrap_or_default().to_string()))
}

fn mod_title(info: &serde_json::Value, domain: &str, mod_id: u64) -> Option<String> {
    if info["mod_id"].as_u64() != Some(mod_id) || info["domain_name"].as_str().is_some_and(|d| d != domain) || info["available"].as_bool() == Some(false) { return None; }
    info["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).map(str::to_string)
}

/// Whether an MD5 search lists the exact `(mod_id, file_id)` of this game.
pub(super) fn md5_lists_file(value: &serde_json::Value, domain: &str, mod_id: u64, file_id: u64) -> bool {
    value.as_array().is_some_and(|results| results.iter().any(|result| {
        result["mod"]["domain_name"].as_str() == Some(domain)
            && result["mod"]["mod_id"].as_u64() == Some(mod_id)
            && result["file_details"]["file_id"].as_u64() == Some(file_id)
    }))
}

// A hash can refer to more than one Nexus upload. Never pick an arbitrary ID.
fn unique_archive_match(value: &serde_json::Value, domain: &str) -> Result<Option<(u64, u64, String, String)>, String> {
    let results = value.as_array().ok_or("Resposta de identificação inválida.")?;
    let mut candidates = std::collections::BTreeMap::new();
    for result in results {
        let info = &result["mod"]; let file = &result["file_details"];
        if info["domain_name"].as_str() != Some(domain) { continue; }
        let mod_id = info["mod_id"].as_u64().filter(|id| *id > 0).ok_or("ID de mod inválido.")?;
        let file_id = file["file_id"].as_u64().filter(|id| *id > 0).ok_or("ID de arquivo inválido.")?;
        let name = info["name"].as_str().filter(|s| !s.trim().is_empty()).ok_or("Nome do mod indisponível.")?.to_string();
        let version = file["version"].as_str().unwrap_or_default().to_string();
        candidates.insert((mod_id, file_id), (name, version));
    }
    if candidates.len() != 1 { return Ok(None); }
    Ok(candidates.into_iter().next().map(|((mod_id, file_id), (name, version))| (mod_id, file_id, name, version)))
}

#[tauri::command]
pub async fn refresh_nexus_mod_labels(game_id: String) -> Result<Vec<LocalMod>, String> {
    let game = diagnostic_game(&game_id)?;
    let mut updates = Vec::new();
    let mut local_updates = Vec::new();
    for item in &game.mods {
        let Some(source) = &item.nexus_source else {
            if item.version.is_empty() {
                if let Some((name, version)) = package_identity(Path::new(&item.archive)) { local_updates.push((item.id.clone(), name, version)); }
            }
            continue;
        };
        if !source.name.is_empty() && !item.version.is_empty() { continue; }
        // Never infer a Nexus identity from numbers in an archive name.
        let Ok(info) = super::nexus_account::metadata(&format!("games/{}/mods/{}.json", source.domain, source.mod_id)).await else { continue; };
        let Ok(file) = super::nexus_account::metadata(&format!("games/{}/mods/{}/files/{}.json", source.domain, source.mod_id, source.file_id)).await else { continue; };
        let Some(name) = info["name"].as_str().filter(|s| !s.trim().is_empty()) else { continue; };
        updates.push((item.id.clone(), source.domain.clone(), source.mod_id, source.file_id, name.to_string(), file["version"].as_str().unwrap_or_default().to_string()));
    }
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let mut data = load()?;
    let game = data.games.iter_mut().find(|g| g.id == game_id).ok_or("Jogo não encontrado.")?;
    for (id, name, version) in &local_updates {
        if let Some(item) = game.mods.iter_mut().find(|m| m.id == *id && m.nexus_source.is_none() && m.version.is_empty()) {
            item.name = name.clone(); item.version = version.clone();
        }
    }
    for (id, domain, mod_id, file_id, name, version) in &updates {
        if let Some(item) = game.mods.iter_mut().find(|m| m.id == *id) {
            if let Some(source) = &mut item.nexus_source {
                if source.domain == *domain && source.mod_id == *mod_id && source.file_id == *file_id {
                    item.name = name.clone(); item.version = version.clone(); source.name = name.clone(); source.version = version.clone();
                }
            }
        }
    }
    let result = game.mods.clone();
    if !updates.is_empty() || !local_updates.is_empty() { save(&data)?; }
    Ok(result)
}

fn package_identity(path: &Path) -> Option<(String, String)> {
    use std::io::Read;
    if !path.extension()?.to_str()?.eq_ignore_ascii_case("zip") { return None; }
    let mut archive = zip::ZipArchive::new(fs::File::open(path).ok()?).ok()?;
    let manifest = archive.by_name("manifest.json").ok()?;
    if manifest.size() > 65536 { return None; }
    let mut bytes = Vec::new(); manifest.take(65537).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 65536 { return None; }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let name = value["name"].as_str()?.trim();
    let version = value["version_number"].as_str()?.trim();
    if name.is_empty() || name.len() > 256 || version.is_empty() || version.len() > 128 { return None; }
    Some((name.replace('_', " "), version.to_string()))
}

#[cfg(test)]
mod identity_tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn identifies_content_independently_of_filename() {
        let first = std::env::temp_dir().join(format!("{}.zip", uuid::Uuid::new_v4()));
        let renamed = std::env::temp_dir().join(format!("renamed-{}.zip", uuid::Uuid::new_v4()));
        fs::write(&first, b"abc").unwrap(); fs::copy(&first, &renamed).unwrap();
        assert_eq!(archive_md5(&first).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(archive_md5(&first).unwrap(), archive_md5(&renamed).unwrap());
        fs::remove_file(first).unwrap(); fs::remove_file(renamed).unwrap();
    }
    #[test]
    fn filename_convention_suggests_mod_ids_only_as_candidates() {
        assert_eq!(filename_mod_ids("A StoryWealth - Caves Of The Commonwealth-60927-1-1-1-1758182764.7z"), vec![60927]);
        assert_eq!(filename_mod_ids("UIExtensions v1-2-0-17561-1-2-0.zip"), vec![17561, 2]);
        // Um timestamp isolado (sufixo de versão não numérico) não vira ID.
        assert!(filename_mod_ids("SkyUI_5_2_SE-12604-5-2SE-1700000000.zip").is_empty());
        assert!(filename_mod_ids("readme.zip").is_empty());
        assert!(filename_mod_ids("12345-1-0.zip").is_empty(), "sem título");
        assert_eq!(filename_mod_ids("Better Menu-577-1-0-3-1712345678.ZIP"), vec![577]);
    }
    #[test]
    fn md5_confirmation_requires_the_exact_file_of_the_game() {
        let result = serde_json::json!([lookup(577, 10, "palworld"), lookup(600, 11, "palworld")]);
        assert!(md5_lists_file(&result, "palworld", 577, 10));
        assert!(!md5_lists_file(&result, "palworld", 577, 11));
        assert!(!md5_lists_file(&result, "skyrim", 577, 10));
        assert!(!md5_lists_file(&serde_json::json!({"error": "not found"}), "palworld", 577, 10));
    }
    #[test]
    fn filename_match_requires_one_exact_name_and_size() {
        let files = serde_json::json!({ "files": [
            { "file_id": 10, "file_name": "Mod-577-1-0-1700000000.zip", "version": "1.0", "size_in_bytes": 1234 },
            { "file_id": 11, "file_name": "Mod-577-1-1-1700000001.zip", "version": "1.1", "size_in_bytes": 999 },
        ]});
        assert_eq!(filename_match(&files, "mod-577-1-0-1700000000.zip", Some(1234)), Some((10, "1.0".into())));
        assert_eq!(filename_match(&files, "Mod-577-1-0-1700000000.zip", Some(1)), None, "tamanho diferente");
        assert_eq!(filename_match(&files, "Mod-577-1-0-1700000000 (1).zip", Some(1234)), None, "nome diferente");
        let duplicated = serde_json::json!({ "files": [
            { "file_id": 10, "file_name": "Mod.zip" }, { "file_id": 12, "file_name": "Mod.zip" } ]});
        assert_eq!(filename_match(&duplicated, "Mod.zip", None), None, "ambíguo");
        assert_eq!(mod_title(&serde_json::json!({"mod_id":577,"domain_name":"palworld","name":"Better Menu","available":true}), "palworld", 577).as_deref(), Some("Better Menu"));
        assert!(mod_title(&serde_json::json!({"mod_id":578,"domain_name":"palworld","name":"Other"}), "palworld", 577).is_none());
        assert!(mod_title(&serde_json::json!({"mod_id":577,"domain_name":"skyrim","name":"Other"}), "palworld", 577).is_none());
        assert!(mod_title(&serde_json::json!({"mod_id":577,"domain_name":"palworld","name":"Hidden","available":false}), "palworld", 577).is_none());
    }
    fn lookup(mod_id: u64, file_id: u64, domain: &str) -> serde_json::Value {
        serde_json::json!({"mod":{"mod_id":mod_id,"domain_name":domain,"name":"TweakXL","version":"newest"},"file_details":{"file_id":file_id,"version":"1.11.4"}})
    }
    #[test]
    fn hash_lookup_uses_exact_file_version_and_collapses_duplicate_results() {
        let value = lookup(4197, 10, "cyberpunk2077");
        assert_eq!(unique_archive_match(&serde_json::json!([value.clone(), value]), "cyberpunk2077").unwrap(), Some((4197,10,"TweakXL".into(),"1.11.4".into())));
    }
    #[test]
    fn hash_lookup_does_not_guess_between_uploads_or_other_games() {
        assert!(unique_archive_match(&serde_json::json!([lookup(1,10,"cyberpunk2077"),lookup(2,20,"cyberpunk2077")]), "cyberpunk2077").unwrap().is_none());
        assert!(unique_archive_match(&serde_json::json!([lookup(1,10,"valheim")]), "cyberpunk2077").unwrap().is_none());
        assert!(unique_archive_match(&serde_json::json!([]), "cyberpunk2077").unwrap().is_none());
        assert!(unique_archive_match(&serde_json::json!([lookup(0,10,"cyberpunk2077")]), "cyberpunk2077").is_err());
        assert!(unique_archive_match(&serde_json::json!({}), "cyberpunk2077").is_err());
    }
    fn package(entry: &str, content: &[u8]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("peligames-identity-{}.zip", uuid::Uuid::new_v4()));
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        writer.start_file(entry, zip::write::FileOptions::default()).unwrap();
        writer.write_all(content).unwrap(); writer.finish().unwrap(); path
    }
    #[test]
    fn uses_package_name_and_pinned_version_from_root_manifest() {
        let path = package("manifest.json", br#"{"name":"BepInExPack_Valheim","version_number":"5.4.2351"}"#);
        assert_eq!(package_identity(&path), Some(("BepInExPack Valheim".into(), "5.4.2351".into())));
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn ignores_nested_plugin_names_and_oversized_manifests() {
        let nested = package("plugin/manifest.json", br#"{"name":"OtherPlugin","version_number":"1.0"}"#);
        let large = package("manifest.json", &vec![b' ';65537]);
        assert!(package_identity(&nested).is_none()); assert!(package_identity(&large).is_none());
        fs::remove_file(nested).unwrap(); fs::remove_file(large).unwrap();
    }
    #[test]
    fn existing_library_entries_do_not_require_version_metadata() {
        let item: LocalMod = serde_json::from_value(serde_json::json!({"id":"legacy","name":"Old file","archive":"/tmp/old.zip","size":1})).unwrap();
        assert!(item.version.is_empty()); assert!(!item.installed);
    }
}
pub(super) fn diagnostic_game(game_id: &str) -> Result<NexusGame,String> {
    let _lock = STORAGE.lock().map_err(|e|e.to_string())?;
    load()?.games.into_iter().find(|game|game.id == game_id).ok_or_else(||"Jogo não encontrado.".into())
}
pub(super) async fn import_archive(game_id: String, source: String, nexus_source: Option<NexusSource>) -> Result<Workspace,String> {
    let game = diagnostic_game(&game_id).ok();
    if let Some(game)=&game {super::nexus_reports::event(game,"INFO","importar",Path::new(&source).file_name().and_then(|n|n.to_str()).unwrap_or("arquivo"));}
    let result = import_archive_inner(game_id,source,nexus_source).await;
    if let Some(game)=&game {super::nexus_reports::outcome(game,"importar",&result);}
    result
}
async fn import_archive_inner(
    game_id: String,
    source: String,
    nexus_source: Option<NexusSource>,
) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = load()?;
        let game = data
            .games
            .iter_mut()
            .find(|g| g.id == game_id)
            .ok_or("Jogo não encontrado.")?;
        let path = Path::new(&source);
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !super::nexus_archive::supported_filename(path.file_name().and_then(|name|name.to_str()).unwrap_or("")) {
            return Err("Selecione um pacote ou arquivo de mod reconhecido.".into());
        }
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len()>super::nexus_archive::TOTAL_LIMIT || !["zip","7z","rar"].contains(&extension.as_str()) && metadata.len()>super::nexus_archive::FILE_LIMIT {
            return Err("Selecione um arquivo de mod dentro dos limites: 2 GiB por pacote ou 512 MiB por arquivo solto.".into());
        }
        let id = uuid::Uuid::new_v4().to_string();
        // IDs in persisted metadata must never become arbitrary filesystem paths.
        uuid::Uuid::parse_str(&game.id).map_err(|_| "Identificador inválido.")?;
        let directory = super::nexus_profile::directory(game)?.join("archives");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let destination = directory.join(format!("{id}.{extension}"));
        let package = package_identity(path);
        if let Err(error) = fs::copy(path, &destination) {
            let _ = fs::remove_file(&destination);
            return Err(error.to_string());
        }
        game.mods.push(LocalMod {
            id,
            identification_note: None,
            name: nexus_source.as_ref().filter(|s| !s.name.trim().is_empty()).map(|s| s.name.clone()).or_else(|| package.as_ref().map(|(name,_)|name.clone())).unwrap_or_else(|| path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()),
            version: nexus_source.as_ref().map(|s| s.version.clone()).or_else(||package.as_ref().map(|(_,version)|version.clone())).unwrap_or_default(),
            archive: destination.to_string_lossy().to_string(),
            size: metadata.len(),
            installed: false,
            enabled: false,
            nexus_source,
            fomod_selection: None,
            fomod_files: None,
            installed_files: None,
            manual_root: None,
            replace_originals: false,
        });
        dependency_order(game);
        if let Err(error) = save(&data) {
            let _ = fs::remove_file(&destination);
            return Err(error);
        }
        Ok(data)
    })
    .await
    .map_err(|e| e.to_string())?
}
// Save the new revision before discarding any previously installed files.
fn change_mod(game_id: String, mod_id: String, operation: &str, enabled: bool, selection: Option<Vec<String>>) -> Result<Workspace,String> {
    let game = diagnostic_game(&game_id).ok();
    let action = format!("{operation} enabled={enabled} mod={mod_id}");
    if let Some(game)=&game {let name=game.mods.iter().find(|m|m.id==mod_id).map(|m|m.name.as_str()).unwrap_or("");super::nexus_reports::event(game,"INFO",&action,name);}
    let result=change_mod_inner(game_id,mod_id,operation,enabled,selection);
    if let Some(game)=&game {super::nexus_reports::outcome(game,&action,&result);}
    result
}
fn change_mod_inner(
    game_id: String,
    mod_id: String,
    operation: &str,
    enabled: bool,
    selection: Option<Vec<String>>,
) -> Result<Workspace, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let mut data = load()?;
    let game = data
        .games
        .iter_mut()
        .find(|g| g.id == game_id)
        .ok_or("Jogo não encontrado.")?;
    super::nexus_profile::idle(game)?;
    let index = game
        .mods
        .iter()
        .position(|m| m.id == mod_id)
        .ok_or("Mod não encontrado.")?;
    uuid::Uuid::parse_str(&mod_id).map_err(|_| "Mod inválido.")?;
    let archive_extension = Path::new(&game.mods[index].archive)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !super::nexus_archive::FORMATS.contains(&archive_extension.as_str())
        && super::nexus_patches::parts(&format!("{}.{}", game.mods[index].name, archive_extension)).is_none()
    {
        return Err("Arquivo inválido.".into());
    }
    if operation == "install" {
        game.mods[index].fomod_selection = selection;
        let plan = super::nexus_deployment::plan(game, &mod_id)?;
        if plan.fomod.is_some() && game.mods[index].fomod_selection.is_none() {return Err("Confirme as opções FOMOD na janela de instalação antes de continuar.".into());}
        if !plan.issues.is_empty() { return Err(plan.issues.join("\n")); }
        if !plan.missing.is_empty() { return Err("Instale as dependências indicadas antes deste mod.".into()); }
        if !plan.originals.is_empty() && !game.mods[index].replace_originals { return Err("Este mod substitui arquivos originais do jogo. Confirme “Instalar como substituição” na janela de instalação.".into()); }
        game.mods[index].installed_files = Some(plan.files.clone());
        game.mods[index].fomod_files = plan.fomod.is_some().then_some(plan.files);
        game.mods[index].fomod_selection = Some(plan.selection);
    }
    let old_profile = game.profile.clone();
    let rebuild = game.mods[index].installed || !matches!(operation, "remove" | "move_up" | "move_down");
    match operation {
        "remove" => {
            game.mods.remove(index);
        }
        "install" => {
            game.mods[index].installed = true;
            game.mods[index].enabled = true;
        }
        "toggle" => {
            if !game.mods[index].installed {
                return Err("Instale o mod antes de ativá-lo.".into());
            }
            game.mods[index].enabled = enabled;
        }
        // Lower in the list = higher priority when two mods supply the same file.
        "move_up" | "move_down" => {
            let other = if operation == "move_up" { index.checked_sub(1) } else { Some(index + 1).filter(|i| *i < game.mods.len()) };
            let Some(other) = other else { return Err("O mod já está no limite da lista.".into()); };
            game.mods.swap(index, other);
        }
        _ => return Err("Operação inválida.".into()),
    }
    dependency_order(game);
    if matches!(operation, "move_up" | "move_down") && game.mods.iter().position(|item| item.id == mod_id) == Some(index) {
        return Err("Os requisitos precisam permanecer antes dos mods que dependem deles.".into());
    }
    if rebuild {
        game.profile = Some(if (operation == "remove" || operation == "toggle" && !enabled) && super::nexus_deployment::supports(game) {
            super::nexus_deployment::build_after_removal(game)?
        } else { super::nexus_profile::build(game)? });
    }
    let updated_game = game.clone();
    let deployment = if rebuild && super::nexus_deployment::supports(&updated_game) {
        match super::nexus_deployment::apply(&updated_game) {
            Ok(transaction) => Some(transaction),
            Err(error) => {
                if let Some(revision) = &updated_game.profile {
                    super::nexus_profile::discard(&updated_game, revision);
                }
                return Err(error);
            }
        }
    } else {
        None
    };
    if let Err(error) = save(&data) {
        if rebuild {
            if let Some(revision) = &updated_game.profile {
                super::nexus_profile::discard(&updated_game, revision);
            }
        }
        return Err(error);
    }
    if let Some(transaction) = deployment {
        transaction.commit();
    }
    if rebuild {
        if let Some(revision) = old_profile {
            super::nexus_profile::discard(&updated_game, &revision);
        }
    }
    if operation == "remove" {
        let owned = super::nexus_profile::directory(&updated_game)?
            .join("archives")
            .join(format!("{mod_id}.{archive_extension}"));
        let _ = fs::remove_file(owned);
        if let Ok(entries) = fs::read_dir(super::nexus_profile::directory(&updated_game)?.join("archive-cache")) {
            for entry in entries.flatten() { if entry.file_name().to_str().is_some_and(|name|name.starts_with(&format!("{mod_id}-")) && name.ends_with(".zip")) {let _ = fs::remove_file(entry.path());} }
        }
    }
    Ok(data)
}
#[tauri::command]
pub async fn install_nexus_mod(game_id: String, mod_id: String, selection: Option<Vec<String>>) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || change_mod(game_id, mod_id, "install", true, selection))
        .await
        .map_err(|e| e.to_string())?
}
/// Vortex "Install as Replacer": the mod may replace game files, kept in
/// `originals/` and restored when the mod no longer provides them.
#[tauri::command]
pub async fn set_nexus_mod_replace_originals(game_id: String, mod_id: String, enabled: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = load()?;
        let game = data.games.iter_mut().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
        let item = game.mods.iter_mut().find(|item| item.id == mod_id).ok_or("Mod não encontrado.")?;
        if item.installed { return Err("Desinstale o mod antes de mudar esta opção.".into()); }
        item.replace_originals = enabled;
        save(&data)
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_nexus_mod_enabled(
    game_id: String,
    mod_id: String,
    enabled: bool,
) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || change_mod(game_id, mod_id, "toggle", enabled, None))
        .await
        .map_err(|e| e.to_string())?
}
/// Moves a mod one position in the priority order and redeploys when needed.
#[tauri::command]
pub async fn move_nexus_mod(game_id: String, mod_id: String, up: bool) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || change_mod(game_id, mod_id, if up { "move_up" } else { "move_down" }, false, None))
        .await
        .map_err(|e| e.to_string())?
}

/// Files that several enabled mods supply, with the mod deployed for each.
#[tauri::command]
pub async fn nexus_mod_overrides(game_id: String) -> Result<Vec<super::nexus_deployment::Override>, String> {
    tauri::async_runtime::spawn_blocking(move || super::nexus_deployment::overrides(&diagnostic_game(&game_id)?))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn remove_nexus_archive(game_id: String, mod_id: String) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || change_mod(game_id, mod_id, "remove", false, None))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn launch_nexus_game(game_id: String) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || launch_game_blocking(game_id))
        .await.map_err(|error| error.to_string())?
}
fn launch_game_blocking(game_id: String) -> Result<Workspace, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let data = load()?;
    let game = data
        .games
        .iter()
        .find(|g| g.id == game_id)
        .ok_or("Jogo não encontrado.")?;
    let result = super::nexus_profile::launch(game);
    super::nexus_reports::outcome(game,"iniciar jogo",&result);
    result?;
    let mut data = data;
    if let Some(game) = data.games.iter_mut().find(|g| g.id == game_id) {
        game.running = true;
    }
    Ok(data)
}
#[tauri::command]
pub async fn stop_nexus_game(game_id: String) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
        let data = load()?;
        let game = data
            .games
            .iter()
            .find(|g| g.id == game_id)
            .ok_or("Jogo não encontrado.")?;
        super::nexus_profile::stop(game)?;
        Ok(data)

    }).await.map_err(|error| error.to_string())?
}
// library.json is replaced atomically by save(). Log monitors can read its last
// committed snapshot without waiting for deployment or rescanning every game.
/// Clears the launch logs of every Nexus game that is not running, so each
/// launcher start begins with empty logs. Runs off the window thread.
pub fn reset_session_logs_at_startup() {
    let games = match workspace() { Ok(data) => data.games, Err(_) => return };
    for game in games {
        if super::nexus_profile::idle(&game).is_err() { continue; }
        if let Err(error) = super::nexus_profile::clear_session_logs(&game) {
            super::nexus_reports::event(&game, "WARN", "limpar logs ao abrir", &error);
        }
    }
}
pub(super) fn log_game(game_id: &str) -> Result<NexusGame, String> {
    uuid::Uuid::parse_str(game_id).map_err(|_| "Identificador de jogo inválido.")?;
    let bytes = fs::read(root()?.join("library.json")).map_err(|e| e.to_string())?;
    let mut game = serde_json::from_slice::<Workspace>(&bytes).map_err(|e| e.to_string())?
        .games.into_iter().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    game.adapter = game.nexus_domain.clone();
    Ok(game)
}

#[tauri::command]
pub async fn read_nexus_mod_logs(game_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let game = log_game(&game_id)?;
        super::nexus_loader_logs::read(&game).map(|mut text| { text.push_str(&super::nexus_crash_logs::read(&game)); text })
    }).await.map_err(|e|e.to_string())?
}

#[tauri::command]
pub async fn read_nexus_logs(game_id: String, previous: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let game = log_game(&game_id)?;
        super::nexus_profile::logs(&game, previous)
    }).await.map_err(|e|e.to_string())?
}

/// Update only this Nexus library entry; the main launcher keeps its own settings.
#[tauri::command]
pub async fn configure_nexus_settings(
    game_id: String,
    directory: String,
    compat_data: String,
    proton: String,
) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = load()?;
        let game = data.games.iter_mut().find(|g| g.id == game_id).ok_or("Jogo não encontrado.")?;
        if (game.platform == "native" || game.game["native_linux"] == true) && proton != game.proton {
            return Err("Este jogo usa a versão nativa de Linux. A troca de Proton está desabilitada.".into());
        }
        super::nexus_deployment::idle(game)?;
        let requested = fs::canonicalize(directory.trim()).map_err(|e| format!("Pasta do jogo inválida: {e}"))?;
        if !requested.is_dir() { return Err("Selecione uma pasta do jogo válida.".into()); }
        let current = game.game["directory"].as_str().and_then(|p| fs::canonicalize(p).ok());
        let changed = current.as_ref() != Some(&requested);
        if changed && (game.profile.is_some() || game.mods.iter().any(|m| m.installed)) {
            return Err("Remova os mods instalados antes de alterar a pasta do jogo.".into());
        }
        let destination = if changed && !game.adapter.is_empty() {
            let (root, definition) = super::nexus_discovery::resolve(&requested)?;
            if Some(game.adapter.as_str()) != definition.as_ref().map(|value| value.domain.as_str()) {
                return Err("A pasta selecionada pertence a outro jogo.".into());
            }
            root
        } else { requested };
        if game.platform == "proton" {
            let (prefix, runner) = super::nexus_deployment::validate_runner(&compat_data, &proton)?;
            game.compat_data = prefix.to_string_lossy().into();
            game.proton = runner.to_string_lossy().into();
            game.game["prefix"] = game.compat_data.clone().into();
        }
        game.game["directory"] = destination.to_string_lossy().to_string().into();
        save(&data)?;
        Ok(data)

    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn configure_nexus_proton(
    game_id: String,
    compat_data: String,
    proton: String,
) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = load()?;
        let game = data
            .games
            .iter_mut()
            .find(|g| g.id == game_id)
            .ok_or("Jogo não encontrado.")?;
        if game.platform == "native" || game.game["native_linux"] == true {
            return Err("Este jogo usa a versão nativa de Linux. A troca de Proton está desabilitada.".into());
        }
        if !super::nexus_deployment::supports(game) {
            return Err("Este jogo não possui um módulo de implantação de arquivos.".into());
        }
        super::nexus_deployment::idle(game)?;
        let (prefix, runner) = super::nexus_deployment::validate_runner(&compat_data, &proton)?;
        game.compat_data = prefix.to_string_lossy().into();
        game.proton = runner.to_string_lossy().into();
        save(&data)?;
        Ok(data)

    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn configure_nexus_domain(game_id: String, game_domain: String) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        super::nexus_catalog::domain(&game_domain)?;
        let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = load()?;
        let game = data
            .games
            .iter_mut()
            .find(|g| g.id == game_id)
            .ok_or("Jogo não encontrado.")?;
        if !game.adapter.is_empty() && game_domain != game.adapter {
            return Err(
                "Este jogo já foi identificado. Use a página Nexus correspondente ao jogo.".into(),
            );
        }
        game.nexus_domain = game_domain;
        save(&data)?;
        Ok(data)

    }).await.map_err(|error| error.to_string())?
}

#[derive(Serialize)]
pub struct PackageLayout {
    folders: Vec<String>,
    destinations: Vec<String>,
    current: Option<ManualRoot>,
}

/// Folders inside the archive and destinations allowed by the game's module.
#[tauri::command]
pub async fn nexus_package_layout(game_id: String, mod_id: String) -> Result<PackageLayout, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|error| error.to_string())?;
        let data = load()?;
        let game = data.games.iter().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
        let item = game.mods.iter().find(|item| item.id == mod_id).ok_or("Mod não encontrado.")?;
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        let mut archive = super::nexus_archive::open(game, item)?;
        let entries = (0..archive.len()).map(|i| archive.by_index(i).map(|e| e.name().replace('\\', "/")).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?;
        Ok(PackageLayout { folders: super::nexus_deployment::package_folders(&entries), destinations: module.destinations.clone(), current: item.manual_root.clone() })
    }).await.map_err(|error| error.to_string())?
}

/// Stores a manual root without validating it (the caller re-plans).
pub(super) fn set_manual_root_unchecked(game_id: &str, mod_id: &str, root: Option<ManualRoot>) -> Result<(), String> {
    let _lock = STORAGE.lock().map_err(|error| error.to_string())?;
    let mut data = load()?;
    let game = data.games.iter_mut().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    let item = game.mods.iter_mut().find(|item| item.id == mod_id).ok_or("Mod não encontrado.")?;
    if item.installed { return Err("Mod já instalado.".into()); }
    item.manual_root = root;
    save(&data)
}

/// Keeps the mods of a collection in the curator's order (other mods keep their places).
pub(super) fn order_collection(game_id: &str, order: &[(u64, u64)]) -> Result<(), String> {
    let _lock = STORAGE.lock().map_err(|error| error.to_string())?;
    let mut data = load()?;
    let game = data.games.iter_mut().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    let rank = |item: &LocalMod| item.nexus_source.as_ref().filter(|s| s.domain == game.nexus_domain)
        .and_then(|s| order.iter().position(|key| *key == (s.mod_id, s.file_id)));
    let slots: Vec<usize> = (0..game.mods.len()).filter(|i| rank(&game.mods[*i]).is_some()).collect();
    let mut members: Vec<LocalMod> = slots.iter().map(|i| game.mods[*i].clone()).collect();
    members.sort_by_key(|item| rank(item));
    for (slot, item) in slots.into_iter().zip(members) { game.mods[slot] = item; }
    save(&data)
}

/// Saves (or clears) the manual root after checking it produces a valid plan.
#[tauri::command]
pub async fn set_nexus_manual_root(game_id: String, mod_id: String, root: Option<ManualRoot>) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|error| error.to_string())?;
        let mut data = load()?;
        let game = data.games.iter_mut().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
        let item = game.mods.iter_mut().find(|item| item.id == mod_id).ok_or("Mod não encontrado.")?;
        if item.installed { return Err("Remova a instalação do mod antes de alterar a pasta raiz.".into()); }
        item.manual_root = root;
        if game.mods.iter().find(|item| item.id == mod_id).is_some_and(|item| item.manual_root.is_some()) {
            if let Err(error) = super::nexus_deployment::plan(game, &mod_id) {
                super::nexus_reports::event(game, "WARN", &format!("pasta raiz manual mod={mod_id}"), &error);
                return Err(error);
            }
        }
        super::nexus_reports::event(game, "INFO", &format!("pasta raiz manual mod={mod_id}"), "definida");
        save(&data)?;
        Ok(data)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn plan_nexus_installation(game_id: String, mod_id: String, selection: Option<Vec<String>>) -> Result<super::nexus_deployment::InstallationPlan,String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|error|error.to_string())?;
        let mut data = load()?;
        let game = data.games.iter_mut().find(|game|game.id == game_id).ok_or("Jogo não encontrado.")?;
        if let Some(selection) = selection { game.mods.iter_mut().find(|item|item.id == mod_id).ok_or("Mod não encontrado.")?.fomod_selection = Some(selection); }
        let result = super::nexus_deployment::plan(game, &mod_id);
        match &result {
            Err(error) => super::nexus_reports::event(game,"ERROR",&format!("planejar instalação mod={mod_id}"),error),
            Ok(plan) if !plan.issues.is_empty() || !plan.missing.is_empty() => super::nexus_reports::event(game,"WARN",&format!("planejar instalação mod={mod_id}"),&format!("{}; dependências: {}",plan.issues.join("; "),plan.missing.iter().map(|m|m.name.as_str()).collect::<Vec<_>>().join(", "))),
            _ => {}
        }
        result
    }).await.map_err(|error|error.to_string())?
}

#[tauri::command]
pub async fn open_nexus_game_tool(game_id:String,name:String)->Result<(),String>{
    tauri::async_runtime::spawn_blocking(move || {
        let _lock=STORAGE.lock().map_err(|e|e.to_string())?;
        let data=load()?;
        let game=data.games.iter().find(|game|game.id==game_id).ok_or("Jogo não encontrado.")?;
        super::nexus_deployment::launch_tool(game,&name)

    }).await.map_err(|error| error.to_string())?
}

#[derive(Serialize)]
pub struct DeployOption { method: super::nexus_deploy_methods::Method, available: bool, reason: String }
#[tauri::command]
pub async fn nexus_deploy_options(game_id: String) -> Result<Vec<DeployOption>, String> {
    tauri::async_runtime::spawn_blocking(move || deploy_options(&game_id)).await.map_err(|e|e.to_string())?
}
// Filesystem discovery and STORAGE contention must never block the window event loop.
fn deploy_options(game_id: &str) -> Result<Vec<DeployOption>, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let data = load()?;
    let game = data.games.iter().find(|g| g.id == game_id).ok_or("Jogo não encontrado.")?;
    use super::nexus_deploy_methods::{Method, vfs_available};
    let supported = super::nexus_deployment::supports(game);
    use std::os::unix::fs::MetadataExt;
    let profile_base = super::nexus_profile::directory(game)?;
    fs::create_dir_all(&profile_base).map_err(|e|e.to_string())?;
    let same_filesystem = fs::metadata(&profile_base).ok().zip(game.game["directory"].as_str().and_then(|root|fs::metadata(root).ok())).is_some_and(|(profile,root)|profile.dev() == root.dev());
    let vfs_module = super::nexus_deployment::definition(game).is_some_and(|module| module.activation.is_none() && !module.routes.iter().any(|route| match route { super::nexus_modules::Route::Prefix {target,..} | super::nexus_modules::Route::File {target,..} | super::nexus_modules::Route::Extension {target,..} | super::nexus_modules::Route::Manifest {target,..} | super::nexus_modules::Route::Anchor {target,..} | super::nexus_modules::Route::Patch {target} => target.starts_with('@') }));
    Ok([Method::Copy,Method::Symlink,Method::Hardlink,Method::Vfs].into_iter().map(|method| {
        let (available,reason) = if !supported {(false,"Este jogo utiliza um perfil isolado ou não possui módulo de deploy.".to_string())}
        else if method == Method::Hardlink && !same_filesystem {(false,"Hardlink exige que o perfil e o jogo estejam no mesmo sistema de arquivos.".into())}
        else if method == Method::Vfs && !vfs_module {(false,"Este módulo usa ativação especial ou destinos no prefixo; utilize cópia ou links.".into())}
        else if method == Method::Vfs && !vfs_available() {(false,"Requer fuse-overlayfs, fusermount3, bubblewrap e /dev/fuse.".into())}
        else {(true,String::new())};
        DeployOption {method,available,reason}
    }).collect())
}
#[tauri::command]
pub async fn configure_nexus_deploy(game_id: String, method: super::nexus_deploy_methods::Method) -> Result<Workspace,String> {
    tauri::async_runtime::spawn_blocking(move || {
        let option = deploy_options(&game_id)?.into_iter().find(|option|option.method == method).ok_or("Método inválido.")?;
        if !option.available {return Err(option.reason);}
        let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = load()?;
        let game = data.games.iter_mut().find(|game|game.id == game_id).ok_or("Jogo não encontrado.")?;
        super::nexus_profile::idle(game)?;
        if game.deploy_method == method {return Ok(data);}
        game.deploy_method = method;
        let old = game.profile.clone();
        let tx = if old.is_some() {
            game.profile = Some(super::nexus_profile::build(game)?);
            match super::nexus_deployment::apply(game) {
                Ok(tx) => Some(tx),
                Err(error) => {super::nexus_profile::discard(game,game.profile.as_deref().unwrap());return Err(error);}
            }
        } else {None};
        let updated = game.clone();
        if let Err(error) = save(&data) { drop(tx); if old.is_some() {super::nexus_profile::discard(&updated,updated.profile.as_deref().unwrap());} return Err(error); }
        if let Some(tx) = tx {tx.commit();}
        if let Some(old) = old {super::nexus_profile::discard(&updated,&old);}
        Ok(data)
    }).await.map_err(|e|e.to_string())?
}

pub(super) fn save_launch_options(game_id: &str, options: Option<String>) -> Result<Workspace, String> {
    let _lock = STORAGE.lock().map_err(|error| error.to_string())?;
    let mut data = load()?;
    let game = data.games.iter_mut().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    super::nexus_profile::idle(game)?;
    game.launch_options = options;
    save(&data)?;
    Ok(data)
}

#[cfg(test)]
mod prune_tests {
    use super::*;
    fn game(launcher: &str, directory: &Path, installed: bool) -> NexusGame {
        serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "platform": "proton",
            "game": {"name": launcher, "launcher": launcher, "directory": directory},
            "mods": if installed { serde_json::json!([{"id": "m", "name": "m", "archive": "/x.zip", "size": 1, "installed": true}]) } else { serde_json::json!([]) },
        })).unwrap()
    }
    #[test]
    fn rescan_drops_unsupported_automatic_games_but_keeps_manual_and_installed_ones() {
        let base = std::env::temp_dir().join(format!("peligames-prune-{}", uuid::Uuid::new_v4()));
        let (unsupported, supported, mele) = (base.join("Other"), base.join("Valheim"), base.join("MELE"));
        fs::create_dir_all(&unsupported).unwrap();
        fs::create_dir_all(supported.join("valheim_Data")).unwrap();
        fs::write(supported.join("valheim.x86_64"), b"\x7fELFfixture").unwrap();
        fs::write(supported.join("valheim_Data/globalgamemanagers"), b"fixture").unwrap();
        let mut games = vec![
            game("Steam", &unsupported, false), game("Heroic", &unsupported, false), game("Lutris", &unsupported, false),
            game("Faugus", &unsupported, false), game("Bottles", &unsupported, false), game("PeliGames", &unsupported, false),
            game("Steam", &base.join("gone"), false),
            game("Manual", &unsupported, false), game("Steam", &supported, false), game("Heroic", &mele, true),
        ];
        let kept = prune_unsupported(&mut games);
        assert_eq!(kept.len(), 1);
        let left: Vec<&str> = games.iter().map(|g| g.game["launcher"].as_str().unwrap()).collect();
        assert_eq!(left, ["Manual", "Steam", "Heroic"]);
        assert_eq!(games[1].game["directory"].as_str(), supported.to_str());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn manual_removal_requires_no_installed_mods() {
        let folder = std::env::temp_dir();
        assert!(removable(&game("Manual", &folder, false)).is_ok());
        assert!(removable(&game("Manual", &folder, true)).unwrap_err().contains("mods instalados"));
    }
}

pub(super) fn save_advanced(game_id: &str, settings: crate::core::advanced_settings::AdvancedSettings) -> Result<Workspace,String> {
    settings.validate()?;
    let _lock = STORAGE.lock().map_err(|e|e.to_string())?;
    let mut data = load()?;
    let game = data.games.iter_mut().find(|g|g.id==game_id).ok_or("Jogo não encontrado.")?;
    super::nexus_profile::idle(game)?;
    // DLL priorities now belong to the dedicated editor. Clearing them must also
    // remove overrides saved by the previous launch-options editor.
    if settings.options.get("dll_overrides") != Some(&true) {
        if let Some(value) = game.launch_options.as_deref() {
            if let Some(cleaned) = super::nexus_launch_options::without_saved_dll_overrides(value)? {
                game.launch_options = if cleaned.is_empty() { None } else { Some(cleaned) };
            }
        }
    }
    let path = game.game["path"].as_str().or(game.game["directory"].as_str()).ok_or("Instalação indisponível.")?;
    let name = game.game["name"].as_str().unwrap_or("Jogo");
    super::launch_profiles::refresh(std::path::Path::new(path), name, &settings)?;
    game.advanced = settings;
    save(&data)?;
    Ok(data)
}

/// Registered context for desktop shortcuts and Wine tools; never trusts a caller-supplied prefix.
pub(crate) fn maintenance_entry(game_id: &str) -> Result<(String, String, String, String), String> {
    let game = diagnostic_game(game_id)?;
    Ok((game.game.get("name").and_then(|v|v.as_str()).unwrap_or("Jogo").into(), game.compat_data, game.proton,
        game.game.get("path").and_then(|v|v.as_str()).unwrap_or_default().into()))
}
