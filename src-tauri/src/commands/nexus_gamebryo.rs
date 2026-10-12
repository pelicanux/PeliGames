//! Gamebryo archives (Vortex gamebryo-archive-invalidation) and game folder
//! checks (Fallout New Vegas Sanity Checks).
use super::nexus_local::NexusGame;
use super::nexus_modules::{Definition, GameCheck};
use std::{fs, path::{Path, PathBuf}, time::{Duration, SystemTime}};

/// Generated archive of the BSA redirection (Vortex: `Vortex - BSA Redirection.bsa`).
pub(super) const REDIRECTION_FILE: &str = "PeliGames - BSA Redirection.bsa";

/// `YYYY-MM-DD` at midnight UTC.
pub(super) fn date(text: &str) -> Option<SystemTime> {
    let mut parts = text.split('-').map(|part| part.parse::<i64>().ok());
    let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
    if parts.next().is_some() || year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day) { return None; }
    // Days from the civil calendar (proleptic Gregorian).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146097 + day_of_era - 719468;
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(u64::try_from(days * 86400).ok()?))
}
/// TES4 archive name hash (Oblivion, Fallout 3, New Vegas).
fn hash(name: &str) -> u64 {
    let name = name.to_ascii_lowercase().replace('/', "\\");
    let (root, extension) = match name.rfind('.') { Some(i) if !name[i..].contains('\\') => (&name[..i], &name[i..]), _ => (name.as_str(), "") };
    let bytes = root.as_bytes();
    let len = bytes.len();
    let mut low: u32 = 0;
    if len > 0 {
        low = bytes[len - 1] as u32 | if len > 2 { (bytes[len - 2] as u32) << 8 } else { 0 } | (len as u32) << 16 | (bytes[0] as u32) << 24;
    }
    low |= match extension { ".kf" => 0x80, ".nif" => 0x8000, ".dds" => 0x8080, ".wav" => 0x8000_0000, _ => 0 };
    let mut middle: u32 = 0;
    if len > 3 { for &c in &bytes[1..len - 2] { middle = middle.wrapping_mul(0x1003F).wrapping_add(c as u32); } }
    let mut tail: u32 = 0;
    for &c in extension.as_bytes() { tail = tail.wrapping_mul(0x1003F).wrapping_add(c as u32); }
    ((middle.wrapping_add(tail) as u64) << 32) | low as u64
}
/// Archive holding only an empty `dummy/dummy.dds`, as Vortex writes for BSA
/// redirection: listed first in the INI, it makes the game prefer loose files.
pub(super) fn redirection_archive(version: u32) -> Vec<u8> {
    let (folder, file) = ("dummy", "dummy.dds");
    let folder_names = folder.len() as u32 + 1;
    let file_names = file.len() as u32 + 1;
    let records = 36 + 16;
    let names = records + 1 + folder_names + 16;
    let data = names + file_names;
    let mut out = Vec::new();
    out.extend_from_slice(b"BSA\0");
    for value in [version, 36, 0x3, 1, 1, folder_names, file_names, 0x2] { out.extend_from_slice(&value.to_le_bytes()); }
    out.extend_from_slice(&hash(folder).to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    // The folder offset counts the file name block, as the format defines it.
    out.extend_from_slice(&(records + file_names).to_le_bytes());
    out.push(folder_names as u8);
    out.extend_from_slice(folder.as_bytes());
    out.push(0);
    out.extend_from_slice(&hash(file).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&data.to_le_bytes());
    out.extend_from_slice(file.as_bytes());
    out.push(0);
    out
}
fn data_folder(module: &Definition) -> &str {
    module.activation.as_ref().map(|rule| rule.data.as_str()).unwrap_or("Data")
}
fn redirection_marker(game: &NexusGame) -> Result<PathBuf, String> {
    Ok(super::nexus_profile::directory(game)?.join("bsa-redirection"))
}
pub(super) fn redirection_enabled(game: &NexusGame) -> bool {
    redirection_marker(game).is_ok_and(|path| path.is_file())
}
pub(super) fn set_redirection(game: &NexusGame, enabled: bool) -> Result<(), String> {
    let marker = redirection_marker(game)?;
    if enabled {
        fs::create_dir_all(marker.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
        fs::write(marker, b"").map_err(|e| e.to_string())
    } else {
        match fs::remove_file(marker) { Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()), _ => Ok(()) }
    }
}
/// Files generated into the profile before deployment: `.override` beside
/// each mod BSA, and the redirection archive when switched on.
pub(super) fn prepare(game: &NexusGame, module: &Definition, runtime: &Path) -> Result<(), String> {
    let data = runtime.join(data_folder(module));
    if module.bsa_override && data.is_dir() {
        for entry in fs::read_dir(&data).map_err(|e| e.to_string())?.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".bsa").or_else(|| name.strip_suffix(".BSA")) {
                if !name.eq_ignore_ascii_case(REDIRECTION_FILE) { fs::write(data.join(format!("{stem}.override")), b"").map_err(|e| e.to_string())?; }
            }
        }
    }
    if let Some(redirection) = module.archives.as_ref().and_then(|archives| archives.redirection.as_ref()) {
        let path = data.join(REDIRECTION_FILE);
        if redirection_enabled(game) {
            fs::create_dir_all(&data).map_err(|e| e.to_string())?;
            fs::write(path, redirection_archive(redirection.version)).map_err(|e| e.to_string())?;
        } else if path.is_file() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
/// The archive list with the redirection archive first, or without it.
pub(super) fn archive_list(current: Option<&str>, defaults: &[String], enabled: bool) -> String {
    let mut archives: Vec<String> = match current {
        Some(list) => list.split(',').map(|item| item.trim().to_string()).filter(|item| !item.is_empty()).collect(),
        None => defaults.to_vec(),
    };
    archives.retain(|item| !item.eq_ignore_ascii_case(REDIRECTION_FILE));
    if enabled { archives.insert(0, REDIRECTION_FILE.to_string()); }
    archives.join(", ")
}
/// Gives the game's own archives an old date so loose files override them
/// (Vortex "archive-backdate"). Only the modification time changes.
pub(super) fn backdate(root: &Path, module: &Definition) -> Result<(), String> {
    let Some(archives) = &module.archives else { return Ok(()); };
    let when = date(&archives.backdate).ok_or("Data de arquivos inválida no módulo.")?;
    let Some(data) = super::nexus_discovery::locate(root, data_folder(module)).filter(|path| path.is_dir()) else { return Ok(()); };
    for entry in fs::read_dir(&data).map_err(|e| e.to_string())?.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let original = name.ends_with(&archives.extension.to_ascii_lowercase())
            && archives.prefixes.iter().any(|prefix| name.starts_with(&prefix.to_ascii_lowercase()));
        if !original || !entry.file_type().is_ok_and(|t| t.is_file()) { continue; }
        if entry.metadata().and_then(|m| m.modified()).is_ok_and(|modified| modified > when) {
            fs::File::open(entry.path()).and_then(|file| file.set_modified(when))
                .map_err(|e| format!("Falha ao ajustar a data de {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}
/// PE header flag IMAGE_FILE_LARGE_ADDRESS_AWARE.
fn large_address_aware(path: &Path) -> Option<bool> {
    let mut file = fs::File::open(path).ok()?;
    let dos = super::nexus_bg3::chunk(&mut file, 0, 64, 64).ok()?;
    if &dos[..2] != b"MZ" { return None; }
    let pe = u32::from_le_bytes(dos[60..64].try_into().ok()?) as u64;
    let head = super::nexus_bg3::chunk(&mut file, pe, 24, 24).ok()?;
    if &head[..4] != b"PE\0\0" { return None; }
    Some(u16::from_le_bytes(head[22..24].try_into().ok()?) & 0x20 != 0)
}
#[derive(serde::Serialize)]
pub struct CheckResult {
    kind: &'static str,
    path: String,
    url: Option<String>,
    /// Extra text: extender name, game version and store.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    /// Unwanted file already set aside by the player.
    hidden: bool,
    /// The extension downloads it without asking (`autoDownloadUe4ss`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    automatic: bool,
    /// Recommended choice among alternative loaders.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    recommended: bool,
}
fn hidden_store(game: &NexusGame) -> Result<PathBuf, String> {
    Ok(super::nexus_profile::directory(game)?.join("set-aside"))
}
pub(super) fn checks(game: &NexusGame) -> Result<Vec<CheckResult>, String> {
    let Some(module) = super::nexus_deployment::definition(game) else { return Ok(Vec::new()); };
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let store = hidden_store(game)?;
    let mut found = Vec::new();
    for check in &module.checks {
        match check {
            GameCheck::UnwantedFile { path, url } => {
                let present = super::nexus_discovery::locate(root, path).is_some_and(|p| p.is_file());
                let hidden = store.join(path).is_file();
                if present || hidden { found.push(CheckResult { kind: "unwanted_file", path: path.clone(), url: url.clone(), detail: None, hidden: hidden && !present, automatic: false, recommended: false }); }
            }
            GameCheck::LargeAddress { executable, url } => {
                let path = super::nexus_discovery::locate(root, executable);
                if path.as_deref().and_then(large_address_aware) == Some(false) {
                    found.push(CheckResult { kind: "large_address", path: executable.clone(), url: Some(url.clone()), detail: None, hidden: false, automatic: false, recommended: false });
                }
            }
            GameCheck::LegacyExtender { loader, minimum, url } => {
                // xSE loaders carry a leading zero (0.6.3 is xNVSE 6.3), as Vortex reads them.
                let version = super::nexus_discovery::locate(root, loader).and_then(|path| super::nexus_bg3::file_version(&path));
                if let Some([first, second, _]) = version {
                    let major = if first == 0 { second } else { first };
                    if major < *minimum { found.push(CheckResult { kind: "legacy_extender", path: loader.clone(), url: Some(url.clone()), detail: None, hidden: false, automatic: false, recommended: false }); }
                }
            }
        }
    }
    // UE4SS the extension downloads from GitHub (ChemBoy1 template, Palworld).
    let installed = |download: &super::nexus_modules::GithubDownload| {
        let files: Vec<String> = if download.present.is_empty() { module.ue4ss().map(|layout| vec![format!("{}/dwmapi.dll", layout.binaries)]).unwrap_or_default() } else { download.present.clone() };
        !files.is_empty() && files.iter().all(|file| super::nexus_discovery::locate(root, file).is_some())
    };
    for download in &module.downloads {
        // Alternatives (BepInEx or MelonLoader): any installed one satisfies the group.
        let satisfied = match &download.group {
            Some(group) => module.downloads.iter().filter(|other| other.group.as_ref() == Some(group)).any(installed),
            None => installed(download),
        };
        if satisfied { continue; }
        let (kind, url, detail) = match download.source {
            super::nexus_modules::DownloadSource::Nexus => {
                let id = download.mod_id.unwrap_or_default();
                let domain = download.domain.clone().unwrap_or_else(|| module.domain.clone());
                ("missing_nexus_download", format!("https://www.nexusmods.com/{domain}/mods/{id}"), format!("{} · Nexus {domain}/{id}", download.name))
            }
            super::nexus_modules::DownloadSource::BepinexBe => ("missing_download", "https://builds.bepinex.dev/projects/bepinex_be".to_string(), "builds.bepinex.dev · Bleeding Edge".to_string()),
            super::nexus_modules::DownloadSource::Github => ("missing_download", format!("https://github.com/{}/releases", download.repository),
                match &download.tag { Some(tag) => format!("{} · {tag}", download.repository), None => download.repository.clone() }),
        };
        found.push(CheckResult { kind, path: download.name.clone(), url: Some(url), detail: Some(detail), hidden: false, automatic: download.automatic, recommended: download.recommended });
    }
    // BepInEx the extension would download (Vortex modtype-bepinex).
    if let Some(loader) = &module.loader_download {
        let present = ["winhttp.dll", "BepInEx/core/BepInEx.Preloader.dll"].iter().all(|file| super::nexus_discovery::locate(root, file).is_some());
        if !present {
            found.push(CheckResult { kind: "missing_loader", path: "BepInEx".into(), url: Some("https://github.com/BepInEx/BepInEx/releases".into()),
                detail: Some(format!("BepInEx {} {}", loader.version, loader.architecture)), hidden: false, automatic: false, recommended: false });
        }
    }
    // Script extender missing (Vortex "script-extender-missing"): the page and the
    // game version, so the player picks the file built for it.
    if let Some(extender) = &module.script_extender {
        let loader = extender.at(&extender.loader);
        if super::nexus_discovery::locate(root, &loader).is_none() {
            let version = super::nexus_discovery::locate(root, &module.executable).and_then(|path| super::nexus_bg3::file_version(&path))
                .map(|[a, b, c]| format!("{a}.{b}.{c}"));
            let store = [("steam_api64.dll", "Steam"), ("steam_api.dll", "Steam"), ("Galaxy64.dll", "GOG"), ("Galaxy.dll", "GOG"), ("EOSSDK-Win64-Shipping.dll", "Epic"), ("EOSSDK-Win32-Shipping.dll", "Epic")]
                .iter().find(|(file, _)| super::nexus_discovery::locate(root, file).is_some()).map(|(_, store)| *store);
            let domain = extender.domain.clone().unwrap_or_else(|| module.domain.clone());
            let game_label = match (version, store) { (Some(v), Some(s)) => format!(" · {v} ({s})"), (Some(v), None) => format!(" · {v}"), (None, Some(s)) => format!(" · {s}"), _ => String::new() };
            found.push(CheckResult { kind: "missing_extender", path: loader, url: Some(format!("https://www.nexusmods.com/{domain}/mods/{}", extender.mod_id)),
                detail: Some(format!("{}{game_label}", extender.name)), hidden: false, automatic: false, recommended: false });
        }
    }
    Ok(found)
}
/// A script extender plugin that failed to load (Vortex script-extender-error-check).
#[derive(serde::Serialize, Clone)]
pub struct ExtenderError {
    dll: String,
    message: String,
    mod_id: Option<String>,
    mod_name: Option<String>,
    log: String,
}
const LOAD_STATUS: [&str; 6] = [
    "reported as incompatible during query",
    "reported as incompatible during load",
    "disabled, fatal error occurred while loading plugin",
    "disabled, no name specified",
    "disabled, fatal error occurred while checking plugin compatibility",
    "disabled, fatal error occurred while querying plugin",
];
fn dll_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).trim().to_string()
}
/// Lines of an xSE log naming a plugin that did not load, as (dll, message).
pub(super) fn parse_log(text: &str) -> Vec<(String, String)> {
    let mut errors = Vec::new();
    for line in text.lines() {
        if let Some(message) = LOAD_STATUS.iter().find(|message| line.contains(*message)) {
            // plugin C:\...\Data\SKSE\Plugins\\Fuz Ro D'oh.dll (00000001 Fuz Ro D'oh 010513CC) reported as incompatible during query
            let Some(start) = line.find("plugin ") else { continue; };
            let Some(end) = line[start..].to_ascii_lowercase().find(".dll (").map(|end| start + end + 4) else { continue; };
            errors.push((dll_name(&line[start + 7..end]), message.to_string()));
        } else if let Some(rest) = line.split("couldn't load plugin ").nth(1) {
            let Some((path, code)) = rest.rsplit_once(" (Error ") else { continue; };
            let message = match code.trim_end_matches(')').trim() {
                "126" => "dependent dll not found (code 126)".to_string(),
                "193" => "not a valid dll (code 193)".to_string(),
                other => format!("error code {other}"),
            };
            errors.push((dll_name(path), message));
        }
    }
    errors
}
fn ignored_marker(game: &NexusGame) -> Result<PathBuf, String> {
    Ok(super::nexus_profile::directory(game)?.join("extender-errors-ignored"))
}
/// Errors in the script extender logs written after the last "Ignore".
pub(super) fn extender_errors(game: &NexusGame) -> Result<Vec<ExtenderError>, String> {
    let Some(module) = super::nexus_deployment::definition(game) else { return Ok(Vec::new()); };
    let Some(extender) = &module.script_extender else { return Ok(Vec::new()); };
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let stem = extender.dll_prefix.trim_end_matches('_').to_ascii_lowercase();
    let ignored = ignored_marker(game).ok().and_then(|path| fs::metadata(path).and_then(|m| m.modified()).ok());
    let mut files = Vec::new();
    for location in &extender.logs {
        let path = if location.starts_with('@') {
            match super::nexus_targets::root(root, &game.compat_data, location) { Ok((base, relative)) => base.join(relative), Err(_) => continue }
        } else { root.join(location) };
        if path.is_dir() {
            for name in [format!("{stem}.log"), format!("{stem}_editor.log")] { if let Some(file) = super::nexus_discovery::locate(&path, &name) { files.push(file); } }
        } else if path.is_file() {
            files.push(path.clone());
            if let Some(editor) = path.parent().and_then(|dir| super::nexus_discovery::locate(dir, &format!("{stem}_editor.log"))) { files.push(editor); }
        }
    }
    let plugins = format!("{}/", extender.plugins.to_ascii_lowercase());
    let mut errors = Vec::new();
    for file in files {
        let Ok(meta) = fs::metadata(&file) else { continue; };
        if meta.len() > 4 * 1024 * 1024 || ignored.is_some_and(|ignored| meta.modified().is_ok_and(|modified| modified <= ignored)) { continue; }
        let text = String::from_utf8_lossy(&fs::read(&file).map_err(|e| e.to_string())?).into_owned();
        for (dll, message) in parse_log(&text) {
            let owner = game.mods.iter().find(|m| m.installed_files.iter().chain(m.fomod_files.iter()).flatten()
                .any(|(_, target)| target.to_ascii_lowercase().strip_prefix(&plugins).is_some_and(|rest| rest.eq_ignore_ascii_case(&dll))));
            errors.push(ExtenderError { dll, message, mod_id: owner.map(|m| m.id.clone()),
                mod_name: owner.map(|m| m.nexus_source.as_ref().map(|s| s.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| m.name.clone())),
                log: file.to_string_lossy().into_owned() });
        }
    }
    Ok(errors)
}
/// Moves an unwanted game file aside (kept in the game's PeliGames folder) or
/// puts it back. Only files named by a module check are accepted.
pub(super) fn set_aside(game: &NexusGame, path: &str, aside: bool) -> Result<(), String> {
    let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
    if !module.checks.iter().any(|check| matches!(check, GameCheck::UnwantedFile { path: known, .. } if known == path)) {
        return Err("Arquivo não previsto pelas verificações do jogo.".into());
    }
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let kept = hidden_store(game)?.join(path);
    let (from, to) = if aside {
        (super::nexus_discovery::locate(root, path).filter(|p| p.is_file()).ok_or("Arquivo não encontrado no jogo.")?, kept.clone())
    } else {
        (kept.clone(), root.join(path))
    };
    if !from.is_file() { return Err("Cópia guardada não encontrada.".into()); }
    if to.exists() { return Err(format!("Já existe um arquivo em {}; nada foi mudado.", to.display())); }
    fs::create_dir_all(to.parent().ok_or("Destino inválido.")?).map_err(|e| e.to_string())?;
    // The game and the configuration may be on different disks: copy, then remove.
    fs::copy(&from, &to).map_err(|e| e.to_string())?;
    fs::remove_file(&from).map_err(|e| { let _ = fs::remove_file(&to); e.to_string() })
}
#[derive(serde::Serialize)]
pub struct GameExtras {
    checks: Vec<CheckResult>,
    extender_errors: Vec<ExtenderError>,
    /// `None` when the game has no BSA redirection.
    redirection: Option<bool>,
}
fn extras(game: &NexusGame) -> Result<GameExtras, String> {
    let redirection = super::nexus_deployment::definition(game)
        .and_then(|module| module.archives.and_then(|archives| archives.redirection))
        .map(|_| redirection_enabled(game));
    Ok(GameExtras { checks: checks(game)?, extender_errors: extender_errors(game).unwrap_or_default(), redirection })
}
fn with_game<T>(game_id: &str, action: impl FnOnce(&mut NexusGame, &mut super::nexus_local::Workspace) -> Result<T, String>) -> Result<T, String> {
    let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
    let mut data = super::nexus_local::load()?;
    let index = data.games.iter().position(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    let mut game = data.games[index].clone();
    action(&mut game, &mut data)
}
#[tauri::command]
pub async fn nexus_game_checks(game_id: String) -> Result<GameExtras, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game, _| extras(game))).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_nexus_game_file_aside(game_id: String, path: String, aside: bool) -> Result<GameExtras, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game, _| {
        super::nexus_deployment::idle(game)?;
        set_aside(game, &path, aside)?;
        extras(game)
    })).await.map_err(|e| e.to_string())?
}
/// GitHub release asset of BepInEx for the game: Windows build (the game runs
/// in Proton), newest patch of the declared version, as the Vortex downloader picks.
pub(super) fn bepinex_asset(releases: &serde_json::Value, version: &str, architecture: &str) -> Option<(String, String)> {
    // Builds from 5.4.23 on name the platform (`BepInEx_win_x64_…`); older ones do not.
    let numbers: Vec<u32> = version.split('.').filter_map(|part| part.parse().ok()).collect();
    let platform = if numbers.as_slice() >= [5, 4, 23].as_slice() { "win_" } else { "" };
    let prefix = format!("bepinex_{platform}{}_{}", architecture.to_ascii_lowercase(), version);
    releases.as_array()?.iter().filter(|release| !release["prerelease"].as_bool().unwrap_or(false))
        .filter(|release| release["tag_name"].as_str().is_some_and(|tag| {
            let tag = tag.trim_start_matches('v');
            tag == version || tag.strip_prefix(version).is_some_and(|rest| rest.starts_with('.'))
        }))
        .flat_map(|release| release["assets"].as_array().cloned().unwrap_or_default())
        .find_map(|asset| {
            let name = asset["name"].as_str()?;
            let lower = name.to_ascii_lowercase();
            let url = asset["browser_download_url"].as_str()?;
            (lower.starts_with(&prefix) && lower.ends_with(".zip") && !name.contains(['/', '\\']))
                .then(|| (name.to_string(), url.to_string()))
        })
        .filter(|(_, url)| url.starts_with("https://github.com/BepInEx/BepInEx/releases/download/"))
}
const BE_INDEX: &str = "https://builds.bepinex.dev/projects/bepinex_be";
/// Newest Bleeding Edge artifact on the builds page whose name starts with
/// `prefix` (`BepInEx-Unity.IL2CPP-win-x64-`), as (file name, URL).
pub(super) fn bepinex_be_artifact(page: &str, prefix: &str) -> Option<(String, String)> {
    let marker = "/projects/bepinex_be/";
    let mut best: Option<(u64, String, String)> = None;
    for (index, _) in page.match_indices(marker) {
        let rest = &page[index + marker.len()..];
        let end = rest.find(['"', '\'', '<', ' ']).unwrap_or(rest.len());
        let Some((build, file)) = rest[..end].split_once('/') else { continue; };
        let Ok(number) = build.parse::<u64>() else { continue; };
        let name = file.replace("%2B", "+");
        if !name.starts_with(prefix) || !name.to_ascii_lowercase().ends_with(".zip") || name.contains(['/', '\\']) { continue; }
        if best.as_ref().is_none_or(|(known, _, _)| number > *known) {
            best = Some((number, name, format!("{BE_INDEX}/{build}/{file}")));
        }
    }
    best.map(|(_, name, url)| (name, url))
}
/// Release asset of a GitHub download: the fixed tag, or the newest release
/// that has a matching asset (as the Palworld extension searches).
pub(super) fn github_asset(releases: &serde_json::Value, download: &super::nexus_modules::GithubDownload) -> Option<(String, String)> {
    let list: Vec<serde_json::Value> = match releases.as_array() { Some(list) => list.clone(), None => vec![releases.clone()] };
    let prefix = download.prefix.to_ascii_lowercase();
    let origin = format!("https://github.com/{}/releases/download/", download.repository);
    list.iter().find_map(|release| release["assets"].as_array()?.iter().find_map(|asset| {
        let name = asset["name"].as_str()?;
        let url = asset["browser_download_url"].as_str()?;
        let lower = name.to_ascii_lowercase();
        (lower.starts_with(&prefix) && lower.ends_with(".zip") && !name.contains(['/', '\\']) && url.starts_with(&origin)
            && !download.exclude.iter().any(|word| lower.contains(&word.to_ascii_lowercase())))
            .then(|| (name.to_string(), url.to_string()))
    }))
}
/// Downloads a loader the module names (BepInEx, or a GitHub download such as
/// UE4SS) and imports it as a mod; the player installs it through the usual window.
#[tauri::command]
pub async fn download_nexus_loader(game_id: String, name: Option<String>) -> Result<super::nexus_local::Workspace, String> {
    let (module, target) = {
        let id = game_id.clone();
        tauri::async_runtime::spawn_blocking(move || with_game(&id, |game, _| {
            let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
            Ok((module, super::nexus_profile::directory(game)?.join("downloads")))
        })).await.map_err(|e| e.to_string())??
    };
    let client = reqwest::Client::builder().user_agent("PeliGames").timeout(std::time::Duration::from_secs(120)).build().map_err(|e| e.to_string())?;
    let query = |url: String| { let client = client.clone(); async move {
        client.get(&url).send().await.map_err(|_| "Falha ao consultar as versões no GitHub.".to_string())?
            .error_for_status().map_err(|e| format!("GitHub recusou a consulta: {e}"))?
            .json::<serde_json::Value>().await.map_err(|_| "Resposta inválida do GitHub.".to_string())
    } };
    let requested = name.clone();
    let (name, url) = match name {
        Some(wanted) => {
            let download = module.downloads.iter().find(|d| d.name == wanted).ok_or("Download não previsto para este jogo.")?;
            match download.source {
                super::nexus_modules::DownloadSource::Nexus => return Err("Este loader vem da página do Nexus; use o botão de download do requisito.".into()),
                super::nexus_modules::DownloadSource::BepinexBe => {
                    // builds.bepinex.dev has no API: the newest build is read from its page.
                    let page = match client.get(BE_INDEX).send().await { Ok(response) => response.text().await.unwrap_or_default(), Err(_) => String::new() };
                    bepinex_be_artifact(&page, &download.prefix).or_else(|| {
                        let url = download.fallback.clone()?;
                        Some((url.rsplit('/').next()?.replace("%2B", "+"), url))
                    }).ok_or("Nenhuma build Bleeding Edge do BepInEx encontrada.")?
                }
                super::nexus_modules::DownloadSource::Github => {
                    let releases = match download.tag.as_deref() {
                        // `latest`: the newest stable release (GitHub's /releases/latest).
                        Some("latest") => query(format!("https://api.github.com/repos/{}/releases/latest", download.repository)).await?,
                        Some(tag) => query(format!("https://api.github.com/repos/{}/releases/tags/{tag}", download.repository)).await?,
                        None => query(format!("https://api.github.com/repos/{}/releases?per_page=30", download.repository)).await?,
                    };
                    github_asset(&releases, download).ok_or_else(|| format!("Nenhum pacote {}* nas versões de {}.", download.prefix, download.repository))?
                }
            }
        }
        None => {
            let loader = module.loader_download.ok_or("Este jogo não tem download automático de loader.")?;
            let releases = query("https://api.github.com/repos/BepInEx/BepInEx/releases?per_page=100".into()).await?;
            bepinex_asset(&releases, &loader.version, &loader.architecture)
                .ok_or_else(|| format!("Nenhum pacote BepInEx {} {} para Windows nas versões do GitHub.", loader.version, loader.architecture))?
        }
    };
    let bytes = client.get(&url).send().await.map_err(|_| "Falha ao baixar o BepInEx.")?.error_for_status().map_err(|e| e.to_string())?
        .bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 || !bytes.starts_with(b"PK") { return Err("O arquivo baixado não é um pacote ZIP válido.".into()); }
    let path = target.join(&name);
    {
        let (path, bytes) = (path.clone(), bytes.to_vec());
        tauri::async_runtime::spawn_blocking(move || {
            fs::create_dir_all(path.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
            fs::write(&path, bytes).map_err(|e| e.to_string())
        }).await.map_err(|e| e.to_string())??;
    }
    let workspace = super::nexus_local::import_archive(game_id.clone(), path.to_string_lossy().into_owned(), None).await?;
    // A download with a root folder is installed into it, as with the manual root.
    let Some(root) = requested.as_ref().and_then(|wanted| module.downloads.iter().find(|d| &d.name == wanted)).and_then(|d| d.root.clone()) else { return Ok(workspace); };
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
        let mut data = super::nexus_local::load()?;
        let game = data.games.iter_mut().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
        let item = game.mods.last_mut().ok_or("Pacote importado não encontrado.")?;
        item.manual_root = Some(super::nexus_local::ManualRoot { source: String::new(), destination: root });
        super::nexus_local::save(&data)?;
        Ok(data)
    }).await.map_err(|e| e.to_string())?
}
/// Hides the current extender errors until the logs are written again (Vortex "Ignore").
#[tauri::command]
pub async fn ignore_nexus_extender_errors(game_id: String) -> Result<GameExtras, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game, _| {
        let marker = ignored_marker(game)?;
        fs::create_dir_all(marker.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
        fs::write(marker, b"").map_err(|e| e.to_string())?;
        extras(game)
    })).await.map_err(|e| e.to_string())?
}
/// Switches BSA redirection and deploys the profile again, as the Vortex
/// "Archive Invalidation" toggle adds or removes its mod.
#[tauri::command]
pub async fn set_nexus_bsa_redirection(game_id: String, enabled: bool) -> Result<GameExtras, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, |game, data| {
        super::nexus_deployment::idle(game)?;
        if extras(game)?.redirection.is_none() { return Err("Este jogo não tem redirecionamento de BSA.".into()); }
        let previous = redirection_enabled(game);
        if previous == enabled { return extras(game); }
        set_redirection(game, enabled)?;
        let old = game.profile.clone();
        let deployed = super::nexus_profile::build(game).and_then(|revision| {
            game.profile = Some(revision.clone());
            super::nexus_deployment::apply(game).map_err(|error| { super::nexus_profile::discard(game, &revision); error })
        });
        let tx = match deployed {
            Ok(tx) => tx,
            Err(error) => { let _ = set_redirection(game, previous); return Err(error); }
        };
        let slot = data.games.iter_mut().find(|item| item.id == game.id).ok_or("Jogo não encontrado.")?;
        *slot = game.clone();
        if let Err(error) = super::nexus_local::save(data) {
            drop(tx);
            if let Some(revision) = &game.profile { super::nexus_profile::discard(game, revision); }
            let _ = set_redirection(game, previous);
            return Err(error);
        }
        tx.commit();
        if let Some(old) = old.filter(|old| game.profile.as_ref() != Some(old)) { super::nexus_profile::discard(game, &old); }
        extras(game)
    })).await.map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashes_and_redirection_archive_match_the_tes4_format() {
        // Low word: last char, second to last << 8, length << 16, first char << 24, plus the extension bits.
        assert_eq!(hash("dummy"), 0x0075_1d38_6405_6d79);
        assert_eq!(hash("dummy.dds"), 0x8e50_c6fd_6405_edf9);
        assert_eq!(hash("DUMMY/"), hash("dummy\\"));
        let bsa = redirection_archive(104);
        assert_eq!(&bsa[..4], b"BSA\0");
        assert_eq!(u32::from_le_bytes(bsa[4..8].try_into().unwrap()), 104);
        assert_eq!(bsa.len(), 36 + 16 + 1 + 6 + 16 + 10);
        assert!(bsa.ends_with(b"dummy.dds\0"));
        // The file record points to the end of the archive (empty file).
        let offset = u32::from_le_bytes(bsa[36 + 16 + 7 + 12..36 + 16 + 7 + 16].try_into().unwrap()) as usize;
        assert_eq!(offset, bsa.len());
    }
    #[test]
    fn archive_list_puts_the_redirection_first_and_removes_it() {
        let defaults = vec!["Fallout - Textures.bsa".to_string(), "Fallout - Meshes.bsa".to_string()];
        let on = archive_list(None, &defaults, true);
        assert_eq!(on, format!("{REDIRECTION_FILE}, Fallout - Textures.bsa, Fallout - Meshes.bsa"));
        assert_eq!(archive_list(Some(&on), &defaults, true), on);
        assert_eq!(archive_list(Some(&on), &defaults, false), "Fallout - Textures.bsa, Fallout - Meshes.bsa");
        assert_eq!(archive_list(Some("A.bsa,B.bsa"), &defaults, false), "A.bsa, B.bsa");
    }
    #[test]
    fn extender_logs_name_the_plugins_that_failed() {
        let log = "SKSE64 runtime: initialize (version = 2.2.6)\r\n\
plugin C:\\Games\\Skyrim Special Edition\\Data\\SKSE\\Plugins\\\\Fuz Ro D'oh.dll (00000001 Fuz Ro D'oh 010513CC) reported as incompatible during query\r\n\
couldn't load plugin Z:\\home\\user\\Data\\SKSE\\Plugins\\engine.dll (Error 126)\r\n\
plugin C:\\Data\\SKSE\\Plugins\\Fine.dll (00000001 Fine 00000001) loaded correctly\r\n";
        assert_eq!(parse_log(log), vec![
            ("Fuz Ro D'oh.dll".to_string(), "reported as incompatible during query".to_string()),
            ("engine.dll".to_string(), "dependent dll not found (code 126)".to_string()),
        ]);
    }
    #[test]
    fn bepinex_asset_is_the_windows_build_of_the_declared_version() {
        let releases = serde_json::json!([
            {"tag_name": "v6.0.0-pre.2", "prerelease": true, "assets": [{"name": "BepInEx-Unity.Mono-win-x64-6.0.0-pre.2.zip", "browser_download_url": "https://github.com/BepInEx/BepInEx/releases/download/v6.0.0-pre.2/x.zip"}]},
            {"tag_name": "v5.4.23.3", "prerelease": false, "assets": [
                {"name": "BepInEx_linux_x64_5.4.23.3.zip", "browser_download_url": "https://github.com/BepInEx/BepInEx/releases/download/v5.4.23.3/BepInEx_linux_x64_5.4.23.3.zip"},
                {"name": "BepInEx_win_x86_5.4.23.3.zip", "browser_download_url": "https://github.com/BepInEx/BepInEx/releases/download/v5.4.23.3/BepInEx_win_x86_5.4.23.3.zip"},
                {"name": "BepInEx_win_x64_5.4.23.3.zip", "browser_download_url": "https://github.com/BepInEx/BepInEx/releases/download/v5.4.23.3/BepInEx_win_x64_5.4.23.3.zip"}]},
            {"tag_name": "v5.4.22", "prerelease": false, "assets": [{"name": "BepInEx_x64_5.4.22.0.zip", "browser_download_url": "https://github.com/BepInEx/BepInEx/releases/download/v5.4.22/BepInEx_x64_5.4.22.0.zip"}]}
        ]);
        assert_eq!(bepinex_asset(&releases, "5.4.23", "x64").unwrap().0, "BepInEx_win_x64_5.4.23.3.zip");
        assert_eq!(bepinex_asset(&releases, "5.4.23", "x86").unwrap().0, "BepInEx_win_x86_5.4.23.3.zip");
        assert_eq!(bepinex_asset(&releases, "5.4.22", "x64").unwrap().0, "BepInEx_x64_5.4.22.0.zip", "antes da 5.4.23 o nome não traz a plataforma");
        let foreign = serde_json::json!([{"tag_name": "v5.4.23", "assets": [{"name": "BepInEx_win_x64_5.4.23.0.zip", "browser_download_url": "https://example.com/BepInEx_win_x64_5.4.23.0.zip"}]}]);
        assert!(bepinex_asset(&foreign, "5.4.23", "x64").is_none(), "só o repositório oficial");
    }
    #[test]
    fn github_asset_follows_tag_prefix_and_exclusions() {
        let ue4ss: super::super::nexus_modules::GithubDownload = serde_json::from_value(serde_json::json!({"name": "UE4SS", "repository": "UE4SS-RE/RE-UE4SS", "tag": "experimental-latest", "prefix": "UE4SS_v"})).unwrap();
        let tagged = serde_json::json!({"tag_name": "experimental-latest", "assets": [
            {"name": "zDEV-UE4SS_v3.0.1-596-g96fe8f4.zip", "browser_download_url": "https://github.com/UE4SS-RE/RE-UE4SS/releases/download/experimental-latest/zDEV-UE4SS_v3.0.1-596-g96fe8f4.zip"},
            {"name": "UE4SS_v3.0.1-596-g96fe8f4.zip", "browser_download_url": "https://github.com/UE4SS-RE/RE-UE4SS/releases/download/experimental-latest/UE4SS_v3.0.1-596-g96fe8f4.zip"}]});
        assert_eq!(github_asset(&tagged, &ue4ss).unwrap().0, "UE4SS_v3.0.1-596-g96fe8f4.zip");
        let palworld: super::super::nexus_modules::GithubDownload = serde_json::from_value(serde_json::json!({"name": "UE4SS", "repository": "Okaetsu/RE-UE4SS", "prefix": "UE4SS-Palworld", "exclude": ["_zDev"]})).unwrap();
        let list = serde_json::json!([
            {"tag_name": "dev", "assets": [{"name": "UE4SS-Palworld_zDev.zip", "browser_download_url": "https://github.com/Okaetsu/RE-UE4SS/releases/download/dev/UE4SS-Palworld_zDev.zip"}]},
            {"tag_name": "v1", "assets": [{"name": "UE4SS-Palworld.zip", "browser_download_url": "https://github.com/Okaetsu/RE-UE4SS/releases/download/v1/UE4SS-Palworld.zip"}]}]);
        assert_eq!(github_asset(&list, &palworld).unwrap().0, "UE4SS-Palworld.zip");
        let foreign = serde_json::json!({"assets": [{"name": "UE4SS_v3.zip", "browser_download_url": "https://example.com/UE4SS_v3.zip"}]});
        assert!(github_asset(&foreign, &ue4ss).is_none());
    }
    #[test]
    fn bleeding_edge_artifact_is_the_newest_build_of_the_runtime() {
        let page = r#"<div class="artifact-item"><span class="artifact-id">#788</span>
            <a class="artifact-link" href="/projects/bepinex_be/788/BepInEx-Unity.IL2CPP-win-x64-6.0.0-be.788%2B5b766a3.zip">x</a>
            <a class="artifact-link" href="/projects/bepinex_be/788/BepInEx-Unity.Mono-win-x64-6.0.0-be.788%2B5b766a3.zip">x</a></div>
            <div class="artifact-item"><span class="artifact-id">#790</span>
            <a class="artifact-link" href="/projects/bepinex_be/790/BepInEx-Unity.IL2CPP-win-x64-6.0.0-be.790%2Babcdef0.zip">x</a></div>"#;
        let (name, url) = bepinex_be_artifact(page, "BepInEx-Unity.IL2CPP-win-x64-").unwrap();
        assert_eq!(name, "BepInEx-Unity.IL2CPP-win-x64-6.0.0-be.790+abcdef0.zip");
        assert_eq!(url, "https://builds.bepinex.dev/projects/bepinex_be/790/BepInEx-Unity.IL2CPP-win-x64-6.0.0-be.790%2Babcdef0.zip");
        assert!(bepinex_be_artifact(page, "BepInEx-Unity.IL2CPP-win-x86-").is_none());
    }
    #[test]
    fn dates_are_read_as_utc_midnight() {
        assert_eq!(date("1970-01-02"), Some(SystemTime::UNIX_EPOCH + Duration::from_secs(86400)));
        assert_eq!(date("2006-02-01").unwrap().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(), 1138752000);
        assert!(date("2006-13-01").is_none() && date("x").is_none());
    }
}
