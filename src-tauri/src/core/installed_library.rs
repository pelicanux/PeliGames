//! Persistent library of destinations installed through PeliGames.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
fn deserialize_proton<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    Ok(super::paths::migrated_runner_path(&value))
}
static LIBRARY_LOCK: Mutex<()> = Mutex::new(());
// The launcher and Pelinstall are separate processes sharing the same manifests.
fn process_lock(index: &Path) -> Result<fs::File, String> {
    fs::create_dir_all(index.parent().ok_or("Registro inválido.")?).map_err(|e|e.to_string())?;
    let file=fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(index.with_extension("lock")).map_err(|e|e.to_string())?;
    #[cfg(unix)] { use std::os::fd::AsRawFd; if unsafe {libc::flock(file.as_raw_fd(),libc::LOCK_EX)} != 0 { return Err(std::io::Error::last_os_error().to_string()); } }
    Ok(file)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct InstalledEntry {
    #[serde(default)]
    pub discovery_source: Option<String>,
    /// The clear favourite among the installation's executables (see `rank`).
    #[serde(default)]
    pub recommended: bool,
    /// Launcher shortcut this library entry opens (Programs panel).
    #[serde(default)]
    pub shortcut: Option<String>,
    /// Its own prefix, mirrored from the launcher's (overlay or reflink).
    #[serde(default)]
    pub profile: Option<super::game_profiles::GameProfile>,
    #[serde(default)]
    pub launch_arguments: Vec<String>,
    #[serde(default)]
    pub advanced: super::advanced_settings::AdvancedSettings,
    pub name: String,
    pub path: String,
    pub directory: String,
    pub executable: String,
    pub prefix: String,
    #[serde(deserialize_with = "deserialize_proton")]
    pub proton: String,
    pub cover_url: Option<String>,
    pub launcher: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EntryOverride {
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub advanced: Option<super::advanced_settings::AdvancedSettings>,
    #[serde(default)]
    pub prefix: Option<String>,
    pub source: String,
    pub name: String,
    #[serde(deserialize_with = "deserialize_proton")]
    pub proton: String,
    pub executable: String,
}
#[derive(Deserialize)]
pub struct EntrySettings {
    #[serde(default)]
    pub advanced: Option<super::advanced_settings::AdvancedSettings>,
    #[serde(default)]
    pub prefix: Option<String>,
    pub path: String,
    pub name: String,
    #[serde(deserialize_with = "deserialize_proton")]
    pub proton: String,
    pub executable: String,
}
#[derive(Serialize, Deserialize)]
pub struct InstallationRecord {
    pub name: String,
    pub directory: String,
    pub prefix: String,
    #[serde(deserialize_with = "deserialize_proton")]
    pub proton: String,
    pub installer: String,
    pub cover_url: Option<String>,
    pub entries: Vec<InstalledEntry>,
    #[serde(default)]
    pub overrides: Vec<EntryOverride>,
    /// Executables already in the prefix before the installer ran (Proton's own
    /// files, other games): never offered as this installation's game.
    #[serde(default)]
    pub baseline: Vec<String>,
    /// Shortcut targets already seen in the prefix. A launcher (EA app,
    /// Battle.net…) that installs a game adds a shortcut; new ones are offered.
    #[serde(default)]
    pub known_shortcuts: Option<Vec<String>>,
    /// Executables of games a launcher installed (added or declined): separate
    /// games, never alternatives to another executable of the installation.
    #[serde(default)]
    pub launcher_games: Vec<String>,
    /// Shortcut files the installer created: later ones belong to games a
    /// launcher installed (Programs panel).
    #[serde(default)]
    pub install_shortcuts: Option<Vec<String>>,
    /// How profiles of this launcher's games are made.
    #[serde(default)]
    pub profile_kind: super::game_profiles::ProfileKind,
}
fn library_file() -> Result<PathBuf, String> {
    Ok(super::paths::app_root()?.join("installed-destinations.json"))
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("Destino inválido.")?).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(
        &temp,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(temp);
        return Err(error.to_string());
    }
    Ok(())
}
fn destinations(index: &Path) -> Result<BTreeSet<String>, String> {
    match fs::read(index) {
        Ok(data) => serde_json::from_slice(&data)
            .map_err(|e| format!("Registro de instalações inválido: {e}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(error) => Err(error.to_string()),
    }
}
fn excluded_file(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "setup",
        "unins",
        "uninstall",
        "vc_redist",
        "vcredist",
        "dxsetup",
        "crashreport",
        "unitycrashhandler",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
        // Crash reporters, embedded browser helpers and anti-cheat or runtime installers.
        || ["crashpad", "crashreport", "cefsubprocess", "webhelper", "qtwebengine", "anticheat_setup", "anticheat_eos_setup", "dxwebsetup", "dotnetfx", "oalinst", "physx"]
            .iter().any(|part| name.contains(part))
}
/// Proton's stand-in Steam client, present in every prefix it creates.
fn proton_steam(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    lower.ends_with("/drive_c/program files (x86)/steam/steam.exe")
}
/// Executables in a prefix, to tell later which ones an installer created.
pub fn prefix_executables(prefix: &Path) -> Result<Vec<String>, String> {
    let mut files = BTreeSet::new();
    let mut visited = 0;
    walk(&prefix.join("drive_c"), Path::new(""), &mut files, 0, &mut visited)?;
    Ok(files.into_iter().map(|file| file.to_string_lossy().into_owned()).collect())
}
fn walk(
    root: &Path,
    installer: &Path,
    files: &mut BTreeSet<PathBuf>,
    depth: usize,
    visited: &mut usize,
) -> Result<(), String> {
    if depth > 24 {
        return Err("A instalação excede o limite de profundidade da varredura.".into());
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Não foi possível examinar {}: {e}", root.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        *visited += 1;
        if *visited > 100_000 {
            return Err("A instalação excede o limite de arquivos da varredura.".into());
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        // Never follow prefix links to home, mounts or another installation.
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if [
                ".peligames",
                "prefix",
                "logs",
                "windows",
                "$recycle.bin",
                "system volume information",
                "_commonredist",
                "redist",
                "redistributables",
            ]
            .contains(&name.as_str())
            {
                continue;
            }
            walk(&path, installer, files, depth + 1, visited)?;
        } else if kind.is_file() && name.ends_with(".exe") && !excluded_file(&name) && !proton_steam(&path) {
            let canonical = path.canonicalize().map_err(|e| e.to_string())?;
            if canonical != installer {
                files.insert(canonical);
            }
        }
    }
    Ok(())
}
/// Executables the prefix's Windows shortcuts point to, with the shortcut name.
fn shortcut_targets(prefix: &Path) -> BTreeMap<String, String> {
    super::installation_targets::discover(prefix).into_iter()
        .filter(|(path, target)| target.source == "Atalho do Windows" && path.is_file()
            && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            && !excluded_file(&path.file_name().unwrap_or_default().to_string_lossy()) && !proton_steam(path))
        .map(|(path, target)| { let name = target.label.unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().into_owned()); (path.to_string_lossy().into_owned(), name) })
        .collect()
}
/// A shortcut a launcher created for a game it installed (Programs panel).
#[derive(Clone, Serialize, Deserialize)]
pub struct LauncherShortcut { pub id: String, pub name: String, pub executable: Option<String>, pub url: Option<String>, pub icon: Option<String>, pub in_library: bool }
fn record_shortcuts(record: &InstallationRecord) -> Vec<super::installation_targets::Shortcut> {
    let mut seen = BTreeSet::new();
    // Epic manifests first: they name every installed game, with or without a shortcut.
    let prefix = Path::new(&record.prefix);
    super::installation_targets::epic_games(prefix).into_iter().chain(super::installation_targets::shortcut_files(prefix)).filter(|item| {
        let file = item.file.to_string_lossy();
        let own = match &record.install_shortcuts {
            Some(installed) => installed.iter().any(|old| old == file.as_ref()),
            // Older installations: the installation's own programs opened without arguments.
            None => item.url.is_none() && item.arguments.is_empty() && item.target.as_ref().is_some_and(|target| record.entries.iter()
                .any(|entry| Path::new(&entry.executable) == target && !record.launcher_games.contains(&entry.path))),
        };
        // The same game is usually on the Desktop and in the Start Menu.
        !own && seen.insert(item.label.to_lowercase())
    }).collect()
}
/// Games launchers installed in the prefix of `path`, by the shortcuts they created.
pub fn launcher_shortcuts(path: &str) -> Result<Vec<LauncherShortcut>, String> {
    launcher_shortcuts_at(&library_file()?, path)
}
fn launcher_shortcuts_at(index: &Path, path: &str) -> Result<Vec<LauncherShortcut>, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let Some((_, record)) = record_of(index, path)? else { return Ok(vec![]); };
    // A game's profile is its own prefix: the launcher's shortcuts are not its programs.
    if record.entries.iter().any(|entry| entry.path == path && entry.profile.is_some()) { return Ok(vec![]); }
    Ok(record_shortcuts(&record).into_iter().map(|item| LauncherShortcut {
        in_library: record.entries.iter().any(|entry| entry.shortcut.as_deref() == Some(item.file.to_string_lossy().as_ref())),
        id: item.file.to_string_lossy().into_owned(), name: item.label,
        executable: item.target.map(|target| target.to_string_lossy().into_owned()), url: item.url,
        icon: item.icon.map(|icon| icon.to_string_lossy().into_owned()),
    }).collect())
}
/// Program and arguments that open the shortcut `id` of `path`'s prefix as the
/// Windows shell would (a `.url` goes to Wine's `start`, which calls the launcher).
pub fn shortcut_launch(path: &str, id: &str) -> Result<(String, Vec<String>), String> {
    shortcut_launch_at(&library_file()?, path, id)
}
fn shortcut_launch_at(index: &Path, path: &str, id: &str) -> Result<(String, Vec<String>), String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let (_, record) = record_of(index, path)?.ok_or("Entrada não registrada na biblioteca PeliGames.")?;
    let item = record_shortcuts(&record).into_iter().find(|item| item.file.to_string_lossy() == id).ok_or("Atalho não encontrado no prefixo.")?;
    shortcut_program(&record, item)
}
fn shortcut_program(record: &InstallationRecord, item: super::installation_targets::Shortcut) -> Result<(String, Vec<String>), String> {
    if let Some(target) = item.target { return Ok((target.to_string_lossy().into_owned(), item.arguments)); }
    let url = item.url.ok_or("Atalho sem destino.")?;
    let start = Path::new(&record.prefix).join("drive_c/windows/system32/start.exe");
    if !start.is_file() { return Err("O prefixo não tem o start.exe do Wine.".into()); }
    Ok((start.to_string_lossy().into_owned(), vec![url]))
}
/// Puts a launcher's game in the library: an entry of its own that opens the
/// shortcut, with the launcher's prefix, Proton and advanced settings.
pub fn add_shortcut_to_library(path: &str, id: &str) -> Result<InstalledEntry, String> {
    add_shortcut_to_library_at(&library_file()?, path, id, super::game_profiles::create)
}
/// Profile kind of a launcher's games and which kinds its prefix supports.
#[derive(Serialize)]
pub struct ProfileOptions { pub kind: super::game_profiles::ProfileKind, pub overlay: bool, pub reflink: bool }
pub fn profile_options(path: &str) -> Result<ProfileOptions, String> {
    let index = library_file()?;
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(&index)?;
    let (_, record) = record_of(&index, path)?.ok_or("Entrada não registrada na biblioteca PeliGames.")?;
    let reflink = super::game_profiles::reflink_available(Path::new(&record.prefix)) && super::game_profiles::reflink_available(Path::new(&record.directory));
    Ok(ProfileOptions { kind: record.profile_kind, overlay: super::game_profiles::overlay_available(), reflink })
}
pub fn set_profile_kind(path: &str, kind: super::game_profiles::ProfileKind) -> Result<ProfileOptions, String> {
    {
        let index = library_file()?;
        let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
        let _process_guard = process_lock(&index)?;
        let (manifest, mut record) = record_of(&index, path)?.ok_or("Entrada não registrada na biblioteca PeliGames.")?;
        record.profile_kind = kind;
        write_json(&manifest, &record)?;
    }
    profile_options(path)
}
fn add_shortcut_to_library_at(index: &Path, path: &str, id: &str,
    create: impl FnOnce(&Path, &Path, super::game_profiles::ProfileKind) -> Result<super::game_profiles::GameProfile, String>) -> Result<InstalledEntry, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let (manifest, mut record) = record_of(index, path)?.ok_or("Entrada não registrada na biblioteca PeliGames.")?;
    if let Some(existing) = record.entries.iter().find(|entry| entry.shortcut.as_deref() == Some(id)) { return Ok(existing.clone()); }
    let launcher = record.entries.iter().find(|entry| entry.path == path).cloned().ok_or("Entrada não registrada na biblioteca PeliGames.")?;
    let item = record_shortcuts(&record).into_iter().find(|item| item.file.to_string_lossy() == id).ok_or("Atalho não encontrado no prefixo.")?;
    let name = item.label.clone();
    let folder = item.icon.as_ref().and_then(|icon| icon.parent()).map(Path::to_path_buf);
    let (executable, arguments) = shortcut_program(&record, item)?;
    // Every game gets its own prefix, mirrored from the launcher's.
    let profile = create(Path::new(&record.prefix), &Path::new(&record.directory).join("profiles"), record.profile_kind)?;
    let directory = folder.or_else(|| Path::new(&executable).parent().map(Path::to_path_buf)).unwrap_or_default().to_string_lossy().into_owned();
    let entry = InstalledEntry {
        discovery_source: Some("Atalho do launcher".into()), recommended: false, shortcut: Some(id.to_string()),
        launch_arguments: arguments.iter().map(|argument| profile.translate(argument)).collect(), advanced: launcher.advanced.clone(), name: name.clone(),
        path: format!("peligames:{}", uuid::Uuid::new_v4()),
        directory: profile.translate(&directory), executable: profile.translate(&executable),
        prefix: profile.prefix().to_string_lossy().into_owned(), proton: launcher.proton.clone(), cover_url: None, launcher: "PeliGames".into(),
        profile: Some(profile),
    };
    record.entries.push(entry.clone());
    write_json(&manifest, &record)?;
    Ok(entry)
}
/// Takes a launcher's game out of the library again (the game stays installed).
pub fn remove_shortcut_from_library(path: &str, id: &str) -> Result<(), String> {
    remove_shortcut_from_library_at(&library_file()?, path, id)
}
fn remove_shortcut_from_library_at(index: &Path, path: &str, id: &str) -> Result<(), String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let (manifest, mut record) = record_of(index, path)?.ok_or("Entrada não registrada na biblioteca PeliGames.")?;
    let removed: Vec<String> = record.entries.iter().filter(|entry| entry.shortcut.as_deref() == Some(id)).map(|entry| entry.path.clone()).collect();
    // The game's profile goes with it (its changes, mods and saves); the launcher's prefix stays.
    for profile in record.entries.iter().filter(|entry| entry.shortcut.as_deref() == Some(id)).filter_map(|entry| entry.profile.as_ref()) {
        super::game_profiles::remove(profile)?;
    }
    record.entries.retain(|entry| entry.shortcut.as_deref() != Some(id));
    record.overrides.retain(|item| !removed.contains(&item.source));
    write_json(&manifest, &record)?;
    Ok(())
}
/// A game a launcher installed in the prefix, found by its new shortcut.
#[derive(Clone, Serialize, Deserialize)]
pub struct LauncherGame { pub name: String, pub executable: String }
/// The launcher entry of the installation that holds `path` (a profile entry's owner).
fn record_owner(index: &Path, path: &str) -> Option<String> {
    let (_, record) = record_of(index, path).ok()??;
    record.entries.iter().find(|entry| entry.profile.is_none() && !entry.path.starts_with("peligames:")).or_else(|| record.entries.iter().find(|entry| entry.profile.is_none())).map(|entry| entry.path.clone())
}
fn record_of(index: &Path, path: &str) -> Result<Option<(PathBuf, InstallationRecord)>, String> {
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.is_file() { continue; }
        let record: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if record.entries.iter().any(|entry| entry.path == path) { return Ok(Some((manifest, record))); }
    }
    Ok(None)
}
/// After a program of the library closes: games its launcher installed since the
/// last check (new shortcuts to executables not in the library). Each shortcut
/// is offered once; a record without the list only starts tracking.
pub fn new_launcher_games(path: &str) -> Result<Vec<LauncherGame>, String> {
    new_launcher_games_at(&library_file()?, path)
}
fn new_launcher_games_at(index: &Path, path: &str) -> Result<Vec<LauncherGame>, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let Some((manifest, mut record)) = record_of(index, path)? else { return Ok(vec![]); };
    if record.entries.iter().any(|entry| entry.path == path && entry.profile.is_some()) { return Ok(vec![]); }
    let current = shortcut_targets(Path::new(&record.prefix));
    let Some(known) = record.known_shortcuts.clone() else {
        record.known_shortcuts = Some(current.into_keys().collect());
        write_json(&manifest, &record)?;
        return Ok(vec![]);
    };
    // Executables the player already named, chose or hid; one only found by a
    // rescan ("Launcher · game") is still offered, to get its own name.
    let listed = |executable: &str| record.overrides.iter().any(|item| item.source == executable || item.executable == executable)
        || record.baseline.iter().any(|old| old == executable);
    let found: Vec<LauncherGame> = current.iter().filter(|(executable, _)| !known.contains(executable) && !listed(executable))
        .map(|(executable, name)| LauncherGame { name: name.clone(), executable: executable.clone() }).collect();
    if current.keys().any(|executable| !known.contains(executable)) {
        record.known_shortcuts = Some(known.into_iter().chain(current.into_keys()).collect::<BTreeSet<_>>().into_iter().collect());
        write_json(&manifest, &record)?;
    }
    Ok(found)
}
/// Adds the chosen launcher games to the library (same prefix and Proton as the
/// launcher, named after their shortcut) and hides the declined ones.
pub fn add_launcher_games(path: &str, added: Vec<LauncherGame>, declined: Vec<String>) -> Result<Vec<InstalledEntry>, String> {
    add_launcher_games_at(&library_file()?, path, added, declined)
}
fn add_launcher_games_at(index: &Path, path: &str, added: Vec<LauncherGame>, declined: Vec<String>) -> Result<Vec<InstalledEntry>, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let Some((manifest, mut record)) = record_of(index, path)? else { return Err("Entrada não registrada na biblioteca PeliGames.".into()); };
    let launcher = record.entries.iter().find(|entry| entry.path == path).cloned().ok_or("Entrada não registrada na biblioteca PeliGames.")?;
    let drive = Path::new(&record.prefix).join("drive_c").canonicalize().map_err(|e| e.to_string())?;
    let inside = |executable: &str| Path::new(executable).canonicalize().is_ok_and(|real| real.starts_with(&drive) && real.is_file());
    let mut entries = Vec::new();
    for game in added {
        let name = game.name.trim();
        if name.is_empty() || !inside(&game.executable) { return Err(format!("Jogo do launcher inválido: {}", game.name)); }
        record.overrides.retain(|item| item.source != game.executable);
        record.overrides.push(EntryOverride {
            cover_url: None, advanced: Some(launcher.advanced.clone()), prefix: Some(launcher.prefix.clone()), source: game.executable.clone(),
            name: name.chars().take(120).collect(), proton: launcher.proton.clone(), executable: game.executable.clone(),
        });
        let entry = InstalledEntry {
            discovery_source: Some("Atalho do launcher".into()), recommended: false, shortcut: None, profile: None, launch_arguments: vec![], advanced: launcher.advanced.clone(),
            name: name.chars().take(120).collect(), path: game.executable.clone(),
            directory: Path::new(&game.executable).parent().unwrap_or(Path::new("")).to_string_lossy().into_owned(),
            executable: game.executable.clone(), prefix: launcher.prefix.clone(), proton: launcher.proton.clone(), cover_url: None, launcher: "PeliGames".into(),
        };
        record.entries.retain(|item| item.path != entry.path);
        record.entries.push(entry.clone());
        entries.push(entry);
    }
    for game in &entries { if !record.launcher_games.contains(&game.path) { record.launcher_games.push(game.path.clone()); } }
    for executable in declined {
        if !inside(&executable) || record.entries.iter().any(|entry| entry.path == executable) { continue; }
        if !record.launcher_games.contains(&executable) { record.launcher_games.push(executable.clone()); }
        record.overrides.retain(|item| item.source != executable);
        record.overrides.push(EntryOverride {
            cover_url: None, advanced: None, source: executable, name: String::new(),
            proton: String::new(), executable: String::new(), prefix: Some(launcher.prefix.clone()),
        });
    }
    write_json(&manifest, &record)?;
    Ok(entries)
}
/// Lower-case letters and digits only.
fn compact(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}
/// How likely an executable is the game itself.
fn score(entry: &InstalledEntry, game: &str, largest: u64) -> i32 {
    let path = Path::new(&entry.executable);
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let lower = stem.to_ascii_lowercase();
    let folder = path.parent().unwrap_or(Path::new(""));
    // What the installer pointed at: a shortcut or a registry entry.
    let mut points = match entry.discovery_source.as_deref() {
        Some("Atalho do Windows") => 50,
        Some("Registro: App Paths") => 35,
        Some(_) => 30,
        None => 0,
    };
    // Unity: the game is the executable beside `<name>_Data`.
    if folder.join(format!("{stem}_Data")).is_dir() { points += 40; }
    // Unreal: the root launcher wins over the inner `-Shipping` build.
    if lower.ends_with("-shipping") || lower.contains("-win64-shipping") || lower.contains("-wingdk-shipping") { points -= 30; }
    if folder.join("Engine").is_dir() { points += 20; }
    // Name close to the one the player gave the game.
    let (name, file) = (compact(game), compact(&stem));
    if !file.is_empty() && !name.is_empty() && (name.contains(&file) || file.contains(&name)) {
        points += 25;
    } else if game.split(|c: char| !c.is_alphanumeric()).map(compact).any(|word| word.len() >= 3 && file.contains(&word)) {
        points += 12;
    }
    // Tools beside the game.
    if ["editor", "server", "dedicated", "config", "settings", "benchmark", "updater", "patcher", "helper", "redist", "sdk", "modmanager", "toolkit"]
        .iter().any(|word| lower.contains(word)) { points -= 25; }
    // The game binary is usually the largest one.
    if largest > 0 && fs::metadata(path).is_ok_and(|meta| meta.len() == largest) { points += 10; }
    points
}
/// Orders the discovered executables, most likely game first, and marks the
/// first one as recommended when it clearly wins (then no question is asked).
fn rank(entries: &mut [InstalledEntry], game: &str) {
    let largest = entries.iter().filter_map(|entry| fs::metadata(&entry.executable).ok()).map(|meta| meta.len()).max().unwrap_or(0);
    let mut scored: Vec<(i32, InstalledEntry)> = entries.iter().map(|entry| (score(entry, game, largest), entry.clone())).collect();
    scored.sort_by_key(|(points, _)| std::cmp::Reverse(*points));
    let clear = match scored.as_slice() {
        [_] => true,
        [(first, _), (second, _), ..] => *first >= 30 && first - second >= 25,
        [] => false,
    };
    for (index, (_, entry)) in scored.into_iter().enumerate() {
        entries[index] = InstalledEntry { recommended: index == 0 && clear, ..entry };
    }
}
fn scan(record: &mut InstallationRecord) -> Result<(), String> {
    // Explicit registrations have independent identities, even when they share
    // an executable and prefix. Only discovered executables are deduplicated.
    let added: Vec<_> = record.entries.iter()
        .filter(|entry| entry.path.starts_with("peligames:") && (entry.profile.is_some() || Path::new(&entry.executable).is_file()))
        .cloned().collect();
    let mut files = BTreeSet::new();
    let mut visited = 0;
    let installer = PathBuf::from(&record.installer);
    // Added entries have no installer: rescan only their explicitly chosen executable.
    if !record.installer.is_empty() {
        walk(
            Path::new(&record.directory),
            &installer,
            &mut files,
            0,
            &mut visited,
        )?;
        walk(
            &Path::new(&record.prefix).join("drive_c"),
            &installer,
            &mut files,
            0,
            &mut visited,
        )?;
    }
    // Only what the installer created, unless that leaves nothing (a reinstall
    // over the same prefix); executables already registered always stay.
    if !record.baseline.is_empty() {
        let kept: BTreeSet<PathBuf> = files.iter().filter(|file| {
            !record.baseline.iter().any(|old| Path::new(old) == file.as_path())
                || record.entries.iter().any(|entry| Path::new(&entry.path) == file.as_path())
        }).cloned().collect();
        if !kept.is_empty() { files = kept; }
    }
    for item in &record.overrides {
        if !item.source.starts_with("peligames:") && Path::new(&item.executable).is_file() {
            files.insert(PathBuf::from(&item.source));
        }
    }
    files.retain(|file| {
        if record
            .overrides
            .iter()
            .any(|item| item.executable.is_empty() && Path::new(&item.source) == file)
        {
            return false;
        }
        !record.overrides.iter().any(|item| {
            item.executable != item.source
                && Path::new(&item.executable) == file
                && !record
                    .entries
                    .iter()
                    .any(|entry| Path::new(&entry.path) == file)
        })
    });
    let hints = if record.installer.is_empty() { Default::default() } else { super::installation_targets::discover(Path::new(&record.prefix)) };
    let multiple = files.len() > 1;
    record.entries = files
        .into_iter()
        .map(|file| {
            let executable = file.to_string_lossy().into_owned();
            let stem = file.file_stem().unwrap_or_default().to_string_lossy();
            let custom = record
                .overrides
                .iter()
                .find(|item| item.source == executable);
            let target = custom
                .map(|item| Path::new(&item.executable))
                .unwrap_or(&file);
            let hint = hints.get(target).filter(|_| custom.and_then(|c| c.prefix.as_deref()).is_none_or(|p| p == record.prefix));
            InstalledEntry {
                discovery_source: hint.map(|h| h.source.clone()),
                recommended: false, shortcut: None, profile: None,
                launch_arguments: hint.map(|h| h.arguments.clone()).unwrap_or_default(),
                advanced: custom
                    .and_then(|item| item.advanced.clone())
                    .unwrap_or_default(),
                name: custom.map(|item| item.name.clone()).unwrap_or_else(|| {
                    if multiple {
                        format!("{} · {stem}", record.name)
                    } else {
                        record.name.clone()
                    }
                }),
                path: executable.clone(),
                directory: target
                    .parent()
                    .unwrap_or(Path::new(&record.directory))
                    .to_string_lossy()
                    .into_owned(),
                executable: target.to_string_lossy().into_owned(),
                prefix: custom
                    .and_then(|item| item.prefix.clone())
                    .unwrap_or_else(|| record.prefix.clone()),
                proton: custom
                    .map(|item| item.proton.clone())
                    .unwrap_or_else(|| record.proton.clone()),
                cover_url: custom.and_then(|item| item.cover_url.clone()).or_else(|| record.cover_url.clone()),
                launcher: "PeliGames".into(),
            }
        })
        .collect();
    rank(&mut record.entries, &record.name);
    record.entries.extend(added);
    Ok(())
}
pub fn register(mut record: InstallationRecord) -> Result<usize, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let index = library_file()?;
    let _process_guard = process_lock(&index)?;
    let manifest = Path::new(&record.directory).join("installation.json");
    if manifest.is_file() {
        let previous: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        record.entries = previous.entries;
        record.overrides = previous.overrides;
    }
    scan(&mut record)?;
    record.known_shortcuts = Some(shortcut_targets(Path::new(&record.prefix)).into_keys().collect());
    record.install_shortcuts = Some(super::installation_targets::shortcut_files(Path::new(&record.prefix)).into_iter().map(|item| item.file.to_string_lossy().into_owned()).collect());
    let count = record.entries.len();
    write_json(
        &Path::new(&record.directory).join("installation.json"),
        &record,
    )?;
    let mut roots = destinations(&index)?;
    roots.insert(record.directory.clone());
    write_json(&index, &roots)?;
    Ok(count)
}
pub fn list(rescan: bool) -> Result<Vec<InstalledEntry>, String> {
    list_at(&library_file()?, rescan)
}
fn list_at(index: &Path, rescan: bool) -> Result<Vec<InstalledEntry>, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for directory in destinations(index)? {
        super::installation_layout::migrate(Path::new(&directory))?;
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.exists() {
            continue;
        }
        let mut record: InstallationRecord =
            serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| format!("Registro inválido em {directory}: {e}"))?;
        let old_prefix = Path::new(&directory).join(".peligames/prefix");
        let moved = Path::new(&record.prefix) == old_prefix;
        if moved {
            let prefix = Path::new(&directory).join("prefix");
            if let Ok(relative) = Path::new(&record.installer).strip_prefix(&old_prefix) {
                record.installer = prefix.join(relative).to_string_lossy().into_owned();
            }
            record.prefix = prefix.to_string_lossy().into_owned();
        }
        if rescan || moved {
            scan(&mut record)?;
            write_json(&manifest, &record)?;
        }
        // A profile's prefix is mounted again when needed (after a reboot).
        for profile in record.entries.iter().filter_map(|entry| entry.profile.as_ref()) {
            if let Err(error) = super::game_profiles::ensure_ready(profile) { eprintln!("Perfil {}: {error}", profile.root); }
        }
        // A profile whose prefix could not be mounted stays listed: starting it shows why.
        entries.extend(record.entries.into_iter().filter(|entry| {
            (entry.profile.is_some() || Path::new(&entry.executable).is_file()) && seen.insert(entry.path.clone())
        }));
    }
    Ok(entries)
}

