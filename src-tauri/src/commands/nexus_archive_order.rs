//! Fallout 76 archive list, as the community Vortex extensions "Fallout 76
//! Support" (BA2 archives of enabled mods in `[Archive] sResourceArchive2List`
//! of the Custom INI) and "BA2 Archive Load Order" (the player's order).
use super::nexus_local::NexusGame;
use std::{collections::BTreeMap, fs, path::PathBuf};

/// Custom INI the extension creates when the game has none yet.
const DEFAULT_INI: &str = "[Archive]\r\nsResourceArchive2List=\r\nsResourceDataDirsFinal=STRINGS\\\r\nbInvalidateOlderFiles=1\r\n";
const KEY: &str = "sResourceArchive2List";

fn list_of(text: &str) -> Vec<String> {
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') { inside = trimmed.eq_ignore_ascii_case("[Archive]"); continue; }
        if let Some((key, value)) = trimmed.split_once('=').filter(|_| inside) {
            if key.trim().eq_ignore_ascii_case(KEY) {
                return value.split(',').map(str::trim).filter(|item| !item.is_empty()).map(str::to_string).collect();
            }
        }
    }
    Vec::new()
}
fn write_list(text: &str, list: &[String]) -> String {
    let base = if text.trim().is_empty() { DEFAULT_INI } else { text };
    super::nexus_activation::ini(base, "Archive", &[format!("{KEY}={}", list.join(", "))], |line| line.split_once('=').is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case(KEY)))
}
/// Archives of enabled mods first (as the extension writes them), then the
/// entries already listed; archives of mods no longer enabled leave the list;
/// a saved order is applied last (BA2 Archive Load Order).
pub(super) fn compose(original: &str, enabled: &[String], removed: &[String], saved: &[String]) -> String {
    let base = if original.trim().is_empty() { DEFAULT_INI } else { original };
    let mut list: Vec<String> = Vec::new();
    for item in enabled.iter().chain(list_of(base).iter().filter(|item| !removed.iter().any(|gone| gone.eq_ignore_ascii_case(item)))) {
        if !list.iter().any(|known| known.eq_ignore_ascii_case(item)) { list.push(item.clone()); }
    }
    if !saved.is_empty() {
        let position = |name: &String| saved.iter().position(|known| known.eq_ignore_ascii_case(name)).unwrap_or(usize::MAX);
        let mut indexed: Vec<(usize, String)> = list.into_iter().enumerate().collect();
        indexed.sort_by_key(|(index, name)| (position(name), *index));
        list = indexed.into_iter().map(|(_, name)| name).collect();
    }
    write_list(base, &list)
}
fn top_archives<'a>(paths: impl Iterator<Item = &'a String>, data: &str) -> Vec<String> {
    paths.filter_map(|path| path.strip_prefix(&format!("{data}/"))).filter(|name| !name.contains('/') && name.to_ascii_lowercase().ends_with(".ba2"))
        .map(str::to_string).collect()
}
fn store(game: &NexusGame) -> Result<PathBuf, String> {
    Ok(super::nexus_profile::directory(game)?.join("ba2-order.json"))
}
fn saved(game: &NexusGame) -> Vec<String> {
    store(game).ok().and_then(|path| fs::read(path).ok()).and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}
