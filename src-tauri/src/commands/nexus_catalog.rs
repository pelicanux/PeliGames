//! Read-only Nexus metadata; browser downloads never carry the personal API key.
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
static CACHE: Mutex<Option<HashMap<String, (Instant, serde_json::Value)>>> = Mutex::const_new(None);
pub(super) fn domain(value: &str) -> Result<&str, String> {
    if value.is_empty()
        || value.len() > 100
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("Associação Nexus inválida.".into());
    }
    Ok(value)
}
async fn query(path: String) -> Result<serde_json::Value, String> {
    let mut cache = CACHE.lock().await;
    let entries = cache.get_or_insert_with(HashMap::new);
    if let Some((time, data)) = entries.get(&path) {
        if time.elapsed() < Duration::from_secs(300) {
            return Ok(data.clone());
        }
    }
    let data = super::nexus_account::metadata(&path).await?;
    entries.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(300));
    if entries.len() >= 100 {
        entries.clear();
    }
    entries.insert(path, (Instant::now(), data.clone()));
    Ok(data)
}
#[derive(Serialize, Deserialize)]
pub struct CatalogGame {
    name: String,
    domain_name: String,
    #[serde(default)]
    id: u64,
    #[serde(default)]
    vortex_supported: bool,
}

// A Nexus catalog entry alone does not prove Vortex game support. Keep an
// independently verified snapshot for offline use; refresh community extensions
// from the official public manifest without sending the user's API key.
async fn vortex_support() -> (Vec<String>, Vec<u64>) {
    let snapshot: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../resources/nexus-games/vortex-support.json"
    )).expect("Invalid Vortex support snapshot");
    let domains = serde_json::from_value(snapshot["domains"].clone()).unwrap();
    let mut ids: Vec<u64> = serde_json::from_value(snapshot["extension_game_ids"].clone()).unwrap();
    if let Ok(client) = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none()).build() {
        if let Ok(response) = client.get("https://api.nexusmods.com/v3/vortex/extensions").send().await {
            if response.status().is_success() {
                if let Ok(value) = response.json::<serde_json::Value>().await {
                    if let Some(extensions) = value.pointer("/data/extensions").and_then(|v| v.as_array()) {
                        ids.extend(extensions.iter().filter(|e| e["type"] == "game")
                            .filter_map(|e| e["game_id"].as_str().and_then(|id| id.parse::<u64>().ok())
                                .or_else(|| e["game_id"].as_u64())).filter(|id| *id > 0));
                    }
                }
            }
        }
    }
    (domains, ids)
}
#[derive(Serialize, Deserialize)]
pub struct CatalogMod {
    mod_id: u64,
    #[serde(default, deserialize_with = "nullable_text")]
    name: String,
    #[serde(default, deserialize_with = "nullable_text")]
    summary: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    picture_url: Option<String>,
    #[serde(default, deserialize_with = "nullable_text")]
    author: String,
    #[serde(default, deserialize_with = "nullable_text")]
    version: String,
    #[serde(default)]
    downloads: Option<u64>,
}
#[derive(Serialize)]
pub struct CatalogPage {
    mods: Vec<CatalogMod>,
    total_count: u64,
    next_offset: u32,
}
#[tauri::command]
pub async fn list_nexus_catalog_page(game_domain: String, feed: String, offset: u32, count: Option<u32>, search: Option<String>) -> Result<CatalogPage, String> {
    domain(&game_domain)?;
    let count = count.unwrap_or(50);
    if ![20, 30, 40, 50].contains(&count) { return Err("Quantidade de mods inválida.".into()); }
    if !["most_downloaded", "all"].contains(&feed.as_str()) || offset > i32::MAX as u32 - 50 {
        return Err("Página Nexus inválida.".into());
    }
    let search = search.unwrap_or_default().trim().to_owned();
    if search.chars().count() > 200 { return Err("Busca Nexus muito longa.".into()); }
    let key = serde_json::json!(["catalog", game_domain, feed, offset, count, search]).to_string();
    let mut cache = CACHE.lock().await;
    let entries = cache.get_or_insert_with(HashMap::new);
    let data = match entries.get(&key).filter(|(time, _)| time.elapsed() < Duration::from_secs(300)) {
        Some((_, data)) => data.clone(),
        None => {
            let data = super::nexus_account::catalog_page(&game_domain, offset, count, feed == "most_downloaded", &search).await?;
            entries.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(300));
            if entries.len() >= 100 { entries.clear(); }
            entries.insert(key, (Instant::now(), data.clone()));
            data
        }
    };
    let total_count = data["totalCount"].as_u64().ok_or("Contagem Nexus inválida.")?;
    let mut mods: Vec<CatalogMod> = serde_json::from_value(data["nodes"].clone()).map_err(|_| "Página Nexus inválida.")?;
    let next_offset = if mods.is_empty() { total_count.min(u32::MAX as u64) as u32 } else { offset + mods.len() as u32 };
    mods.retain(|item| item.mod_id > 0 && !item.name.is_empty());
    Ok(CatalogPage { mods, total_count, next_offset })
}
#[derive(Serialize, Deserialize)]
pub struct CatalogFile {
    pub(super) file_id: u64,
    pub(super) name: String,
    pub(super) file_name: String,
    #[serde(default)]
    is_primary: bool,
    #[serde(default, deserialize_with = "nullable_text")]
    version: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    category_name: Option<String>,
    #[serde(default)]
    category_id: Option<u64>,
    #[serde(default)]
    description: Option<String>,
}
#[tauri::command]
pub async fn list_nexus_catalog_games() -> Result<Vec<CatalogGame>, String> {
    let cache_file = super::nexus_local::root()?.join("catalog-games.json");
    let (value, fresh) = match query("games.json".into()).await {
        Ok(value) => (value, true),
        Err(error) => {
            let bytes = std::fs::read(&cache_file).map_err(|_| error)?;
            (serde_json::from_slice(&bytes).map_err(|_| "Cache de jogos Nexus inválido.")?, false)
        }
    };
    let mut games: Vec<CatalogGame> = serde_json::from_value(value)
        .map_err(|_| "Lista de jogos Nexus inválida.")?;
    games.retain(|g| domain(&g.domain_name).is_ok());
    if fresh {
        let (domains, ids) = vortex_support().await;
        for game in &mut games {
            game.vortex_supported = domains.contains(&game.domain_name)
                || (game.id > 0 && ids.contains(&game.id));
        }
    }
    games.sort_by_key(|g| g.name.to_lowercase());
    if fresh {
        // Public catalog metadata only; never persist the API key in this cache.
        let temp = cache_file.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        if let Ok(bytes) = serde_json::to_vec(&games) {
            if std::fs::write(&temp, bytes).is_ok() { let _ = std::fs::rename(&temp, &cache_file); }
            let _ = std::fs::remove_file(temp);
        }
    }
    Ok(games)
}
#[tauri::command]
pub async fn list_nexus_catalog_mods(
    game_domain: String,
    feed: String,
) -> Result<Vec<CatalogMod>, String> {
    domain(&game_domain)?;
    if !["trending", "latest_added", "latest_updated"].contains(&feed.as_str()) {
        return Err("Categoria Nexus inválida.".into());
    }
    let mut mods: Vec<CatalogMod> =
        serde_json::from_value(query(format!("games/{game_domain}/mods/{feed}.json")).await?)
            .map_err(|_| "Lista de mods Nexus inválida.")?;
    // Nexus includes unavailable records without display metadata in some feeds.
    mods.retain(|item| item.mod_id > 0 && !item.name.is_empty());
    Ok(mods)
}
#[derive(Serialize)]
pub struct ModDetails {
    info: CatalogMod,
    pub(super) files: Vec<CatalogFile>,
    pub(super) requirements: Requirements,
    requirements_file_id: Option<u64>,
}
async fn verified_mod_info(game_domain: &str, mod_id: u64) -> Result<CatalogMod, String> {
    domain(game_domain)?;
    if mod_id == 0 { return Err("Mod inválido.".into()); }
    let info: CatalogMod = serde_json::from_value(query(format!("games/{game_domain}/mods/{mod_id}.json")).await?)
        .map_err(|_| "Descrição Nexus inválida.")?;
    if info.mod_id != mod_id || info.name.trim().is_empty() {
        return Err("A identificação do mod no Nexus não corresponde ao link solicitado.".into());
    }
    super::nexus_modules::verify_framework_identity(game_domain, mod_id, &info.name)?;
    Ok(info)
}
#[tauri::command]
pub async fn get_nexus_catalog_mod(game_domain: String, mod_id: u64, file_id: Option<u64>) -> Result<ModDetails, String> {
    let info = verified_mod_info(&game_domain, mod_id).await?;
    let data = query(format!("games/{game_domain}/mods/{mod_id}/files.json")).await?;
    let mut files: Vec<CatalogFile> =
        serde_json::from_value(data.get("files").cloned().unwrap_or(data))
            .map_err(|_| "Lista de arquivos Nexus inválida.")?;
    files.retain(|f| f.file_id > 0 && !matches!(f.category_id, Some(6 | 7)));
    let requirements_file_id = select_requirements_file(&files, file_id).ok();
    let requirements = requirements_for_file(&game_domain, mod_id, file_id).await;
    Ok(ModDetails {
        info,
        files,
        requirements,
        requirements_file_id,
    })
}

