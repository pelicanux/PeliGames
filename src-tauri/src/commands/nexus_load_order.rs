//! Baldur's Gate 3 load order page, as in the Vortex extension: arrange the
//! paks of the Mods folder, import from the game, a .lsx or a BG3 Mod Manager
//! file, and export to the game or a file.
use super::nexus_bg3::{self, Store, View};
use super::nexus_local::NexusGame;
use std::fs;

fn with_game<T>(game_id: &str, change: bool, action: impl FnOnce(&NexusGame) -> Result<T, String>) -> Result<T, String> {
    let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
    let data = super::nexus_local::load()?;
    let game = data.games.iter().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    if change { super::nexus_deployment::idle(game)?; }
    action(game)
}
/// Writes the load order into the game's modsettings.lsx (a managed
/// configuration file, so later deployments read it back from the prefix).
fn export_to_game(game: &NexusGame) -> Result<(), String> {
    let content = nexus_bg3::exported(game)?;
    let path = super::nexus_deployment::destination(game, &nexus_bg3::rule(game)?.path)?;
    fs::create_dir_all(path.parent().ok_or("Destino inválido.")?).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}
fn save_order(game: &NexusGame, order: Vec<String>) -> Result<View, String> {
    let mut store = Store::load(game)?;
    store.order = order;
    store.save(game)?;
    if store.auto_export { export_to_game(game)?; }
    nexus_bg3::view(game)
}
#[tauri::command]
pub async fn nexus_load_order(game_id: String) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, false, nexus_bg3::view))
        .await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_nexus_load_order(game_id: String, order: Vec<String>) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, true, |game| {
        let current: Vec<String> = nexus_bg3::current(game)?.into_iter().map(|(name, _)| name).collect();
        let mut sorted = order.clone(); sorted.sort();
        let mut known = current.clone(); known.sort();
        if sorted != known { return Err("A pasta Mods mudou; a ordem foi recarregada.".into()); }
        save_order(game, order)
    })).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_nexus_load_order_auto_export(game_id: String, enabled: bool) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, false, |game| {
        let mut store = Store::load(game)?;
        store.auto_export = enabled;
        store.save(game)?;
        nexus_bg3::view(game)
    })).await.map_err(|e| e.to_string())?
}
/// `source`: empty for the game's modsettings.lsx, otherwise a .lsx or a BG3 Mod Manager .json.
#[tauri::command]
pub async fn import_nexus_load_order(game_id: String, source: String) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, true, |game| {
        let path = if source.is_empty() { super::nexus_deployment::destination(game, &nexus_bg3::rule(game)?.path)? } else { source.clone().into() };
        if fs::metadata(&path).map_err(|_| "Arquivo de ordem de carga não encontrado. Inicie o jogo uma vez.")?.len() > 2 * 1024 * 1024 {
            return Err("Arquivo de ordem de carga grande demais.".into());
        }
        let text = fs::read_to_string(&path).map_err(|_| "O arquivo de ordem de carga não é texto UTF-8.")?;
        let json = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
        let uuids = if json { nexus_bg3::bg3mm_order(&text)? } else { nexus_bg3::lsx_order(&text)? };
        let order = nexus_bg3::reorder(&nexus_bg3::current(game)?, &uuids);
        save_order(game, order)
    })).await.map_err(|e| e.to_string())?
}
/// `target`: empty for the game's modsettings.lsx, otherwise a file chosen by the player.
#[tauri::command]
pub async fn export_nexus_load_order(game_id: String, target: String) -> Result<View, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, target.is_empty(), |game| {
        if target.is_empty() { export_to_game(game)?; }
        else { fs::write(&target, nexus_bg3::exported(game)?).map_err(|e| e.to_string())?; }
        nexus_bg3::view(game)
    })).await.map_err(|e| e.to_string())?
}
