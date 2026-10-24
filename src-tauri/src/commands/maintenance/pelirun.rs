//! Complete removal of PeliRun, optional on "Reset PeliGames". A plain reset
//! keeps its executable so Steam lines that call it still open the games; a
//! complete removal first takes that call out of each Steam line (the rest of
//! the line stays), then deletes its folders.
use std::{fs, path::{Path, PathBuf}};

/// PeliRun's folders: the launcher-neutral one and the one inside each Steam
/// configuration (with the older per-game scripts beside it).
pub(super) fn folders() -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();
    let data = dirs::data_dir().ok_or("Pasta de dados indisponível.")?;
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let mut candidates = vec![data.join("pelirun")];
    for flatpak in [false, true] {
        for root in super::super::steam_installation::roots(&home, flatpak) {
            let Ok(config) = fs::canonicalize(root.join("config")) else { continue };
            candidates.push(config.join("peligames-launcher"));
            candidates.push(config.join("peligames-game-settings"));
        }
    }
    for path in candidates {
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        // A link is never followed into another folder; it is listed and removed itself.
        if (meta.is_dir() || meta.file_type().is_symlink()) && !found.contains(&path) { found.push(path); }
    }
    Ok(found)
}

fn quote(word: &str) -> String {
    if word == "%command%" { word.into() } else { format!("'{}'", word.replace('\'', "'\"'\"'")) }
}

/// A Steam line without the PeliRun call in front of `%command%`, or `None`
/// when the line does not call PeliRun from one of `folders`.
pub(super) fn unwrapped(line: &str, folders: &[PathBuf]) -> Result<Option<String>, String> {
    let words = crate::core::advanced_settings::wrapper_args(line)?;
    let inside = |word: &str| folders.iter().any(|folder| Path::new(word).starts_with(folder));
    let start = match words.as_slice() {
        // `'…/pelirun' --app 42 -- %command% …` or `--profile pg-… -- %command% …`
        [first, ..] if first.ends_with("/pelirun") && inside(first) => match words.iter().position(|word| word == "--") {
            Some(index) => index + 1,
            None => return Ok(None),
        },
        // The previous release's `/bin/sh '…/peligames-game-settings/native-42.sh' %command% …`
        [shell, script, ..] if shell == "/bin/sh" && inside(script) => 2,
        _ => return Ok(None),
    };
    let rest: Vec<String> = words[start..].iter().map(|word| quote(word)).collect();
    // Only `%command%` left is Steam's default: an empty line.
    Ok(Some(if rest == ["%command%"] { String::new() } else { rest.join(" ") }))
}

/// Steam games whose line PeliGames may have changed: the Steam library it
/// knows and every game with a profile in a PeliRun folder.
fn candidate_apps(folders: &[PathBuf]) -> Vec<String> {
    let mut apps: Vec<String> = super::super::external_library::all().unwrap_or_default().into_iter()
        .filter(|game| game.launcher == "Steam").filter_map(|game| game.app_id).collect();
    for folder in folders {
        let Ok(entries) = fs::read_dir(folder.join("profiles")) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                    if !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit()) { apps.push(stem.into()); }
                }
            }
        }
    }
    apps.sort();
    apps.dedup();
    apps
}

fn local_configs() -> Vec<PathBuf> {
    let Some(home) = dirs::home_dir() else { return Vec::new() };
    let mut configs = Vec::new();
    for flatpak in [false, true] {
        for root in super::super::steam_installation::roots(&home, flatpak) {
            let Ok(users) = fs::read_dir(root.join("userdata")) else { continue };
            for user in users.flatten() {
                if let Ok(file) = fs::canonicalize(user.path().join("config/localconfig.vdf")) {
                    if !configs.contains(&file) { configs.push(file); }
                }
            }
        }
    }
    configs
}

/// Takes PeliRun out of every Steam line, then deletes its folders. Steam
/// must be closed: it rewrites its configuration when it exits.
pub(super) fn remove() -> Result<(), String> {
    let folders = folders()?;
    if folders.is_empty() { return Ok(()); }
    if super::super::external_library::launcher_open("Steam") {
        return Err("Feche a Steam antes de remover o PeliRun: as opções de inicialização dos jogos precisam ser corrigidas com ela fechada.".into());
    }
    let apps = candidate_apps(&folders);
    for config in local_configs() {
        let before = fs::read_to_string(&config).map_err(|e| format!("Não foi possível ler {}: {e}", config.display()))?;
        let mut text = before.clone();
        for app in &apps {
            let line = super::super::steam_launch_options::read_launch_options(&text, app)?;
            if let Some(clean) = unwrapped(&line, &folders)? {
                text = super::super::steam_launch_options::write_launch_options(&text, app, &clean)?;
            }
        }
        if text != before { super::super::steam_launch_options::write_file(&config, &text)?; }
    }
    for folder in folders {
        let meta = fs::symlink_metadata(&folder).map_err(|e| e.to_string())?;
        if meta.file_type().is_symlink() { fs::remove_file(&folder) } else { fs::remove_dir_all(&folder) }
            .map_err(|e| format!("Não foi possível apagar {}: {e}", folder.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steam_lines_lose_only_the_pelirun_call() {
        let folders = vec![PathBuf::from("/home/u/.local/share/pelirun"), PathBuf::from("/home/u/.steam/config/peligames-launcher"), PathBuf::from("/home/u/.steam/config/peligames-game-settings")];
        let clean = |line: &str| unwrapped(line, &folders).unwrap();
        assert_eq!(clean("'/home/u/.steam/config/peligames-launcher/pelirun' --app 42 -- %command%").as_deref(), Some(""));
        assert_eq!(clean("'/home/u/.steam/config/peligames-launcher/pelirun' --app 42 -- %command% -dx11").as_deref(), Some("%command% '-dx11'"));
        assert_eq!(clean("'/home/u/.local/share/pelirun/pelirun' --profile pg-1a2b -- %command% 'two words'").as_deref(), Some("%command% 'two words'"));
        assert_eq!(clean("/bin/sh '/home/u/.steam/config/peligames-game-settings/native-42.sh' %command%").as_deref(), Some(""));
        // Lines PeliGames did not write stay untouched.
        assert_eq!(clean("PROTON_LOG=1 %command% -dx11"), None);
        assert_eq!(clean("'/opt/other/pelirun' --app 42 -- %command%"), None);
        assert_eq!(clean(""), None);
    }
}