fn nullable_text<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Option::<String>::deserialize(deserializer).map(|value| value.unwrap_or_default())
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub name: String,
    pub notes: String,
    pub kind: String,
    pub url: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Requirements {
    pub items: Vec<Requirement>,
    pub complete: bool,
    pub error: Option<String>,
}
pub(super) async fn requirements_for_file(game_domain: &str, mod_id: u64, file_id: Option<u64>) -> Requirements {
    match fetch_requirements(game_domain, mod_id, file_id).await {
        Ok(value) => value,
        Err(error) => Requirements {
            items: Vec::new(),
            complete: false,
            error: Some(error),
        },
    }
}
fn safe_url(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    Some(url.to_string())
}
async fn fetch_requirements(game_domain: &str, mod_id: u64, file_id: Option<u64>) -> Result<Requirements, String> {
    domain(game_domain)?;
    let metadata = query(format!("games/{game_domain}/mods/{mod_id}.json")).await?;
    let game_id = metadata["game_id"]
        .as_u64()
        .ok_or("Identificação Nexus indisponível.")?;
    let key = format!("requirements-v2/{game_domain}/{mod_id}");
    let cached = CACHE
        .lock()
        .await
        .as_ref()
        .and_then(|c| c.get(&key))
        .filter(|(time, _)| time.elapsed() < Duration::from_secs(300))
        .map(|(_, v)| v.clone());
    let data = match cached {
        Some(value) => value,
        None => {
            let value = super::nexus_account::mod_requirements(game_id, mod_id).await?;
            let mut cache = CACHE.lock().await;
            let entries = cache.get_or_insert_with(HashMap::new);
            entries.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(300));
            if entries.len() >= 100 {
                entries.clear();
            }
            entries.insert(key, (Instant::now(), value.clone()));
            value
        }
    };
    match data["legacyModRequirementsEnabled"].as_bool() {
        Some(true) => {},
        Some(false) => {
            let files = query(format!("games/{game_domain}/mods/{mod_id}/files.json")).await?;
            let files: Vec<CatalogFile> = serde_json::from_value(files["files"].clone())
                .map_err(|_| "Lista de arquivos Nexus inválida.")?;
            let selected = select_requirements_file(&files, file_id)?;
            let key = format!("file-requirements/{game_domain}/{mod_id}/{selected}");
            let cached = CACHE.lock().await.as_ref().and_then(|c| c.get(&key))
                .filter(|(time, _)| time.elapsed() < Duration::from_secs(300)).map(|(_, v)| v.clone());
            let value = match cached {
                Some(value) => value,
                None => {
                    let value = super::nexus_account::file_requirements(game_domain, selected).await?;
                    let mut cache = CACHE.lock().await;
                    let entries = cache.get_or_insert_with(HashMap::new);
                    entries.retain(|_, (time, _)| time.elapsed() < Duration::from_secs(300));
                    if entries.len() >= 100 { entries.clear(); }
                    entries.insert(key, (Instant::now(), value.clone()));
                    value
                }
            };
            return parse_file_requirements(&value);
        },
        None => return Err("Formato dos requisitos Nexus não identificado.".into()),
    }
    let data = &data["modRequirements"];
    let nodes = data
        .pointer("/nexusRequirements/nodes")
        .and_then(|v| v.as_array())
        .ok_or("Lista de requisitos inválida.")?;
    let total = data
        .pointer("/nexusRequirements/totalCount")
        .and_then(|v| v.as_u64())
        .ok_or("Contagem de requisitos inválida.")?;
    let dlcs = data["dlcRequirements"]
        .as_array()
        .ok_or("Lista de DLCs inválida.")?;
    // API gameId is a numeric Nexus ID, not necessarily a domain name.
    let other_games = if nodes.iter().any(|n| {
        n["externalRequirement"] == false && numeric_id(&n["gameId"]) != Some(game_id)
    }) {
        query("games.json".into()).await.ok()
    } else {
        None
    };
    let mut items = Vec::new();
    for node in nodes {
        let external = node["externalRequirement"]
            .as_bool()
            .ok_or("Requisito inválido.")?;
        let name = node["modName"]
            .as_str()
            .ok_or("Nome de requisito ausente.")?
            .to_string();
        let notes = node["notes"].as_str().unwrap_or_default().to_string();
        let url = if external {
            safe_url(node["url"].as_str().unwrap_or_default())
        } else {
            let target_game = numeric_id(&node["gameId"]);
            let target_mod = numeric_id(&node["modId"])
                .filter(|v| *v > 0);
            let target_domain = if target_game == Some(game_id) {
                Some(game_domain)
            } else {
                other_games
                    .as_ref()
                    .and_then(|v| v.as_array())
                    .and_then(|games| games.iter().find(|g| g["id"].as_u64() == target_game))
                    .and_then(|g| g["domain_name"].as_str())
                    .filter(|d| domain(d).is_ok())
            };
            target_domain
                .zip(target_mod)
                .map(|(d, id)| format!("https://www.nexusmods.com/{d}/mods/{id}"))
        };
        items.push(Requirement {
            name,
            notes,
            kind: if external { "external" } else { "nexus" }.into(),
            url,
        });
    }
    for dlc in dlcs {
        items.push(Requirement {
            name: dlc["gameExpansion"]["name"]
                .as_str()
                .ok_or("DLC inválida.")?
                .into(),
            notes: dlc["notes"].as_str().unwrap_or_default().into(),
            kind: "dlc".into(),
            url: None,
        });
    }
    Ok(Requirements {
        items,
        complete: total == nodes.len() as u64,
        error: None,
    })
}

