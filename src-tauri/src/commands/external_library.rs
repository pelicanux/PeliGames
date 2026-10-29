//! Read-only discovery and launcher-owned execution. A user-selected Proton is
//! applied to the source launcher's per-game runner configuration before launch.
use super::launcher_library::{self, LauncherGame};
use crate::core::advanced_settings::{launch_command, AdvancedSettings};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());
#[path = "external_proton.rs"]
mod external_proton;
/// Steam adapters follow reorganized runners and get readable names (see `external_proton`).
pub(crate) fn organize_steam_tools() -> Vec<String> { external_proton::organize_steam_tools() }
#[path = "epic_runtime.rs"]
mod epic_runtime;
#[path = "steam_settings.rs"]
mod steam_settings;
#[path = "steam_proton_state.rs"]
mod steam_proton_state;
#[derive(Clone, Serialize)]
pub struct ExternalGame {
    pub name: String,
    pub native_linux: bool,
    pub steam_pending: bool,
    pub steam_proton_name: Option<String>,
    pub path: String,
    pub directory: String,
    pub launcher: String,
    pub app_id: Option<String>,
    pub cover_url: Option<String>,
    pub prefix: Option<String>,
    pub proton: Option<String>,
    pub executable: Option<String>,
    pub external_id: String,
    pub external_store: Option<String>,
    pub flatpak: bool,
    pub advanced: AdvancedSettings,
}
#[derive(Deserialize)]
pub struct ExternalSettings {
    pub external_id: String,
    pub advanced: AdvancedSettings,
    #[serde(default)]
    pub proton: Option<String>,
    #[serde(default)]
    pub directory: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct SavedExternalSettings {
    #[serde(flatten)]
    advanced: AdvancedSettings,
    #[serde(default)]
    proton: Option<String>,
    #[serde(default)]
    directory: Option<String>,
}
fn settings_file() -> Result<PathBuf, String> {
    Ok(crate::core::paths::app_path("external-game-settings.json")?)
}
fn read_settings() -> Result<BTreeMap<String, SavedExternalSettings>, String> {
    let path = settings_file()?;
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "Configurações locais dos jogos externos inválidas.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(_) => Err("Não foi possível ler as configurações dos jogos externos.".into()),
    }
}
fn identity(launcher: &str, id: &str, flatpak: bool) -> String {
    format!(
        "{launcher}:{}:{id}",
        if flatpak { "flatpak" } else { "native" }
    )
}
// The config filename is not a Lutris slug. Resolve it against its read-only DB.
fn lutris_ids(home: &Path, flatpak: bool) -> BTreeMap<String, (String, String)> {
    let db = if flatpak {
        home.join(".var/app/net.lutris.Lutris/data/lutris/pga.db")
    } else {
        home.join(".local/share/lutris/pga.db")
    };
    if !db.is_file() {
        return BTreeMap::new();
    }
    read_lutris_ids(&db).unwrap_or_default()
}
/// `configpath → (id, name)` of the installed games, from a read-only connection.
fn read_lutris_ids(db: &Path) -> rusqlite::Result<BTreeMap<String, (String, String)>> {
    use rusqlite::{types::Value, Connection, OpenFlags};
    let connection = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    connection.busy_timeout(std::time::Duration::from_secs(1))?;
    connection.pragma_update(None, "query_only", true)?;
    let text = |value: Value| match value {
        Value::Integer(number) => number.to_string(),
        Value::Real(number) => number.to_string(),
        Value::Text(text) => text,
        _ => String::new(),
    };
    let mut query = connection.prepare("SELECT id, name, configpath FROM games WHERE installed=1 LIMIT 5000")?;
    let rows = query.query_map([], |row| Ok((text(row.get(0)?), text(row.get(1)?), text(row.get(2)?))))?;
    Ok(rows.flatten().filter(|(_, _, config)| !config.is_empty()).map(|(id, name, config)| (config, (id, name))).collect())
}
fn discover(home: &Path) -> Vec<ExternalGame> {
    let mut result = Vec::new();
    for (steam, flatpak) in [false, true].into_iter().flat_map(|flatpak| super::scanner::scan_steam_main(home, flatpak).into_iter().map(move |game| (game, flatpak))) {
        let Some(id) = steam.app_id.clone() else { continue; };
        let prefix = Path::new(&steam.path)
            .ancestors()
            .find(|p| p.file_name().is_some_and(|n| n == "steamapps"))
            .map(|p| p.join("compatdata").join(&id).join("pfx"))
            .filter(|p| p.is_dir())
            .map(|p| p.to_string_lossy().into_owned());
        let key = identity("Steam", &id, flatpak);
        if result.iter().any(|g: &ExternalGame| g.external_id == key) {
            continue;
        }
        result.push(ExternalGame {
            steam_pending: false, steam_proton_name: None,
            native_linux: super::native_games::steam_native(Path::new(&steam.path), &id), name: steam.name,
            directory: steam.path.clone(),
            path: steam.path,
            launcher: "Steam".into(),
            app_id: Some(id.clone()),
            cover_url: steam.cover_url,
            prefix,
            proton: None,
            executable: None,
            external_id: key,
            external_store: None,
            flatpak,
            advanced: AdvancedSettings::default(),
        });
    }
    let lutris_native = lutris_ids(home, false);
    let lutris_flatpak = lutris_ids(home, true);
    for game in launcher_library::heroic(home)
        .into_iter()
        .chain(super::epic_library::scan(home))
        .chain(launcher_library::lutris(home))
        .chain(launcher_library::faugus(home))
        .chain(launcher_library::bottles(home))
    {
        let Some(mut id) = game.external_id.clone().filter(|id| !id.is_empty()) else {
            continue;
        };
        let mut name = game.name.clone();
        if game.launcher == "Lutris" {
            let database = if game.flatpak {
                &lutris_flatpak
            } else {
                &lutris_native
            };
            let Some((db_id, db_name)) = database.get(&id) else {
                continue;
            };
            id = db_id.clone();
            name = db_name.clone();
        }
        let scoped_id = format!(
            "{}:{id}",
            game.external_store.as_deref().unwrap_or_default()
        );
        let key = identity(&game.launcher, &scoped_id, game.flatpak);
        if result.iter().any(|g| g.external_id == key) {
            continue;
        }
        result.push(from_launcher(game, key, id, name));
    }
    result
}
fn from_launcher(game: LauncherGame, key: String, id: String, name: String) -> ExternalGame {
    ExternalGame {
        steam_pending: false, steam_proton_name: None,
        native_linux: game.native_linux || (game.prefix.is_none() && game.proton.is_none() && game.executable.as_deref().map(|exe| super::native_games::executable_is_native(Path::new(exe))).unwrap_or_else(|| super::native_games::installed_native(Path::new(&game.path)))),
        name,
        path: game.path.clone(),
        directory: game.path,
        launcher: game.launcher,
        app_id: Some(id),
        cover_url: game.cover_url,
        prefix: game.prefix,
        proton: game.proton,
        executable: game.executable,
        external_id: key,
        external_store: game.external_store,
        flatpak: game.flatpak,
        advanced: AdvancedSettings::default(),
    }
}
pub(super) fn all() -> Result<Vec<ExternalGame>, String> {
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let preferences = read_settings()?;
    let mut games = discover(&home);
    let steam_pending = steam_settings::pending_ids()?;
    for game in &mut games {
        if let Some(settings) = preferences.get(&game.external_id) {
            game.advanced = settings.advanced.clone();
            if let Some(directory) = &settings.directory { game.directory = directory.clone(); }
            if !game.native_linux && (game.launcher != "Steam" || steam_pending.contains(&game.external_id)) { if let Some(proton) = &settings.proton { game.proton = Some(proton.clone()); } }
        }
        if game.launcher == "Steam" { game.steam_pending = steam_pending.contains(&game.external_id); }
        if game.launcher == "Steam" && !game.native_linux && !game.steam_pending {
            if let Some(selection) = game.app_id.as_deref().and_then(|app| steam_proton_state::selected(&home, game.flatpak, app)) {
                game.proton = selection.proton;
                game.steam_proton_name = Some(selection.name);
            }
        }
        // Without a choice here, show the Proton the launcher last ran the prefix with.
        if game.native_linux { game.proton = None; }
        else if game.proton.is_none() && game.steam_proton_name.is_none() {
            game.proton = game.prefix.as_deref().and_then(|prefix| crate::core::game_installation::proton_from_prefix(Path::new(prefix)));
        }
    }
    Ok(games)
}
#[tauri::command]
pub async fn scan_external_library() -> Result<Vec<ExternalGame>, String> {
    let mut games = tauri::async_runtime::spawn_blocking(all)
        .await
        .map_err(|_| "Não foi possível buscar jogos externos.")??;
    for game in &mut games {
        if let Some(url) = game.cover_url.as_ref().filter(|u| u.starts_with("file://")) {
            game.cover_url = match reqwest::Url::parse(url)
                .ok()
                .and_then(|u| u.to_file_path().ok())
            {
                Some(path) => super::covers::cache_heroic_cover(path,game.name.clone())
                    .await
                    .ok()
                    .and_then(|p| reqwest::Url::from_file_path(p).ok())
                    .map(|u| u.to_string()),
                None => None,
            };
        }
    }
    Ok(games)
}
#[tauri::command]
pub async fn configure_external_game(settings: ExternalSettings) -> Result<ExternalGame, String> {
    tauri::async_runtime::spawn_blocking(move || {
        settings.advanced.validate()?;
        let _guard = SETTINGS_LOCK
            .lock()
            .map_err(|_| "Configurações ocupadas.")?;
        let mut game = all()?
            .into_iter()
            .find(|g| g.external_id == settings.external_id)
            .ok_or("Jogo não encontrado no launcher de origem. Atualize a biblioteca.")?;
        if game.flatpak && settings.advanced.overlay_enabled == Some(true) {return Err("O overlay interno ainda não está disponível no Flatpak.".into());}
        let mut saved = read_settings()?;
        if game.native_linux && settings.proton.is_some() { return Err("Este jogo usa a versão nativa de Linux. A troca de Proton está desabilitada.".into()); }
        let previous_proton = if game.launcher == "Steam" { game.proton.clone() } else { saved.get(&settings.external_id).filter(|_| !game.native_linux).and_then(|saved| saved.proton.clone()) };
        let proton = match settings.proton.as_deref() {
            Some(value) => {
                if game.launcher == "Bottles" {return Err("O Bottles compartilha a versão entre os programas da mesma bottle. Altere o runner no Bottles.".into());}
                let runner = crate::core::game_installation::validate_proton(value)?;
                if crate::core::game_installation::native_wine(&runner).is_some() && !matches!(game.launcher.as_str(), "Heroic" | "Lutris") {
                    return Err("Este launcher exige um runner Proton. Use Wine nos jogos do PeliGames, Heroic ou Lutris.".into());
                }
                Some(runner.to_string_lossy().into_owned())
            },
            None => previous_proton,
        };
        let directory = match settings.directory.as_deref() {
            Some(value) => {
                let path = Path::new(value);
                if !path.is_absolute() || !path.is_dir() { return Err("Escolha um diretório do jogo existente.".into()); }
                Some(path.canonicalize().map_err(|e|e.to_string())?.to_string_lossy().into_owned())
            },
            None => saved.get(&settings.external_id).and_then(|saved| saved.directory.clone()),
        };
        let next = SavedExternalSettings {advanced: settings.advanced.clone(), proton: proton.clone(), directory: directory.clone()};
        let previous = saved.get(&game.external_id).cloned();
        let before = saved.clone();
        saved.insert(settings.external_id, next.clone());
        let file = settings_file()?;
        let temporary = file.with_extension("json.part");
        std::fs::write(
            &temporary,
            serde_json::to_vec_pretty(&saved).map_err(|_| "Configurações inválidas.")?,
        )
        .map_err(|_| "Não foi possível gravar configurações.")?;
        std::fs::rename(temporary, file).map_err(|_| "Não foi possível salvar configurações.")?;
        if game.launcher == "Epic Games" && !game.native_linux {
            let mut proposed = game.clone();
            proposed.advanced = settings.advanced.clone();
            let selected = proton.as_deref().map(Path::new);
            if let Err(error) = epic_runtime::configure(&dirs::home_dir().ok_or("Pasta pessoal indisponível.")?, &proposed, selected) {
                std::fs::write(settings_file()?, serde_json::to_vec_pretty(&before).map_err(|e|e.to_string())?).map_err(|e|format!("{error}; falha ao restaurar preferências: {e}"))?;
                return Err(error);
            }
        }
        if game.launcher == "Steam" {
            let baseline = Some(SavedExternalSettings { advanced: previous.as_ref().map(|s| s.advanced.clone()).unwrap_or_default(), proton: game.proton.clone(), directory: previous.and_then(|s| s.directory) });
            if let Err(error) = steam_settings::stage(&game, baseline, next) {
                std::fs::write(settings_file()?, serde_json::to_vec_pretty(&before).map_err(|e|e.to_string())?).map_err(|e| format!("{error}; falha ao restaurar configurações: {e}"))?;
                return Err(error);
            }
        }
        if game.launcher != "Steam" {
            super::launch_profiles::refresh(Path::new(&game.path), &game.name, &settings.advanced)?;
        }
        game.steam_pending = game.launcher == "Steam" && steam_settings::pending(&game.external_id)?;
        game.advanced = settings.advanced;
        if let Some(directory) = directory { game.directory = directory; }
        if let Some(proton) = proton { game.proton = Some(proton); game.steam_proton_name = None; }
        Ok(game)
    })
    .await
    .map_err(|_| "Falha ao salvar configurações externas.")?
}
fn custom_options(settings: &AdvancedSettings) -> Result<Command, String> {
    let mut options = Command::new("true");
    if let Some(value) = &settings.launch_options {
        super::nexus_launch_options::apply_custom(&mut options, value)?;
    }
    Ok(options)
}