/// Activation of a deployment: `paths` are the files being deployed, `old` the previous ones.
pub(super) fn activation(game: &NexusGame, data: &str, paths: &[String], original: &str, old: &BTreeMap<String, String>) -> String {
    // Mod priority order: archives of earlier mods come first.
    let mut enabled: Vec<String> = Vec::new();
    for m in game.mods.iter().filter(|m| m.installed && m.enabled) {
        for name in top_archives(m.installed_files.iter().chain(m.fomod_files.iter()).flatten().map(|(_, target)| target), data) {
            if paths.iter().any(|path| path.eq_ignore_ascii_case(&format!("{data}/{name}"))) && !enabled.contains(&name) { enabled.push(name); }
        }
    }
    for name in top_archives(paths.iter(), data) { if !enabled.iter().any(|known| known.eq_ignore_ascii_case(&name)) { enabled.push(name); } }
    let removed: Vec<String> = top_archives(old.keys(), data).into_iter().filter(|name| !enabled.iter().any(|known| known.eq_ignore_ascii_case(name))).collect();
    compose(original, &enabled, &removed, &saved(game))
}
#[derive(serde::Serialize)]
pub struct Entry { name: String, managed: Option<String> }
#[derive(serde::Serialize)]
pub struct View { entries: Vec<Entry>, ini: String }
fn rule(game: &NexusGame) -> Result<super::nexus_modules::Activation, String> {
    super::nexus_deployment::definition(game).and_then(|module| module.activation)
        .filter(|rule| rule.kind == super::nexus_modules::ActivationKind::ArchiveList)
        .ok_or_else(|| "Este jogo não tem lista de arquivos BA2.".into())
}
fn view(game: &NexusGame) -> Result<View, String> {
    let rule = rule(game)?;
    let ini = super::nexus_deployment::destination(game, &rule.path)?;
    let text = fs::read(&ini).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
    let owner = |name: &str| game.mods.iter().filter(|m| m.installed && m.enabled).find(|m| m.installed_files.iter().chain(m.fomod_files.iter()).flatten()
        .any(|(_, target)| target.eq_ignore_ascii_case(&format!("{}/{name}", rule.data))))
        .map(|m| m.nexus_source.as_ref().map(|s| s.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| m.name.clone()));
    let entries = list_of(&text).into_iter().map(|name| Entry { managed: owner(&name), name }).collect();
    Ok(View { entries, ini: ini.to_string_lossy().into_owned() })
}
fn with_game<T>(game_id: &str, change: bool, action: impl FnOnce(&NexusGame) -> Result<T, String>) -> Result<T, String> {
    let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
    let data = super::nexus_local::load()?;
    let game = data.games.iter().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    if change { super::nexus_deployment::idle(game)?; }
    action(game)
}
#[tauri::command]
pub async fn nexus_archive_order(game_id: String) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, false, view)).await.map_err(|e| e.to_string())?
}
/// Saves the order and writes it to the Custom INI ("Save & Apply").
#[tauri::command]
pub async fn set_nexus_archive_order(game_id: String, order: Vec<String>) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, true, |game| {
        let rule = rule(game)?;
        let ini = super::nexus_deployment::destination(game, &rule.path)?;
        let text = fs::read(&ini).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
        let mut current: Vec<String> = list_of(&text).iter().map(|name| name.to_ascii_lowercase()).collect();
        let mut asked: Vec<String> = order.iter().map(|name| name.to_ascii_lowercase()).collect();
        current.sort(); asked.sort();
        if current != asked { return Err("A lista de arquivos mudou; ela foi recarregada.".into()); }
        let path = store(game)?;
        fs::create_dir_all(path.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
        fs::write(&path, serde_json::to_vec_pretty(&order).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        fs::create_dir_all(ini.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
        fs::write(&ini, write_list(&text, &order)).map_err(|e| e.to_string())?;
        view(game)
    })).await.map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_list_follows_enabled_mods_and_keeps_manual_entries() {
        let original = "[Display]\r\nx=1\r\n[Archive]\r\nsResourceArchive2List=Manual.ba2, Old.ba2\r\n";
        let out = compose(original, &["New.ba2".into()], &["Old.ba2".into()], &[]);
        assert_eq!(list_of(&out), ["New.ba2", "Manual.ba2"]);
        assert!(out.contains("[Display]\r\nx=1"));
        // Saved order wins; entries it does not know keep their place after it.
        let out = compose(&out, &["New.ba2".into(), "Other.ba2".into()], &[], &["Manual.ba2".into(), "New.ba2".into()]);
        assert_eq!(list_of(&out), ["Manual.ba2", "New.ba2", "Other.ba2"]);
        // A missing Custom INI is created like the extension does.
        let created = compose("", &["A.ba2".into()], &[], &[]);
        assert!(created.contains("sResourceDataDirsFinal=STRINGS\\\r\n") && created.contains("bInvalidateOlderFiles=1"));
        assert_eq!(list_of(&created), ["A.ba2"]);
    }
}
