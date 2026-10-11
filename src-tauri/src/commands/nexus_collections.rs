//! Nexus Collections: reads a published revision, queues its files through the
//! regular download queue and replays the curator's FOMOD choices.
//!
//! Formats: GraphQL `collectionRevision` (api.nexusmods.com/v2) and the
//! `collection.json` manifest written by Vortex (Nexus-Mods/extension-collections,
//! `src/types/ICollection.ts`, commit 109ae729e82a9b27ade67969f771ba7f6542346b).
use serde::{Deserialize, Serialize};

/// Collection identified by the user: from a page URL, an `nxm://` link or a slug.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct Reference {
    pub domain: Option<String>,
    pub slug: String,
    pub revision: Option<u64>,
}

fn valid_slug(slug: &str) -> bool {
    (1..=32).contains(&slug.len()) && slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}
fn valid_domain(domain: &str) -> bool {
    (1..=100).contains(&domain.len()) && domain.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

pub(super) fn parse_reference(input: &str) -> Result<Reference, String> {
    let input = input.trim();
    let invalid = || "Informe o link de uma coleção do Nexus (página da coleção ou nxm://).".to_string();
    if input.len() > 2048 { return Err(invalid()); }
    if valid_slug(input) { return Ok(Reference { domain: None, slug: input.into(), revision: None }); }
    let url = reqwest::Url::parse(input).map_err(|_| invalid())?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let mut segments: Vec<String> = url.path_segments().map(|s| s.filter(|p| !p.is_empty()).map(|p| p.to_ascii_lowercase()).collect()).unwrap_or_default();
    let domain = match url.scheme() {
        "nxm" => Some(host.clone()),
        "https" if host == "www.nexusmods.com" || host == "nexusmods.com" || host == "next.nexusmods.com" => {
            // /games/<domain>/collections/<slug>, /<domain>/collections/<slug>
            if segments.first().is_some_and(|s| s == "games") { segments.remove(0); }
            (!segments.is_empty()).then(|| segments.remove(0))
        }
        _ => None,
    }.ok_or_else(invalid)?;
    if !valid_domain(&domain) || segments.first().map(String::as_str) != Some("collections") { return Err(invalid()); }
    let slug = segments.get(1).filter(|s| valid_slug(s)).ok_or_else(invalid)?.clone();
    let revision = match (segments.get(2).map(String::as_str), segments.get(3)) {
        (Some("revisions"), Some(number)) => Some(number.parse::<u64>().ok().filter(|n| *n > 0).ok_or_else(invalid)?),
        _ => None,
    };
    Ok(Reference { domain: Some(domain), slug, revision })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CollectionMod {
    pub mod_id: u64,
    pub file_id: u64,
    pub name: String,
    pub file_name: String,
    pub size: Option<u64>,
    pub version: String,
    pub optional: bool,
    /// The same Nexus file is already in this game's library.
    #[serde(default)]
    pub present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionPlan {
    pub domain: String,
    pub slug: String,
    pub revision: u64,
    pub name: String,
    pub mods: Vec<CollectionMod>,
    #[serde(skip_serializing)]
    pub download_link: Option<String>,
}

/// Parses a `collectionRevision` GraphQL response; mods of other games are refused.
pub(super) fn parse_revision(value: &serde_json::Value, domain: &str, slug: &str) -> Result<CollectionPlan, String> {
    if value.get("errors").is_some_and(|e| e.as_array().is_none_or(|a| !a.is_empty())) {
        return Err("Coleção não encontrada no Nexus para este jogo.".into());
    }
    let revision = value.pointer("/data/collectionRevision").filter(|v| v.is_object()).ok_or("Coleção não encontrada no Nexus para este jogo.")?;
    let game = revision.pointer("/collection/game/domainName").and_then(|v| v.as_str()).unwrap_or(domain);
    if game != domain { return Err(format!("Esta coleção pertence a outro jogo ({game}).")); }
    let number = revision["revisionNumber"].as_u64().filter(|n| *n > 0).ok_or("Revisão da coleção inválida.")?;
    let name = revision.pointer("/collection/name").and_then(|v| v.as_str()).unwrap_or(slug).to_string();
    let files = revision["modFiles"].as_array().ok_or("Lista de mods da coleção ausente.")?;
    if files.len() > 5000 { return Err("Coleção grande demais.".into()); }
    let mut mods = Vec::with_capacity(files.len());
    let mut seen = std::collections::BTreeSet::new();
    for entry in files {
        let file = &entry["file"];
        let file_id = entry["fileId"].as_u64().or_else(|| file["fileId"].as_u64()).filter(|n| *n > 0).ok_or("Arquivo da coleção inválido.")?;
        let mod_id = file["modId"].as_u64().or_else(|| file.pointer("/mod/modId").and_then(|v| v.as_u64())).filter(|n| *n > 0).ok_or("Mod da coleção inválido.")?;
        if !seen.insert((mod_id, file_id)) { continue; }
        let size = file["sizeInBytes"].as_str().and_then(|s| s.parse().ok()).or_else(|| file["sizeInBytes"].as_u64());
        mods.push(CollectionMod {
            mod_id,
            file_id,
            name: file.pointer("/mod/name").and_then(|v| v.as_str()).or(file["name"].as_str()).unwrap_or("Mod").to_string(),
            file_name: file["uri"].as_str().unwrap_or_default().to_string(),
            size,
            version: entry["version"].as_str().or(file["version"].as_str()).unwrap_or_default().to_string(),
            optional: entry["optional"].as_bool().unwrap_or(false),
            present: false,
        });
    }
    let download_link = revision["downloadLink"].as_str().filter(|l| l.starts_with("/v2/collections/") && !l.contains("..")).map(str::to_string);
    Ok(CollectionPlan { domain: domain.into(), slug: slug.into(), revision: number, name, mods, download_link })
}

/// Curator data for one mod from `collection.json`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Curated {
    pub choices: Option<serde_json::Value>,
    pub mod_type: Option<String>,
    pub phase: u64,
}

/// `(mod_id, file_id)` → curator data, for Nexus sources of this game.
pub(super) fn parse_manifest(value: &serde_json::Value, domain: &str) -> std::collections::HashMap<(u64, u64), Curated> {
    let mut result = std::collections::HashMap::new();
    for item in value["mods"].as_array().into_iter().flatten().take(5000) {
        let source = &item["source"];
        if source["type"].as_str() != Some("nexus") { continue; }
        if item["domainName"].as_str().is_some_and(|d| d != domain) { continue; }
        let (Some(mod_id), Some(file_id)) = (source["modId"].as_u64(), source["fileId"].as_u64()) else { continue };
        let choices = item.get("choices").filter(|c| c["type"].as_str() == Some("fomod")).cloned();
        let mod_type = item.pointer("/details/type").and_then(|v| v.as_str()).filter(|t| !t.is_empty()).map(str::to_string);
        result.insert((mod_id, file_id), Curated { choices, mod_type, phase: item["phase"].as_u64().unwrap_or(0) });
    }
    result
}

/// Maps Vortex installer choices (`{type:"fomod", options:[{name, groups:[{name,
/// choices:[{name, idx}]}]}]}`) to PeliGames option ids, by step, group and
/// option names. Any name that cannot be matched returns `None`: the user then
/// chooses in the installation window instead of a guessed selection.
pub(super) fn fomod_selection(installer: &super::nexus_fomod::Installer, choices: &serde_json::Value) -> Option<Vec<String>> {
    let mut selection = Vec::new();
    for step in choices["options"].as_array()? {
        let step_name = step["name"].as_str()?;
        let local_step = installer.steps.iter().find(|s| s.name == step_name)?;
        for group in step["groups"].as_array()? {
            let group_name = group["name"].as_str()?;
            let local_group = local_step.groups.iter().find(|g| g.name == group_name)?;
            for choice in group["choices"].as_array()? {
                let name = choice["name"].as_str()?;
                let options: Vec<_> = local_group.options.iter().filter(|o| o.name == name).collect();
                let option = match options.as_slice() {
                    [one] => *one,
                    // Duplicate names inside a group: use the curator's index.
                    _ => local_group.options.get(choice["idx"].as_u64()? as usize).filter(|o| o.name == name)?,
                };
                selection.push(option.id.clone());
            }
        }
    }
    Some(selection)
}

const REVISION_QUERY: &str = "query($slug: String!, $revision: Int, $domain: String) { collectionRevision(slug: $slug, revision: $revision, domainName: $domain) { revisionNumber downloadLink collection { name game { domainName } } modFiles { fileId optional version file { modId name uri sizeInBytes mod { name modId } } } } }";
const LATEST_QUERY: &str = "query($slug: String!, $domain: String) { collection(slug: $slug, domainName: $domain) { latestPublishedRevision { revisionNumber } } }";

pub(super) async fn load(domain: &str, reference: &Reference) -> Result<CollectionPlan, String> {
    if let Some(other) = reference.domain.as_deref().filter(|d| *d != domain) {
        return Err(format!("Esta coleção pertence a outro jogo ({other})."));
    }
    let revision = match reference.revision {
        Some(revision) => revision,
        None => {
            let latest = super::nexus_account::graphql(serde_json::json!({ "query": LATEST_QUERY, "variables": { "slug": reference.slug, "domain": domain } })).await?;
            latest.pointer("/data/collection/latestPublishedRevision/revisionNumber").and_then(|v| v.as_u64())
                .ok_or("Coleção sem revisão publicada para este jogo.")?
        }
    };
    let value = super::nexus_account::graphql(serde_json::json!({ "query": REVISION_QUERY, "variables": { "slug": reference.slug, "revision": revision, "domain": domain } })).await?;
    parse_revision(&value, domain, &reference.slug)
}

/// Downloads the revision archive and reads its `collection.json`.
pub(super) async fn manifest(plan: &CollectionPlan, work: &std::path::Path) -> Result<std::collections::HashMap<(u64, u64), Curated>, String> {
    use std::io::Read;
    let link = plan.download_link.as_deref().ok_or("A coleção não informou o pacote de instruções.")?;
    let links = super::nexus_account::api(&format!("https://api.nexusmods.com{link}")).await?;
    let url = links["download_links"].as_array().into_iter().flatten()
        .filter_map(|l| l["URI"].as_str()).filter_map(|s| reqwest::Url::parse(s).ok())
        .find(crate::core::nxm::trusted_download).ok_or("O Nexus não forneceu o pacote de instruções da coleção.")?;
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::custom(|a| if a.previous().len() >= 5 || !crate::core::nxm::trusted_download(a.url()) { a.stop() } else { a.follow() }))
        .build().map_err(|e| e.to_string())?;
    let mut response = client.get(url).send().await.map_err(|_| "Falha ao baixar as instruções da coleção.")?;
    if response.status().as_u16() != 200 { return Err("O pacote de instruções da coleção foi recusado pelo servidor.".into()); }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "Download das instruções interrompido.")? {
        if bytes.len() + chunk.len() > 256 * 1024 * 1024 { return Err("Pacote de instruções grande demais.".into()); }
        bytes.extend_from_slice(&chunk);
    }
    // Writing, 7z conversion and parsing are disk/CPU work: keep them off the async threads.
    let work = work.to_path_buf();
    let domain = plan.domain.clone();
    blocking(move || {
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let archive = work.join("collection.7z");
        std::fs::write(&archive, &bytes).map_err(|e| e.to_string())?;
        let zip_path = work.join("collection.zip");
        // The archive is usually 7z; anything else is re-packed the same way.
        super::nexus_archive::normalize(&archive, &zip_path)?;
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut entry = zip.by_name("collection.json").map_err(|_| "collection.json ausente no pacote da coleção.")?;
        if entry.size() > 64 * 1024 * 1024 { return Err("collection.json grande demais.".into()); }
        let mut text = String::new();
        (&mut entry).take(64 * 1024 * 1024).read_to_string(&mut text).map_err(|e| e.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| "collection.json inválido.")?;
        Ok(parse_manifest(&value, &domain))
    }).await
}

