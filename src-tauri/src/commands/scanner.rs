use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;
use tauri::{AppHandle, Manager};
use reqwest::Client;

#[derive(Debug, Serialize, Clone)]
pub struct GameInfo {
    pub name: String,
    pub path: String,
    pub app_id: Option<String>,
    pub cover_url: Option<String>,
    pub launcher: String,
}

#[derive(Deserialize, Debug)]
struct SteamGridSearchResponse {
    pub success: bool,
    pub data: Vec<SteamGridGame>,
}

#[derive(Deserialize, Debug)]
struct SteamGridGame {
    pub id: u32,
    pub name: String,
}

#[derive(Deserialize, Debug)]
struct SteamGridGridResponse {
    pub success: bool,
    pub data: Vec<SteamGridGrid>,
}

#[derive(Deserialize, Debug)]
struct SteamGridGrid {
    pub url: String,
}

/// Helper to read ACF files
pub(super) fn parse_acf_file(path: &Path) -> Option<(String, String, String)> {
    let content = fs::read_to_string(path).ok()?;
    
    let mut appid = String::new();
    let mut name = String::new();
    let mut installdir = String::new();
    
    for line in content.lines() {
        let l = line.trim();
        if l.starts_with("\"appid\"") {
            appid = l.replace("\"appid\"", "").replace("\"", "").trim().to_string();
        } else if l.starts_with("\"name\"") {
            name = l.replace("\"name\"", "").replace("\"", "").trim().to_string();
        } else if l.starts_with("\"installdir\"") {
            installdir = l.replace("\"installdir\"", "").replace("\"", "").trim().to_string();
        }
    }
    
    if !appid.is_empty() && !name.is_empty() && !installdir.is_empty() {
        Some((appid, name, installdir))
    } else {
        None
    }
}

/// Helper to read libraryfolders.vdf and extract all library paths
fn parse_library_folders(path: &Path) -> Vec<PathBuf> {
    let mut folders = Vec::new();
    if let Ok(content) = fs::read_to_string(path) {
        for line in content.lines() {
            let l = line.trim();
            if l.starts_with("\"path\"") {
                let p = l.replace("\"path\"", "").replace("\"", "").trim().to_string();
                // vdf often double escapes slashes in windows but on linux it's usually just strings
                let p = p.replace("\\\\", "/");
                let folder_path = PathBuf::from(p);
                if folder_path.exists() {
                    folders.push(folder_path);
                }
            }
        }
    }
    folders
}