#[derive(Clone, Serialize)]
pub struct ExecutableMatch {
    pub shortcuts: Option<Vec<String>>,
    pub entry: InstalledEntry,
    pub destination: String,
    pub installer: Option<String>,
}
pub fn find_executable(executable: &str) -> Result<Vec<ExecutableMatch>, String> {
    find_executable_at(&library_file()?, executable)
}
fn find_executable_at(index: &Path, executable: &str) -> Result<Vec<ExecutableMatch>, String> {
    let selected = super::game_installation::validate_executable(executable)?;
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let mut matches = vec![];
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.is_file() { continue; }
        let record: InstallationRecord = serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let installer = Path::new(&record.installer).canonicalize().ok().filter(|path| path.is_file());
        for entry in record.entries {
            if !Path::new(&entry.executable).is_file() { continue; }
            if Path::new(&entry.executable).canonicalize().ok().as_ref() == Some(&selected) || installer.as_ref() == Some(&selected) {
                matches.push(ExecutableMatch { shortcuts: super::pelinstall::shortcut_paths(&entry.path).ok(), entry, destination: directory.clone(), installer: installer.as_ref().map(|path| path.to_string_lossy().into_owned()) });
            }
        }
    }
    Ok(matches)
}

pub(crate) fn validate_selected_prefix(path: &str) -> Result<PathBuf, String> {
    let path = super::game_installation::absolute(path)?;
    let metadata = fs::symlink_metadata(&path).map_err(|e| format!("Prefixo indisponível: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Escolha uma pasta real de prefixo Wine/Proton.".into());
    }
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if canonical.parent().is_none()
        || !fs::symlink_metadata(canonical.join("drive_c"))
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        || !fs::symlink_metadata(canonical.join("system.reg"))
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
    {
        return Err("Selecione o prefixo Wine/Proton que contém drive_c e system.reg.".into());
    }
    if super::game_execution::prefix_in_use(&canonical) {
        return Err("Encerre os programas deste prefixo antes de alterá-lo.".into());
    }
    Ok(canonical)
}