/// Mod deployment already owns the game files. Its launch requirements are
/// saved in the same persistent runtime profile as monitoring and wrappers.
pub(super) fn save_steam_mod_runtime(external_id: &str, app_id: &str, launch_options: Option<String>, line: Option<String>) -> Result<(bool, Option<String>), String> {
    let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?;
    let game = all()?.into_iter().find(|game| game.external_id == external_id && game.launcher == "Steam" && game.app_id.as_deref() == Some(app_id)).ok_or("Jogo Steam não encontrado. Atualize a biblioteca.")?;
    let mut saved = read_settings()?;
    let snapshot = saved.clone();
    let before = saved.get(external_id).cloned().unwrap_or_else(|| SavedExternalSettings { advanced: game.advanced.clone(), proton: game.proton.clone(), directory: None });
    let mut after = before.clone();
    let previous_mods = super::steam_runtime::mod_options(&game)?;
    let launch_options = super::mod_launch_options::merge(launch_options.as_deref(), previous_mods.as_deref(), None)?;
    after.advanced.launch_options = launch_options.clone();
    saved.insert(external_id.into(), after.clone());
    let path = settings_file()?;
    let temporary = path.with_extension("json.part");
    std::fs::write(&temporary, serde_json::to_vec_pretty(&saved).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    std::fs::rename(temporary, &path).map_err(|error| error.to_string())?;
    if let Err(error) = steam_settings::stage_with_mods(&game, Some(before), after, line) {
        std::fs::write(&path, serde_json::to_vec_pretty(&snapshot).map_err(|rollback| format!("{error}; {rollback}"))?).map_err(|rollback| format!("{error}; não foi possível restaurar as preferências: {rollback}"))?;
        return Err(error);
    }
    Ok((steam_settings::pending(external_id)?, launch_options))
}
fn invocation(game: &ExternalGame) -> Result<(String, Vec<String>), String> {
    let id = game
        .app_id
        .as_deref()
        .ok_or("Identificação externa ausente.")?;
    if game.launcher == "Epic Games" {
        return super::epic_library::command(&dirs::home_dir().ok_or("Pasta pessoal indisponível.")?, id);
    }
    let custom = custom_options(&game.advanced)?;
    let extra: Vec<String> = custom
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    if !extra.is_empty() && !["Steam", "Heroic", "Bottles"].contains(&game.launcher.as_str()) {
        return Err(format!("{} não aceita argumentos de jogo por esta integração. Configure esses argumentos no launcher de origem; os ajustes de FPS, FSR e variáveis continuam disponíveis aqui.",game.launcher));
    }
    let (program, args) = match game.launcher.as_str() {
        "Steam" => {
            if id.parse::<u32>().ok().filter(|i| *i > 0).is_none() {
                return Err("Steam AppID inválido.".into());
            }
            (
                "steam",
                if extra.is_empty() {
                    vec![format!("steam://launch/{id}/dialog")]
                } else {
                    let mut args = vec!["-applaunch".into(), id.into()];
                    args.extend(extra.clone());
                    args
                },
            )
        }
        "Heroic" => {
            let store = game
                .external_store
                .as_deref()
                .ok_or("Loja Heroic ausente.")?;
            if !["legendary", "gog", "nile", "sideload"].contains(&store) {
                return Err("Loja Heroic não reconhecida.".into());
            }
            let mut url =
                reqwest::Url::parse("heroic://launch").map_err(|_| "Endereço Heroic inválido.")?;
            url.query_pairs_mut()
                .append_pair("appName", id)
                .append_pair("runner", store);
            for argument in &extra {
                url.query_pairs_mut()
                    .append_pair("arg", &urlencoding::encode(argument));
            }
            ("heroic", vec!["--no-gui".into(), url.to_string()])
        }
        "Lutris" => {
            if !id.bytes().all(|b| b.is_ascii_digit()) {
                return Err("ID Lutris inválido.".into());
            }
            ("lutris", vec![format!("lutris:rungameid/{id}")])
        }
        "Faugus" => ("faugus-launcher", vec!["--game".into(), id.into()]),
        "Bottles" => {
            let mut args = vec![
                "run".into(),
                "-b".into(),
                game.external_store
                    .clone()
                    .ok_or("Nome da bottle ausente.")?,
                "-p".into(),
                game.name.clone(),
            ];
            if !extra.is_empty() {
                args.extend([
                    "-a".into(),
                    extra
                        .iter()
                        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
                        .collect::<Vec<_>>()
                        .join(" "),
                ]);
            }
            ("bottles-cli", args)
        }
        _ => return Err("Launcher externo não suportado.".into()),
    };
    if !game.flatpak {
        if program == "heroic"
            && !std::env::var_os("PATH").is_some_and(|path| {
                std::env::split_paths(&path).any(|folder| folder.join("heroic").is_file())
            })
        {
            if let Some(command) =
                heroic_desktop_command(args.last().map(String::as_str).unwrap_or_default())
            {
                return Ok(command);
            }
        }
        return Ok((program.into(), args));
    }
    let app = match game.launcher.as_str() {
        "Steam" => "com.valvesoftware.Steam",
        "Heroic" => "com.heroicgameslauncher.hgl",
        "Lutris" => "net.lutris.Lutris",
        "Faugus" => "io.github.Faugus.faugus-launcher",
        "Bottles" => "com.usebottles.bottles",
        _ => unreachable!(),
    };
    let mut flatpak = vec!["run".into()];
    // Per-invocation access for a selected host Proton; no persistent Flatpak override.
    if let Some(proton) = read_settings()?.get(&game.external_id).filter(|_| !game.native_linux).and_then(|s| s.proton.clone()) {
        flatpak.push(format!("--filesystem={}:ro", crate::core::game_installation::validate_proton(&proton)?.display()));
        if game.launcher == "Lutris" {
            let umu = crate::core::game_installation::find_umu()?;
            if let Some(parent) = umu.parent() { flatpak.push(format!("--filesystem={}:ro",parent.display())); }
        }
    }
    if game.launcher == "Bottles" {
        flatpak.push("--command=bottles-cli".into());
    }
    // Flatpak does not forward arbitrary host environment variables.
    let mut env_command = launch_command(Path::new(program), Some(&game.advanced))?;
    for (key, value) in custom.get_envs() {
        if let Some(value) = value {
            env_command.env(key, value);
        }
    }
    crate::core::mangohud_config::apply_with_preset(&mut env_command, game.advanced.options.get("mangohud") == Some(&true) || game.advanced.options.get("mangohud_env") == Some(&true), game.advanced.mangohud_fps_limit, game.advanced.mangohud_enabled, game.advanced.mangohud_preset.as_ref())?;
    for (key, value) in env_command.get_envs() {
        if let Some(value) = value {
            flatpak.push(format!(
                "--env={}={}",
                key.to_string_lossy(),
                value.to_string_lossy()
            ));
        }
    }
    if game.advanced.mangohud_fps_limit.unwrap_or(0) > 0 || crate::core::mangohud_config::global_enabled()? || game.advanced.options.get("mangohud") == Some(&true) || game.advanced.options.get("mangohud_env") == Some(&true) {
        flatpak.push("--env=MANGOHUD=1".into());
        flatpak.push(format!("--filesystem={}", crate::core::mangohud_config::directory()?.display()));
    }
    flatpak.push(app.into());
    flatpak.extend(args);
    Ok(("flatpak".into(), flatpak))
}
// Heroic AppImages often register a desktop handler without a PATH executable.
fn heroic_desktop_command(url: &str) -> Option<(String, Vec<String>)> {
    let home = dirs::home_dir()?;
    for folder in [
        home.join(".local/share/applications"),
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ] {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten().take(5000) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
                continue;
            }
            if !std::fs::metadata(&path)
                .ok()
                .is_some_and(|m| m.len() < 65536)
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            if !text.lines().any(|line| {
                line.strip_prefix("MimeType=")
                    .is_some_and(|v| v.split(';').any(|mime| mime == "x-scheme-handler/heroic"))
            }) {
                continue;
            }
            let Some(exec) = text.lines().find_map(|line| line.strip_prefix("Exec=")) else {
                continue;
            };
            let Ok(parsed) = crate::core::advanced_settings::wrapper_args(exec) else {
                continue;
            };
            let mut tokens = parsed.into_iter();
            let Some(program) = tokens.next() else {
                continue;
            };
            let mut args = Vec::new();
            let mut placed = false;
            for token in tokens {
                if token == "%u" || token == "%U" {
                    args.push(url.into());
                    placed = true;
                } else if !token.contains('%') {
                    args.push(token);
                }
            }
            if !placed {
                args.push(url.into());
            }
            return Some((program, args));
        }
    }
    None
}
fn has_local_adjustments(settings: &AdvancedSettings) -> bool {
    if settings.overlay_enabled == Some(true) || settings.background_fps.is_some() || settings.mangohud_enabled == Some(true)
        || settings.options.get("mangohud") == Some(&true)
        || settings.options.get("mangohud_env") == Some(&true)
    {
        return true;
    }
    if settings.mangohud_fps_limit.is_some_and(|limit| limit > 0) {
        return true;
    }
    if settings.launch_options.as_deref().is_some_and(|s| !s.trim().is_empty()) {
        return true;
    }
    if settings.options.iter().any(|(id, &enabled)| {
        if !enabled {
            return false;
        }
        if id == "dll_overrides" {
            return !settings.dll_overrides.trim().is_empty();
        }
        if id == "mangohud" || id == "mangohud_env" {
            return false;
        }
        true
    }) {
        return true;
    }
    if settings.launch_backend == "gamescope" && settings.gamescope.enabled() {
        return true;
    }
    if settings.wrappers.iter().any(|w| w.enabled && !w.program.trim().is_empty()) {
        return true;
    }
    false
}
pub(crate) fn launcher_open(launcher: &str) -> bool { launcher_needs_environment(launcher, None) }
fn monitoring_environment_matches(environment: &[u8], config: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let mut expected=b"MANGOHUD_CONFIGFILE=".to_vec();expected.extend_from_slice(config.as_os_str().as_bytes());
    environment.split(|b|*b==0).any(|v|v==b"MANGOHUD=1") && environment.split(|b|*b==0).any(|v|v==expected)
}
fn launcher_needs_environment(launcher: &str, config: Option<&Path>) -> bool {
    let expected: &[&str] = match launcher {
        "Steam" => &["steam"],
        "Heroic" => &["heroic", "heroicgameslauncher"],
        "Lutris" => &["lutris"],
        "Bottles" => &["bottles"],
        "Faugus" => &["faugus-launcher", "faugus-run"],
        "Epic Games" => &["epicgameslauncher", "epic-games-launcher"],
        _ => &[],
    };
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
        })
        .any(|entry| {
            let executable = std::fs::read_link(entry.path().join("exe"))
                .ok()
                .and_then(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().to_ascii_lowercase())
                });
            let command = std::fs::read(entry.path().join("cmdline")).unwrap_or_default();
            let matches_command = command
                .split(|b| *b == 0)
                .take(3)
                .filter_map(|s| std::str::from_utf8(s).ok())
                .any(|arg| {
                    Path::new(arg).file_name().is_some_and(|n| {
                        expected.contains(&n.to_string_lossy().to_ascii_lowercase().as_str())
                    })
                });
            let matches_process = executable.as_deref().is_some_and(|name|expected.contains(&name)) || matches_command;
            if matches_process && config.is_some_and(|path| monitoring_environment_matches(&std::fs::read(entry.path().join("environ")).unwrap_or_default(),path)) { return false; }
            matches_process
        })
}
/// Migrate older per-directory mod records into the persistent runtime profile.
fn steam_mod_options(game: &mut ExternalGame) -> Result<(), String> {
    if game.app_id.is_none() { return Ok(()); }
    let mut taken = super::mod_launch_options::take_records_under(&game.path)?;
    if game.directory != game.path { taken.extend(super::mod_launch_options::take_records_under(&game.directory)?); }
    if let Some(line) = taken.last() {
        let mut cleaned = game.advanced.launch_options.clone();
        for previous in &taken { cleaned = super::mod_launch_options::merge(cleaned.as_deref(), Some(previous), None)?; }
        let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?;
        let mut saved = read_settings()?;
        if let Some(entry) = saved.get_mut(&game.external_id) {
            entry.advanced.launch_options = cleaned.clone();
            let file = settings_file()?;
            let temporary = file.with_extension("json.part");
            std::fs::write(&temporary, serde_json::to_vec_pretty(&saved).map_err(|_| "Configurações inválidas.")?).map_err(|_| "Não foi possível gravar configurações.")?;
            std::fs::rename(temporary, file).map_err(|_| "Não foi possível salvar configurações.")?;
        }
        game.advanced.launch_options = cleaned;
        let previous = saved.get(&game.external_id).cloned().unwrap_or_else(|| SavedExternalSettings { advanced: game.advanced.clone(), proton: game.proton.clone(), directory: None });
        steam_settings::stage_with_mods(game, Some(previous.clone()), previous, Some(line.clone()))?;
    } else {
        // A converted game no longer needs a VDF write when its mod changes.
        // Retain the old path only for games not converted to a runtime profile.
        if !super::steam_runtime::has_profile(game)? {
            let _ = super::steam_launch_options::waiting(game.app_id.as_deref().unwrap_or_default());
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn launch_external_game(external_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut game=all()?.into_iter().find(|game|game.external_id==external_id).ok_or("Jogo não encontrado no launcher de origem. Atualize a biblioteca.")?;
        if game.launcher == "Steam" { steam_mod_options(&mut game)?; let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?; steam_settings::before_launch(&game)?; }
        let selected_proton = read_settings()?.get(&game.external_id).filter(|_| !game.native_linux).and_then(|settings| settings.proton.clone());
        let global_monitoring = crate::core::mangohud_config::global_enabled()? && game.advanced.mangohud_enabled != Some(false);
        let global_config = crate::core::mangohud_config::directory()?.join("MangoHud.conf");
        if !matches!(game.launcher.as_str(), "Steam" | "Epic Games") && (((has_local_adjustments(&game.advanced) || selected_proton.is_some()) && launcher_open(&game.launcher))
            || (global_monitoring && launcher_needs_environment(&game.launcher, Some(&global_config)))) {
            return Err(format!("Feche {} antes de iniciar por aqui para aplicar os ajustes avançados do PeliGames. Um launcher que já está aberto não recebe o novo ambiente de execução.", game.launcher));
        }
        if game.launcher == "Epic Games" && !game.native_linux {
            let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?;
            let proton = selected_proton.as_deref().map(crate::core::game_installation::validate_proton).transpose()?;
            epic_runtime::configure(&dirs::home_dir().ok_or("Pasta pessoal indisponível.")?, &game, proton.as_deref())?;
        } else if let Some(proton) = selected_proton.filter(|_| game.launcher != "Steam") {
            let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?;
            let proton = crate::core::game_installation::validate_proton(&proton)?;
            external_proton::apply(&game, &proton)?;
        }
        let mut source_game = game.clone();
        if matches!(game.launcher.as_str(), "Steam" | "Epic Games") { source_game.advanced = AdvancedSettings::default(); }
        let (program,args)=invocation(&source_game)?;
        let mut command=if matches!(game.launcher.as_str(), "Steam" | "Epic Games") { Command::new(&program) } else { launch_command(Path::new(&program),Some(&game.advanced))? };
        crate::core::env_utils::host_environment(&mut command);
        command.args(args).env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if !matches!(game.launcher.as_str(), "Steam" | "Epic Games") {
        for (key,value) in custom_options(&game.advanced)?.get_envs() {if let Some(value)=value {command.env(key,value);} }
        crate::core::mangohud_config::apply_with_preset(&mut command, game.advanced.options.get("mangohud") == Some(&true) || game.advanced.options.get("mangohud_env") == Some(&true), game.advanced.mangohud_fps_limit, game.advanced.mangohud_enabled, game.advanced.mangohud_preset.as_ref())?;
        }
        #[cfg(target_os = "linux")]
        if game.launcher == "Steam" {
            let id = game.app_id.clone().ok_or("Steam AppID indisponível.")?;
            return crate::core::game_execution::steam::start_with(game.path.clone(), id, || command.spawn().map_err(|e| format!("Não foi possível abrir Steam: {e}"))).map(|_| ());
        }
        // With its install folder known, the game is followed like a Steam one:
        // the button shows it opening, running and closing.
        #[cfg(target_os = "linux")]
        if Path::new(&game.path).is_dir() {
            let launcher = game.launcher.clone();
            return crate::core::game_execution::steam::start_external(game.path.clone(), &game.launcher, || command.spawn().map_err(|_| format!("Não foi possível abrir {launcher}. Verifique se está instalado e se os wrappers estão disponíveis."))).map(|_| ());
        }
        let mut child=command.spawn().map_err(|_|format!("Não foi possível abrir {}. Verifique se está instalado e se os wrappers estão disponíveis.",game.launcher))?;
        for _ in 0..5 {
            if let Some(status)=child.try_wait().map_err(|_|"Não foi possível verificar a solicitação ao launcher.")? {
                return if status.success() {Ok(())} else {Err(format!("{} recusou a execução. Abra o launcher de origem e confira o jogo e os ajustes avançados.",game.launcher))};
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        std::thread::spawn(move || {let _=child.wait();});
        Ok(())
    }).await.map_err(|_|"Não foi possível solicitar a execução externa.")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "explicit local Steam smoke test; prepares only the selected game's pelirun profile"]
    fn installed_steam_pelirun_preparation() {
        let snapshot =
            std::fs::read(std::env::var("PELI_STEAM_SMOKE_SNAPSHOT").unwrap()).unwrap();
        let snapshot: serde_json::Value = serde_json::from_slice(&snapshot).unwrap();
        let app = snapshot["app_id"].as_str().unwrap();
        let mut game = all()
            .unwrap()
            .into_iter()
            .find(|game| {
                game.launcher == "Steam"
                    && !game.flatpak
                    && game.app_id.as_deref() == Some(app)
            })
            .expect("The explicitly selected Steam game must be installed");
        // Change only this generated test profile, never the saved preferences.
        if let Some(enabled) = snapshot["mangohud_enabled"].as_bool() {
            game.advanced.mangohud_enabled = Some(enabled);
        }
        if let Some(phase) = snapshot["phase"].as_str() {
            assert!(["first", "second"].contains(&phase));
            let mut options = super::super::nexus_launch_options::parse(
                game.advanced.launch_options.as_deref().unwrap_or(""),
            )
            .unwrap();
            options.env.insert("PELIRUN_SMOKE_TEST".into(), phase.into());
            game.advanced.launch_options =
                Some(super::super::nexus_launch_options::describe(&options));
        }
        let config = Path::new(snapshot["localconfig"].as_str().unwrap());
        let before = std::fs::read(config).unwrap();
        let exported = steam_settings::manual(&game).unwrap();
        assert_eq!(std::fs::read(config).unwrap(), before);
        assert!(super::super::steam_runtime::has_profile(&game).unwrap());
        println!("{} ({app}): {}", game.name, exported.line);
    }
    #[test]
    #[ignore = "reads the current user's installed launchers without launching or changing games"]
    fn native_linux_local_inventory() {
        let games = all().unwrap();
        let native: Vec<_> = games.iter().filter(|game| game.native_linux).collect();
        println!("Native Linux games: {}", native.len());
        for game in native {
            println!("{}: {}", game.launcher, game.name);
            assert!(game.proton.is_none(), "Native games must not inherit a saved Proton override");
        }
    }
    pub(super) fn game(launcher: &str, id: &str) -> ExternalGame {
        ExternalGame {
            steam_pending: false, steam_proton_name: None,
            native_linux: false, name: "Game with spaces".into(),
            path: "/games/test".into(),
            directory: "/games/test".into(),
            launcher: launcher.into(),
            app_id: Some(id.into()),
            cover_url: None,
            prefix: None,
            proton: None,
            executable: None,
            external_id: identity(launcher, id, false),
            external_store: None,
            flatpak: false,
            advanced: AdvancedSettings::default(),
        }
    }
    #[test]
    fn local_proton_overrides_preserve_legacy_advanced_settings() {
        let mut advanced = AdvancedSettings::default();
        advanced.launch_options=Some("MANGOHUD=1 %command%".into());
        let legacy=serde_json::to_value(&advanced).unwrap();
        let mut saved:SavedExternalSettings=serde_json::from_value(legacy).unwrap();
        assert!(saved.proton.is_none());assert_eq!(saved.advanced.launch_options,advanced.launch_options);
        saved.proton=Some("/selected/proton".into());
        let reread:SavedExternalSettings=serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        assert_eq!(reread.proton,saved.proton);assert_eq!(reread.advanced.launch_options,advanced.launch_options);
    }
    #[test]
    fn local_adjustments_semantic_check() {
        let mut advanced = AdvancedSettings::default();
        assert!(!has_local_adjustments(&advanced));

        advanced.overlay_enabled=Some(true);assert!(has_local_adjustments(&advanced));advanced.overlay_enabled=Some(false);assert!(!has_local_adjustments(&advanced));
        advanced.mangohud_enabled = Some(true);
        assert!(has_local_adjustments(&advanced));

        advanced.mangohud_enabled = Some(false);
        assert!(!has_local_adjustments(&advanced));

        advanced.mangohud_fps_limit = Some(60);
        assert!(has_local_adjustments(&advanced));
        advanced.mangohud_fps_limit = Some(0);
        assert!(!has_local_adjustments(&advanced));
        advanced.mangohud_fps_limit = None;
        assert!(!has_local_adjustments(&advanced));

        advanced.options.insert("fsr4".into(), true);
        assert!(has_local_adjustments(&advanced));
        advanced.options.insert("fsr4".into(), false);
        assert!(!has_local_adjustments(&advanced));

        advanced.launch_options = Some("PROTON_ENABLE_NVAPI=1 %command%".into());
        assert!(has_local_adjustments(&advanced));
        advanced.launch_options = Some("   ".into());
        assert!(!has_local_adjustments(&advanced));
        advanced.launch_options = None;
        assert!(!has_local_adjustments(&advanced));
    }
    #[test]
    fn launchers_receive_their_own_identifiers_without_shell_execution() {
        let steam = game("Steam", "22380");
        assert_eq!(
            invocation(&steam).unwrap(),
            ("steam".into(), vec!["steam://launch/22380/dialog".into()])
        );
        assert!(invocation(&game("Steam", "12;touch /tmp/test")).is_err());
        let mut heroic = game("Heroic", "app/id & spaces");
        heroic.external_store = Some("legendary".into());
        heroic.advanced.launch_options =
            Some("MANGOHUD=1 %command% '-argument with spaces'".into());
        let (program, args) = invocation(&heroic).unwrap();
        assert_eq!(program, "heroic");
        let url = reqwest::Url::parse(&args[1]).unwrap();
        let params: BTreeMap<_, _> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        assert_eq!(params["appName"], "app/id & spaces");
        assert_eq!(params["runner"], "legendary");
        assert_eq!(
            urlencoding::decode(&params["arg"]).unwrap(),
            "-argument with spaces"
        );
        assert_eq!(
            invocation(&game("Lutris", "42")).unwrap().1,
            vec!["lutris:rungameid/42"]
        );
        assert_eq!(
            invocation(&game("Faugus", "test-game")).unwrap().1,
            vec!["--game", "test-game"]
        );
        let mut bottles = game("Bottles", "internal-id");
        bottles.external_store = Some("Gaming bottle".into());
        assert_eq!(
            invocation(&bottles).unwrap().1,
            vec!["run", "-b", "Gaming bottle", "-p", "Game with spaces"]
        );
    }
    #[test]
    fn flatpak_receives_advanced_environment_explicitly() {
        let mut heroic = game("Heroic", "epic-id");
        heroic.external_store = Some("legendary".into());
        heroic.flatpak = true;
        heroic.advanced.launch_options = Some("MANGOHUD=1 %command%".into());
        let (program, args) = invocation(&heroic).unwrap();
        assert_eq!(program, "flatpak");
        assert!(args.contains(&"--env=MANGOHUD=1".into()));
        assert!(args.contains(&"com.heroicgameslauncher.hgl".into()));
        assert!(args.last().unwrap().starts_with("heroic://launch"));
    }
    #[test]
    fn installed_games_keep_store_paths_and_duplicate_titles() {
        let home =
            std::env::temp_dir().join(format!("peligames-external-{}", uuid::Uuid::new_v4()));
        let steam = home.join(".local/share/Steam/steamapps");
        std::fs::create_dir_all(steam.join("common/SniperGame")).unwrap();
        std::fs::write(
            steam.join("appmanifest_1.acf"),
            "\"appid\" \"1\"\n\"name\" \"Sniper Legacy\"\n\"installdir\" \"SniperGame\"",
        )
        .unwrap();
        let heroic = home.join(".config/heroic");
        std::fs::create_dir_all(heroic.join("gog_store")).unwrap();
        std::fs::create_dir_all(home.join("Games/gog")).unwrap();
        std::fs::write(heroic.join("gog_store/installed.json"),format!(r#"{{"installed":[{{"appName":"2","title":"Sniper Legacy","install_path":"{}"}}]}}"#,home.join("Games/gog").display())).unwrap();
        let games = discover(&home);
        assert_eq!(
            games.len(),
            2,
            "Games with the same title in two stores must remain separate"
        );
        assert!(games
            .iter()
            .any(|g| g.launcher == "Steam" && g.path.ends_with("common/SniperGame")));
        assert!(games
            .iter()
            .any(|g| g.launcher == "Heroic" && g.app_id.as_deref() == Some("2")));
        assert_eq!(
            games.iter().filter(|g| g.name == "Sniper Legacy").count(),
            2
        );
        std::fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn lutris_config_is_resolved_to_database_id_read_only() {
        let home = std::env::temp_dir().join(format!("peligames-lutris-{}", uuid::Uuid::new_v4()));
        let data = home.join(".local/share/lutris");
        std::fs::create_dir_all(data.join("games")).unwrap();
        std::fs::create_dir_all(home.join("Games/lutris")).unwrap();
        std::fs::write(
            data.join("games/test-game-123.yml"),
            format!(
                "game:\n  exe: game.exe\n  working_dir: {}\n",
                home.join("Games/lutris").display()
            ),
        )
        .unwrap();
        let db = rusqlite::Connection::open(data.join("pga.db")).unwrap();
        db.execute_batch("CREATE TABLE games(id,name,configpath,installed); INSERT INTO games VALUES(42,'Exact game name','test-game-123',1); INSERT INTO games VALUES(43,'Not installed','other-456',0);").unwrap();
        drop(db);
        let before = std::fs::read(data.join("pga.db")).unwrap();
        let games = discover(&home);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].app_id.as_deref(), Some("42"));
        assert_eq!(games[0].name, "Exact game name");
        assert_eq!(before, std::fs::read(data.join("pga.db")).unwrap());
        std::fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(test)]
mod global_hud_environment_tests {
 use super::*;
 #[test] fn recognizes_only_the_launcher_owned_hud_environment(){
   let path=Path::new("/home/test/PeliGames/MangoHUD/MangoHud.conf");
   assert!(monitoring_environment_matches(b"MANGOHUD=1\0MANGOHUD_CONFIGFILE=/home/test/PeliGames/MangoHUD/MangoHud.conf\0",path));
   assert!(!monitoring_environment_matches(b"MANGOHUD=1\0MANGOHUD_CONFIGFILE=/other.conf\0",path));
   assert!(!monitoring_environment_matches(b"MANGOHUD=0\0MANGOHUD_CONFIGFILE=/home/test/PeliGames/MangoHUD/MangoHud.conf\0",path));
 }
}

pub(crate) fn maintenance_entry(id: &str) -> Result<(String, String, String, String), String> {
    let game = all()?.into_iter().find(|game|game.external_id == id).ok_or("Jogo não encontrado no launcher de origem.")?;
    Ok((game.name, game.prefix.unwrap_or_default(), game.proton.unwrap_or_default(), game.executable.unwrap_or(game.path)))
}

#[tauri::command]
pub async fn steam_game_settings_action(app: tauri::AppHandle, external_id: String, action: String) -> Result<ExternalGame, String> {
    use tauri::Emitter;
    let handle = app.clone();
    let operation = format!("Steam · {action}");
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?;
        let game = all()?.into_iter().find(|g| g.external_id == external_id && g.launcher == "Steam").ok_or("Jogo Steam não encontrado.")?;
        match action.as_str() {
            "status" => return Ok(game),
            "revert" => steam_settings::revert(&game)?,
            "confirm-manual" => steam_settings::confirm_manual(&game)?,
            "restart" => steam_settings::restart(&game, |phase| {
                crate::core::logger::log_launcher(&handle,"INFO",&format!("Reinício Steam: instalação={}, jogo={}, etapa={phase}",if game.flatpak {"flatpak"} else {"nativa"},game.name));
                let _=handle.emit("steam-settings-progress",serde_json::json!({"external_id":game.external_id,"phase":phase}));
            })?,
            _ => return Err("Ação Steam inválida.".into()),
        }
        all()?.into_iter().find(|g| g.external_id == external_id).ok_or("Jogo não encontrado.".into())
    }).await.map_err(|_| "Falha ao gerenciar as configurações da Steam.")?;
    crate::core::logger::log_result(&app,&operation,&result);
    result
}

#[tauri::command]
pub async fn steam_game_launch_preview(external_id: String) -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?;
        let game = all()?.into_iter().find(|g| g.external_id == external_id && g.launcher == "Steam").ok_or("Jogo Steam não encontrado.")?;
        serde_json::to_value(steam_settings::manual(&game)?).map_err(|e| e.to_string())
    }).await.map_err(|_| "Não foi possível gerar as opções da Steam.")?
}

