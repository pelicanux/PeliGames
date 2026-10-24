//! Launch options a mod needs (the DLSSNR backend's Vulkan layers and DLL
//! overrides), kept in the game's own PeliGames launch options. Installing,
//! repairing or removing the mod swaps the entries it added before for the
//! ones it needs now; everything else the user typed stays as it was.
use super::nexus_launch_options::{describe, parse, Options};
use std::{collections::BTreeMap, fs, path::PathBuf};

/// Variables whose value is a list: the mod adds and removes single items.
fn separator(key: &str) -> Option<char> {
    match key {
        "VK_ADD_LAYER_PATH" | "VK_INSTANCE_LAYERS" => Some(':'),
        "WINEDLLOVERRIDES" => Some(';'),
        _ => None,
    }
}
/// The item's identity: a DLL override is replaced by another one for the same DLLs.
fn identity(key: &str, item: &str) -> String {
    if key == "WINEDLLOVERRIDES" { item.split('=').next().unwrap_or(item).trim().to_ascii_lowercase() } else { item.to_string() }
}
fn items(value: &str, separator: char) -> Vec<String> {
    value.split(separator).map(str::trim).filter(|item| !item.is_empty()).map(str::to_string).collect()
}

/// `current` with the variables of `previous` taken out and those of `next` put in.
pub(crate) fn merge(current: Option<&str>, previous: Option<&str>, next: Option<&str>) -> Result<Option<String>, String> {
    let mut options = match current.map(str::trim).filter(|value| !value.is_empty()) { Some(value) => parse(value)?, None => Options::default() };
    let previous = match previous { Some(value) => parse(value)?.env, None => BTreeMap::new() };
    let next = match next { Some(value) => parse(value)?.env, None => BTreeMap::new() };
    for (key, value) in &previous {
        let Some(current) = options.env.get(key).cloned() else { continue };
        match separator(key) {
            Some(separator) => {
                let gone: Vec<String> = items(value, separator).iter().map(|item| identity(key, item)).collect();
                let kept: Vec<String> = items(&current, separator).into_iter().filter(|item| !gone.contains(&identity(key, item))).collect();
                if kept.is_empty() { options.env.remove(key); } else { options.env.insert(key.clone(), kept.join(&separator.to_string())); }
            }
            None => if &current == value { options.env.remove(key); },
        }
    }
    for (key, value) in next {
        match (separator(&key), options.env.get(&key).cloned()) {
            (Some(separator), Some(current)) => {
                let added = items(&value, separator);
                let ids: Vec<String> = added.iter().map(|item| identity(&key, item)).collect();
                let mut merged: Vec<String> = items(&current, separator).into_iter().filter(|item| !ids.contains(&identity(&key, item))).collect();
                merged.extend(added);
                options.env.insert(key, merged.join(&separator.to_string()));
            }
            _ => { options.env.insert(key, value); }
        }
    }
    let text = describe(&options);
    Ok((!text.trim().is_empty()).then_some(text))
}

fn record_file() -> PathBuf {
    crate::core::config_manager::get_mod_config_path().with_file_name("launch-options.json")
}
fn read_records() -> Result<BTreeMap<String, String>, String> {
    match fs::read(record_file()) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Registro das opções de inicialização do mod inválido.".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(format!("Não foi possível ler as opções de inicialização do mod: {error}")),
    }
}
fn key(game_dir: &str) -> String {
    fs::canonicalize(game_dir).map(|path| path.to_string_lossy().into_owned()).unwrap_or_else(|_| game_dir.trim_end_matches('/').to_string())
}

fn write_records(records: &BTreeMap<String, String>) -> Result<(), String> {
    let path = record_file();
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let temporary = path.with_extension("json.part");
    fs::write(&temporary, serde_json::to_vec_pretty(records).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&temporary, &path).map_err(|e| e.to_string())
}
/// Takes out the line the mod had put in the PeliGames launch options of
/// `game_dir` (a Steam game now keeps it in Steam), returning it.
pub(crate) fn take_record(game_dir: &str) -> Result<Option<String>, String> {
    let mut records = read_records()?;
    let taken = records.remove(&key(game_dir));
    if taken.is_some() { write_records(&records)?; }
    Ok(taken)
}
/// Lines recorded for folders inside `root` (the DLSS view uses the
/// executable's folder, which may be below the game's own folder).
pub(crate) fn take_records_under(root: &str) -> Result<Vec<String>, String> {
    let root = key(root);
    let mut records = read_records()?;
    let inside: Vec<String> = records.keys().filter(|path| *path == &root || path.starts_with(&format!("{root}/"))).cloned().collect();
    let taken: Vec<String> = inside.iter().filter_map(|path| records.remove(path)).collect();
    if !taken.is_empty() { write_records(&records)?; }
    Ok(taken)
}

