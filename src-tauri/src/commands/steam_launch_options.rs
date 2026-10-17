//! Launch options a mod needs, for Steam games, written into Steam's own
//! per-game launch options (`userdata/<user>/config/localconfig.vdf`). Steam
//! then applies them however the game is opened, from PeliGames or from the
//! Steam client, and an open Steam no longer blocks the launch.
//!
//! Steam keeps that file in memory and rewrites it when it closes, so it is
//! only edited with Steam closed. Until then the change waits in PeliGames'
//! own record and is written on the next chance (before a launch, or when the
//! mod is installed again).
use super::mod_launch_options::merge;
use super::nexus_launch_options::{parse, Options};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// What the mods want in a game's Steam launch options, and what is already there.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct Entry {
    /// The mod line last written into Steam.
    applied: Option<String>,
    /// The mod line Steam should have.
    wanted: Option<String>,
}

fn record_file() -> PathBuf {
    crate::core::config_manager::get_mod_config_path().with_file_name("steam-launch-options.json")
}
fn read_records() -> Result<BTreeMap<String, Entry>, String> {
    match fs::read(record_file()) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Registro das opções da Steam inválido.".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(format!("Não foi possível ler as opções da Steam: {error}")),
    }
}
fn write_records(records: &BTreeMap<String, Entry>) -> Result<(), String> {
    let path = record_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temporary = path.with_extension("json.part");
    fs::write(&temporary, serde_json::to_vec_pretty(records).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&temporary, &path).map_err(|e| e.to_string())
}
fn valid_app(app_id: &str) -> Result<(), String> {
    if app_id.is_empty() || app_id.len() > 10 || !app_id.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Steam AppID inválido.".into());
    }
    Ok(())
}

/// Splits Steam launch options into the variables set at the very start and
/// the rest, kept as typed: wrappers (`gamemoderun`, `mangohud`), `%command%`
/// and the game's arguments. Without `%command%` Steam passes everything to
/// the game, so the rest then becomes `%command% <everything>`.
fn split(current: &str) -> (Vec<&str>, String) {
    let current = current.trim();
    if !current.contains("%command%") {
        return (Vec::new(), if current.is_empty() { "%command%".into() } else { format!("%command% {current}") });
    }
    let bytes = current.as_bytes();
    let mut variables = Vec::new();
    let mut index = 0;
    loop {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let start = index;
        let mut quote = None;
        while index < bytes.len() {
            match (bytes[index], quote) {
                (b'\\', _) => index += 1,
                (b'"' | b'\'', None) => quote = Some(bytes[index]),
                (byte, Some(open)) if byte == open => quote = None,
                (byte, None) if byte.is_ascii_whitespace() => break,
                _ => {}
            }
            index += 1;
        }
        let token = &current[start..index.min(current.len())];
        let variable = token.split_once('=').is_some_and(|(key, _)| {
            !key.is_empty() && key.bytes().enumerate().all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit())
        });
        if token.is_empty() || !variable {
            return (variables, current[start..].to_string());
        }
        variables.push(token);
    }
}
/// `current` (Steam's launch options) without the mod's `previous` line and
/// with `next`; only the leading variables change.
fn merged(current: &str, previous: Option<&str>, next: Option<&str>) -> Result<String, String> {
    let (variables, rest) = split(current);
    let start = (!variables.is_empty()).then(|| variables.join(" "));
    let variables = merge(start.as_deref(), previous, next)?.map(|value| parse(&value)).transpose()?.unwrap_or_else(Options::default);
    let text = super::nexus_launch_options::describe(&Options { env: variables.env, args: Vec::new() });
    let joined = format!("{text} {rest}");
    let joined = joined.trim();
    Ok(if joined == "%command%" { String::new() } else { joined.to_string() })
}

// --- localconfig.vdf: a minimal KeyValues reader that keeps the file as it is.