fn numeric_id(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok()).filter(|id| *id > 0)
}
fn select_requirements_file(files: &[CatalogFile], requested: Option<u64>) -> Result<u64, String> {
    if let Some(id) = requested {
        return files.iter().find(|f| f.file_id == id).map(|f| f.file_id)
            .ok_or_else(|| "Arquivo não pertence ao mod consultado.".into());
    }
    if let Some(file) = files.iter().find(|f| f.is_primary && !matches!(f.category_id, Some(6 | 7))) {
        return Ok(file.file_id);
    }
    let mut main = files.iter().filter(|f| f.category_id == Some(1));
    match (main.next(), main.next()) {
        (Some(file), None) => Ok(file.file_id),
        _ => Err("Os requisitos dependem do arquivo escolhido. Selecione um arquivo para consultá-los.".into()),
    }
}
fn parse_file_requirements(data: &serde_json::Value) -> Result<Requirements, String> {
    let definitions = data["dependency_definitions"].as_array().ok_or("Lista de requisitos por arquivo inválida.")?;
    let dlcs = data["dlc_dependency_definitions"].as_array().ok_or("Lista de DLCs por arquivo inválida.")?;
    let mut items = Vec::new();
    let mut complete = true;
    for definition in definitions {
        let ranges = definition["ranges"].as_array().filter(|r| !r.is_empty()).ok_or("Intervalo de requisitos ausente.")?;
        let mut alternatives = Vec::new();
        for range in ranges {
            let file = &range["target_mod_file"];
            let target = &file["mod"];
            let name = target["name"].as_str().filter(|s| !s.trim().is_empty()).ok_or("Nome de requisito ausente.")?;
            let game = target["game"]["domain_name"].as_str().ok_or("Jogo do requisito ausente.")?;
            domain(game)?;
            // game_scoped_id is the website mod ID; id is a different, global ID.
            let id = numeric_id(&target["game_scoped_id"]).ok_or("ID de requisito inválido.")?;
            let min = range["min_version"]["version"].as_str().ok_or("Versão mínima ausente.")?;
            let max = if range["max_version"].is_null() { None } else {
                Some(range["max_version"]["version"].as_str().ok_or("Versão máxima inválida.")?)
            };
            let file_name = file["name"].as_str().ok_or("Arquivo do requisito ausente.")?;
            alternatives.push(Requirement {
                name: name.into(), kind: "nexus".into(), url: Some(format!("https://www.nexusmods.com/{game}/mods/{id}")),
                notes: match max {
                    Some(max) => format!("Arquivo: {file_name}. Versões permitidas: {min} até {max}."),
                    None => format!("Arquivo: {file_name}. Versão mínima: {min}."),
                },
            });
        }
        if alternatives.len() == 1 { items.extend(alternatives); } else {
            // Ranges in a definition are OR alternatives, not additional mandatory mods.
            // Keep the choice visible without automatically queuing every alternative.
            complete = false;
            items.push(Requirement {
                name: alternatives.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(" / "),
                notes: format!("Escolha uma alternativa na página do mod. {}", alternatives.iter().map(|r| format!("{}: {} {}", r.name, r.notes, r.url.as_deref().unwrap_or_default())).collect::<Vec<_>>().join("; ")),
                kind: "external".into(), url: None,
            });
        }
    }
    for definition in dlcs {
        let targets = definition["dlc_targets"].as_array().filter(|v| !v.is_empty()).ok_or("Requisito de DLC inválido.")?;
        let names: Result<Vec<_>, _> = targets.iter().map(|t| t["name"].as_str().ok_or("Nome de DLC ausente.")).collect();
        items.push(Requirement { name: names?.join(" / "), notes: if targets.len() > 1 { "Uma destas DLCs é necessária.".into() } else { String::new() }, kind: "dlc".into(), url: None });
    }
    Ok(Requirements { items, complete, error: if complete { None } else { Some("Este arquivo possui requisitos alternativos. Confira a escolha na página do mod.".into()) } })
}