pub fn set_cover(path: &str, cover: &str) -> Result<(), String> { set_cover_at(&library_file()?,path,cover) }
fn set_cover_at(index: &Path, path: &str, cover: &str) -> Result<(), String> {
    if !cover.starts_with("https://") || cover.len() > 8192 || cover.contains(['\n','\r','\0']) { return Err("Capa inválida.".into()); }
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?; let _process_guard = process_lock(index)?;
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json"); if !manifest.is_file() { continue; }
        let mut record: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if let Some(entry) = record.entries.iter_mut().find(|entry| entry.path == path) {
            entry.cover_url = Some(cover.into());
            if let Some(custom) = record.overrides.iter_mut().find(|item| item.source == path) { custom.cover_url = Some(cover.into()); }
            else { record.overrides.push(EntryOverride { source: entry.path.clone(), name: entry.name.clone(), proton: entry.proton.clone(), executable: entry.executable.clone(), prefix: Some(entry.prefix.clone()), advanced: Some(entry.advanced.clone()), cover_url: Some(cover.into()) }); }
            return write_json(&manifest,&record);
        }
    }
    Err("Entrada não registrada na biblioteca PeliGames.".into())
}
pub fn update_settings(settings: EntrySettings) -> Result<InstalledEntry, String> {
    update_at(&library_file()?, settings)
}
fn update_at(index: &Path, settings: EntrySettings) -> Result<InstalledEntry, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let name = settings.name.trim();
    if name.is_empty() {
        return Err("Informe o nome do jogo ou programa.".into());
    }
    let executable = super::game_installation::validate_executable(&settings.executable)?;
    let proton = super::game_installation::validate_proton(&settings.proton)?;
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.is_file() {
            continue;
        }
        let mut record: InstallationRecord =
            serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if let Some(entry) = record
            .entries
            .iter_mut()
            .find(|entry| entry.path == settings.path)
        {
            if let Some(prefix) = settings.prefix.as_deref() {
                if prefix != entry.prefix {
                    entry.prefix = validate_selected_prefix(prefix)?
                        .to_string_lossy()
                        .into_owned();
                }
            }
            if let Some(advanced) = &settings.advanced {
                advanced.validate_runner(&proton)?;
                entry.advanced = advanced.clone();
            }
            entry.name = name.into();
            entry.proton = proton.to_string_lossy().into_owned();
            if Path::new(&entry.executable) != executable { entry.launch_arguments.clear(); entry.discovery_source = None; }
            entry.executable = executable.to_string_lossy().into_owned();
            entry.directory = executable
                .parent()
                .ok_or("Diretório inválido.")?
                .to_string_lossy()
                .into_owned();
            let updated = entry.clone();
            record.overrides.retain(|item| item.source != settings.path);
            record.overrides.push(EntryOverride {
                cover_url: updated.cover_url.clone(),
                advanced: Some(updated.advanced.clone()),
                prefix: Some(updated.prefix.clone()),
                source: settings.path,
                name: updated.name.clone(),
                proton: updated.proton.clone(),
                executable: updated.executable.clone(),
            });
            write_json(&manifest, &record)?;
            return Ok(updated);
        }
    }
    Err("Entrada não registrada na biblioteca PeliGames.".into())
}

