//! Games installed by other Linux launchers (Heroic, Lutris, Faugus, Bottles),
//! native and Flatpak, with the Wine/Proton prefix each launcher uses. Only
//! read: launcher files are never changed. A runner is reported only when it is
//! a Proton build (a `proton` script), the runner the Nexus launch path uses.
use saphyr::{LoadableYamlNode, YamlOwned};
use serde::Serialize;
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LauncherGame {
    pub name: String,
    pub path: String,
    pub app_id: Option<String>,
    pub cover_url: Option<String>,
    pub launcher: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proton: Option<String>,
    /// Store that installed the game (`gog`, `epic`, `amazon`), when the launcher knows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_store: Option<String>,
    pub flatpak: bool,
    pub executable: Option<String>,
}

const MAX_FILE: u64 = 4 * 1024 * 1024;

fn read(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    (metadata.is_file() && metadata.len() <= MAX_FILE).then(|| fs::read_to_string(path).ok()).flatten()
}

fn yaml(path: &Path) -> Option<YamlOwned> {
    YamlOwned::load_from_str(&read(path)?).ok()?.into_iter().next()
}

fn text<'a>(node: &'a YamlOwned, keys: &[&str]) -> Option<&'a str> {
    let mut current = node;
    for key in keys { current = current.as_mapping_get(key)?; }
    current.as_str().map(str::trim).filter(|value| !value.is_empty())
}

/// A Proton directory (with its `proton` script), canonicalized.
fn proton_dir(path: &Path) -> Option<String> {
    let path = fs::canonicalize(path).ok()?;
    path.join("proton").is_file().then(|| path.to_string_lossy().into_owned())
}

/// Proton builds found by name in the usual compatibility tool folders.
fn named_proton(home: &Path, extra: &[PathBuf], name: &str) -> Option<String> {
    if name.is_empty() || name.contains('/') || name.starts_with('.') { return None; }
    extra.iter().cloned().chain([
        home.join(".local/share/Steam/compatibilitytools.d"),
        home.join(".steam/root/compatibilitytools.d"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/compatibilitytools.d"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d"),
    ]).find_map(|folder| proton_dir(&folder.join(name)))
}

fn existing_dir(path: &str) -> Option<String> {
    Path::new(path).is_dir().then(|| path.to_string())
}

/// Heroic (Legendary/Epic, GOG, Nile/Amazon, sideload) with `GamesConfig/<app>.json`.
pub(super) fn heroic(home: &Path) -> Vec<LauncherGame> {
    let mut games = Vec::new();
    for base in [home.join(".config/heroic"), home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic")] {
        for game in super::heroic_library::scan(&base) {
            let config = (!game.id.is_empty() && !game.id.contains(['/', '\\']))
                .then(|| read(&base.join("GamesConfig").join(format!("{}.json", game.id))))
                .flatten().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
            let settings = config.as_ref().map(|value| &value[game.id.as_str()]);
            let prefix = settings.and_then(|value| value["winePrefix"].as_str()).and_then(existing_dir);
            let proton = settings.filter(|value| value["wineVersion"]["type"] == "proton")
                .and_then(|value| value["wineVersion"]["bin"].as_str())
                .and_then(|bin| Path::new(bin).parent().and_then(proton_dir));
            let store = match game.store { "legendary" => Some("epic"), "gog" => Some("gog"), "nile" => Some("amazon"), _ => None };
            games.push(LauncherGame {
                name: game.name, path: game.path, app_id: None, cover_url: game.cover, launcher: "Heroic".into(),
                prefix, proton, store: store.map(str::to_string),
                external_id: Some(game.id), external_store: Some(game.store.into()), flatpak: base.starts_with(home.join(".var")), executable: None,
            });
        }
    }
    games
}

/// `the-elder-scrolls-v-skyrim-1712345678` → `The Elder Scrolls V Skyrim`.
fn title_from_slug(file: &str) -> String {
    let parts: Vec<&str> = file.split('-').collect();
    let parts = match parts.split_last() { Some((last, rest)) if !rest.is_empty() && last.chars().all(|c| c.is_ascii_digit()) => rest, _ => &parts[..] };
    parts.iter().filter(|word| !word.is_empty()).map(|word| {
        let mut chars = word.chars();
        let first = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
        let rest: String = chars.collect();
        // Roman numerals stay upper case (Skyrim V, Fallout IV).
        if word.chars().all(|c| "ivx".contains(c)) { word.to_uppercase() } else { first + &rest }
    }).collect::<Vec<_>>().join(" ")
}

