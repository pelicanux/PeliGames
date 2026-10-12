//! LOOT load order sorting (libloot, GPL-3.0-or-later). Opt-in per game: the
//! plugin list is sorted at deployment only after the player downloaded the
//! game's masterlist. A LOOT failure keeps the current order and is logged;
//! it never blocks a deployment.
use super::{nexus_local::NexusGame, nexus_modules::Definition};
use libloot::GameType;
use serde::Serialize;
use std::{fs, path::{Path, PathBuf}};

/// Masterlist branch matching the libloot metadata syntax (libloot 0.29).
const BRANCH: &str = "v0.29";

/// LOOT game type and masterlist repository, as in LOOT's own game settings
/// (VR editions use the flat games' lists; Enderal SE uses `enderal`).
pub(super) fn kind(module: &Definition) -> Option<(GameType, &'static str)> {
    if !module.activation.as_ref().is_some_and(|rule| rule.kind == super::nexus_modules::ActivationKind::Bethesda) { return None; }
    Some(match module.name.as_str() {
        "Skyrim Special Edition" => (GameType::SkyrimSE, "skyrimse"),
        "Skyrim VR" => (GameType::SkyrimVR, "skyrimse"),
        "Skyrim" => (GameType::Skyrim, "skyrim"),
        "Enderal" => (GameType::Skyrim, "enderal"),
        "Enderal Special Edition" => (GameType::SkyrimSE, "enderal"),
        "Fallout 4" => (GameType::Fallout4, "fallout4"),
        "Fallout 4 VR" => (GameType::Fallout4VR, "fallout4"),
        "Fallout New Vegas" => (GameType::FalloutNV, "falloutnv"),
        "Fallout 3" => (GameType::Fallout3, "fallout3"),
        "Oblivion" => (GameType::Oblivion, "oblivion"),
        "Starfield" => (GameType::Starfield, "starfield"),
        _ => return None,
    })
}

pub(super) fn folder(repository: &str) -> Result<PathBuf, String> {
    Ok(crate::core::paths::app_root().map_err(|e| e.to_string())?.join("Mod/Nexus/loot").join(repository))
}

#[derive(Serialize)]
pub struct LootStatus { pub supported: bool, pub repository: String, pub updated: Option<u64> }

fn status_of(module: Option<&Definition>) -> Result<LootStatus, String> {
    let Some((_, repository)) = module.and_then(kind) else { return Ok(LootStatus { supported: false, repository: String::new(), updated: None }); };
    let updated = fs::metadata(folder(repository)?.join("masterlist.yaml")).and_then(|m| m.modified()).ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
    Ok(LootStatus { supported: true, repository: repository.into(), updated })
}

fn game_module(game_id: &str) -> Result<Option<Definition>, String> {
    let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
    let data = super::nexus_local::load()?;
    let game = data.games.iter().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    Ok(super::nexus_deployment::definition(game))
}

#[tauri::command]
pub async fn nexus_loot_status(game_id: String) -> Result<LootStatus, String> {
    tauri::async_runtime::spawn_blocking(move || status_of(game_module(&game_id)?.as_ref())).await.map_err(|e| e.to_string())?
}

/// Downloads the game's masterlist and the shared prelude, validated by libloot before replacing the cache.
#[tauri::command]
pub async fn update_nexus_loot_masterlist(game_id: String) -> Result<LootStatus, String> {
    let module = { let id = game_id.clone(); tauri::async_runtime::spawn_blocking(move || game_module(&id)).await.map_err(|e| e.to_string())?? };
    let (game_type, repository) = module.as_ref().and_then(kind).ok_or("Este jogo não tem ordenação LOOT.")?;
    let client = reqwest::Client::builder().user_agent("PeliGames").timeout(std::time::Duration::from_secs(120))
        .build().map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for (repo, name) in [(repository, "masterlist.yaml"), ("prelude", "prelude.yaml")] {
        let url = format!("https://raw.githubusercontent.com/loot/{repo}/{BRANCH}/{name}");
        let response = client.get(&url).send().await.map_err(|_| format!("Falha ao baixar {name} do LOOT."))?;
        if !response.status().is_success() { return Err(format!("O LOOT recusou {name} ({}).", response.status())); }
        let bytes = response.bytes().await.map_err(|_| format!("Download de {name} interrompido."))?;
        if bytes.len() > 64 * 1024 * 1024 { return Err(format!("{name} grande demais.")); }
        files.push((name, bytes.to_vec()));
    }
    tauri::async_runtime::spawn_blocking(move || {
        let target = folder(repository)?;
        let staging = target.with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        let result = (|| {
            for (name, bytes) in &files { fs::write(staging.join(name), bytes).map_err(|e| e.to_string())?; }
            // Parse before replacing a working cache.
            let game = libloot::Game::with_local_path(game_type, &staging, &staging).map_err(|e| e.to_string())?;
            game.database().write().map_err(|e| e.to_string())?
                .load_masterlist_with_prelude(&staging.join("masterlist.yaml"), &staging.join("prelude.yaml"))
                .map_err(|e| format!("Masterlist LOOT inválida: {e}"))?;
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            for (name, _) in &files { fs::rename(staging.join(name), target.join(name)).map_err(|e| e.to_string())?; }
            Ok::<_, String>(())
        })();
        let _ = fs::remove_dir_all(&staging);
        result?;
        status_of(module.as_ref())
    }).await.map_err(|e| e.to_string())?
}