/// What the launch options of the game in `game_dir` are reset to: only the
/// lines that installed mods (recorded below that folder) need, nothing typed by hand.
#[tauri::command]
pub fn mod_launch_options_for(game_dir: String) -> Result<Option<String>, String> {
    restored(&read_records()?, &key(&game_dir))
}
fn restored(records: &BTreeMap<String, String>, root: &str) -> Result<Option<String>, String> {
    records.iter().filter(|(path, _)| path.as_str() == root || path.starts_with(&format!("{root}/")))
        .try_fold(None, |current: Option<String>, (_, line)| merge(current.as_deref(), None, Some(line)))
}

/// Updates a game's launch options after the mod was installed (`line`: the
/// options the backend printed) or removed (`line` empty) in `game_dir`.
/// Returns the new launch options; the caller saves them in the game's settings.
#[tauri::command]
pub fn apply_mod_launch_options(game_dir: String, current: Option<String>, line: Option<String>) -> Result<Option<String>, String> {
    let line = line.map(|value| value.trim().to_string()).filter(|value| !value.is_empty());
    let mut records = read_records()?;
    let key = key(&game_dir);
    let merged = merge(current.as_deref(), records.get(&key).map(String::as_str), line.as_deref())?;
    match line { Some(line) => { records.insert(key, line); } None => { records.remove(&key); } }
    write_records(&records)?;
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::merge;
    #[test]
    fn reset_keeps_only_the_lines_of_mods_installed_in_the_game() {
        let records = std::collections::BTreeMap::from([
            ("/games/Hades/bin".to_string(), "WINEDLLOVERRIDES=dxgi=n,b %command%".to_string()),
            ("/games/Hades2".to_string(), "OTHER=1 %command%".to_string()),
        ]);
        assert_eq!(super::restored(&records, "/games/Hades").unwrap().as_deref(), Some(r#"WINEDLLOVERRIDES="dxgi=n,b""#));
        assert_eq!(super::restored(&records, "/games/Celeste").unwrap(), None);
    }

    #[test]
    fn int4_layers_are_added_kept_apart_from_user_options_and_removed() {
        let int4 = r#"VK_ADD_LAYER_PATH="/g/dlssnr-amd/int4/layer" VK_INSTANCE_LAYERS=VK_LAYER_DLSSNR_int4 WINEDLLOVERRIDES="dxgi=n,b" %command%"#;
        let added = merge(Some("MANGOHUD=1 WINEDLLOVERRIDES=\"winmm=n,b\" %command% -dx11"), None, Some(int4)).unwrap().unwrap();
        assert_eq!(added, r#"MANGOHUD="1" VK_ADD_LAYER_PATH="/g/dlssnr-amd/int4/layer" VK_INSTANCE_LAYERS="VK_LAYER_DLSSNR_int4" WINEDLLOVERRIDES="winmm=n,b;dxgi=n,b" %command% "-dx11""#);
        // Repair without int4: the layers leave, the route's override stays.
        let repaired = merge(Some(&added), Some(int4), Some(r#"WINEDLLOVERRIDES="dxgi=n,b" %command%"#)).unwrap().unwrap();
        assert_eq!(repaired, r#"MANGOHUD="1" WINEDLLOVERRIDES="winmm=n,b;dxgi=n,b" %command% "-dx11""#);
        // Removal takes out everything the mod added, nothing else.
        let removed = merge(Some(&repaired), Some(r#"WINEDLLOVERRIDES="dxgi=n,b" %command%"#), None).unwrap().unwrap();
        assert_eq!(removed, r#"MANGOHUD="1" WINEDLLOVERRIDES="winmm=n,b" %command% "-dx11""#);
    }

    #[test]
    fn empty_options_stay_empty_and_existing_entries_are_not_duplicated() {
        let line = r#"VK_INSTANCE_LAYERS=VK_LAYER_DLSSNR_int4 WINEDLLOVERRIDES="dxgi=n,b" %command%"#;
        let added = merge(None, None, Some(line)).unwrap().unwrap();
        assert_eq!(merge(Some(&added), None, Some(line)).unwrap().unwrap(), added);
        assert_eq!(merge(Some(&added), Some(line), None).unwrap(), None);
        // A DLL the user set another way is replaced by the mod's, not repeated.
        assert_eq!(merge(Some("WINEDLLOVERRIDES=\"dxgi=b\""), None, Some(line)).unwrap().unwrap(), r#"VK_INSTANCE_LAYERS="VK_LAYER_DLSSNR_int4" WINEDLLOVERRIDES="dxgi=n,b""#);
    }
}