/// Lutris per-game YAML (`games/<slug>-<id>.yml`): `game.exe`, `game.prefix`, `wine.version`.
pub(super) fn lutris(home: &Path) -> Vec<LauncherGame> {
    let flatpak = home.join(".var/app/net.lutris.Lutris");
    let sets = [
        (home.join(".local/share/lutris"), vec![home.join(".local/share/lutris/games"), home.join(".config/lutris/games")]),
        (flatpak.join("data/lutris"), vec![flatpak.join("data/lutris/games"), flatpak.join("config/lutris/games")]),
    ];
    let mut games = Vec::new();
    for (data, folders) in sets {
        for folder in folders {
            let Ok(entries) = fs::read_dir(&folder) else { continue };
            for entry in entries.flatten().take(5000) {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("yml") { continue; }
                let Some(config) = yaml(&path) else { continue };
                let Some(exe) = text(&config, &["game", "exe"]) else { continue };
                let exe = if Path::new(exe).is_absolute() { PathBuf::from(exe) } else {
                    match text(&config, &["game", "working_dir"]) { Some(dir) => Path::new(dir).join(exe), None => continue }
                };
                let Some(directory) = exe.parent().filter(|dir| dir.is_dir()) else { continue };
                let prefix = text(&config, &["game", "prefix"]).and_then(existing_dir);
                let proton = text(&config, &["wine", "version"]).and_then(|version| {
                    let runners = [data.join("runners/wine"), data.join("runners/proton")];
                    named_proton(home, &runners, version)
                });
                let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                games.push(LauncherGame {
                    name: title_from_slug(&stem), path: directory.to_string_lossy().into_owned(), app_id: None, cover_url: None,
                    launcher: "Lutris".into(), prefix, proton, store: None,
                    external_id: Some(stem), external_store: None, flatpak: data.starts_with(&flatpak), executable: Some(exe.to_string_lossy().into_owned()),
                });
            }
        }
    }
    games
}

/// Faugus Launcher `games.json`: `title`, `path` (executable), `prefix`, `runner`.
pub(super) fn faugus(home: &Path) -> Vec<LauncherGame> {
    let mut games = Vec::new();
    for file in [home.join(".local/share/faugus-launcher/games.json"), home.join(".var/app/io.github.Faugus.faugus-launcher/data/faugus-launcher/games.json")] {
        let Some(list) = read(&file).and_then(|text| serde_json::from_str::<Vec<serde_json::Value>>(&text).ok()) else { continue };
        for item in list.iter().take(5000) {
            let (Some(title), Some(exe)) = (item["title"].as_str(), item["path"].as_str()) else { continue };
            let Some(directory) = Path::new(exe).parent().filter(|dir| dir.is_dir()) else { continue };
            let runner = item["runner"].as_str().unwrap_or_default();
            let native = matches!(runner, "Linux-Native" | "Steam");
            games.push(LauncherGame {
                name: title.to_string(), path: directory.to_string_lossy().into_owned(), app_id: None, cover_url: None,
                launcher: "Faugus".into(),
                prefix: (!native).then(|| item["prefix"].as_str().and_then(existing_dir)).flatten(),
                proton: (!native).then(|| named_proton(home, &[], runner)).flatten(),
                store: None, external_id: item["gameid"].as_str().map(str::to_string), external_store: None, flatpak: file.starts_with(home.join(".var")), executable: Some(exe.into()),
            });
        }
    }
    games
}

/// Windows path inside a bottle (`C:\Games\x.exe`) → `<bottle>/drive_c/Games/x.exe`.
fn bottle_path(bottle: &Path, program: &str) -> Option<PathBuf> {
    let bytes = program.as_bytes();
    if bytes.len() > 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        let rest = program[2..].replace('\\', "/");
        let rest = rest.trim_start_matches('/');
        if rest.split('/').any(|part| part == "..") { return None; }
        return Some(bottle.join(format!("drive_{}", (bytes[0] as char).to_ascii_lowercase())).join(rest));
    }
    Path::new(program).is_absolute().then(|| PathBuf::from(program))
}