#[derive(Serialize)]
pub struct SteamProtonStatus { proton: Option<String>, steam_proton_name: Option<String>, steam_pending: bool }
/// Targeted refresh for the open game. Never scans the installed game library.
#[tauri::command]
pub async fn steam_game_proton_status(external_id: String, prefix: Option<String>) -> Result<SteamProtonStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?;
        let parts: Vec<_> = external_id.split(':').collect();
        if parts.len() != 3 || parts[0] != "Steam" || !["native", "flatpak"].contains(&parts[1]) || parts[2].is_empty() || !parts[2].bytes().all(|b| b.is_ascii_digit()) { return Err("Identificação Steam inválida.".into()); }
        let pending = steam_settings::pending(&external_id)?;
        if pending {
            let proton = read_settings()?.get(&external_id).and_then(|s|s.proton.clone());
            return Ok(SteamProtonStatus { proton, steam_proton_name: None, steam_pending: true });
        }
        let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
        let selection = steam_proton_state::selected(&home, parts[1] == "flatpak", parts[2]);
        let (proton, name) = match selection {
            Some(value) => (value.proton, Some(value.name)),
            None => (prefix.as_deref().filter(|p| Path::new(p).is_absolute()).and_then(|p|crate::core::game_installation::proton_from_prefix(Path::new(p))), None),
        };
        Ok(SteamProtonStatus { proton, steam_proton_name: name, steam_pending: false })
    }).await.map_err(|_| "Não foi possível atualizar o Proton da Steam.")?
}