#[derive(Debug)]
enum Token {
    /// A quoted string: its text (unescaped) and its byte range, quotes included.
    Text(String, std::ops::Range<usize>),
    Open(usize),
    Close(usize),
}
fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let bytes = text.as_bytes();
    let mut list = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                let start = index;
                let mut value = Vec::new();
                index += 1;
                loop {
                    let Some(&byte) = bytes.get(index) else { return Err("localconfig.vdf incompleto.".into()) };
                    match byte {
                        b'\\' if index + 1 < bytes.len() => {
                            value.push(bytes[index + 1]);
                            index += 2;
                        }
                        b'"' => break,
                        _ => {
                            value.push(byte);
                            index += 1;
                        }
                    }
                }
                index += 1;
                list.push(Token::Text(String::from_utf8_lossy(&value).into_owned(), start..index));
            }
            b'{' => {
                list.push(Token::Open(index));
                index += 1;
            }
            b'}' => {
                list.push(Token::Close(index));
                index += 1;
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }
    Ok(list)
}
/// One `"key" { ... }` block: where its braces are and its children's token range.
struct Block {
    open: usize,
    close: usize,
    first: usize,
    end: usize,
}
/// Finds the child block `key` (case-insensitive) among the tokens `from..to`.
fn child(list: &[Token], from: usize, to: usize, key: &str) -> Option<Block> {
    let mut index = from;
    while index < to {
        let Token::Text(name, _) = &list[index] else { index += 1; continue };
        match list.get(index + 1) {
            Some(Token::Open(open)) => {
                let mut depth = 0;
                let mut end = index + 1;
                for (position, token) in list.iter().enumerate().skip(index + 1) {
                    match token {
                        Token::Open(_) => depth += 1,
                        Token::Close(_) => {
                            depth -= 1;
                            if depth == 0 {
                                end = position;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let Token::Close(close) = list[end] else { return None };
                if name.eq_ignore_ascii_case(key) {
                    return Some(Block { open: *open, close, first: index + 2, end });
                }
                index = end + 1;
            }
            Some(Token::Text(..)) => index += 2,
            _ => index += 1,
        }
    }
    None
}
fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
/// Tabs before the line holding byte `at`.
fn indentation(text: &str, at: usize) -> String {
    let line = text[..at].rfind('\n').map_or(0, |n| n + 1);
    text[line..at].chars().take_while(|c| *c == '\t' || *c == ' ').collect()
}
/// Reads the launch options of `app_id` from a localconfig.vdf text.
fn read_launch_options(text: &str, app_id: &str) -> Result<String, String> {
    let list = tokens(text)?;
    let apps = apps_block(&list)?;
    let Some(app) = child(&list, apps.first, apps.end, app_id) else { return Ok(String::new()) };
    let mut index = app.first;
    while index < app.end {
        if let (Token::Text(key, _), Some(Token::Text(value, _))) = (&list[index], list.get(index + 1)) {
            if key.eq_ignore_ascii_case("LaunchOptions") {
                return Ok(value.clone());
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    Ok(String::new())
}
fn apps_block(list: &[Token]) -> Result<Block, String> {
    let mut range = (0, list.len());
    for key in ["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"] {
        let block = child(list, range.0, range.1, key).ok_or("A Steam ainda não registrou jogos nesta conta.")?;
        range = (block.first, block.end);
        if key == "apps" {
            return Ok(block);
        }
    }
    unreachable!()
}
/// `text` with the launch options of `app_id` set to `value`.
fn write_launch_options(text: &str, app_id: &str, value: &str) -> Result<String, String> {
    let list = tokens(text)?;
    let apps = apps_block(&list)?;
    let quoted = format!("\"{}\"", escape(value));
    let Some(app) = child(&list, apps.first, apps.end, app_id) else {
        // The game has no entry yet: one is added at the end of "apps".
        let inner = indentation(text, apps.close);
        let entry = format!("{inner}\t\"{app_id}\"\n{inner}\t{{\n{inner}\t\t\"LaunchOptions\"\t\t{quoted}\n{inner}\t}}\n");
        let line = text[..apps.close].rfind('\n').map_or(apps.close, |n| n + 1);
        return Ok(format!("{}{entry}{}", &text[..line], &text[line..]));
    };
    let mut index = app.first;
    while index < app.end {
        if let (Token::Text(key, _), Some(Token::Text(_, range))) = (&list[index], list.get(index + 1)) {
            if key.eq_ignore_ascii_case("LaunchOptions") {
                return Ok(format!("{}{quoted}{}", &text[..range.start], &text[range.end..]));
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    let inner = indentation(text, app.close);
    let line = text[..app.close].rfind('\n').map_or(app.close, |n| n + 1);
    let _ = app.open;
    Ok(format!("{}{inner}\t\"LaunchOptions\"\t\t{quoted}\n{}", &text[..line], &text[line..]))
}

/// Every Steam account's localconfig.vdf on this computer (native and Flatpak).
fn local_configs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return Vec::new() };
    let mut seen = Vec::new();
    for root in [".local/share/Steam", ".steam/steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam", ".var/app/com.valvesoftware.Steam/data/Steam"] {
        let Ok(users) = fs::read_dir(home.join(root).join("userdata")) else { continue };
        for user in users.flatten() {
            let file = user.path().join("config/localconfig.vdf");
            let Ok(real) = fs::canonicalize(&file) else { continue };
            if !seen.contains(&real) {
                seen.push(real);
            }
        }
    }
    seen
}
fn write_file(path: &Path, text: &str) -> Result<(), String> {
    // One copy of the file as Steam left it, before PeliGames' first change.
    let backup = path.with_extension("vdf.peligames-backup");
    if !backup.exists() {
        fs::copy(path, &backup).map_err(|e| format!("Não foi possível guardar uma cópia do localconfig.vdf: {e}"))?;
    }
    let temporary = path.with_extension("vdf.peligames-part");
    fs::write(&temporary, text).map_err(|e| format!("Não foi possível gravar as opções da Steam: {e}"))?;
    fs::rename(&temporary, path).map_err(|e| format!("Não foi possível gravar as opções da Steam: {e}"))
}

/// Writes every waiting change into Steam (only with Steam closed). Returns
/// the games still waiting.
fn apply(records: &mut BTreeMap<String, Entry>, steam_open: bool) -> Result<Vec<String>, String> {
    let waiting: Vec<String> = records.iter().filter(|(_, entry)| entry.applied != entry.wanted).map(|(id, _)| id.clone()).collect();
    if waiting.is_empty() || steam_open {
        return Ok(waiting);
    }
    let configs = local_configs();
    if configs.is_empty() {
        return Err("Não encontrei a configuração da Steam (userdata/*/config/localconfig.vdf).".into());
    }
    for path in &configs {
        let mut text = fs::read_to_string(path).map_err(|e| format!("Não foi possível ler {}: {e}", path.display()))?;
        for app in &waiting {
            let entry = &records[app];
            let current = read_launch_options(&text, app)?;
            let next = merged(&current, entry.applied.as_deref(), entry.wanted.as_deref())?;
            if next != current {
                text = write_launch_options(&text, app, &next)?;
            }
        }
        write_file(path, &text)?;
    }
    for app in &waiting {
        let entry = records.get_mut(app).expect("waiting entry");
        entry.applied = entry.wanted.clone();
    }
    records.retain(|_, entry| entry.wanted.is_some());
    Ok(Vec::new())
}

fn steam_open() -> bool {
    super::external_library::launcher_open("Steam")
}

/// Sets the mod line a Steam game should have (`None` takes it out) and writes
/// it now if Steam is closed. Returns whether it is still waiting.
pub(crate) fn want(app_id: &str, line: Option<String>) -> Result<bool, String> {
    valid_app(app_id)?;
    let mut records = read_records()?;
    let entry = records.entry(app_id.to_string()).or_default();
    entry.wanted = line;
    // Kept first: a failed write into Steam leaves the change waiting, not lost.
    write_records(&records)?;
    let waiting = apply(&mut records, steam_open())?;
    write_records(&records)?;
    Ok(waiting.iter().any(|id| id == app_id))
}
/// Before a launch: writes what is waiting if Steam is closed. Returns whether
/// this game's options are still waiting (Steam open).
pub(crate) fn waiting(app_id: &str) -> Result<bool, String> {
    let mut records = read_records()?;
    let still = apply(&mut records, steam_open())?;
    write_records(&records)?;
    Ok(still.iter().any(|id| id == app_id))
}

#[derive(serde::Serialize)]
pub struct SteamModOptions {
    /// The game's PeliGames launch options, without the mod line (now in Steam).
    launch_options: Option<String>,
    /// Steam is open: the line is written when it closes.
    waiting: bool,
}
/// After the mod was installed (`line`) or removed (`None`) in `game_dir` of a
/// Steam game: the line goes to Steam's own launch options, and whatever the
/// mod had put in the PeliGames launch options before is taken out of them.
#[tauri::command]
pub fn apply_steam_mod_launch_options(app_id: String, game_dir: String, current: Option<String>, line: Option<String>) -> Result<SteamModOptions, String> {
    let line = line.map(|value| value.trim().to_string()).filter(|value| !value.is_empty());
    let previous = super::mod_launch_options::take_record(&game_dir)?;
    let launch_options = match &previous {
        Some(previous) => merge(current.as_deref(), Some(previous), None)?,
        None => current,
    };
    let waiting = want(&app_id, line)?;
    Ok(SteamModOptions { launch_options, waiting })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = "\"UserLocalConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"apps\"\n\t\t\t\t{\n\t\t\t\t\t\"203160\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"LastPlayed\"\t\t\"1760000000\"\n\t\t\t\t\t\t\"LaunchOptions\"\t\t\"PROTON_LOG=1 %command% -dx11\"\n\t\t\t\t\t}\n\t\t\t\t\t\"70\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"Playtime\"\t\t\"5\"\n\t\t\t\t\t}\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n";

    #[test]
    fn reads_and_replaces_launch_options_keeping_the_rest() {
        assert_eq!(read_launch_options(CONFIG, "203160").unwrap(), "PROTON_LOG=1 %command% -dx11");
        assert_eq!(read_launch_options(CONFIG, "70").unwrap(), "");
        let line = r#"WINEDLLOVERRIDES="dxgi=n,b" %command%"#;
        let next = merged("PROTON_LOG=1 %command% -dx11", None, Some(line)).unwrap();
        assert_eq!(next, r#"PROTON_LOG="1" WINEDLLOVERRIDES="dxgi=n,b" %command% -dx11"#);
        let written = write_launch_options(CONFIG, "203160", &next).unwrap();
        assert_eq!(read_launch_options(&written, "203160").unwrap(), next);
        assert!(written.contains("\\\"dxgi=n,b\\\""));
        assert!(written.contains("\"LastPlayed\"\t\t\"1760000000\""));
        // Removing the mod gives back what the user had.
        assert_eq!(merged(&next, Some(line), None).unwrap(), r#"PROTON_LOG="1" %command% -dx11"#);
    }

    #[test]
    fn wrappers_and_arguments_stay_as_typed() {
        let line = r#"WINEDLLOVERRIDES="dxgi=n,b" %command%"#;
        let typed = r#"gamemoderun mangohud %command% -dx11 "two words""#;
        let next = merged(typed, None, Some(line)).unwrap();
        assert_eq!(next, format!(r#"WINEDLLOVERRIDES="dxgi=n,b" {typed}"#));
        assert_eq!(merged(&next, Some(line), None).unwrap(), typed);
        // An existing override list gains the mod's DLL and loses only it.
        let overrides = merged(r#"WINEDLLOVERRIDES="winmm=n,b" %command%"#, None, Some(line)).unwrap();
        assert_eq!(overrides, r#"WINEDLLOVERRIDES="winmm=n,b;dxgi=n,b" %command%"#);
        assert_eq!(merged(&overrides, Some(line), None).unwrap(), r#"WINEDLLOVERRIDES="winmm=n,b" %command%"#);
        // Without %command% Steam passes everything to the game: it moves after %command%.
        assert_eq!(merged("-novid -dx11", None, Some(line)).unwrap(), r#"WINEDLLOVERRIDES="dxgi=n,b" %command% -novid -dx11"#);
    }

    #[test]
    fn adds_the_key_or_the_game_when_missing() {
        let line = r#"WINEDLLOVERRIDES="dxgi=n,b" %command%"#;
        let value = merged("", None, Some(line)).unwrap();
        assert_eq!(value, r#"WINEDLLOVERRIDES="dxgi=n,b" %command%"#);
        let in_game = write_launch_options(CONFIG, "70", &value).unwrap();
        assert_eq!(read_launch_options(&in_game, "70").unwrap(), value);
        assert_eq!(read_launch_options(&in_game, "203160").unwrap(), "PROTON_LOG=1 %command% -dx11");
        let new_game = write_launch_options(CONFIG, "1091500", &value).unwrap();
        assert_eq!(read_launch_options(&new_game, "1091500").unwrap(), value);
        assert!(tokens(&new_game).is_ok());
        assert_eq!(merged(&value, Some(line), None).unwrap(), "");
    }

    #[test]
    fn app_ids_are_checked() {
        assert!(valid_app("203160").is_ok() && valid_app("20x").is_err() && valid_app("").is_err());
    }
}