/// Bottles: each `bottle.yml` is a prefix; its `External_Programs` are the games.
pub(super) fn bottles(home: &Path) -> Vec<LauncherGame> {
    let mut games = Vec::new();
    for data in [home.join(".local/share/bottles"), home.join(".var/app/com.usebottles.bottles/data/bottles")] {
        let Ok(entries) = fs::read_dir(data.join("bottles")) else { continue };
        for entry in entries.flatten().take(1000) {
            let Some(config) = yaml(&entry.path().join("bottle.yml")) else { continue };
            let bottle = text(&config, &["Custom_Path"]).map(PathBuf::from).unwrap_or_else(|| entry.path());
            let proton = text(&config, &["Runner"]).and_then(|runner| named_proton(home, &[data.join("runners")], runner));
            let Some(programs) = config.as_mapping_get("External_Programs").and_then(YamlOwned::as_mapping) else { continue };
            for (program_id, program) in programs.iter().take(1000) {
                let Some(exe) = text(program, &["path"]).and_then(|path| bottle_path(&bottle, path)) else { continue };
                let Some(directory) = exe.parent().filter(|dir| dir.is_dir()) else { continue };
                let name = text(program, &["name"]).map(str::to_string)
                    .unwrap_or_else(|| exe.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
                games.push(LauncherGame {
                    name, path: directory.to_string_lossy().into_owned(), app_id: None, cover_url: None, launcher: "Bottles".into(),
                    prefix: existing_dir(&bottle.to_string_lossy()), proton: proton.clone(), store: None,
                    external_id: program_id.as_str().map(str::to_string), external_store: text(&config, &["Name"]).map(str::to_string), flatpak: data.starts_with(home.join(".var")), executable: Some(exe.to_string_lossy().into_owned()),
                });
            }
        }
    }
    games
}

/// All launchers; the first record of an installation folder wins.
pub(super) fn all(home: &Path) -> Vec<LauncherGame> {
    let mut games: Vec<LauncherGame> = Vec::new();
    for game in heroic(home).into_iter().chain(lutris(home)).chain(faugus(home)).chain(bottles(home)) {
        if !games.iter().any(|known| known.path == game.path) { games.push(game); }
    }
    games
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn proton(path: &Path) { write(&path.join("proton"), "#!/bin/sh\n"); }

    #[test]
    fn reads_every_launcher_with_its_prefix_and_proton_runner() {
        let home = std::env::temp_dir().join(format!("peligames-launchers-{}", uuid::Uuid::new_v4()));
        let h = |relative: &str| home.join(relative);
        proton(&h(".local/share/Steam/compatibilitytools.d/GE-Proton10-1"));
        // Heroic Flatpak, GOG game with a Proton prefix.
        let heroic = h(".var/app/com.heroicgameslauncher.hgl/config/heroic");
        fs::create_dir_all(h("Games/Skyrim")).unwrap();
        fs::create_dir_all(h("Prefixes/Skyrim/pfx")).unwrap();
        write(&heroic.join("gog_store/installed.json"), &format!(r#"{{"installed":[{{"appName":"1711230643","install_path":"{}","platform":"windows"}}]}}"#, h("Games/Skyrim").display()));
        write(&heroic.join("store_cache/gog_library.json"), r#"{"games":[{"app_name":"1711230643","title":"The Elder Scrolls V: Skyrim Special Edition"}]}"#);
        write(&heroic.join("GamesConfig/1711230643.json"), &format!(r#"{{"1711230643":{{"winePrefix":"{}","wineVersion":{{"type":"proton","bin":"{}"}}}}}}"#, h("Prefixes/Skyrim").display(), h(".local/share/Steam/compatibilitytools.d/GE-Proton10-1/proton").display()));
        // Lutris native, relative executable with working_dir and a wine (non-Proton) runner.
        fs::create_dir_all(h("Games/fallout-4")).unwrap();
        fs::create_dir_all(h("Games/fallout-4-prefix")).unwrap();
        fs::create_dir_all(h(".local/share/lutris/runners/wine/wine-ge-8-26")).unwrap();
        write(&h(".local/share/lutris/games/fallout-4-1712345678.yml"), &format!("game:\n  exe: Fallout4.exe\n  working_dir: {}\n  prefix: {}\nwine:\n  version: wine-ge-8-26\n", h("Games/fallout-4").display(), h("Games/fallout-4-prefix").display()));
        // Faugus Flatpak, Proton runner by name.
        fs::create_dir_all(h("Games/Oblivion")).unwrap();
        fs::create_dir_all(h("Faugus/oblivion")).unwrap();
        write(&h(".var/app/io.github.Faugus.faugus-launcher/data/faugus-launcher/games.json"), &format!(r#"[{{"title":"Oblivion","path":"{}","prefix":"{}","runner":"GE-Proton10-1"}}]"#, h("Games/Oblivion/Oblivion.exe").display(), h("Faugus/oblivion").display()));
        // Bottles, Windows path inside the bottle and a Proton runner.
        let bottle = h(".local/share/bottles/bottles/Gaming");
        fs::create_dir_all(bottle.join("drive_c/Games/Starfield")).unwrap();
        proton(&h(".local/share/bottles/runners/proton-ge-9"));
        write(&bottle.join("bottle.yml"), "Name: Gaming\nPath: Gaming\nRunner: proton-ge-9\nExternal_Programs:\n  abc-123:\n    executable: Starfield.exe\n    name: Starfield\n    path: C:\\Games\\Starfield\\Starfield.exe\n");

        let games = all(&home);
        let find = |launcher: &str| games.iter().find(|game| game.launcher == launcher).unwrap_or_else(|| panic!("{launcher}: {games:?}"));
        let canonical = |relative: &str| fs::canonicalize(h(relative)).unwrap().to_string_lossy().into_owned();

        let skyrim = find("Heroic");
        assert_eq!(skyrim.name, "The Elder Scrolls V: Skyrim Special Edition");
        assert_eq!(skyrim.store.as_deref(), Some("gog"));
        assert_eq!(skyrim.prefix.as_deref(), Some(h("Prefixes/Skyrim").to_string_lossy().as_ref()));
        assert_eq!(skyrim.proton.as_deref(), Some(canonical(".local/share/Steam/compatibilitytools.d/GE-Proton10-1").as_str()));

        let fallout = find("Lutris");
        assert_eq!(fallout.name, "Fallout 4");
        assert_eq!(fallout.path, h("Games/fallout-4").to_string_lossy());
        assert!(fallout.prefix.is_some() && fallout.proton.is_none(), "wine sem script proton não vira runner Proton");

        let oblivion = find("Faugus");
        assert_eq!(oblivion.path, h("Games/Oblivion").to_string_lossy());
        assert!(oblivion.prefix.is_some() && oblivion.proton.is_some());

        let starfield = find("Bottles");
        assert_eq!(starfield.path, bottle.join("drive_c/Games/Starfield").to_string_lossy());
        assert_eq!(starfield.prefix.as_deref(), Some(bottle.to_string_lossy().as_ref()));
        assert_eq!(starfield.proton.as_deref(), Some(canonical(".local/share/bottles/runners/proton-ge-9").as_str()));
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn rejects_paths_escaping_a_bottle_and_names_slugs() {
        let bottle = Path::new("/b");
        assert_eq!(bottle_path(bottle, "C:\\Games\\x.exe"), Some(PathBuf::from("/b/drive_c/Games/x.exe")));
        assert_eq!(bottle_path(bottle, "D:\\x.exe"), Some(PathBuf::from("/b/drive_d/x.exe")));
        assert_eq!(bottle_path(bottle, "C:\\..\\..\\etc\\passwd"), None);
        assert_eq!(bottle_path(bottle, "relative.exe"), None);
        assert_eq!(title_from_slug("the-elder-scrolls-v-skyrim-1712345678"), "The Elder Scrolls V Skyrim");
        assert_eq!(title_from_slug("fallout-new-vegas"), "Fallout New Vegas");
    }
    #[test]
    #[ignore = "Reads the real launchers of PELIGAMES_LAUNCHER_HOME (read-only)"]
    fn lists_real_launcher_games() {
        let home = PathBuf::from(std::env::var("PELIGAMES_LAUNCHER_HOME").expect("PELIGAMES_LAUNCHER_HOME"));
        for game in all(&home) { println!("{} | {} | {} | prefixo={:?} | proton={:?} | loja={:?}", game.launcher, game.name, game.path, game.prefix, game.proton, game.store); }
    }
}