#[cfg(test)]
mod requirements_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn file_requirements_use_website_ids_and_version_bounds() {
        let value: serde_json::Value = serde_json::from_str(include_str!("../../resources/nexus-games/file-requirements-test.json")).unwrap();
        let result = parse_file_requirements(&value).unwrap();
        assert!(result.complete);
        assert_eq!(result.items.len(), 4);
        let expected = [("Cyber Engine Tweaks", 107), ("redscript", 1511), ("Codeware", 7780), ("TweakXL", 4197)];
        for (item, (name, id)) in result.items.iter().zip(expected) {
            assert_eq!(item.name, name);
            assert_eq!(item.url.as_deref(), Some(format!("https://www.nexusmods.com/cyberpunk2077/mods/{id}").as_str()));
        }
        assert!(result.items[2].notes.contains("1.20.3"));
    }
    #[test]
    fn alternatives_are_not_queued_as_multiple_mandatory_mods() {
        let mut value: serde_json::Value = serde_json::from_str(include_str!("../../resources/nexus-games/file-requirements-test.json")).unwrap();
        let other = value["dependency_definitions"][1]["ranges"][0].clone();
        value["dependency_definitions"][0]["ranges"].as_array_mut().unwrap().push(other);
        let result = parse_file_requirements(&value).unwrap();
        assert!(!result.complete);
        assert_eq!(result.items[0].kind, "external");
        assert!(result.items[0].url.is_none());
        assert!(result.items[0].name.contains("redscript"));
    }
    #[test]
    fn missing_response_is_not_no_requirements() {
        assert!(parse_file_requirements(&json!({})).is_err());
        let empty = parse_file_requirements(&json!({"dependency_definitions":[],"dlc_dependency_definitions":[]})).unwrap();
        assert!(empty.complete && empty.items.is_empty());
        assert_eq!(numeric_id(&json!(107)), numeric_id(&json!("107")));
        assert_eq!(numeric_id(&json!(0)), None);
    }
    #[test]
    fn selected_file_is_verified_and_multiple_main_files_need_selection() {
        let files: Vec<CatalogFile> = serde_json::from_value(json!([
            {"file_id":1,"name":"A","file_name":"a.zip","category_id":1},
            {"file_id":2,"name":"B","file_name":"b.zip","category_id":1}
        ])).unwrap();
        assert!(select_requirements_file(&files, None).is_err());
        assert_eq!(select_requirements_file(&files, Some(2)).unwrap(), 2);
        assert!(select_requirements_file(&files, Some(3)).is_err());
    }
}