/// Keeps `path` as the installation's executable: the other discovered ones are
/// hidden (they stay hidden on later scans) and the chosen one takes the game's
/// name. Executables the player added by hand are kept.
fn record_executable_choices(record: &InstallationRecord, path: &str) -> Vec<InstalledEntry> {
    // A game a launcher installed (or its profile) is its own game: no alternatives from the launcher.
    if record.launcher_games.iter().any(|game| game == path) || record.entries.iter().any(|entry| entry.path == path && entry.profile.is_some()) {
        return record.entries.iter().filter(|entry| entry.path == path).cloned().collect();
    }
    let separate = |candidate: &str| record.launcher_games.iter().any(|game| game == candidate);
    let Some(template) = record.entries.iter().find(|entry| entry.path == path)
        .or_else(|| record.entries.iter().find(|entry| !entry.path.starts_with("peligames:"))) else { return vec![]; };
    let mut choices: Vec<_> = record.entries.iter()
        .filter(|entry| (entry.path == path || !entry.path.starts_with("peligames:") && !separate(&entry.path)) && Path::new(&entry.executable).is_file())
        .cloned().collect();
    for hidden in &record.overrides {
        if !hidden.executable.is_empty() || hidden.source.starts_with("peligames:") || separate(&hidden.source) || !Path::new(&hidden.source).is_file()
            || choices.iter().any(|entry| entry.path == hidden.source) { continue; }
        let mut entry = template.clone();
        entry.path = hidden.source.clone();
        entry.executable = hidden.source.clone();
        entry.discovery_source = None;
        entry.launch_arguments.clear();
        choices.push(entry);
    }
    choices
}

