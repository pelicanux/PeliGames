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
#[derive(Clone, Serialize)]
pub struct ExternalGame {
    pub name: String,
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
}
#[derive(Clone, Serialize, Deserialize)]
struct SavedExternalSettings {
    #[serde(flatten)]
    advanced: AdvancedSettings,
    #[serde(default)]
    proton: Option<String>,
}
fn settings_file() -> Result<PathBuf, String> {
    Ok(crate::core::paths::app_root()?.join("external-game-settings.json"))
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
    for steam in super::scanner::scan_steam_main(home) {
        let Some(id) = steam.app_id.clone() else {
            continue;
        };
        let flatpak = Path::new(&steam.path)
            .starts_with(home.join(".var/app/com.valvesoftware.Steam"))
            || (!home.join(".local/share/Steam/steamapps").is_dir()
                && !home.join(".steam/steam/steamapps").is_dir()
                && home.join(".var/app/com.valvesoftware.Steam").is_dir());
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
            name: steam.name,
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
fn all() -> Result<Vec<ExternalGame>, String> {
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let preferences = read_settings()?;
    let mut games = discover(&home);
    for game in &mut games {
        if let Some(settings) = preferences.get(&game.external_id) {
            game.advanced = settings.advanced.clone();
            if let Some(proton) = &settings.proton { game.proton = Some(proton.clone()); }
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
                Some(path) => super::covers::cache_heroic_cover(path)
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
        let mut saved = read_settings()?;
        let previous_proton = saved.get(&settings.external_id).and_then(|saved| saved.proton.clone());
        let proton = match settings.proton.as_deref() {
            Some(value) => {
                if game.launcher == "Bottles" {return Err("O Bottles compartilha a versão entre os programas da mesma bottle. Altere o runner no Bottles.".into());}
                Some(crate::core::game_installation::validate_proton(value)?.to_string_lossy().into_owned())
            },
            None => previous_proton,
        };
        saved.insert(settings.external_id, SavedExternalSettings {advanced: settings.advanced.clone(), proton: proton.clone()});
        let file = settings_file()?;
        let temporary = file.with_extension("json.part");
        std::fs::write(
            &temporary,
            serde_json::to_vec_pretty(&saved).map_err(|_| "Configurações inválidas.")?,
        )
        .map_err(|_| "Não foi possível gravar configurações.")?;
        std::fs::rename(temporary, file).map_err(|_| "Não foi possível salvar configurações.")?;
        game.advanced = settings.advanced;
        if let Some(proton) = proton { game.proton = Some(proton); }
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
fn invocation(game: &ExternalGame) -> Result<(String, Vec<String>), String> {
    let id = game
        .app_id
        .as_deref()
        .ok_or("Identificação externa ausente.")?;
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
    if let Some(proton) = read_settings()?.get(&game.external_id).and_then(|s| s.proton.clone()) {
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
    crate::core::mangohud_config::apply(&mut env_command, game.advanced.options.get("mangohud") == Some(&true) || game.advanced.options.get("mangohud_env") == Some(&true), game.advanced.mangohud_fps_limit)?;
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
    serde_json::to_value(settings).ok() != serde_json::to_value(AdvancedSettings::default()).ok()
}
fn launcher_open(launcher: &str) -> bool { launcher_needs_environment(launcher, None) }
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
#[tauri::command]
pub async fn launch_external_game(external_id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let game=all()?.into_iter().find(|game|game.external_id==external_id).ok_or("Jogo não encontrado no launcher de origem. Atualize a biblioteca.")?;
        let selected_proton = read_settings()?.get(&game.external_id).and_then(|settings| settings.proton.clone());
        let global_monitoring=crate::core::mangohud_config::global_enabled()?;
        let global_config=crate::core::mangohud_config::directory()?.join("MangoHud.conf");
        if ((has_local_adjustments(&game.advanced) || selected_proton.is_some()) && launcher_open(&game.launcher))
            || (global_monitoring && launcher_needs_environment(&game.launcher,Some(&global_config))) {
            return Err(format!("Feche {} antes de iniciar por aqui para aplicar os ajustes avançados do PeliGames. Um launcher que já está aberto não recebe o novo ambiente de execução.",game.launcher));
        }
        if let Some(proton) = selected_proton {
            let _guard = SETTINGS_LOCK.lock().map_err(|_| "Configurações ocupadas.")?;
            let proton = crate::core::game_installation::validate_proton(&proton)?;
            external_proton::apply(&game, &proton)?;
        }
        let (program,args)=invocation(&game)?;
        let mut command=launch_command(Path::new(&program),Some(&game.advanced))?;
        command.args(args).env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        for (key,value) in custom_options(&game.advanced)?.get_envs() {if let Some(value)=value {command.env(key,value);} }
        crate::core::mangohud_config::apply(&mut command, game.advanced.options.get("mangohud") == Some(&true) || game.advanced.options.get("mangohud_env") == Some(&true), game.advanced.mangohud_fps_limit)?;
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
    pub(super) fn game(launcher: &str, id: &str) -> ExternalGame {
        ExternalGame {
            name: "Game with spaces".into(),
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
