//! Plugins page of the Bethesda games (Vortex gamebryo-plugin-management):
//! enable or disable each plugin, manual order or LOOT autosort, plugin flags,
//! missing masters, plugin limits and LOOT messages.
use super::nexus_local::NexusGame;
use super::nexus_modules::{ActivationKind, Activation, Definition};
use libloot::metadata::{select_message_content, MessageType};
use libloot::{EvalMode, MergeMode};
use std::{collections::{BTreeMap, BTreeSet}, fs, path::{Path, PathBuf}, time::Duration};

/// Choices of the plugins page, kept beside the game's profiles.
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Store {
    /// Plugins the player switched off (lower case).
    #[serde(default)]
    pub disabled: Vec<String>,
    /// Manual order, used while autosort is off.
    #[serde(default)]
    pub order: Vec<String>,
    /// LOOT sorts on every change (Vortex default) when the masterlist exists.
    #[serde(default = "enabled")]
    pub autosort: bool,
    /// Plugins hidden from the game as `<name>.ghost` (lower case).
    #[serde(default)]
    pub ghosted: Vec<String>,
    /// Light flag set (true) or cleared (false) on plugins of mods.
    #[serde(default)]
    pub light: BTreeMap<String, bool>,
}
fn enabled() -> bool { true }
impl Default for Store {
    fn default() -> Self { Store { disabled: Vec::new(), order: Vec::new(), autosort: true, ghosted: Vec::new(), light: BTreeMap::new() } }
}
impl Store {
    fn path(game: &NexusGame) -> Result<PathBuf, String> {
        Ok(super::nexus_profile::directory(game)?.join("plugins.json"))
    }
    pub(super) fn load(game: &NexusGame) -> Result<Self, String> {
        match fs::read(Self::path(game)?) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Escolhas de plugins salvas inválidas.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn save(&self, game: &NexusGame) -> Result<(), String> {
        let path = Self::path(game)?;
        fs::create_dir_all(path.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        fs::write(&temporary, serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        fs::rename(&temporary, &path).map_err(|e| { let _ = fs::remove_file(&temporary); e.to_string() })
    }
    /// Off on the page, or ghosted (hidden plugins are never active).
    fn is_disabled(&self, name: &str) -> bool { self.disabled.iter().chain(&self.ghosted).any(|item| item.eq_ignore_ascii_case(name)) }
    fn is_ghosted(&self, name: &str) -> bool { self.ghosted.iter().any(|item| item.eq_ignore_ascii_case(name)) }
}
pub(super) fn plugin_name(line: &str) -> Option<String> {
    let name = line.trim().trim_start_matches('*').trim();
    (!name.is_empty() && !name.starts_with('#')).then(|| name.to_string())
}
fn is_plugin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".esp", ".esm", ".esl"].iter().any(|ext| lower.ends_with(ext))
}
fn native(module: &Definition, name: &str) -> bool {
    module.plugins.as_ref().is_some_and(|rules| rules.native.iter().any(|item| item.eq_ignore_ascii_case(name)))
}
/// Rules and groups of the plugins page (LOOT userlist), per game.
pub(super) fn userlist(game: &NexusGame) -> Result<PathBuf, String> {
    Ok(super::nexus_profile::directory(game)?.join("userlist.yaml"))
}
/// Header flag of a light plugin: Starfield uses 0x100, Skyrim SE and Fallout 4 0x200.
fn light_flag(module: &Definition) -> u32 {
    if module.domain == "starfield" { 0x100 } else { 0x200 }
}
/// Profile copies before deployment: ghosted plugins of mods are renamed to
/// `.ghost` (or back), and the light flag chosen on the page is written into
/// the header of the copy; the archive itself is never changed.
pub(super) fn prepare(game: &NexusGame, module: &Definition, runtime: &Path) -> Result<(), String> {
    let Ok(rule) = rule(module) else { return Ok(()); };
    let data = runtime.join(&rule.data);
    if !data.is_dir() { return Ok(()); }
    let store = Store::load(game).unwrap_or_default();
    for entry in fs::read_dir(&data).map_err(|e| e.to_string())?.filter_map(Result::ok) {
        let file = entry.file_name().to_string_lossy().into_owned();
        let name = file.strip_suffix(".ghost").unwrap_or(&file).to_string();
        if !is_plugin(&name) { continue; }
        let ghost = store.is_ghosted(&name);
        let target = data.join(if ghost { format!("{name}.ghost") } else { name.clone() });
        if entry.path() != target { fs::rename(entry.path(), &target).map_err(|e| e.to_string())?; }
        if let Some(light) = store.light.iter().find(|(key, _)| key.eq_ignore_ascii_case(&name)).map(|(_, value)| *value) {
            let mut bytes = fs::read(&target).map_err(|e| e.to_string())?;
            if bytes.len() < 12 || !bytes.starts_with(b"TES4") { return Err(format!("{name} não é um plugin válido.")); }
            let mut flags = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
            if light { flags |= light_flag(module); } else { flags &= !light_flag(module); }
            bytes[8..12].copy_from_slice(&flags.to_le_bytes());
            fs::write(&target, bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
/// Opens the game's LOOT database with the masterlist (when downloaded) and the userlist.
fn database(game: &NexusGame, module: &Definition) -> Result<libloot::Game, String> {
    let (game_type, repository) = super::nexus_loot::kind(module).ok_or("Este jogo não tem lista de plugins.")?;
    let root = PathBuf::from(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let handle = libloot::Game::with_local_path(game_type, &root, &super::nexus_profile::directory(game)?).map_err(|e| e.to_string())?;
    {
        let database = handle.database();
        let mut database = database.write().map_err(|e| e.to_string())?;
        let cache = super::nexus_loot::folder(repository)?;
        if cache.join("masterlist.yaml").is_file() {
            database.load_masterlist_with_prelude(&cache.join("masterlist.yaml"), &cache.join("prelude.yaml")).map_err(|e| e.to_string())?;
        }
        let path = userlist(game)?;
        if path.is_file() { database.load_userlist(&path).map_err(|e| format!("userlist inválida: {e}"))?; }
    }
    Ok(handle)
}
/// Changes the userlist and writes it back.
fn edit_userlist(game: &NexusGame, module: &Definition, edit: impl FnOnce(&mut libloot::Database) -> Result<(), String>) -> Result<(), String> {
    let handle = database(game, module)?;
    let database = handle.database();
    let mut database = database.write().map_err(|e| e.to_string())?;
    edit(&mut database)?;
    let mut options = libloot::MetadataWriteOptions::new();
    options.set_truncate(true);
    let path = userlist(game)?;
    fs::create_dir_all(path.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
    database.write_user_metadata(&path, &options).map_err(|e| e.to_string())
}
/// Applies the page choices to the plugin lines composed by a deployment:
/// the manual order while autosort is off, then the disabled plugins
/// (without the asterisk, or left out where the list holds only active ones).
pub(super) fn arrange(store: &Store, module: &Definition, starred: bool, lines: Vec<String>) -> Vec<String> {
    let mut lines = lines;
    if !store.autosort && !store.order.is_empty() {
        let position = |line: &String| plugin_name(line).and_then(|name| store.order.iter().position(|known| known.eq_ignore_ascii_case(&name))).unwrap_or(usize::MAX);
        let mut indexed: Vec<(usize, String)> = lines.into_iter().enumerate().collect();
        indexed.sort_by_key(|(index, line)| (plugin_name(line).is_some(), position(line), *index));
        lines = indexed.into_iter().map(|(_, line)| line).collect();
    }
    lines.into_iter().filter_map(|line| match plugin_name(&line) {
        Some(name) if store.is_disabled(&name) && !native(module, &name) => starred.then_some(name),
        _ => Some(line),
    }).collect()
}
/// Orders plugins by modification time (Oblivion, Fallout 3, New Vegas, Skyrim LE):
/// each plugin, in list order, gets one minute more than the previous.
pub(super) fn order_by_time(data: &Path, names: &[String]) -> Result<(), String> {
    let files: Vec<PathBuf> = names.iter().filter_map(|name| super::nexus_discovery::locate(data, name)).filter(|path| path.is_file()).collect();
    let Some(base) = files.iter().filter_map(|path| fs::metadata(path).and_then(|m| m.modified()).ok()).min() else { return Ok(()); };
    for (index, path) in files.iter().enumerate() {
        let time = base.checked_add(Duration::from_secs(60 * index as u64)).ok_or("Data de plugin inválida.")?;
        fs::File::open(path).and_then(|file| file.set_modified(time)).map_err(|e| format!("Falha ao ordenar {}: {e}", path.display()))?;
    }
    Ok(())
}
pub(super) fn by_time(module: &Definition) -> bool {
    ["oblivion", "fallout3", "newvegas", "skyrim", "enderal"].contains(&module.domain.as_str())
}
fn rule(module: &Definition) -> Result<&Activation, String> {
    module.activation.as_ref().filter(|rule| rule.kind == ActivationKind::Bethesda).ok_or_else(|| "Este jogo não tem lista de plugins.".into())
}
#[derive(serde::Serialize)]
pub struct Message { kind: &'static str, text: String }
#[derive(serde::Serialize)]
pub struct PluginEntry {
    name: String,
    enabled: bool,
    native: bool,
    /// Load order slot of an active plugin (`0A`, `FE:001`, `FD:01`).
    index: Option<String>,
    managed: Option<String>,
    master: bool,
    light: bool,
    medium: bool,
    archive: bool,
    masters: Vec<String>,
    missing: Vec<String>,
    messages: Vec<Message>,
    dirty: Vec<String>,
    incompatible: Vec<String>,
    requires: Vec<String>,
    group: Option<String>,
    /// Rules the player added (userlist).
    rules: Vec<Rule>,
    ghost: bool,
    light_choice: Option<bool>,
}
#[derive(serde::Serialize)]
pub struct Rule { kind: &'static str, plugin: String }
#[derive(serde::Serialize)]
pub struct PluginsView {
    /// Masterlist and user groups, for the group selector.
    groups: Vec<String>,
    plugins: Vec<PluginEntry>,
    autosort: bool,
    /// LOOT masterlist downloaded (sorting and messages available).
    loot: bool,
    light: bool,
    medium: bool,
    counts: [usize; 3],
    limits: [usize; 3],
    list: String,
}
/// libloot reads the plugin list from `local`; on Linux it only finds one
/// spelling, so a private copy holds both before reading.
fn reading_copy(game: &NexusGame, list: &Path) -> Result<PathBuf, String> {
    let local = super::nexus_profile::directory(game)?.join("plugins-reading");
    let _ = fs::remove_dir_all(&local);
    fs::create_dir_all(&local).map_err(|e| e.to_string())?;
    let text = fs::read(list).unwrap_or_default();
    for name in ["plugins.txt", "Plugins.txt"] { fs::write(local.join(name), &text).map_err(|e| e.to_string())?; }
    if let Some(order) = list.parent().and_then(|dir| super::nexus_discovery::locate(dir, "loadorder.txt")) {
        fs::copy(order, local.join("loadorder.txt")).map_err(|e| e.to_string())?;
    }
    Ok(local)
}
pub(super) fn view(game: &NexusGame) -> Result<PluginsView, String> {
    let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
    let rule = rule(&module)?;
    let (game_type, repository) = super::nexus_loot::kind(&module).ok_or("Este jogo não tem lista de plugins.")?;
    let root = PathBuf::from(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let list = super::nexus_deployment::destination(game, &rule.path)?;
    let local = reading_copy(game, &list)?;
    let store = Store::load(game)?;
    let mut handle = libloot::Game::with_local_path(game_type, &root, &local).map_err(|e| e.to_string())?;
    handle.load_current_load_order_state().map_err(|e| format!("Não foi possível ler a ordem de plugins: {e}"))?;
    let data = super::nexus_discovery::locate(&root, &rule.data).unwrap_or_else(|| root.join(&rule.data));
    let order: Vec<String> = handle.load_order().into_iter().map(str::to_string).filter(|name| is_plugin(name)).collect();
    let locate = |name: &str| super::nexus_discovery::locate(&data, name).or_else(|| super::nexus_discovery::locate(&data, &format!("{name}.ghost")));
    let paths: Vec<PathBuf> = order.iter().filter_map(|name| locate(name)).collect();
    handle.load_plugin_headers(&paths.iter().map(PathBuf::as_path).collect::<Vec<_>>()).map_err(|e| e.to_string())?;
    let cache = super::nexus_loot::folder(repository)?;
    let loot = cache.join("masterlist.yaml").is_file();
    if loot {
        handle.database().write().map_err(|e| e.to_string())?
            .load_masterlist_with_prelude(&cache.join("masterlist.yaml"), &cache.join("prelude.yaml")).map_err(|e| e.to_string())?;
    }
    let user = userlist(game)?;
    if user.is_file() { handle.database().write().map_err(|e| e.to_string())?.load_userlist(&user).map_err(|e| format!("userlist inválida: {e}"))?; }
    let _ = fs::remove_dir_all(&local);
    let rules = module.plugins.clone();
    let (light_game, medium_game) = (rules.as_ref().is_some_and(|r| r.light), rules.as_ref().is_some_and(|r| r.medium));
    let owners: Vec<(String, String)> = game.mods.iter().filter(|m| m.installed && m.enabled).flat_map(|m| {
        let label = m.nexus_source.as_ref().map(|s| s.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| m.name.clone());
        m.installed_files.iter().chain(m.fomod_files.iter()).flatten()
            .filter_map(move |(_, target)| target.strip_prefix(&format!("{}/", rule.data)).filter(|n| !n.contains('/')).map(|n| (n.to_ascii_lowercase(), label.clone())))
            .collect::<Vec<_>>()
    }).collect();
    let installed: BTreeSet<String> = order.iter().map(|name| name.to_ascii_lowercase()).collect();
    let database = handle.database();
    let database = database.read().map_err(|e| e.to_string())?;
    let (mut full, mut light, mut medium) = (0usize, 0usize, 0usize);
    let mut plugins = Vec::new();
    for name in &order {
        let plugin = handle.plugin(name);
        let enabled = handle.is_plugin_active(name);
        let (is_light, is_medium) = (light_game && plugin.as_ref().is_some_and(|p| p.is_light_plugin()), medium_game && plugin.as_ref().is_some_and(|p| p.is_medium_plugin()));
        let index = enabled.then(|| if is_light { light += 1; format!("FE:{:03X}", light - 1) } else if is_medium { medium += 1; format!("FD:{:02X}", medium - 1) } else { full += 1; format!("{:02X}", full - 1) });
        let masters = plugin.as_ref().and_then(|p| p.masters().ok()).unwrap_or_default();
        let missing = masters.iter().filter(|master| !installed.contains(&master.to_ascii_lowercase())).cloned().collect();
        let (mut messages, mut dirty, mut incompatible, mut requires, mut group) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), None);
        let mut rules = Vec::new();
        if let Ok(Some(metadata)) = database.plugin_user_metadata(name, EvalMode::DoNotEvaluate) {
            for (kind, files) in [("after", metadata.load_after_files()), ("requires", metadata.requirements()), ("incompatible", metadata.incompatibilities())] {
                rules.extend(files.iter().map(|file| Rule { kind, plugin: file.name().as_str().to_string() }));
            }
        }
        {
            if let Ok(Some(metadata)) = database.plugin_metadata(name, MergeMode::WithUserMetadata, EvalMode::Evaluate) {
                for message in metadata.messages() {
                    if let Some(content) = select_message_content(message.content(), "en") {
                        let kind = match message.message_type() { MessageType::Say => "say", MessageType::Warn => "warn", MessageType::Error => "error" };
                        messages.push(Message { kind, text: content.text().to_string() });
                    }
                }
                for info in metadata.dirty_info() {
                    dirty.push(format!("{} ITM, {} UDR, {} navmesh · {}", info.itm_count(), info.deleted_reference_count(), info.deleted_navmesh_count(), info.cleaning_utility()));
                }
                let active = |file: &libloot::metadata::File| handle.is_plugin_active(file.name().as_str());
                incompatible = metadata.incompatibilities().iter().filter(|file| active(file)).map(|file| file.name().as_str().to_string()).collect();
                requires = metadata.requirements().iter().filter(|file| !installed.contains(&file.name().as_str().to_ascii_lowercase())).map(|file| file.name().as_str().to_string()).collect();
                group = metadata.group().map(str::to_string);
            }
        }
        plugins.push(PluginEntry {
            name: name.clone(), enabled, native: native(&module, name),
            index,
            managed: owners.iter().find(|(file, _)| *file == name.to_ascii_lowercase()).map(|(_, label)| label.clone()),
            master: plugin.as_ref().is_some_and(|p| p.is_master()), light: is_light, medium: is_medium,
            archive: plugin.as_ref().is_some_and(|p| p.loads_archive()),
            ghost: store.is_ghosted(name),
            light_choice: store.light.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).map(|(_, value)| *value),
            masters, missing, messages, dirty, incompatible, requires, group, rules,
        });
    }
    // Vortex limits: 255 regular plugins, 254 with light plugins, 253 with medium ones.
    let regular = if medium_game { 253 } else if light_game { 254 } else { 255 };
    let groups = database.groups(MergeMode::WithUserMetadata).iter().map(|group| group.name().to_string()).collect();
    Ok(PluginsView { groups, plugins, autosort: store.autosort, loot, light: light_game, medium: medium_game, counts: [full, light, medium], limits: [regular, 4096, 256], list: list.to_string_lossy().into_owned() })
}
/// Writes the plugin list for `order` (all installed plugins) and `store`,
/// keeping comment lines and, where the list holds only active plugins, the
/// game's own entries; timestamp games also get their file dates.
fn write(game: &NexusGame, module: &Definition, order: &[String], store: &Store) -> Result<(), String> {
    let rule = rule(module)?;
    let list = super::nexus_deployment::destination(game, &rule.path)?;
    let original = fs::read(&list).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
    let listed: BTreeSet<String> = original.lines().filter_map(plugin_name).map(|name| name.to_ascii_lowercase()).collect();
    let mut lines: Vec<String> = original.lines().filter(|line| plugin_name(line).is_none() && !line.trim().is_empty()).map(str::to_string).collect();
    for name in order {
        let active = native(module, name) || !store.is_disabled(name);
        // Starred lists skip the always-active game plugins, as the game writes them.
        if rule.starred {
            if native(module, name) && !listed.contains(&name.to_ascii_lowercase()) { continue; }
            lines.push(if active { format!("*{name}") } else { name.clone() });
        } else if active {
            lines.push(name.clone());
        }
    }
    fs::create_dir_all(list.parent().ok_or("Lista de plugins inválida.")?).map_err(|e| e.to_string())?;
    fs::write(&list, lines.join("\r\n") + "\r\n").map_err(|e| e.to_string())?;
    if by_time(module) {
        let root = PathBuf::from(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
        let data = super::nexus_discovery::locate(&root, &rule.data).unwrap_or_else(|| root.join(&rule.data));
        order_by_time(&data, order)?;
    }
    Ok(())
}
/// Current order of all installed plugins (as the page shows it).
fn current(game: &NexusGame) -> Result<Vec<String>, String> {
    Ok(view(game)?.plugins.into_iter().map(|plugin| plugin.name).collect())
}
/// LOOT order of the installed plugins.
fn loot_order(game: &NexusGame, module: &Definition, order: &[String]) -> Result<Vec<String>, String> {
    let rule = rule(module)?;
    let (game_type, repository) = super::nexus_loot::kind(module).ok_or("Este jogo não tem ordenação LOOT.")?;
    let cache = super::nexus_loot::folder(repository)?;
    if !cache.join("masterlist.yaml").is_file() { return Err("Baixe a masterlist do LOOT nas configurações do jogo para ordenar.".into()); }
    let list = super::nexus_deployment::destination(game, &rule.path)?;
    let local = reading_copy(game, &list)?;
    let root = PathBuf::from(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let lines: Vec<String> = order.to_vec();
    let sorted = super::nexus_loot::sorted(game, game_type, &cache, &root, &rule.data, &local, &lines);
    let _ = fs::remove_dir_all(&local);
    sorted
}
/// LOOT order of the plugins the game sees; ghosted ones keep the end.
fn sorted_visible(game: &NexusGame, module: &Definition, store: &Store, order: &[String]) -> Result<Vec<String>, String> {
    let (hidden, visible): (Vec<String>, Vec<String>) = order.iter().cloned().partition(|name| store.is_ghosted(name));
    Ok(loot_order(game, module, &visible)?.into_iter().chain(hidden).collect())
}
fn change(game: &NexusGame, update: impl FnOnce(&mut Store, &mut Vec<String>) -> Result<(), String>) -> Result<PluginsView, String> {
    super::nexus_deployment::idle(game)?;
    let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
    let mut store = Store::load(game)?;
    let mut order = current(game)?;
    update(&mut store, &mut order)?;
    if store.autosort && super::nexus_loot::kind(&module).is_some_and(|(_, repository)| super::nexus_loot::folder(repository).is_ok_and(|cache| cache.join("masterlist.yaml").is_file())) {
        order = sorted_visible(game, &module, &store, &order)?;
    }
    store.order = order.clone();
    write(game, &module, &order, &store)?;
    store.save(game)?;
    view(game)
}
fn with_game<T>(game_id: &str, action: impl FnOnce(&NexusGame) -> Result<T, String>) -> Result<T, String> {
    let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
    let data = super::nexus_local::load()?;
    let game = data.games.iter().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    action(game)
}
#[tauri::command]
pub async fn nexus_plugins(game_id: String) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, view)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_nexus_plugin_enabled(game_id: String, name: String, enabled: bool) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        if native(&module, &name) { return Err("Plugins do próprio jogo ficam sempre ativos.".into()); }
        change(game, |store, order| {
            if !order.iter().any(|item| item.eq_ignore_ascii_case(&name)) { return Err("Plugin não encontrado na pasta Data.".into()); }
            store.disabled.retain(|item| !item.eq_ignore_ascii_case(&name));
            if !enabled { store.disabled.push(name.to_ascii_lowercase()); }
            Ok(())
        })
    })).await.map_err(|e| e.to_string())?
}
/// Manual order (autosort off): the full list of installed plugins.
#[tauri::command]
pub async fn set_nexus_plugin_order(game_id: String, order: Vec<String>) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        change(game, |store, current| {
            if store.autosort { return Err("Desligue a ordenação automática para ordenar à mão.".into()); }
            let mut known: Vec<String> = current.iter().map(|n| n.to_ascii_lowercase()).collect();
            let mut asked: Vec<String> = order.iter().map(|n| n.to_ascii_lowercase()).collect();
            known.sort(); asked.sort();
            if known != asked { return Err("A pasta Data mudou; a lista foi recarregada.".into()); }
            // Game masters keep their place at the top.
            let natives: Vec<String> = current.iter().filter(|name| native(&module, name)).cloned().collect();
            *current = natives.iter().cloned().chain(order.iter().filter(|name| !native(&module, name)).cloned()).collect();
            Ok(())
        })
    })).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_nexus_plugin_autosort(game_id: String, enabled: bool) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| change(game, |store, _| { store.autosort = enabled; Ok(()) })))
        .await.map_err(|e| e.to_string())?
}
/// "Sort now": one LOOT pass, whatever the autosort setting.
#[tauri::command]
pub async fn sort_nexus_plugins(game_id: String) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        change(game, |store, order| { *order = sorted_visible(game, &module, store, order)?; Ok(()) })
    })).await.map_err(|e| e.to_string())?
}
/// Active plugins of the current load order.
pub(super) fn active(game: &NexusGame) -> Result<Vec<String>, String> {
    Ok(view(game)?.plugins.into_iter().filter(|plugin| plugin.enabled).map(|plugin| plugin.name).collect())
}
/// Applies the plugin list of a save: its plugins on, the others off, and its
/// order while autosort is off (Vortex "set-plugin-list").
pub(super) fn restore(game: &NexusGame, plugins: &[String]) -> Result<PluginsView, String> {
    let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
    let listed = |name: &str| plugins.iter().any(|plugin| plugin.eq_ignore_ascii_case(name));
    change(game, |store, order| {
        let missing: Vec<String> = plugins.iter().filter(|plugin| !order.iter().any(|name| name.eq_ignore_ascii_case(plugin))).cloned().collect();
        if !missing.is_empty() { return Err(format!("Plugins do save que não estão na pasta Data: {}", missing.join(", "))); }
        if let Some(ghost) = store.ghosted.iter().find(|name| listed(name)) { return Err(format!("{ghost} está escondido (ghost); mostre-o ao jogo antes.")); }
        store.disabled = order.iter().filter(|name| !native(&module, name) && !listed(name)).map(|name| name.to_ascii_lowercase()).collect();
        if !store.autosort {
            let (mut first, rest): (Vec<String>, Vec<String>) = order.iter().cloned().partition(|name| native(&module, name));
            first.extend(plugins.iter().filter_map(|plugin| rest.iter().find(|name| name.eq_ignore_ascii_case(plugin)).cloned()).filter(|name| !native(&module, name)));
            first.extend(rest.into_iter().filter(|name| !listed(name)));
            *order = first;
        }
        Ok(())
    })
}
fn file(name: &str) -> libloot::metadata::File { libloot::metadata::File::new(name.to_string()) }
/// Adds or removes a rule (`after`, `requires`, `incompatible`) of the userlist.
#[tauri::command]
pub async fn set_nexus_plugin_rule(game_id: String, plugin: String, kind: String, target: String, add: bool) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| set_rule(game, &plugin, &kind, &target, add))).await.map_err(|e| e.to_string())?
}
fn set_rule(game: &NexusGame, plugin: &str, kind: &str, target: &str, add: bool) -> Result<PluginsView, String> {
    {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        if plugin.eq_ignore_ascii_case(&target) || !is_plugin(&target) { return Err("Escolha outro plugin para a regra.".into()); }
        edit_userlist(game, &module, |database| {
            let mut metadata = database.plugin_user_metadata(plugin, EvalMode::DoNotEvaluate).map_err(|e| e.to_string())?
                .map_or_else(|| libloot::metadata::PluginMetadata::new(plugin).map_err(|e| e.to_string()), Ok)?;
            let current = match kind { "after" => metadata.load_after_files(), "requires" => metadata.requirements(), "incompatible" => metadata.incompatibilities(), _ => return Err("Tipo de regra desconhecido.".into()) };
            let mut files: Vec<_> = current.iter().filter(|item| !item.name().as_str().eq_ignore_ascii_case(target)).cloned().collect();
            if add { files.push(file(target)); }
            match kind { "after" => metadata.set_load_after_files(files), "requires" => metadata.set_requirements(files), _ => metadata.set_incompatibilities(files) }
            database.discard_plugin_user_metadata(plugin);
            if !metadata.has_name_only() { database.set_plugin_user_metadata(metadata); }
            Ok(())
        })?;
        change(game, |_, _| Ok(()))
    }
}
/// Puts a plugin in a LOOT group (or back in the default one) through the userlist.
#[tauri::command]
pub async fn set_nexus_plugin_group(game_id: String, plugin: String, group: Option<String>) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| set_group(game, &plugin, group))).await.map_err(|e| e.to_string())?
}
fn set_group(game: &NexusGame, plugin: &str, group: Option<String>) -> Result<PluginsView, String> {
    {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        edit_userlist(game, &module, |database| {
            if let Some(group) = &group {
                if !database.groups(MergeMode::WithUserMetadata).iter().any(|known| known.name() == group) { return Err("Grupo desconhecido.".into()); }
            }
            let mut metadata = database.plugin_user_metadata(plugin, EvalMode::DoNotEvaluate).map_err(|e| e.to_string())?
                .map_or_else(|| libloot::metadata::PluginMetadata::new(plugin).map_err(|e| e.to_string()), Ok)?;
            match &group { Some(group) => metadata.set_group(group.clone()), None => metadata.unset_group() }
            database.discard_plugin_user_metadata(plugin);
            if !metadata.has_name_only() { database.set_plugin_user_metadata(metadata); }
            Ok(())
        })?;
        change(game, |_, _| Ok(()))
    }
}
/// Creates a user group loading after the given groups (Vortex group editor).
#[tauri::command]
pub async fn add_nexus_plugin_group(game_id: String, name: String, after: Vec<String>) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game| add_group(game, &name, after))).await.map_err(|e| e.to_string())?
}
fn add_group(game: &NexusGame, name: &str, after: Vec<String>) -> Result<PluginsView, String> {
    {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        let name = name.trim().to_string();
        if name.is_empty() || name.len() > 80 { return Err("Nome de grupo inválido.".into()); }
        edit_userlist(game, &module, |database| {
            let known = database.groups(MergeMode::WithUserMetadata);
            if known.iter().any(|group| group.name().eq_ignore_ascii_case(&name)) { return Err("Esse grupo já existe.".into()); }
            if after.iter().any(|group| !known.iter().any(|item| item.name() == group)) { return Err("Grupo anterior desconhecido.".into()); }
            let mut groups = database.user_groups().to_vec();
            groups.push(libloot::metadata::Group::new(name.clone()).with_after_groups(after.clone()));
            database.set_user_groups(groups);
            Ok(())
        })?;
        view(game)
    }
}
/// Deploys the profile again after a page choice that changes profile copies.
fn redeploy(game_id: &str, update: impl FnOnce(&mut Store, bool) -> Result<(), String>, name: &str) -> Result<PluginsView, String> {
    let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
    let mut data = super::nexus_local::load()?;
    let index = data.games.iter().position(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    let mut game = data.games[index].clone();
    super::nexus_deployment::idle(&game)?;
    let module = super::nexus_deployment::definition(&game).ok_or("Módulo do jogo não identificado.")?;
    let rule = rule(&module)?;
    let root = PathBuf::from(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let live = super::nexus_discovery::locate(&root, &rule.data).unwrap_or_else(|| root.join(&rule.data));
    let managed = game.mods.iter().filter(|m| m.installed && m.enabled)
        .flat_map(|m| m.installed_files.iter().chain(m.fomod_files.iter()).flatten())
        .any(|(_, target)| target.strip_prefix(&format!("{}/", rule.data)).is_some_and(|file| file.eq_ignore_ascii_case(name)));
    let previous = Store::load(&game)?;
    let mut store = Store::load(&game)?;
    update(&mut store, managed)?;
    if !managed {
        // External plugin: rename it in the game folder directly.
        let ghost = store.is_ghosted(name);
        let (from, to) = if ghost { (name.to_string(), format!("{name}.ghost")) } else { (format!("{name}.ghost"), name.to_string()) };
        if let Some(path) = super::nexus_discovery::locate(&live, &from) {
            if live.join(&to).exists() { return Err(format!("Já existe {to} na pasta Data; nada foi mudado.")); }
            fs::rename(&path, live.join(&to)).map_err(|e| e.to_string())?;
        }
        store.save(&game)?;
        return change(&game, |_, _| Ok(()));
    }
    store.save(&game)?;
    let old = game.profile.clone();
    let deployed = super::nexus_profile::build(&game).and_then(|revision| {
        game.profile = Some(revision.clone());
        super::nexus_deployment::apply(&game).map_err(|error| { super::nexus_profile::discard(&game, &revision); error })
    });
    let tx = match deployed { Ok(tx) => tx, Err(error) => { let _ = previous.save(&game); return Err(error); } };
    data.games[index] = game.clone();
    if let Err(error) = super::nexus_local::save(&data) {
        drop(tx);
        if let Some(revision) = &game.profile { super::nexus_profile::discard(&game, revision); }
        let _ = previous.save(&game);
        return Err(error);
    }
    tx.commit();
    if let Some(old) = old.filter(|old| game.profile.as_ref() != Some(old)) { super::nexus_profile::discard(&game, &old); }
    view(&game)
}
/// Hides a plugin from the game (`.ghost`) or shows it again (Vortex "Ghost").
#[tauri::command]
pub async fn set_nexus_plugin_ghost(game_id: String, name: String, ghost: bool) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_plugin(&name) || name.contains('/') { return Err("Plugin inválido.".into()); }
        let target = name.clone();
        redeploy(&game_id, move |store, _| {
            store.ghosted.retain(|item| !item.eq_ignore_ascii_case(&target));
            if ghost { store.ghosted.push(target.to_ascii_lowercase()); }
            Ok(())
        }, &name)
    }).await.map_err(|e| e.to_string())?
}
/// Marks a plugin of a mod as light or regular (Vortex "Mark as Light").
#[tauri::command]
pub async fn set_nexus_plugin_light(game_id: String, name: String, light: Option<bool>) -> Result<PluginsView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !is_plugin(&name) || name.contains('/') { return Err("Plugin inválido.".into()); }
        let target = name.clone();
        redeploy(&game_id, move |store, managed| {
            if !managed { return Err("Só plugins instalados pelo PeliGames podem ser marcados; o arquivo original não é alterado.".into()); }
            store.light.retain(|key, _| !key.eq_ignore_ascii_case(&target));
            if let Some(light) = light { store.light.insert(target.to_ascii_lowercase(), light); }
            Ok(())
        }, &name)
    }).await.map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn module(name: &str) -> Definition {
        super::super::nexus_modules::registry().definitions.into_iter().find(|m| m.name == name).unwrap()
    }
    #[test]
    fn deployment_lines_follow_the_manual_order_and_disabled_plugins() {
        let store = Store { disabled: vec!["b.esp".into(), "skyrim.esm".into()], order: vec!["C.esp".into(), "A.esp".into(), "B.esp".into()], autosort: false, ..Store::default() };
        let lines: Vec<String> = ["# comment", "*A.esp", "*B.esp", "*C.esp", "*New.esp"].map(String::from).to_vec();
        let sse = module("Skyrim Special Edition");
        assert_eq!(arrange(&store, &sse, true, lines.clone()), ["# comment", "*C.esp", "*A.esp", "B.esp", "*New.esp"]);
        // Lists of active plugins only drop the disabled one; game plugins stay active.
        let nv = module("Fallout New Vegas");
        let unstarred: Vec<String> = ["FalloutNV.esm", "A.esp", "B.esp"].map(String::from).to_vec();
        let store = Store { disabled: vec!["b.esp".into(), "falloutnv.esm".into()], order: Vec::new(), autosort: true, ..Store::default() };
        assert_eq!(arrange(&store, &nv, false, unstarred), ["FalloutNV.esm", "A.esp"]);
    }
    /// Minimal plugin: TES4 header with HEDR and masters.
    fn plugin(path: &Path, masters: &[&str], flags: u32) {
        let mut body = Vec::new();
        body.extend_from_slice(b"HEDR"); body.extend_from_slice(&12u16.to_le_bytes());
        body.extend_from_slice(&1.34f32.to_le_bytes()); body.extend_from_slice(&0i32.to_le_bytes()); body.extend_from_slice(&0x800u32.to_le_bytes());
        for master in masters {
            let name = [master.as_bytes(), b"\0"].concat();
            body.extend_from_slice(b"MAST"); body.extend_from_slice(&(name.len() as u16).to_le_bytes()); body.extend_from_slice(&name);
            body.extend_from_slice(b"DATA"); body.extend_from_slice(&8u16.to_le_bytes()); body.extend_from_slice(&0u64.to_le_bytes());
        }
        let mut bytes = b"TES4".to_vec();
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&body);
        fs::write(path, bytes).unwrap();
    }
    #[test]
    #[ignore = "Requires an isolated XDG_CONFIG_HOME (/tmp/peligames-*)"]
    fn new_vegas_plugins_page_enables_orders_and_dates_plugins() {
        let config_root = PathBuf::from(std::env::var("XDG_CONFIG_HOME").expect("Isolated config required"));
        assert!(config_root.starts_with("/tmp") && config_root.file_name().unwrap().to_string_lossy().starts_with("peligames-"));
        let base = config_root.join(format!("nv-plugins-{}", uuid::Uuid::new_v4()));
        let (root, prefix) = (base.join("game"), base.join("prefix"));
        fs::create_dir_all(root.join("Data")).unwrap();
        fs::write(root.join("FalloutNV.exe"), b"MZ fixture").unwrap();
        plugin(&root.join("Data/FalloutNV.esm"), &[], 1);
        plugin(&root.join("Data/A.esp"), &["FalloutNV.esm"], 0);
        plugin(&root.join("Data/B.esp"), &["FalloutNV.esm", "Gone.esm"], 0);
        let old = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_300_000_000);
        for (index, name) in ["FalloutNV.esm", "A.esp", "B.esp"].iter().enumerate() {
            fs::File::open(root.join("Data").join(name)).unwrap().set_modified(old + Duration::from_secs(60 * index as u64)).unwrap();
        }
        let local = prefix.join("pfx/drive_c/users/steamuser/AppData/Local/FalloutNV");
        fs::create_dir_all(&local).unwrap();
        fs::write(prefix.join("pfx/system.reg"), "").unwrap();
        fs::write(local.join("plugins.txt"), "FalloutNV.esm\r\nA.esp\r\nB.esp\r\n").unwrap();
        let game: NexusGame = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "game": {"directory": root, "name": "Fallout New Vegas", "path": root.join("FalloutNV.exe")},
            "platform": "proton", "compat_data": prefix, "mods": []
        })).unwrap();
        let names = |view: &PluginsView| view.plugins.iter().map(|p| (p.name.clone(), p.enabled)).collect::<Vec<_>>();
        let page = view(&game).unwrap();
        assert_eq!(names(&page), [("FalloutNV.esm".into(), true), ("A.esp".into(), true), ("B.esp".into(), true)]);
        assert!(page.plugins[0].native && page.plugins[0].master && page.plugins[1].index.as_deref() == Some("01"));
        assert_eq!(page.plugins[2].missing, ["Gone.esm"]);
        assert_eq!((page.counts, page.limits[0]), ([3, 0, 0], 255));
        // Disabling leaves the plugin out of the list of active plugins.
        let page = change(&game, |store, order| { store.autosort = false; store.disabled.push("a.esp".into()); let _ = order; Ok(()) }).unwrap();
        assert_eq!(fs::read_to_string(local.join("plugins.txt")).unwrap(), "FalloutNV.esm\r\nB.esp\r\n");
        assert!(!page.plugins.iter().find(|p| p.name == "A.esp").unwrap().enabled);
        // Manual order: B before A, written as file dates.
        let page = change(&game, |_, order| { *order = vec!["FalloutNV.esm".into(), "B.esp".into(), "A.esp".into()]; Ok(()) }).unwrap();
        assert_eq!(page.plugins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["FalloutNV.esm", "B.esp", "A.esp"]);
        let time = |name: &str| fs::metadata(root.join("Data").join(name)).unwrap().modified().unwrap();
        assert!(time("FalloutNV.esm") < time("B.esp") && time("B.esp") < time("A.esp"));
        let store = Store::load(&game).unwrap();
        assert!(!store.autosort && store.order == ["FalloutNV.esm", "B.esp", "A.esp"]);
        // The game master stays active even if marked off.
        change(&game, |store, _| { store.disabled.push("falloutnv.esm".into()); Ok(()) }).unwrap();
        assert!(fs::read_to_string(local.join("plugins.txt")).unwrap().starts_with("FalloutNV.esm\r\n"));
        fs::remove_dir_all(&base).unwrap();
    }
    #[test]
    #[ignore = "Requires an isolated XDG_CONFIG_HOME (/tmp/peligames-*)"]
    fn userlist_rules_groups_ghost_and_light_flag() {
        let config_root = PathBuf::from(std::env::var("XDG_CONFIG_HOME").expect("Isolated config required"));
        assert!(config_root.starts_with("/tmp") && config_root.file_name().unwrap().to_string_lossy().starts_with("peligames-"));
        let base = config_root.join(format!("sse-plugins-{}", uuid::Uuid::new_v4()));
        let (root, prefix) = (base.join("game"), base.join("prefix"));
        fs::create_dir_all(root.join("Data")).unwrap();
        fs::write(root.join("SkyrimSE.exe"), b"MZ fixture").unwrap();
        plugin(&root.join("Data/Skyrim.esm"), &[], 1);
        plugin(&root.join("Data/A.esp"), &["Skyrim.esm"], 0);
        plugin(&root.join("Data/B.esp"), &["Skyrim.esm"], 0);
        let local = prefix.join("pfx/drive_c/users/steamuser/AppData/Local/Skyrim Special Edition");
        fs::create_dir_all(&local).unwrap();
        fs::write(prefix.join("pfx/system.reg"), "").unwrap();
        fs::write(local.join("plugins.txt"), "*A.esp\r\n*B.esp\r\n").unwrap();
        let game: NexusGame = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "game": {"directory": root, "name": "Skyrim Special Edition", "path": root.join("SkyrimSE.exe")},
            "platform": "proton", "compat_data": prefix, "mods": []
        })).unwrap();
        let page = set_rule(&game, "A.esp", "after", "B.esp", true).unwrap();
        let a = page.plugins.iter().find(|p| p.name == "A.esp").unwrap();
        assert_eq!(a.rules.iter().map(|r| (r.kind, r.plugin.as_str())).collect::<Vec<_>>(), [("after", "B.esp")]);
        assert!(userlist(&game).unwrap().is_file());
        assert!(set_rule(&game, "A.esp", "sideways", "B.esp", true).is_err());
        let page = add_group(&game, "Late", vec!["default".into()]).unwrap();
        assert!(page.groups.contains(&"Late".to_string()));
        let page = set_group(&game, "A.esp", Some("Late".into())).unwrap();
        assert_eq!(page.plugins.iter().find(|p| p.name == "A.esp").unwrap().group.as_deref(), Some("Late"));
        let page = set_rule(&game, "A.esp", "after", "B.esp", false).unwrap();
        assert!(page.plugins.iter().find(|p| p.name == "A.esp").unwrap().rules.is_empty());
        // Profile copies: ghosted plugins are renamed, the light flag goes into the copy's header.
        let runtime = base.join("stage");
        fs::create_dir_all(runtime.join("Data")).unwrap();
        plugin(&runtime.join("Data/A.esp"), &["Skyrim.esm"], 0);
        plugin(&runtime.join("Data/B.esp"), &["Skyrim.esm"], 0);
        let mut store = Store::load(&game).unwrap();
        store.ghosted.push("b.esp".into());
        store.light.insert("a.esp".into(), true);
        store.save(&game).unwrap();
        let module = super::super::nexus_deployment::definition(&game).unwrap();
        prepare(&game, &module, &runtime).unwrap();
        assert!(runtime.join("Data/B.esp.ghost").is_file() && !runtime.join("Data/B.esp").exists());
        let header = fs::read(runtime.join("Data/A.esp")).unwrap();
        assert_eq!(u32::from_le_bytes(header[8..12].try_into().unwrap()) & 0x200, 0x200);
        // Shown again: the copy gets its name back.
        store.ghosted.clear();
        store.save(&game).unwrap();
        prepare(&game, &module, &runtime).unwrap();
        assert!(runtime.join("Data/B.esp").is_file());
        assert_eq!(arrange(&Store { ghosted: vec!["a.esp".into()], ..Store::default() }, &module, true, vec!["*A.esp".into()]), ["A.esp"]);
        fs::remove_dir_all(&base).unwrap();
    }
    #[test]
    fn timestamp_order_gives_each_plugin_a_later_minute() {
        let dir = std::env::temp_dir().join(format!("peligames-plugins-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        for name in ["FalloutNV.esm", "A.esp", "B.esp"] { fs::write(dir.join(name), name).unwrap(); }
        let old = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_300_000_000);
        fs::File::open(dir.join("FalloutNV.esm")).unwrap().set_modified(old).unwrap();
        order_by_time(&dir, &["FalloutNV.esm".into(), "B.esp".into(), "a.ESP".into()]).unwrap();
        let time = |name: &str| fs::metadata(dir.join(name)).unwrap().modified().unwrap();
        assert_eq!(time("FalloutNV.esm"), old);
        assert_eq!(time("B.esp"), old + Duration::from_secs(60));
        assert_eq!(time("A.esp"), old + Duration::from_secs(120));
        fs::remove_dir_all(dir).unwrap();
    }
}
