//! Nexus archive library and transactional, isolated mod profiles.
use crate::core::paths;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

static STORAGE: Mutex<()> = Mutex::new(());
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) nexus_source: Option<NexusSource>,
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
#[derive(Default, Serialize, Deserialize)]
pub struct Workspace {
    pub(super) games: Vec<NexusGame>,
}
pub(super) fn root() -> Result<PathBuf, String> {
    let path = paths::app_root()?.join("Mod/Nexus");
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
fn load() -> Result<Workspace, String> {
    let path = root()?.join("library.json");
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Workspace>(&bytes)
            .map_err(|e| format!("Biblioteca Nexus inválida: {e}"))
            .and_then(|mut data| {
                let mut paths_changed = false;
                for game in &mut data.games {
                    let storage = super::nexus_profile::directory(game)?;
                    let legacy = root()?.join("games").join(&game.id);
                    for item in &mut game.mods {
                        if let Ok(relative) = Path::new(&item.archive).strip_prefix(&legacy) {
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
                    if game.adapter == "palworld" {
                        if let Some(root) = game.game["directory"].as_str() {
                            game.detected_mods = super::nexus_scan::palworld(Path::new(root))
                                .into_iter().map(|(name, enabled)| DetectedMod { name, enabled, mod_id: None }).collect();
                        }
                    }
                    if let Some(module) = game.game["directory"].as_str().and_then(|root| super::nexus_discovery::at(Path::new(root))) {
                        let root = Path::new(game.game["directory"].as_str().unwrap());
                        let active_profile = super::nexus_profile::profile(game).ok();
                        for framework in module.frameworks.iter().filter(|framework| framework.installed(root) || active_profile.as_deref().is_some_and(|profile|framework.installed(profile))) {
                            game.detected_mods.push(DetectedMod {name:framework.name.clone(),enabled:true,mod_id:Some(framework.mod_id)});
                        }
                    }
                    game.running = super::nexus_profile::running(game);
                }
                if paths_changed { save(&data)?; }
                Ok(data)
            }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Workspace::default()),
        Err(e) => Err(e.to_string()),
    }
}
fn save(data: &Workspace) -> Result<(), String> {
    let directory = root()?;
    let temporary = directory.join(format!("library-{}.tmp", uuid::Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    if let Err(error) = fs::rename(&temporary, directory.join("library.json")) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}
#[tauri::command]
pub fn load_nexus_workspace() -> Result<Workspace, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    load()
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
                Some(source) => super::covers::cache_heroic_cover(&app, source).await.ok()
                    .and_then(|path| reqwest::Url::from_file_path(path).ok()).map(|url| serde_json::Value::String(url.to_string())).unwrap_or_default(),
                None => serde_json::Value::Null,
            };
        }
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = STORAGE.lock().map_err(|error| error.to_string())?;
        let mut data = load()?;
        for mut game in candidates {
            let source = game["directory"].as_str().or(game["path"].as_str()).unwrap_or("");
            let Ok((directory, definition)) = super::nexus_discovery::resolve(Path::new(source)) else { continue };
            // Deduplicate installations, preserving IDs, profiles, manually selected settings and mods.
            if data.games.iter().any(|existing| existing.game["directory"].as_str()
                .is_some_and(|root| fs::canonicalize(root).ok().as_ref() == Some(&directory))) { continue; }
            game["directory"] = directory.to_string_lossy().to_string().into();
            game["library_view"] = "nexus".into();
            let mut compat_data = game["prefix"].as_str()
                .and_then(|prefix| super::nexus_prefix::resolve(Path::new(prefix)).ok())
                .map(|(prefix, _)| prefix.to_string_lossy().into_owned()).unwrap_or_default();
            if compat_data.is_empty() && game["launcher"] == "Steam" {
                if let (Some(steamapps), Some(id)) = (directory.parent().and_then(Path::parent), game["app_id"].as_str()) {
                    if id.bytes().all(|byte| byte.is_ascii_digit()) {
                        let prefix = steamapps.join("compatdata").join(id);
                        if let Ok((prefix, _)) = super::nexus_prefix::resolve(&prefix) { compat_data = prefix.to_string_lossy().into_owned(); }
                    }
                }
            }
            let platform = definition.as_ref().map(|item| item.platform.as_str()).unwrap_or("proton").to_string();
            let proton = game["proton"].as_str().unwrap_or("").to_string();
            let mut entry = NexusGame {
                id: uuid::Uuid::new_v4().to_string(), game, platform, deploy_method: Default::default(), mods: Vec::new(), profile: None,
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

#[tauri::command]
pub fn set_nexus_game_cover(game_id: String, name: String, cover_url: String, previous_cover: Option<String>) -> Result<Workspace, String> {
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
            module_version: found.as_ref().map(|definition| definition.version.clone()),
            notes: found.as_ref().map(|definition| definition.notes.clone()).unwrap_or_default(),
            isolated_profile: found.is_some_and(|definition| definition.engine == super::nexus_modules::Engine::ValheimProfile),
        })
    }).await.map_err(|_| "Falha ao identificar a pasta do jogo.".to_string())?
}
#[tauri::command]
pub fn add_nexus_game(mut game: serde_json::Value, platform: String) -> Result<Workspace, String> {
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
        return Err("Este jogo já está na biblioteca Nexus.".into());
    }
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
    let imported = import_archive(game_id.clone(), source, None).await?;
    let item = imported.games.iter().find(|g| g.id == game_id)
        .and_then(|g| g.mods.last())
        .ok_or("Arquivo importado não encontrado.")?.clone();
    // Identify the managed copy, never the user's file after it has been copied.
    let identity = identify_local_archive(&before.nexus_domain, &item.archive).await;
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
    Ok(data)
}