/// Steam library roots (each containing `steamapps`), native and Flatpak.
pub(super) fn steam_libraries(home: &Path) -> Vec<PathBuf> {
    let steam_paths = vec![
        home.join(".local/share/Steam"),
        home.join(".steam/steam"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ];
    let mut libraries = Vec::new();
    for steam_path in steam_paths {
        let vdf_path = steam_path.join("config/libraryfolders.vdf");
        if vdf_path.exists() {
            libraries.extend(parse_library_folders(&vdf_path));
            libraries.push(steam_path.clone());
        }
    }
    libraries.sort();
    libraries.dedup();
    libraries
}

pub(super) fn scan_steam(home: &Path) -> Vec<GameInfo> { scan_steam_inner(home, true, None) }
pub(super) fn scan_steam_main(home: &Path, flatpak: bool) -> Vec<GameInfo> { scan_steam_inner(home, false, Some(flatpak)) }
// Steam also installs support packages as app manifests. These belong to runner
// discovery, not the main game library. Match narrow families, never words like
// "legacy", "soldier" or "runtime" anywhere in a game's title.
fn steam_support_package(appid: &str, name: &str, installdir: &str) -> bool {
    if ["228980", "1070560", "1391110", "1628350", "4183110", "4690330", "1161040", "1826330", "3086180"].contains(&appid) { return true; }
    let name=name.trim().to_ascii_lowercase();
    let folder=installdir.trim().to_ascii_lowercase();
    let proton = name.strip_prefix("proton ").or_else(||name.strip_prefix("proton-")).is_some_and(|suffix|
        suffix.as_bytes().first().is_some_and(u8::is_ascii_digit)
        || ["experimental", "hotfix", "next", "battleye runtime", "easyanticheat runtime", "voice files"].contains(&suffix));
    name == "proton" || proton
        || name.starts_with("steam linux runtime") || name.starts_with("legacy steam runtime")
        || name.starts_with("steamworks common redistributables")
        || folder == "steamlinuxruntime" || folder.starts_with("steamlinuxruntime_")
        || folder == "legacysteamruntime" || folder.starts_with("legacysteamruntime_")
}

fn scan_steam_inner(home: &Path, mod_library: bool, source: Option<bool>) -> Vec<GameInfo> {
    let mut games = Vec::new();
    
    let steam_paths = match source {
        Some(flatpak) => super::steam_installation::roots(home, flatpak),
        None => super::steam_installation::roots(home, false).into_iter().chain(super::steam_installation::roots(home, true)).collect(),
    };

    let mut libraries = Vec::new();

    for steam_path in steam_paths {
        let vdf_path = steam_path.join("config/libraryfolders.vdf");
        if vdf_path.exists() || (!mod_library && steam_path.join("steamapps").is_dir()) {
            libraries.extend(parse_library_folders(&vdf_path));
            // Add the default steamapps folder as well just in case
            libraries.push(steam_path.clone());
        }
    }

    // Deduplicate libraries
    libraries.sort();
    libraries.dedup();

    for lib in libraries {
        let steamapps = lib.join("steamapps");
        if !steamapps.exists() { continue; }

        if let Ok(entries) = fs::read_dir(&steamapps) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("acf") {
                    if let Some((appid, name, installdir)) = parse_acf_file(&p) {
                        let lower_name = name.to_lowercase();
                        if (mod_library && (lower_name.contains("proton") || lower_name.contains("steam linux runtime") || lower_name.contains("steamworks common") || lower_name.contains("sniper") || lower_name.contains("soldier") || lower_name.contains("legacy") || lower_name.contains("runtime"))) || (!mod_library && steam_support_package(&appid, &name, &installdir)) {
                            continue;
                        }
                        let game_path = steamapps.join("common").join(&installdir);
                        if game_path.exists() {
                            games.push(GameInfo {
                                name,
                                path: game_path.to_string_lossy().to_string(),
                                app_id: Some(appid.clone()),
                                cover_url: Some(format!("https://steamcdn-a.akamaihd.net/steam/apps/{}/library_600x900.jpg", appid)),
                                launcher: "Steam".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    games
}

fn scan_heroic(home: &Path) -> Vec<GameInfo> {
    [home.join(".config/heroic"), home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic")]
        .iter().flat_map(|base| super::heroic_library::scan(base)).map(|game| GameInfo {
            name: game.name, path: game.path, app_id: None, cover_url: game.cover,
            launcher: "Heroic".to_string(),
        }).collect()
}

/// Store manifests already identify installation roots; do not apply DLSS executable heuristics.
pub(super) fn nexus_store_games(home: &Path) -> Vec<super::launcher_library::LauncherGame> {
    let mut games: Vec<_> = scan_steam(home).into_iter().map(|game| super::launcher_library::LauncherGame {
        native_linux: super::native_games::steam_native(Path::new(&game.path), game.app_id.as_deref().unwrap_or("")), name: game.name, path: game.path, app_id: game.app_id, cover_url: game.cover_url, launcher: game.launcher,
        prefix: None, proton: None, store: None, external_id: None, external_store: None, flatpak: false, executable: None,
    }).collect();
    // Heroic, Lutris, Faugus and Bottles report their own prefix and Proton runner.
    for game in super::launcher_library::all(home) {
        if !games.iter().any(|known| known.path == game.path) { games.push(game); }
    }
    games
}

async fn fetch_best_cover(name: &str, api_key: &str) -> Option<String> {
    let name = super::store_artwork::catalog_title(name);
    // Try Steam Store first since it might have games SteamGridDB doesn't have yet, and it's official
    if let Some(url) = super::store_artwork::fetch_steam_cover(name).await {
        return Some(url);
    }
    
    // Search official online catalogs directly; Heroic is not needed for artwork.
    if let Some(url) = super::store_artwork::fetch_cover(name).await {
        return Some(url);
    }

    // SteamGridDB is optional; the official stores do not require this key.
    let personal_key = crate::core::config_manager::load_config()
        .and_then(|config| config.steamgriddb_api_key).unwrap_or_default();
    let key = if personal_key.trim().is_empty() { api_key } else { personal_key.trim() };
    if key.is_empty() { return None; }
    fetch_steamgriddb_cover(name, key).await
}

#[tauri::command]
pub async fn fetch_steamgriddb_cover_command(name: String, api_key: String) -> Result<Option<String>, String> {
    Ok(fetch_best_cover(&name, &api_key).await)
}

async fn fetch_steamgriddb_cover(name: &str, api_key: &str) -> Option<String> {
    let client = Client::new();
    
    // 1. Search for the game
    let search_url = format!("https://www.steamgriddb.com/api/v2/search/autocomplete/{}", urlencoding::encode(name));
    let search_resp = client.get(&search_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send().await.ok()?;
        
    let search_json: SteamGridSearchResponse = search_resp.json().await.ok()?;
    let first_game = search_json.data.first()?;
    
    // 2. Get the grid
    let grid_url = format!("https://www.steamgriddb.com/api/v2/grids/game/{}?dimensions=600x900,342x482", first_game.id);
    let grid_resp = client.get(&grid_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send().await.ok()?;
        
    let grid_json: SteamGridGridResponse = grid_resp.json().await.ok()?;
    let first_grid = grid_json.data.first()?;
    
    Some(first_grid.url.clone())
}

fn has_executable(dir: &Path, depth: u8) -> bool {
    if depth == 0 {
        return false;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("exe") {
                        return true;
                    }
                }
            } else if p.is_dir() {
                if has_executable(&p, depth - 1) {
                    return true;
                }
            }
        }
    }
    false
}

pub(super) fn scan_custom(folders: Vec<String>) -> Vec<GameInfo> {
    let mut games = Vec::new();
    for folder in folders {
        let path = Path::new(&folder);
        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        // Check if it actually contains an .exe file (up to depth 3)
                        if has_executable(&p, 3) {
                            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                                games.push(GameInfo {
                                    name: name.to_string(),
                                    path: p.to_string_lossy().to_string(),
                                    app_id: None,
                                    cover_url: None,
                                    launcher: "Custom".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    games
}

#[tauri::command]
pub async fn scan_manual_game_folders(custom_folders: Vec<String>) -> Result<Vec<GameInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || scan_custom(custom_folders)).await.map_err(|e|e.to_string())
}

#[tauri::command]
pub async fn scan_installed_games(app: AppHandle, api_key: String, custom_folders: Option<Vec<String>>) -> Result<Vec<GameInfo>, String> {
    crate::core::logger::log_launcher(&app, "INFO", "Varredura de jogos iniciada (Steam, Heroic e pastas manuais)");
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    
    let mut all_games = Vec::new();
    
    // 1. Scan Steam
    all_games.extend(scan_steam(&home));
    
    // 2. Scan Heroic
    all_games.extend(scan_heroic(&home));
    all_games.extend(super::epic_library::scan(&home).into_iter().map(|game| GameInfo {
        name: game.name, path: game.path, app_id: game.external_id, cover_url: game.cover_url, launcher: game.launcher,
    }));

    // 3. Scan Custom
    if let Some(folders) = custom_folders {
        all_games.extend(scan_custom(folders));
    }
    
    // Deduplicate by path and name
    let mut unique_games: Vec<GameInfo> = Vec::new();
    for game in all_games {
        if !unique_games.iter().any(|g| g.path == game.path || g.name == game.name) {
            unique_games.push(game);
        }
    }
    
    let config = crate::core::config_manager::load_config().unwrap_or_default();
    
    // Process paths: heuristic + user overrides
    for game in &mut unique_games {
        // 1. Check for manual overrides first
        if let Some(custom_path) = config.custom_game_paths.get(&game.name) {
            game.path = custom_path.clone();
            continue; // Skip heuristic if user set a manual override
        }
        
        // 2. Apply heuristics for nested executable paths
        let base_path = Path::new(&game.path);
        if base_path.exists() {
            // The folder of the game's real executable, up to 5 levels deep.
            if let Some(best_dir) = crate::core::game_executable::best_executable_dir(base_path, 5) {
                game.path = best_dir.to_string_lossy().to_string();
            }
        }
    }
    
    // Resolve current Steam artwork filenames in one catalog request.
    let steam_ids: Vec<u32> = unique_games.iter().filter_map(|game| game.app_id.as_ref()?.parse().ok()).collect();
    let steam_assets = super::store_artwork::fetch_steam_assets(&steam_ids).await;
    for game in &mut unique_games {
        if let Some(url) = game.app_id.as_ref().and_then(|id| id.parse::<u32>().ok()).and_then(|id| steam_assets.get(&id)) {
            game.cover_url = Some(url.clone());
        }
    }

    // Copy Heroic file artwork into our scoped asset directory before displaying it.
    for game in &mut unique_games {
        if let Some(url) = game.cover_url.as_ref().filter(|url| url.starts_with("file://")) {
            let source = reqwest::Url::parse(url).ok().and_then(|url| url.to_file_path().ok());
            game.cover_url = match source {
                Some(source) => super::covers::cache_heroic_cover(source).await.ok()
                    .and_then(|path| reqwest::Url::from_file_path(path).ok()).map(|url| url.to_string()),
                None => None,
            };
        }
    }

    // 3. Fetch missing covers using API
    for game in &mut unique_games {
        if game.cover_url.is_none() {
            if let Some(url) = fetch_best_cover(&game.name, &api_key).await {
                game.cover_url = Some(url);
            } else {
                // Fallback to a placeholder or generic Steam search
                // For demonstration, keep it None to show a generic cover
            }
        }
    }

    // Sort alphabetically
    unique_games.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    
    crate::core::logger::log_launcher(&app, "INFO", &format!("Varredura concluída: {} jogos, {} sem capa", unique_games.len(), unique_games.iter().filter(|game| game.cover_url.is_none()).count()));
    Ok(unique_games)
}

#[derive(Deserialize, Debug)]
struct SteamAppDetailsResponse {
    #[serde(flatten)]
    apps: std::collections::HashMap<String, SteamAppDetails>,
}

#[derive(Deserialize, Debug)]
struct SteamAppDetails {
    success: bool,
    data: Option<SteamAppDetailsData>,
}

#[derive(Deserialize, Debug)]
struct SteamAppDetailsData {
    release_date: Option<SteamReleaseDate>,
}

#[derive(Deserialize, Debug)]
struct SteamReleaseDate {
    date: String,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct ReleaseDateCache {
    dates: std::collections::HashMap<String, String>,
}

fn load_date_cache() -> ReleaseDateCache {
    let cache_dir = crate::core::paths::app_root().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let cache_file = cache_dir.join("release_date_cache.json");
    if let Ok(data) = std::fs::read_to_string(cache_file) {
        if let Ok(cache) = serde_json::from_str(&data) {
            return cache;
        }
    }
    ReleaseDateCache::default()
}

fn save_date_cache(cache: &ReleaseDateCache) {
    let cache_dir = crate::core::paths::app_root().unwrap_or_else(|_| std::path::PathBuf::from("."));
    std::fs::create_dir_all(&cache_dir).ok();
    let cache_file = cache_dir.join("release_date_cache.json");
    if let Ok(data) = serde_json::to_string(cache) {
        std::fs::write(cache_file, data).ok();
    }
}

fn cache_release_date(app_id: &str, date: &str, generation: u64) {
    let current_generation = crate::core::game_info_cache::lock_generation();
    if *current_generation != generation { return; }
    let mut cache = load_date_cache();
    cache.dates.insert(app_id.to_string(), date.to_string());
    save_date_cache(&cache);
}

#[tauri::command]
pub async fn fetch_steam_release_date(app_id: String) -> Result<String, String> {
    let (cache, generation) = {
        let generation = crate::core::game_info_cache::lock_generation();
        (load_date_cache(), *generation)
    };
    if let Some(cached_date) = cache.dates.get(&app_id) {
        return Ok(cached_date.clone());
    }

    let url = format!("https://store.steampowered.com/api/appdetails?appids={}", app_id);
    let client = Client::new();
    let res = client.get(&url).send().await.map_err(|e| e.to_string())?;
    let json: SteamAppDetailsResponse = res.json().await.map_err(|e| e.to_string())?;
    
    if let Some(app_data) = json.apps.get(&app_id) {
        if app_data.success {
            if let Some(data) = &app_data.data {
                if let Some(release_date) = &data.release_date {
                    let date = release_date.date.clone();
                    cache_release_date(&app_id, &date, generation);
                    return Ok(date);
                }
            }
        }
    }
    
    // Se chegou aqui, não tem data. Grava como "Desconhecido" para não ficar pedindo de novo.
    let unknown = "Desconhecido".to_string();
    cache_release_date(&app_id, &unknown, generation);
    Ok(unknown)
}

// Delegate launch-mode selection to Steam instead of bypassing it with rungameid.
// No executable, graphics API, or launch arguments come from the game analysis.
fn steam_launch_uri(app_id: &str) -> Result<String, String> {
    let id = app_id.parse::<u32>().map_err(|_| "Invalid Steam AppID".to_string())?;
    if id == 0 || !app_id.bytes().all(|c| c.is_ascii_digit()) {
        return Err("Invalid Steam AppID".to_string());
    }
    Ok(format!("steam://launch/{id}/dialog"))
}

/// Launch through Steam while sharing the launcher's preparation/stop lifecycle.
#[tauri::command]
pub async fn start_steam_game(path: String, app_id: String) -> Result<crate::core::game_execution::ExecutionStatus, String> {
    let uri = steam_launch_uri(&app_id)?;
    #[cfg(target_os = "linux")]
    { tauri::async_runtime::spawn_blocking(move || crate::core::game_execution::steam::start(path, app_id, uri)).await.map_err(|e| e.to_string())? }
    #[cfg(not(target_os = "linux"))]
    { let _ = (path, uri); Err("Monitoramento da Steam disponível no Linux.".into()) }
}

#[tauri::command]
pub async fn launch_game(app: AppHandle, path: String, app_id: Option<String>, launcher: String) -> Result<(), String> {
    if launcher == "PeliGames" { return crate::commands::game_installation::launch_peligames_entry(path).await; }
    crate::core::logger::log_launcher(&app, "INFO", &format!("Abrindo jogo: launcher={launcher}, diretório={path}"));
    
    if launcher == "Steam" {
        let id = app_id.ok_or("Steam AppID unavailable")?;
        let uri = steam_launch_uri(&id)?;
        crate::core::logger::log_launcher(&app, "INFO", &format!("Solicitando inicialização à Steam sem argumentos extras: {uri}"));
        #[cfg(target_os = "linux")]
        let res = {
            let mut cmd = std::process::Command::new("xdg-open");
            cmd.arg(&uri);
            cmd.env_remove("LD_LIBRARY_PATH");
            cmd.spawn()
        };
        #[cfg(target_os = "windows")]
        let res = std::process::Command::new("cmd").args(["/c", "start", &uri]).spawn();
        
        if let Err(e) = res {
            let message = format!("Falha ao iniciar Steam: {e}");
            crate::core::logger::log_launcher(&app, "ERROR", &message);
            return Err(message);
        }
    } else {
        crate::core::logger::log_launcher(&app, "INFO", &format!("Abrindo pasta de jogo fora da Steam: {}", path));
        #[cfg(target_os = "linux")]
        let res = {
            let mut cmd = std::process::Command::new("xdg-open");
            cmd.arg(&path);
            cmd.env_remove("LD_LIBRARY_PATH");
            cmd.spawn()
        };
        #[cfg(target_os = "windows")]
        let res = std::process::Command::new("explorer").arg(&path).spawn();
        
        if let Err(e) = res {
            crate::core::logger::log_launcher(&app, "ERROR", &format!("Falha ao abrir pasta: {}", e));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(&path);
        cmd.env_remove("LD_LIBRARY_PATH");
        cmd.spawn().map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn collect_and_open_logs(app: tauri::AppHandle, game_name: String, game_dir: String) -> Result<(), String> {
    use std::fs;
    let mod_dir = crate::core::paths::mod_root()?;
    
    if game_name.is_empty() {
        let base_logs_dir = mod_dir.join("logs");
        let _ = fs::create_dir_all(&base_logs_dir);
        return open_folder(base_logs_dir.to_string_lossy().to_string());
    }

    let logs_dir = mod_dir.join("logs").join(&game_name);
    
    // Create logs directory if it doesn't exist
    fs::create_dir_all(&logs_dir).map_err(|e| format!("Failed to create logs directory: {}", e))?;
    
    // Common log files generated by the mod
    let log_files = ["OptiScaler.log", "dlssnr-amd.log", "ReShade.log", "dxgi.log", "nvngx.log", "dlssnr-amd-install.txt"];
    let game_path = std::path::Path::new(&game_dir);
    
    // For games, the logs are usually next to the exe. We'll search the base dir and immediate subdirs
    // But since the mod is installed where the EXE is, we should ideally use the same logic as check_game_installation
    let exe_path = crate::commands::analyzer::scan_directory_for_exe(game_path);
    let dir = if let Some(exe) = exe_path {
        exe.parent().unwrap_or(game_path).to_path_buf()
    } else {
        game_path.to_path_buf()
    };

    let mut found_any = false;
    for log_name in log_files.iter() {
        let source_log = dir.join(log_name);
        if source_log.exists() {
            let dest_log = logs_dir.join(log_name);
            let _ = fs::copy(&source_log, &dest_log);
            found_any = true;
        }
    }

    if !found_any {
        // If no logs found, we can at least write a dummy file to inform the user
        let _ = fs::write(logs_dir.join("info.txt"), format!("No logs found for {} in {}.\nThe game might not have been launched yet or the mod is not active.", game_name, dir.display()));
    }

    // Open the logs directory
    open_folder(logs_dir.to_string_lossy().to_string())
}



#[tauri::command]
pub fn request_steam_uninstall(app_id: String) -> Result<(), String> {
    // Steam owns these files and provides its own uninstall confirmation.
    steam_launch_uri(&app_id)?;
    std::process::Command::new("xdg-open").arg(format!("steam://uninstall/{app_id}")).env_remove("LD_LIBRARY_PATH").spawn().map_err(|e|e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod steam_main_filter_tests {
    use super::*;
    #[test]
    fn flatpak_library_on_another_disk_keeps_its_source() {
        let root = std::env::temp_dir().join(format!("peligames-steam-source-{}", uuid::Uuid::new_v4()));
        let home = root.join("home");
        let disk = root.join("another-disk");
        let apps = disk.join("steamapps");
        fs::create_dir_all(apps.join("common/TestGame")).unwrap();
        fs::write(apps.join("appmanifest_42.acf"), "\"AppState\"\n{\n\"appid\" \"42\"\n\"name\" \"TestGame\"\n\"installdir\" \"TestGame\"\n}").unwrap();
        let flatpak = super::super::steam_installation::roots(&home, true).remove(0);
        let native = super::super::steam_installation::roots(&home, false).remove(0);
        fs::create_dir_all(flatpak.join("config")).unwrap();
        fs::create_dir_all(native.join("steamapps")).unwrap();
        fs::write(flatpak.join("config/libraryfolders.vdf"), format!("\"libraryfolders\"\n{{\n\"0\"\n{{\n\"path\" \"{}\"\n}}\n}}", disk.display())).unwrap();
        assert!(scan_steam_main(&home, false).is_empty());
        let games = scan_steam_main(&home, true);
        assert_eq!(games.len(), 1);
        assert_eq!(Path::new(&games[0].path), apps.join("common/TestGame"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn filters_runtime_families_and_renamed_ids() {
        assert!(steam_support_package("4690330", "Legacy Steam Runtime", "LegacySteamRuntime"));
        assert!(steam_support_package("4690330", "Renamed support package", "Renamed"));
        for (id,name,folder) in [
            ("1070560","Steam Linux Runtime 1.0 (scout)","SteamLinuxRuntime"),
            ("4183110","Steam Linux Runtime 4.0","SteamLinuxRuntime_4"),
            ("4628710","Proton 11.0","Proton 11.0"),
            ("228980","Steamworks Common Redistributables","Steamworks Shared"),
            ("999999","Future runtime name","SteamLinuxRuntime_future"),
        ] { assert!(steam_support_package(id,name,folder),"{name}"); }
    }
    #[test]
    fn retains_games_with_similar_words() {
        for name in ["Hogwarts Legacy","Rogue Legacy 2","Legacy of Kain","Sniper Elite 5","Soldier of Fortune","Runtime","Proton Pulse"] {
            assert!(!steam_support_package("123456",name,name),"{name}");
        }
    }
    #[test]
    fn main_scan_omits_support_manifest_without_deleting_it() {
        let root=std::env::temp_dir().join(format!("peligames-steam-filter-{}",uuid::Uuid::new_v4()));
        let apps=root.join(".local/share/Steam/steamapps");
        fs::create_dir_all(apps.join("common/LegacySteamRuntime")).unwrap();
        fs::create_dir_all(apps.join("common/HogwartsLegacy")).unwrap();
        let runtime=apps.join("appmanifest_4690330.acf");
        fs::write(&runtime,"\"AppState\"\n{\n\"appid\" \"4690330\"\n\"name\" \"Legacy Steam Runtime\"\n\"installdir\" \"LegacySteamRuntime\"\n}").unwrap();
        fs::write(apps.join("appmanifest_990000.acf"),"\"AppState\"\n{\n\"appid\" \"990000\"\n\"name\" \"Hogwarts Legacy\"\n\"installdir\" \"HogwartsLegacy\"\n}").unwrap();
        let games=scan_steam_main(&root, false);
        assert_eq!(games.len(),1); assert_eq!(games[0].name,"Hogwarts Legacy");
        assert!(runtime.is_file());
        fs::remove_dir_all(root).unwrap();
    }
}

#[tauri::command]
pub async fn fetch_game_banners(name: String, app_id: Option<String>) -> Result<Vec<String>, String> {
    super::store_artwork::fetch_steam_banners(&name, app_id.as_deref()).await
        .ok_or_else(|| "Artwork catalog is temporarily unavailable".to_string())
}