fn name(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('*').trim();
    (!trimmed.is_empty() && !trimmed.starts_with('#')).then(|| trimmed.to_string())
}

/// Sorts the plugin lines of a Bethesda plugin list with LOOT when the game's
/// masterlist is present. Staged plugins (`runtime`) take precedence over the
/// game's Data folder. Comments keep their place at the top.
pub(super) fn sort(game: &NexusGame, module: &Definition, runtime: &Path, data: &str, local: &Path, lines: Vec<String>) -> Vec<String> {
    let Some((game_type, repository)) = kind(module) else { return lines; };
    let Ok(cache) = folder(repository) else { return lines; };
    if !cache.join("masterlist.yaml").is_file() { return lines; }
    match sorted(game, game_type, &cache, runtime, data, local, &lines) {
        Ok(order) => order,
        Err(error) => {
            super::nexus_reports::event(game, "WARN", "ordenação LOOT", &format!("ordem atual mantida: {error}"));
            lines
        }
    }
}

pub(super) fn sorted(game: &NexusGame, game_type: GameType, cache: &Path, runtime: &Path, data: &str, local: &Path, lines: &[String]) -> Result<Vec<String>, String> {
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let mut handle = libloot::Game::with_local_path(game_type, root, local).map_err(|e| e.to_string())?;
    handle.set_additional_data_paths(vec![runtime.join(data)]).map_err(|e| e.to_string())?;
    handle.database().write().map_err(|e| e.to_string())?
        .load_masterlist_with_prelude(&cache.join("masterlist.yaml"), &cache.join("prelude.yaml")).map_err(|e| e.to_string())?;
    // Rules and groups added on the plugins page (userlist).
    if let Some(userlist) = super::nexus_plugins::userlist(game).ok().filter(|path| path.is_file()) {
        handle.database().write().map_err(|e| e.to_string())?.load_userlist(&userlist).map_err(|e| format!("userlist inválida: {e}"))?;
    }
    let names: Vec<String> = lines.iter().filter_map(|line| name(line)).collect();
    // Every listed plugin and the game's own masters are loaded; staged copies win.
    let locate = |plugin: &str| super::nexus_discovery::locate(&runtime.join(data), plugin).or_else(|| super::nexus_discovery::locate(&root.join(data), plugin));
    handle.load_current_load_order_state().map_err(|e| e.to_string())?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for plugin in handle.load_order().into_iter().map(str::to_string).chain(names.iter().cloned()) {
        if let Some(path) = locate(&plugin) {
            if !paths.iter().any(|known| known.file_name() == path.file_name()) { paths.push(path); }
        } else if names.iter().any(|listed| listed.eq_ignore_ascii_case(&plugin)) {
            return Err(format!("plugin listado não encontrado: {plugin}"));
        }
    }
    let references: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
    handle.load_plugins(&references).map_err(|e| e.to_string())?;
    let order = handle.sort_plugins(&names.iter().map(String::as_str).collect::<Vec<_>>()).map_err(|e| e.to_string())?;
    let mut result: Vec<String> = lines.iter().filter(|line| name(line).is_none()).cloned().collect();
    for plugin in order {
        let line = lines.iter().find(|line| name(line).is_some_and(|n| n.eq_ignore_ascii_case(&plugin))).ok_or("LOOT devolveu um plugin desconhecido.")?;
        result.push(line.clone());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Minimal Skyrim SE plugin: TES4 header with HEDR and optional masters.
    fn plugin(path: &Path, masters: &[&str]) {
        let mut body = Vec::new();
        body.extend_from_slice(b"HEDR"); body.extend_from_slice(&12u16.to_le_bytes());
        body.extend_from_slice(&1.7f32.to_le_bytes()); body.extend_from_slice(&0i32.to_le_bytes()); body.extend_from_slice(&0x800u32.to_le_bytes());
        for master in masters {
            let name = [master.as_bytes(), b"\0"].concat();
            body.extend_from_slice(b"MAST"); body.extend_from_slice(&(name.len() as u16).to_le_bytes()); body.extend_from_slice(&name);
            body.extend_from_slice(b"DATA"); body.extend_from_slice(&8u16.to_le_bytes()); body.extend_from_slice(&0u64.to_le_bytes());
        }
        let mut bytes = b"TES4".to_vec();
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(&body);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    #[test]
    fn fomod_plugin_conditions_follow_the_load_order() {
        let base = std::env::temp_dir().join(format!("peligames-plugin-state-{}", uuid::Uuid::new_v4()));
        let (root, prefix) = (base.join("game"), base.join("prefix"));
        fs::create_dir_all(prefix.join("pfx/drive_c/users/steamuser/AppData/Local/Skyrim Special Edition")).unwrap();
        fs::write(prefix.join("pfx/system.reg"), "").unwrap();
        fs::write(root.join("SkyrimSE.exe").parent().map(|p| { fs::create_dir_all(p).unwrap(); p.join("SkyrimSE.exe") }).unwrap(), b"MZ").unwrap();
        for name in ["Skyrim.esm", "Active.esp", "Disabled.esp"] { plugin(&root.join("Data").join(name), if name == "Skyrim.esm" { &[] } else { &["Skyrim.esm"] }); }
        fs::write(prefix.join("pfx/drive_c/users/steamuser/AppData/Local/Skyrim Special Edition/plugins.txt"), "*Active.esp\r\nDisabled.esp\r\n").unwrap();
        let game: NexusGame = serde_json::from_value(serde_json::json!({"id": uuid::Uuid::new_v4().to_string(), "platform": "proton", "mods": [],
            "game": {"directory": root}, "compat_data": prefix})).unwrap();
        let module = super::super::nexus_modules::registry().definitions.into_iter().find(|m| m.name == "Skyrim Special Edition").unwrap();
        let oracle = PluginOracle::new(&game, &module);
        assert_eq!(oracle.state("Active.esp").unwrap(), PluginState::Active);
        assert_eq!(oracle.state("disabled.ESP").unwrap(), PluginState::Inactive);
        assert_eq!(oracle.state("Missing.esp").unwrap(), PluginState::Missing);
        assert_eq!(oracle.state("Skyrim.esm").unwrap(), PluginState::Active, "mestre do jogo ativo implicitamente");
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn masterlist_rules_reorder_listed_plugins_and_keep_their_flags() {
        let base = std::env::temp_dir().join(format!("peligames-loot-{}", uuid::Uuid::new_v4()));
        let (root, local, runtime, cache) = (base.join("game"), base.join("local"), base.join("stage"), base.join("cache"));
        plugin(&root.join("Data/Skyrim.esm"), &[]);
        plugin(&root.join("Data/External.esp"), &["Skyrim.esm"]);
        plugin(&runtime.join("Data/Patch.esp"), &["Skyrim.esm"]);
        plugin(&runtime.join("Data/Main.esp"), &["Skyrim.esm"]);
        fs::create_dir_all(&local).unwrap();
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("prelude.yaml"), "common: []\n").unwrap();
        fs::write(cache.join("masterlist.yaml"), "plugins:\n  - name: 'Patch.esp'\n    after: [ 'Main.esp', 'External.esp' ]\n").unwrap();
        let game: NexusGame = serde_json::from_value(serde_json::json!({"id": "t", "platform": "proton", "mods": [], "game": {"directory": root}})).unwrap();
        let lines: Vec<String> = ["# managed", "*Patch.esp", "External.esp", "*Main.esp"].map(String::from).to_vec();
        let order = sorted(&game, GameType::SkyrimSE, &cache, &runtime, "Data", &local, &lines).unwrap();
        assert_eq!(order[0], "# managed");
        let patch = order.iter().position(|l| l == "*Patch.esp").unwrap();
        assert!(patch > order.iter().position(|l| l == "*Main.esp").unwrap());
        assert!(patch > order.iter().position(|l| l == "External.esp").unwrap());
        assert_eq!(order.len(), lines.len());
        // A listed plugin missing from both folders keeps the current order.
        let missing: Vec<String> = ["*Gone.esp", "*Main.esp"].map(String::from).to_vec();
        assert!(sorted(&game, GameType::SkyrimSE, &cache, &runtime, "Data", &local, &missing).is_err());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    #[ignore = "Requires masterlists downloaded to PELIGAMES_LOOT_CACHE/<repository>/"]
    fn published_masterlists_parse_with_this_libloot() {
        let cache = PathBuf::from(std::env::var("PELIGAMES_LOOT_CACHE").expect("PELIGAMES_LOOT_CACHE"));
        for (game_type, repository) in [(GameType::SkyrimSE, "skyrimse"), (GameType::Skyrim, "skyrim"), (GameType::SkyrimSE, "enderal"), (GameType::Fallout4, "fallout4"),
            (GameType::FalloutNV, "falloutnv"), (GameType::Fallout3, "fallout3"), (GameType::Oblivion, "oblivion"), (GameType::Starfield, "starfield")] {
            let game = libloot::Game::with_local_path(game_type, &cache, &cache).unwrap();
            game.database().write().unwrap().load_masterlist_with_prelude(&cache.join(repository).join("masterlist.yaml"), &cache.join("prelude.yaml"))
                .unwrap_or_else(|e| panic!("{repository}: {e}"));
        }
    }
}

/// Plugin state asked by FOMOD conditions (`fileDependency` on a plugin), as in Vortex.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PluginState { Active, Inactive, Missing }
struct PluginSets { active: std::collections::BTreeSet<String>, data: Vec<PathBuf> }
/// Reads the load order lazily, only when a FOMOD asks about a plugin.
pub(super) struct PluginOracle<'a> {
    game: &'a NexusGame,
    module: &'a Definition,
    cache: std::cell::OnceCell<Result<PluginSets, String>>,
}
impl<'a> PluginOracle<'a> {
    pub(super) fn new(game: &'a NexusGame, module: &'a Definition) -> Self {
        Self { game, module, cache: std::cell::OnceCell::new() }
    }
    fn load(&self) -> Result<PluginSets, String> {
        let (game_type, _) = kind(self.module).ok_or("Este jogo não tem lista de plugins para consultar.")?;
        let rule = self.module.activation.as_ref().ok_or("Ativação de plugins ausente.")?;
        let root = Path::new(self.game.game["directory"].as_str().ok_or("Pasta ausente.")?);
        let (base, relative) = super::nexus_targets::root(root, &self.game.compat_data, &rule.path)?;
        let list = base.join(&relative);
        let local = list.parent().ok_or("Lista de plugins inválida.")?;
        let mut data = vec![root.join(&rule.data)];
        // Enabled mods of the current profile count even before (or without) deployment.
        if let Ok(profile) = super::nexus_profile::profile(self.game) { data.insert(0, profile.join(&rule.data)); }
        let mut handle = libloot::Game::with_local_path(game_type, root, local).map_err(|e| e.to_string())?;
        handle.set_additional_data_paths(data.iter().filter(|dir| dir.is_dir() && !dir.starts_with(root.join(&rule.data))).cloned().collect()).map_err(|e| e.to_string())?;
        handle.load_current_load_order_state().map_err(|e| format!("Não foi possível ler a ordem de plugins: {e}"))?;
        // libloot knows the implicitly active plugins (Skyrim.esm, DLC). The list itself
        // is read from the real file: on Linux libloot only finds the `Plugins.txt` spelling.
        let mut active: std::collections::BTreeSet<String> = handle.load_order().into_iter().filter(|name| handle.is_plugin_active(name)).map(str::to_ascii_lowercase).collect();
        let text = fs::read(&list).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
            match (rule.starred, line.strip_prefix('*')) {
                (true, Some(name)) => { active.insert(name.trim().to_ascii_lowercase()); }
                (false, _) => { active.insert(line.trim_start_matches('*').to_ascii_lowercase()); }
                _ => {}
            }
        }
        Ok(PluginSets { active, data })
    }
    pub(super) fn state(&self, plugin: &str) -> Result<PluginState, String> {
        let sets = self.cache.get_or_init(|| self.load()).as_ref().map_err(Clone::clone)?;
        if sets.active.contains(&plugin.to_ascii_lowercase()) { return Ok(PluginState::Active); }
        let present = sets.data.iter().any(|dir| super::nexus_discovery::locate(dir, plugin).is_some_and(|path| path.is_file()));
        Ok(if present { PluginState::Inactive } else { PluginState::Missing })
    }
}