async fn identify_local_archive(domain: &str, archive: &str) -> Result<Option<NexusSource>, String> {
    let domain = super::nexus_catalog::domain(domain)?.to_string();
    let archive = archive.to_string();
    let hash = tauri::async_runtime::spawn_blocking(move || archive_md5(Path::new(&archive))).await.map_err(|e| e.to_string())??;
    let result = super::nexus_account::metadata(&format!("games/{domain}/mods/md5_search/{hash}.json")).await?;
    let Some((mod_id, file_id, name, version)) = unique_archive_match(&result, &domain)? else { return Ok(None); };
    let requirements = super::nexus_catalog::requirements(&domain, mod_id).await;
    Ok(Some(NexusSource { domain, mod_id, file_id, name, version, requirements }))
}

fn archive_md5(path: &Path) -> Result<String, String> {
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
        });
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
        game.mods[index].fomod_files = plan.fomod.is_some().then_some(plan.files);
        game.mods[index].fomod_selection = Some(plan.selection);
    }
    let old_profile = game.profile.clone();
    let rebuild = game.mods[index].installed || operation != "remove";
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
        _ => return Err("Operação inválida.".into()),
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
#[tauri::command]
pub async fn remove_nexus_archive(game_id: String, mod_id: String) -> Result<Workspace, String> {
    tauri::async_runtime::spawn_blocking(move || change_mod(game_id, mod_id, "remove", false, None))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn launch_nexus_game(game_id: String) -> Result<Workspace, String> {
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
pub fn stop_nexus_game(game_id: String) -> Result<Workspace, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let data = load()?;
    let game = data
        .games
        .iter()
        .find(|g| g.id == game_id)
        .ok_or("Jogo não encontrado.")?;
    super::nexus_profile::stop(game)?;
    Ok(data)
}
#[tauri::command]
pub fn read_nexus_mod_logs(game_id: String) -> Result<String, String> {
    let _lock = STORAGE.lock().map_err(|e|e.to_string())?;
    let data = load()?;
    let game = data.games.iter().find(|game|game.id == game_id).ok_or("Jogo não encontrado.")?;
    super::nexus_loader_logs::read(game)
}

#[tauri::command]
pub fn read_nexus_logs(game_id: String, previous: bool) -> Result<String, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let data = load()?;
    let game = data
        .games
        .iter()
        .find(|g| g.id == game_id)
        .ok_or("Jogo não encontrado.")?;
    super::nexus_profile::logs(game, previous)
}

/// Update only this Nexus library entry; the main launcher keeps its own settings.
#[tauri::command]
pub fn configure_nexus_settings(
    game_id: String,
    directory: String,
    compat_data: String,
    proton: String,
) -> Result<Workspace, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let mut data = load()?;
    let game = data.games.iter_mut().find(|g| g.id == game_id).ok_or("Jogo não encontrado.")?;
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
}

#[tauri::command]
pub fn configure_nexus_proton(
    game_id: String,
    compat_data: String,
    proton: String,
) -> Result<Workspace, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let mut data = load()?;
    let game = data
        .games
        .iter_mut()
        .find(|g| g.id == game_id)
        .ok_or("Jogo não encontrado.")?;
    if !super::nexus_deployment::supports(game) {
        return Err("Este jogo não possui um módulo de implantação de arquivos.".into());
    }
    super::nexus_deployment::idle(game)?;
    let (prefix, runner) = super::nexus_deployment::validate_runner(&compat_data, &proton)?;
    game.compat_data = prefix.to_string_lossy().into();
    game.proton = runner.to_string_lossy().into();
    save(&data)?;
    Ok(data)
}

#[tauri::command]
pub fn configure_nexus_domain(game_id: String, game_domain: String) -> Result<Workspace, String> {
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
pub fn open_nexus_game_tool(game_id:String,name:String)->Result<(),String>{
    let _lock=STORAGE.lock().map_err(|e|e.to_string())?;
    let data=load()?;
    let game=data.games.iter().find(|game|game.id==game_id).ok_or("Jogo não encontrado.")?;
    super::nexus_deployment::launch_tool(game,&name)
}

#[derive(Serialize)]
pub struct DeployOption { method: super::nexus_deploy_methods::Method, available: bool, reason: String }
#[tauri::command]
pub fn nexus_deploy_options(game_id: String) -> Result<Vec<DeployOption>, String> {
    let _lock = STORAGE.lock().map_err(|e| e.to_string())?;
    let data = load()?;
    let game = data.games.iter().find(|g| g.id == game_id).ok_or("Jogo não encontrado.")?;
    use super::nexus_deploy_methods::{Method, vfs_available};
    let supported = super::nexus_deployment::supports(game);
    use std::os::unix::fs::MetadataExt;
    let profile_base = super::nexus_profile::directory(game)?;
    fs::create_dir_all(&profile_base).map_err(|e|e.to_string())?;
    let same_filesystem = fs::metadata(&profile_base).ok().zip(game.game["directory"].as_str().and_then(|root|fs::metadata(root).ok())).is_some_and(|(profile,root)|profile.dev() == root.dev());
    let vfs_module = super::nexus_deployment::definition(game).is_some_and(|module| module.activation.is_none() && !module.routes.iter().any(|route| match route { super::nexus_modules::Route::Prefix {target,..} | super::nexus_modules::Route::File {target,..} | super::nexus_modules::Route::Extension {target,..} | super::nexus_modules::Route::Manifest {target,..} | super::nexus_modules::Route::Patch {target} => target.starts_with('@') }));
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
        let option = nexus_deploy_options(game_id.clone())?.into_iter().find(|option|option.method == method).ok_or("Método inválido.")?;
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