/// Runs disk work on the blocking pool (see "Fluidez" in the project documentation).
pub(super) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|error| error.to_string())?
}

fn mark_present(plan: &mut CollectionPlan, game: &super::nexus_local::NexusGame) {
    for item in &mut plan.mods {
        item.present = game.mods.iter().any(|local| local.nexus_source.as_ref()
            .is_some_and(|s| s.domain == plan.domain && s.mod_id == item.mod_id && s.file_id == item.file_id));
    }
}

/// Reads a collection for a game of the Nexus library.
#[tauri::command]
pub async fn load_nexus_collection(game_id: String, reference: String) -> Result<CollectionPlan, String> {
    let game = blocking(move || super::nexus_local::diagnostic_game(&game_id)).await?;
    let reference = parse_reference(&reference)?;
    let mut plan = load(&game.nexus_domain, &reference).await?;
    mark_present(&mut plan, &game);
    Ok(plan)
}

#[derive(Serialize)]
pub struct CollectionQueued {
    queued: usize,
    present: usize,
    /// Set when `collection.json` could not be read: files still install, but
    /// FOMOD choices and mod types must be confirmed by the user.
    manifest_error: Option<String>,
}

/// Queues the required files and the optional files chosen by the user.
#[tauri::command]
pub async fn install_nexus_collection(app: tauri::AppHandle, game_id: String, reference: String, optional: Vec<u64>) -> Result<CollectionQueued, String> {
    let game = { let id = game_id.clone(); blocking(move || super::nexus_local::diagnostic_game(&id)).await? };
    if game.running { return Err("Feche o jogo antes de instalar a coleção.".into()); }
    // Re-read the revision: the frontend only chooses optional files.
    let mut plan = load(&game.nexus_domain, &parse_reference(&reference)?).await?;
    mark_present(&mut plan, &game);
    let work = { let game = game.clone(); let name = format!("{}-{}", plan.slug, plan.revision); blocking(move || Ok(super::nexus_profile::directory(&game)?.join("collections").join(name))).await? };
    let (curated, manifest_error) = match manifest(&plan, &work).await {
        Ok(curated) => (curated, None),
        Err(error) => (Default::default(), Some(error)),
    };
    { let work = work.clone(); let _ = blocking(move || { let _ = std::fs::remove_dir_all(&work); Ok(()) }).await; }
    let order = std::sync::Arc::new(plan.mods.iter().map(|m| (m.mod_id, m.file_id)).collect::<Vec<_>>());
    let present = plan.mods.iter().filter(|m| m.present).count();
    let mut items: Vec<_> = plan.mods.into_iter()
        .filter(|m| !m.present && (!m.optional || optional.contains(&m.file_id)))
        .map(|m| {
            let link = super::nexus_downloads::CollectionLink { curated: curated.get(&(m.mod_id, m.file_id)).cloned().unwrap_or_default(), order: order.clone() };
            (m, link)
        }).collect();
    // Vortex installs collection phases in order; keep that order in the queue.
    items.sort_by_key(|(_, link)| link.curated.phase);
    super::nexus_reports::event(&game, "INFO", "coleção", &format!("{} rev {}: {} arquivo(s)", plan.slug, plan.revision, items.len()));
    let queued = super::nexus_downloads::queue_collection(&app, &game_id, &plan.domain, &plan.name, items).await?;
    Ok(CollectionQueued { queued, present, manifest_error })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_from_urls_nxm_and_slugs() {
        let r = |s: &str| parse_reference(s);
        assert_eq!(r("htknoa").unwrap(), Reference { domain: None, slug: "htknoa".into(), revision: None });
        assert_eq!(r("https://www.nexusmods.com/games/stardewvalley/collections/htknoa").unwrap().domain.as_deref(), Some("stardewvalley"));
        assert_eq!(r("https://next.nexusmods.com/stardewvalley/collections/htknoa?tab=mods").unwrap().slug, "htknoa");
        assert_eq!(r("https://www.nexusmods.com/games/stardewvalley/collections/htknoa/revisions/3").unwrap().revision, Some(3));
        assert_eq!(r("nxm://stardewvalley/collections/htknoa/revisions/1").unwrap(), Reference { domain: Some("stardewvalley".into()), slug: "htknoa".into(), revision: Some(1) });
        for bad in ["https://evil.example/stardewvalley/collections/htknoa", "nxm://stardewvalley/mods/1/files/2", "https://www.nexusmods.com/games/stardewvalley/collections/../x", "nxm://stardewvalley/collections/htknoa/revisions/0", "HTKNOA!"] {
            assert!(r(bad).is_err(), "{bad}");
        }
    }

    fn real_revision() -> serde_json::Value {
        // Trimmed public response of collection "htknoa" revision 1 (stardewvalley).
        serde_json::json!({"data":{"collectionRevision":{"revisionNumber":1,"downloadLink":"/v2/collections/636/revisions/948/download_link",
            "collection":{"name":"Coldguy's Stardew Valley Starter Set","game":{"domainName":"stardewvalley"}},
            "modFiles":[
                {"fileId":44014,"optional":false,"version":"1.13.11","file":{"modId":3753,"name":"-Stardew Valley Expanded-","uri":"-Stardew Valley Expanded--3753-1-13-11-1629243294.zip","sizeInBytes":"237905916","mod":{"name":"Stardew Valley Expanded","modId":3753}}},
                {"fileId":38709,"optional":true,"version":"2021.2.27","file":{"modId":2213,"name":"2021_02_27_UnlimitedPlayers","uri":"2021_02_27_UnlimitedPlayers-2213-2021-2-27-1614446233.zip","sizeInBytes":"5109","mod":{"name":"Unlimited Players","modId":2213}}},
                {"fileId":38709,"optional":true,"version":"2021.2.27","file":{"modId":2213,"name":"dup","uri":"dup.zip","sizeInBytes":"1","mod":{"name":"dup","modId":2213}}}
            ]}}})
    }

    #[test]
    fn revision_lists_unique_files_and_rejects_other_games() {
        let plan = parse_revision(&real_revision(), "stardewvalley", "htknoa").unwrap();
        assert_eq!(plan.revision, 1);
        assert_eq!(plan.mods.len(), 2);
        assert_eq!(plan.mods[0], CollectionMod { mod_id: 3753, file_id: 44014, name: "Stardew Valley Expanded".into(), file_name: "-Stardew Valley Expanded--3753-1-13-11-1629243294.zip".into(), size: Some(237905916), version: "1.13.11".into(), optional: false, present: false });
        assert!(plan.mods[1].optional);
        assert_eq!(plan.download_link.as_deref(), Some("/v2/collections/636/revisions/948/download_link"));
        assert!(parse_revision(&real_revision(), "skyrim", "htknoa").is_err());
        assert!(parse_revision(&serde_json::json!({"errors":[{"message":"x"}]}), "stardewvalley", "htknoa").is_err());
        let mut tampered = real_revision();
        tampered["data"]["collectionRevision"]["downloadLink"] = "https://evil.example/x".into();
        assert_eq!(parse_revision(&tampered, "stardewvalley", "htknoa").unwrap().download_link, None);
    }

    #[test]
    fn manifest_keeps_curator_choices_and_types_for_nexus_sources() {
        let manifest = serde_json::json!({"info":{"domainName":"stardewvalley"},"mods":[
            {"name":"A","version":"1","optional":false,"domainName":"stardewvalley","source":{"type":"nexus","modId":1,"fileId":10},
             "choices":{"type":"fomod","options":[{"name":"Main","groups":[{"name":"Style","choices":[{"name":"Dark","idx":1}]}]}]},"details":{"type":"stardewvalley-smapi"},"phase":2},
            {"name":"B","version":"1","optional":false,"domainName":"stardewvalley","source":{"type":"browse","url":"https://example"}},
            {"name":"C","version":"1","optional":false,"domainName":"skyrim","source":{"type":"nexus","modId":3,"fileId":30}}
        ]});
        let curated = parse_manifest(&manifest, "stardewvalley");
        assert_eq!(curated.len(), 1);
        let a = &curated[&(1, 10)];
        assert_eq!(a.mod_type.as_deref(), Some("stardewvalley-smapi"));
        assert_eq!(a.phase, 2);
        assert!(a.choices.is_some());
    }

    #[test]
    fn fomod_choices_map_by_names_and_refuse_guesses() {
        use super::super::nexus_fomod::{Group, Installer, OptionInfo, Step};
        let option = |id: &str, name: &str| OptionInfo { id: id.into(), name: name.into(), description: String::new(), kind: "Optional".into(), selected: false };
        let installer = Installer { name: "Mod".into(), steps: vec![Step { name: "Main".into(), groups: vec![
            Group { name: "Style".into(), kind: "SelectExactlyOne".into(), options: vec![option("0-0-0", "Light"), option("0-0-1", "Dark")] },
            Group { name: "Extras".into(), kind: "SelectAny".into(), options: vec![option("0-1-0", "Icons"), option("0-1-1", "Icons")] },
        ] }] };
        let choices = |groups: serde_json::Value| serde_json::json!({"type":"fomod","options":[{"name":"Main","groups":groups}]});
        assert_eq!(fomod_selection(&installer, &choices(serde_json::json!([{"name":"Style","choices":[{"name":"Dark","idx":1}]}]))), Some(vec!["0-0-1".to_string()]));
        assert_eq!(fomod_selection(&installer, &choices(serde_json::json!([{"name":"Extras","choices":[{"name":"Icons","idx":1}]}]))), Some(vec!["0-1-1".to_string()]), "nome repetido usa o índice");
        assert_eq!(fomod_selection(&installer, &choices(serde_json::json!([{"name":"Style","choices":[{"name":"Neon","idx":2}]}]))), None);
        assert_eq!(fomod_selection(&installer, &choices(serde_json::json!([{"name":"Other","choices":[]}]))), None);
    }
}