pub fn executable_choices(path: &str) -> Result<Vec<InstalledEntry>, String> {
    executable_choices_at(&library_file()?, path)
}
fn executable_choices_at(index: &Path, path: &str) -> Result<Vec<InstalledEntry>, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.is_file() { continue; }
        let record: InstallationRecord = serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if record.entries.iter().any(|entry| entry.path == path) { return Ok(record_executable_choices(&record, path)); }
    }
    Ok(vec![])
}

pub fn choose(path: &str) -> Result<InstalledEntry, String> {
    choose_at(&library_file()?, path)
}
fn choose_at(index: &Path, path: &str) -> Result<InstalledEntry, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.is_file() {
            continue;
        }
        let mut record: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if !record.entries.iter().any(|entry| entry.path == path) {
            if let Some(restored) = record_executable_choices(&record, path).into_iter().find(|entry| entry.path == path) {
                record.entries.push(restored);
            }
        }
        let Some(position) = record.entries.iter().position(|entry| entry.path == path) else { continue; };
        // Games a launcher installed stay: they are not alternatives to this executable.
        let separate = |candidate: &str| record.launcher_games.iter().any(|game| game == candidate);
        let launcher_game = separate(path);
        let hidden: Vec<InstalledEntry> = if launcher_game { vec![] } else { record.entries.iter()
            .filter(|entry| entry.path != path && !entry.path.starts_with("peligames:") && !separate(&entry.path)).cloned().collect() };
        let mut chosen = record.entries[position].clone();
        if !record.overrides.iter().any(|item| item.source == path) { chosen.name = record.name.clone(); }
        if !launcher_game { record.entries.retain(|entry| entry.path == path || entry.path.starts_with("peligames:") || separate(&entry.path)); }
        record.overrides.retain(|item| !hidden.iter().any(|entry| entry.path == item.source) && item.source != path);
        for entry in &hidden {
            record.overrides.push(EntryOverride {
                cover_url: None, advanced: None, source: entry.path.clone(), name: String::new(),
                proton: String::new(), executable: String::new(), prefix: Some(entry.prefix.clone()),
            });
        }
        record.overrides.push(EntryOverride {
            cover_url: chosen.cover_url.clone(), advanced: Some(chosen.advanced.clone()), prefix: Some(chosen.prefix.clone()),
            source: chosen.path.clone(), name: chosen.name.clone(), proton: chosen.proton.clone(), executable: chosen.executable.clone(),
        });
        if let Some(entry) = record.entries.iter_mut().find(|entry| entry.path == path) { *entry = chosen.clone(); }
        write_json(&manifest, &record)?;
        return Ok(chosen);
    }
    Err("Entrada não registrada na biblioteca PeliGames.".into())
}

/// Register an existing executable without running an installer or moving its files.
pub fn add(
    request: super::game_installation::InstallationRequest,
) -> Result<InstalledEntry, String> {
    add_at(&library_file()?, request)
}
fn add_at(
    index: &Path,
    request: super::game_installation::InstallationRequest,
) -> Result<InstalledEntry, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err("Confirme o nome do jogo ou programa.".into());
    }
    let executable = super::game_installation::validate_executable(&request.executable)?;
    let proton = super::game_installation::validate_proton(&request.proton)?;
    let directory = super::game_installation::absolute(&request.directory)?;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    if directory.parent().is_none() {
        return Err("Escolha uma pasta dedicada ao jogo.".into());
    }
    let prefix = directory.join("prefix");
    if fs::symlink_metadata(&prefix).is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()) {
        return Err("O prefixo deve ser uma pasta real dentro do destino.".into());
    }
    fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;
    let entry = InstalledEntry {
        discovery_source: None,
        recommended: false, shortcut: None, profile: None,
        launch_arguments: Vec::new(),
        advanced: Default::default(),
        name: name.into(),
        path: format!("peligames:{}", uuid::Uuid::new_v4()),
        directory: executable
            .parent()
            .ok_or("Executável sem diretório.")?
            .to_string_lossy()
            .into_owned(),
        executable: executable.to_string_lossy().into_owned(),
        prefix: prefix.to_string_lossy().into_owned(),
        proton: proton.to_string_lossy().into_owned(),
        cover_url: request.cover_url,
        launcher: "PeliGames".into(),
    };
    let manifest = directory.join("installation.json");
    let mut record = if manifest.is_file() {
        let existing: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Registro de instalação inválido: {e}"))?;
        if Path::new(&existing.directory) != directory {
            return Err("O destino registrado foi redirecionado.".into());
        }
        existing
    } else {
        InstallationRecord {
            name: entry.name.clone(), directory: directory.to_string_lossy().into_owned(),
            prefix: entry.prefix.clone(), proton: entry.proton.clone(), installer: String::new(),
            cover_url: entry.cover_url.clone(), entries: vec![], overrides: vec![], baseline: vec![], known_shortcuts: None, launcher_games: vec![], install_shortcuts: None, profile_kind: Default::default(),
        }
    };
    record.entries.push(entry.clone());
    record.overrides.push(EntryOverride {
        cover_url: entry.cover_url.clone(),
        advanced: None, prefix: None, source: entry.path.clone(), name: entry.name.clone(),
        proton: entry.proton.clone(), executable: entry.executable.clone(),
    });
    write_json(&directory.join("installation.json"), &record)?;
    let mut roots = destinations(index)?;
    roots.insert(record.directory);
    write_json(index, &roots)?;
    Ok(entry)
}
/// The UI supplies only a registered entry identity, never a directory to erase.
pub fn uninstall(path: String) -> Result<(), String> {
    uninstall_at(&library_file()?, &path)
}
fn uninstall_at(index: &Path, path: &str) -> Result<(), String> {
    uninstall_with_shortcuts_at(index, path, &super::pelinstall::shortcut_directories())
}
fn uninstall_with_shortcuts_at(index: &Path, path: &str, shortcut_directories: &[PathBuf]) -> Result<(), String> {
    // A launcher game's profile: only the profile goes, never the launcher's prefix.
    let profile_shortcut = record_of(index, path)?.and_then(|(_, record)| record.entries.into_iter().find(|entry| entry.path == path && entry.profile.is_some()))
        .and_then(|entry| entry.shortcut.map(|shortcut| (record_owner(index, path), shortcut)));
    if let Some((Some(owner), shortcut)) = profile_shortcut {
        return remove_shortcut_from_library_at(index, &owner, &shortcut);
    }
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let mut roots = destinations(index)?;
    for directory in roots.clone() {
        let root = Path::new(&directory);
        let manifest = root.join("installation.json");
        if !manifest.exists() {
            continue;
        }
        let record: InstallationRecord =
            serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if directory != path && !record.entries.iter().any(|entry| entry.path == path) {
            continue;
        }
        let canonical = root.canonicalize().map_err(|e| e.to_string())?;
        if Path::new(&record.directory) != canonical {
            return Err("O destino registrado foi redirecionado.".into());
        }
        let prefix = PathBuf::from(
            record
                .entries
                .iter()
                .find(|entry| entry.path == path)
                .map(|entry| entry.prefix.as_str())
                .unwrap_or(&record.prefix),
        );
        let mut affected = Vec::new();
        let mut shortcut_entries = BTreeSet::new();
        let mut managed = false;
        for destination in &roots {
            let other_root = Path::new(destination);
            let other_manifest = other_root.join("installation.json");
            if !other_manifest.is_file() {
                continue;
            }
            let mut other: InstallationRecord =
                serde_json::from_slice(&fs::read(&other_manifest).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let same_prefix = |entry: &InstalledEntry| Path::new(&entry.prefix) == prefix;
            let any = other.entries.iter().any(same_prefix);
            if any || (destination == &directory && other.entries.is_empty()) {
                let other_canonical = other_root.canonicalize().map_err(|e| e.to_string())?;
                if Path::new(&other.directory) != other_canonical {
                    return Err("Destino registrado inválido.".into());
                }
                managed |= prefix == other_canonical.join("prefix");
                let removed: BTreeSet<String> = other
                    .entries
                    .iter()
                    .filter(|entry| same_prefix(entry))
                    .map(|entry| entry.path.clone())
                    .collect();
                shortcut_entries.extend(removed.iter().cloned());
                other.entries.retain(|entry| !same_prefix(entry));
                other
                    .overrides
                    .retain(|item| !removed.contains(&item.source));
                for source in removed {
                    other.overrides.push(EntryOverride {
                        cover_url: None,
                        advanced: None,
                        source,
                        name: String::new(),
                        proton: String::new(),
                        executable: String::new(),
                        prefix: Some(prefix.to_string_lossy().into_owned()),
                    });
                }
                affected.push((destination.clone(), other_manifest, other));
            }
        }
        match fs::symlink_metadata(&prefix) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("Não é permitido remover um prefixo redirecionado.".into());
                }
                if !managed
                    && validate_selected_prefix(
                        &record
                            .entries
                            .iter()
                            .find(|entry| entry.path == path)
                            .map(|entry| entry.prefix.clone())
                            .unwrap_or(record.prefix.clone()),
                    )? != prefix
                {
                    return Err("Prefixo registrado inválido.".into());
                }
                if super::game_execution::prefix_in_use(&prefix) {
                    return Err("Encerre os programas deste prefixo antes de desinstalar.".into());
                }
                fs::remove_dir_all(&prefix)
                    .map_err(|e| format!("Não foi possível remover o prefixo: {e}"))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        for entry in shortcut_entries {
            super::pelinstall::remove_shortcuts_at(shortcut_directories, &entry)
                .map_err(|error| format!("O prefixo foi removido, mas não foi possível remover todos os atalhos. Tente desinstalar novamente: {error}"))?;
        }
        for (destination, manifest, remaining) in affected {
            if remaining.entries.is_empty() {
                fs::remove_file(manifest).map_err(|e| e.to_string())?;
                roots.remove(&destination);
            } else {
                write_json(&manifest, &remaining)?;
            }
        }
        write_json(index, &roots)?;
        return Ok(());
    }
    Err("Entrada não registrada na biblioteca PeliGames.".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record(base: &Path, baseline: Vec<String>) -> InstallationRecord {
        InstallationRecord {
            name: "Jogo".into(), directory: base.to_string_lossy().into_owned(),
            prefix: base.join("prefix").to_string_lossy().into_owned(), proton: String::new(),
            installer: base.join("Setup.exe").to_string_lossy().into_owned(), cover_url: None,
            entries: vec![], overrides: vec![], baseline, known_shortcuts: None, launcher_games: vec![], install_shortcuts: None, profile_kind: Default::default(),
        }
    }
    fn entry(executable: &Path, source: Option<&str>) -> InstalledEntry {
        InstalledEntry {
            discovery_source: source.map(String::from), recommended: false, shortcut: None, profile: None, launch_arguments: vec![], advanced: Default::default(),
            name: String::new(), path: executable.to_string_lossy().into_owned(), directory: String::new(),
            executable: executable.to_string_lossy().into_owned(), prefix: String::new(), proton: String::new(), cover_url: None, launcher: "PeliGames".into(),
        }
    }
    /// Minimal Windows shortcut: relative path and working folder (ANSI).
    fn shortcut(target: &str, folder: &str) -> Vec<u8> {
        let mut bytes = vec![0u8; 76];
        bytes[0..4].copy_from_slice(&76u32.to_le_bytes());
        bytes[4..20].copy_from_slice(&[1, 20, 2, 0, 0, 0, 0, 0, 192, 0, 0, 0, 0, 0, 0, 70]);
        bytes[20..24].copy_from_slice(&24u32.to_le_bytes());
        for value in [target, folder] { bytes.extend_from_slice(&(value.len() as u16).to_le_bytes()); bytes.extend_from_slice(value.as_bytes()); }
        bytes
    }
    #[test]
    fn launcher_games_are_offered_once_and_added_with_their_shortcut_name() {
        let base = std::env::temp_dir().join(format!("peligames-launcher-{}", uuid::Uuid::new_v4()));
        let drive = base.join("prefix/drive_c");
        let desktop = drive.join("users/steamuser/Desktop");
        fs::create_dir_all(&desktop).unwrap();
        fs::write(base.join("prefix/system.reg"), "").unwrap();
        let put = |path: &str| { let file = drive.join(path); fs::create_dir_all(file.parent().unwrap()).unwrap(); fs::write(&file, b"MZ").unwrap(); file.canonicalize().unwrap() };
        let ea = put("Program Files/Electronic Arts/EA Desktop/EALauncher.exe");
        fs::write(desktop.join("EA.lnk"), shortcut("C:\\Program Files\\Electronic Arts\\EA Desktop\\EALauncher.exe", "C:\\Program Files\\Electronic Arts\\EA Desktop")).unwrap();
        let mut record = record(&base, vec![]);
        record.name = "EA app".into();
        record.entries = vec![InstalledEntry { name: "EA app".into(), prefix: record.prefix.clone(), ..entry(&ea, Some("Atalho do Windows")) }];
        let index = base.join("destinations.json");
        write_json(&index, &BTreeSet::from([base.to_string_lossy().into_owned()])).unwrap();
        write_json(&base.join("installation.json"), &record).unwrap();
        let launcher = ea.to_string_lossy().into_owned();
        // An older record only starts tracking the shortcuts it already has.
        assert!(new_launcher_games_at(&index, &launcher).unwrap().is_empty());
        // The EA app installs a game and creates its shortcut.
        let game = put("Program Files/EA Games/Battlefield 2042/BF2042.exe");
        fs::write(desktop.join("Battlefield 2042.lnk"), shortcut("C:\\Program Files\\EA Games\\Battlefield 2042\\BF2042.exe", "C:\\Program Files\\EA Games\\Battlefield 2042")).unwrap();
        let found = new_launcher_games_at(&index, &launcher).unwrap();
        assert_eq!(found.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["Battlefield 2042"]);
        assert!(new_launcher_games_at(&index, &launcher).unwrap().is_empty(), "cada atalho é oferecido uma vez");
        let added = add_launcher_games_at(&index, &launcher, found, vec![]).unwrap();
        assert_eq!((added[0].name.as_str(), added[0].executable.as_str()), ("Battlefield 2042", game.to_string_lossy().as_ref()));
        // A later rescan keeps the game with its own name.
        let mut saved: InstallationRecord = serde_json::from_slice(&fs::read(base.join("installation.json")).unwrap()).unwrap();
        scan(&mut saved).unwrap();
        assert!(saved.entries.iter().any(|entry| entry.name == "Battlefield 2042"));
        write_json(&base.join("installation.json"), &saved).unwrap();
        // The launcher's executable choices never include its games, and changing it keeps them.
        let game_path = game.to_string_lossy().into_owned();
        assert!(executable_choices_at(&index, &launcher).unwrap().iter().all(|entry| entry.path != game_path));
        assert_eq!(executable_choices_at(&index, &game_path).unwrap().len(), 1);
        choose_at(&index, &launcher).unwrap();
        let mut saved: InstallationRecord = serde_json::from_slice(&fs::read(base.join("installation.json")).unwrap()).unwrap();
        scan(&mut saved).unwrap();
        assert!(saved.entries.iter().any(|entry| entry.path == game_path), "trocar o executável do launcher não esconde o jogo");
        // Executables outside the prefix are refused.
        assert!(add_launcher_games_at(&index, &launcher, vec![LauncherGame { name: "x".into(), executable: "/bin/sh".into() }], vec![]).is_err());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn programs_panel_lists_the_shortcuts_launchers_created() {
        let base = std::env::temp_dir().join(format!("peligames-programs-{}", uuid::Uuid::new_v4()));
        let drive = base.join("prefix/drive_c");
        let desktop = drive.join("users/steamuser/Desktop");
        let menu = drive.join("ProgramData/Microsoft/Windows/Start Menu/Programs");
        for folder in [&desktop, &menu] { fs::create_dir_all(folder).unwrap(); }
        fs::write(base.join("prefix/system.reg"), "").unwrap();
        let put = |path: &str| { let file = drive.join(path); fs::create_dir_all(file.parent().unwrap()).unwrap(); fs::write(&file, b"MZ").unwrap(); file.canonicalize().unwrap() };
        let launcher = put("Program Files/Battle.net/Battle.net Launcher.exe");
        put("windows/system32/start.exe");
        fs::write(desktop.join("Battle.net.lnk"), shortcut("C:\\Program Files\\Battle.net\\Battle.net Launcher.exe", "C:\\Program Files\\Battle.net")).unwrap();
        let mut record = record(&base, vec![]);
        record.entries = vec![InstalledEntry { prefix: record.prefix.clone(), ..entry(&launcher, Some("Atalho do Windows")) }];
        record.install_shortcuts = Some(super::super::installation_targets::shortcut_files(&base.join("prefix")).into_iter().map(|item| item.file.to_string_lossy().into_owned()).collect());
        let index = base.join("destinations.json");
        write_json(&index, &BTreeSet::from([base.to_string_lossy().into_owned()])).unwrap();
        write_json(&base.join("installation.json"), &record).unwrap();
        let path = launcher.to_string_lossy().into_owned();
        // The launcher installs games: a shortcut to itself with arguments (Desktop and Start Menu) and a launcher address.
        let mut overwatch = shortcut("C:\\Program Files\\Battle.net\\Battle.net Launcher.exe", "C:\\Program Files\\Battle.net");
        overwatch[20..24].copy_from_slice(&(24u32 | 32).to_le_bytes());
        let arguments = "--exec=\"launch Pro\"";
        overwatch.extend_from_slice(&(arguments.len() as u16).to_le_bytes());
        overwatch.extend_from_slice(arguments.as_bytes());
        fs::write(desktop.join("Overwatch.lnk"), &overwatch).unwrap();
        fs::write(menu.join("Overwatch.lnk"), &overwatch).unwrap();
        fs::write(desktop.join("Far Cry 6.url"), "[InternetShortcut]\r\nURL=uplay://launch/5266/0\r\n").unwrap();
        fs::write(desktop.join("Site oficial.url"), "[InternetShortcut]\r\nURL=https://example.com\r\n").unwrap();
        // Epic: games named by its install manifests, without any shortcut.
        let manifests = drive.join("ProgramData/Epic/EpicGamesLauncher/Data/Manifests");
        fs::create_dir_all(&manifests).unwrap();
        put("Program Files/Epic Games/Alan Wake 2/AlanWake2.exe");
        fs::write(manifests.join("A1.item"), r#"{"DisplayName":"Alan Wake 2","AppName":"Dodo","CatalogNamespace":"dc9d2e","CatalogItemId":"4a5b","InstallLocation":"C:\\Program Files\\Epic Games\\Alan Wake 2","LaunchExecutable":"AlanWake2.exe","bIsIncompleteInstall":false}"#).unwrap();
        fs::write(manifests.join("B2.item"), r#"{"DisplayName":"Baixando","AppName":"Half","InstallLocation":"C:\\Games\\Half","LaunchExecutable":"x.exe","bIsIncompleteInstall":true}"#).unwrap();
        let mut list = launcher_shortcuts_at(&index, &path).unwrap();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(list.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(), ["Alan Wake 2", "Far Cry 6", "Overwatch"]);
        let (program, args) = shortcut_launch_at(&index, &path, &list[0].id).unwrap();
        assert!(program.ends_with("system32/start.exe") && args == ["com.epicgames.launcher://apps/dc9d2e%3A4a5b%3ADodo?action=launch&silent=true"], "{args:?}");
        assert!(list[0].icon.as_deref().is_some_and(|icon| icon.ends_with("AlanWake2.exe")));
        let list = list[1..].to_vec();
        let (program, args) = shortcut_launch_at(&index, &path, &list[1].id).unwrap();
        assert!(program.ends_with("Battle.net Launcher.exe") && args == ["--exec=launch Pro"], "{program} {args:?}");
        let (program, args) = shortcut_launch_at(&index, &path, &list[0].id).unwrap();
        assert!(program.ends_with("system32/start.exe") && args == ["uplay://launch/5266/0"]);
        // Right click → add to the library: an entry of its own that survives rescans.
        let overwatch = list[1].id.clone();
        // Each game gets its own profile prefix (a stand-in here: FUSE is not available in tests).
        let fake = |base: &Path, profiles: &Path, _kind| {
            let root = profiles.join(uuid::Uuid::new_v4().to_string());
            fs::create_dir_all(root.join("prefix")).unwrap();
            Ok(super::super::game_profiles::GameProfile { kind: super::super::game_profiles::ProfileKind::Reflink, base: base.to_string_lossy().into_owned(), root: root.to_string_lossy().into_owned() })
        };
        let added = add_shortcut_to_library_at(&index, &path, &overwatch, fake).unwrap();
        assert_eq!((added.name.as_str(), added.launch_arguments.clone()), ("Overwatch", vec!["--exec=launch Pro".to_string()]));
        let profile = added.profile.clone().unwrap();
        assert!(added.prefix.ends_with("/prefix") && added.prefix.contains("/profiles/"));
        assert!(added.executable.starts_with(&added.prefix) && added.executable.ends_with("Battle.net Launcher.exe"));
        assert!(launcher_shortcuts_at(&index, &path).unwrap().iter().any(|item| item.id == overwatch && item.in_library));
        let mut saved: InstallationRecord = serde_json::from_slice(&fs::read(base.join("installation.json")).unwrap()).unwrap();
        scan(&mut saved).unwrap();
        assert!(saved.entries.iter().any(|entry| entry.name == "Overwatch" && entry.launch_arguments == ["--exec=launch Pro"]));
        remove_shortcut_from_library_at(&index, &path, &overwatch).unwrap();
        assert!(launcher_shortcuts_at(&index, &path).unwrap().iter().all(|item| !item.in_library));
        assert!(!Path::new(&profile.root).exists(), "o perfil sai junto");
        assert!(launcher.is_file(), "o prefixo do launcher fica intacto");
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn ranking_picks_the_game_when_it_clearly_wins() {
        let base = std::env::temp_dir().join(format!("peligames-rank-{}", uuid::Uuid::new_v4()));
        let put = |path: &str, size: usize| { let file = base.join(path); fs::create_dir_all(file.parent().unwrap()).unwrap(); fs::write(&file, vec![0u8; size]).unwrap(); file };
        // Unreal (Clair Obscur): the root launcher beside Engine wins over the big Shipping build.
        let root = put("ue/Expedition33_Steam.exe", 10);
        fs::create_dir_all(base.join("ue/Engine")).unwrap();
        let shipping = put("ue/Sandfall/Binaries/Win64/SandFall-Win64-Shipping.exe", 100);
        let mut entries = vec![entry(&shipping, None), entry(&root, None)];
        rank(&mut entries, "Clair Obscur Expedition 33");
        assert!(entries[0].executable.ends_with("Expedition33_Steam.exe") && entries[0].recommended && !entries[1].recommended);
        // Unity: the executable beside <name>_Data wins over a map editor.
        let game = put("unity/Game.exe", 10);
        fs::create_dir_all(base.join("unity/Game_Data")).unwrap();
        let editor = put("unity/MapEditor.exe", 50);
        let mut entries = vec![entry(&editor, None), entry(&game, None)];
        rank(&mut entries, "Some Game");
        assert!(entries[0].executable.ends_with("Game.exe") && entries[0].recommended);
        // Two shortcuts and nothing else to tell them apart: ask.
        let first = put("tie/Alpha.exe", 10);
        let second = put("tie/Beta.exe", 10);
        let mut entries = vec![entry(&first, Some("Atalho do Windows")), entry(&second, Some("Atalho do Windows"))];
        rank(&mut entries, "Collection");
        assert!(entries.iter().all(|entry| !entry.recommended));
        // A single executable needs no question.
        let mut entries = vec![entry(&first, None)];
        rank(&mut entries, "Alpha");
        assert!(entries[0].recommended);
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn scan_offers_only_what_the_installer_created() {
        let base = std::env::temp_dir().join(format!("peligames-scan-{}", uuid::Uuid::new_v4()));
        let drive = base.join("prefix/drive_c");
        for file in ["Program Files (x86)/Steam/steam.exe", "Outro/Antigo.exe", "Games/Jogo/Jogo.exe",
                     "Games/Jogo/Engine/crashpad_handler.exe", "Games/Jogo/QtWebEngineProcess.exe", "Games/Jogo/unins000.exe"] {
            let path = drive.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"MZ").unwrap();
        }
        let prefix = base.join("prefix");
        // The prefix before the installer: Proton's Steam stub (filtered by name anyway) and another program.
        let before: Vec<String> = prefix_executables(&prefix).unwrap().into_iter()
            .filter(|path| !path.contains("/Games/")).collect();
        assert_eq!(before.len(), 1, "o steam.exe do Proton nunca entra: {before:?}");
        let mut with = record(&base, before);
        scan(&mut with).unwrap();
        let found: Vec<_> = with.entries.iter().map(|entry| entry.executable.rsplit('/').next().unwrap().to_string()).collect();
        assert_eq!(found, ["Jogo.exe"]);
        // Without a snapshot (older installations) everything else is still listed.
        let mut without = record(&base, vec![]);
        scan(&mut without).unwrap();
        assert_eq!(without.entries.len(), 2);
        // A reinstall over the same prefix: the snapshot holds the game too, so nothing is new.
        let all = prefix_executables(&prefix).unwrap();
        let mut again = record(&base, all);
        scan(&mut again).unwrap();
        assert_eq!(again.entries.len(), 2, "sem nada novo, volta à lista completa");
        // Choosing one executable hides the others for good and names it after the game.
        let index = base.join("destinations.json");
        write_json(&index, &BTreeSet::from([base.to_string_lossy().into_owned()])).unwrap();
        write_json(&base.join("installation.json"), &without).unwrap();
        let game = without.entries.iter().find(|entry| entry.executable.ends_with("Jogo.exe")).unwrap().path.clone();
        let chosen = choose_at(&index, &game).unwrap();
        assert_eq!(chosen.name, "Jogo");
        let mut saved: InstallationRecord = serde_json::from_slice(&fs::read(base.join("installation.json")).unwrap()).unwrap();
        scan(&mut saved).unwrap();
        assert_eq!(saved.entries.iter().map(|entry| (entry.name.as_str(), entry.path == game)).collect::<Vec<_>>(), [("Jogo", true)]);
        let choices = executable_choices_at(&index, &game).unwrap();
        assert_eq!(choices.len(), 2, "hidden executable remains available for reselection");
        let other = choices.iter().find(|entry| entry.path != game).unwrap().path.clone();
        let changed = choose_at(&index, &other).unwrap();
        assert_eq!(changed.path, other);
        assert_eq!(executable_choices_at(&index, &other).unwrap().len(), 2);
        let restored = choose_at(&index, &game).unwrap();
        assert_eq!(restored.path, game);
        let visible = list_at(&index, true).unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].path, game);
        fs::remove_dir_all(base).unwrap();
    }
}
