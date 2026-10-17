//! Read-only discovery of the native Epic launcher (EGLv2), independent of packaging.
use super::launcher_library::LauncherGame;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::{Component, Path, PathBuf}};

pub(super) const LAUNCHER: &str = "Epic Games";
pub(super) fn roots(home: &Path) -> Vec<PathBuf> {
    let mut roots = vec![home.join(".local/share/com.epicgames.eglv2")];
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()) {
        let path = data.join("com.epicgames.eglv2");
        if !roots.contains(&path) { roots.insert(0, path); }
    }
    roots
}
pub(super) fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&read(path)?).ok()
}
fn read(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    (meta.is_file() && meta.len() <= 32 * 1024 * 1024).then(|| fs::read_to_string(path).ok()).flatten()
}
fn safe_id(value: &str) -> bool {
    !value.is_empty() && value != "." && value != ".." && value.len() <= 200 && value.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
}
pub(super) fn parts(id: &str) -> Result<(&str, &str, &str), String> {
    let pieces: Vec<_> = id.split(':').collect();
    match pieces.as_slice() {
        [namespace, item, artifact] if pieces.iter().all(|v| safe_id(v)) => Ok((namespace, item, artifact)),
        _ => Err("Identificação Epic inválida.".into()),
    }
}
fn installations(text: &str) -> Vec<Value> {
    let mut latest = BTreeMap::new();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else { continue };
        let Some(id) = value["installationId"].as_str().filter(|id| safe_id(id)) else { continue };
        // JSONL is a journal: an incomplete or deleted last entry supersedes an older install.
        latest.insert(id.to_string(), value);
    }
    latest.into_values().filter(|v| v["complete"] == true && v["deleted"] != true).collect()
}
fn executable(root: &Path, entry: &Value) -> Option<String> {
    let relative = entry["launchExecutable"].as_str()?.replace('\\', "/");
    let relative = Path::new(&relative);
    if relative.components().any(|part| !matches!(part, Component::Normal(_) | Component::CurDir)) { return None; }
    let path = root.join(relative).canonicalize().ok()?;
    (path.is_file() && path.starts_with(root.canonicalize().ok()?)).then(|| path.to_string_lossy().into_owned())
}
fn scan_root(base: &Path) -> Vec<LauncherGame> {
    let Some(text) = read(&base.join("installations.jsonl")) else { return vec![] };
    let catalog = read_json(&base.join("cache/prod/catalog-cache.json")).unwrap_or(Value::Null);
    let mut games = Vec::new();
    for entry in installations(&text) {
        let Some(directory) = entry["dir"].as_str().map(Path::new).filter(|p| p.is_absolute() && p.is_dir()) else { continue };
        let (Some(namespace), Some(item), Some(artifact)) = (entry["namespace"].as_str(), entry["itemId"].as_str(), entry["artifactId"].as_str()) else { continue };
        let id = format!("{namespace}:{item}:{artifact}");
        if parts(&id).is_err() { continue; }
        let metadata = &catalog["items"][item]["item"];
        let name = metadata["title"].as_str().filter(|v| !v.trim().is_empty())
            .or_else(|| directory.file_name().and_then(|n| n.to_str())).unwrap_or(artifact).to_string();
        let cover = metadata["keyImages"].as_array().and_then(|images| {
            ["DieselGameBoxTall", "OfferImageTall", "Thumbnail", "DieselGameBox"].into_iter().find_map(|kind| images.iter().find(|v| v["type"] == kind).and_then(|v| v["url"].as_str()))
        }).filter(|url| url.starts_with("https://")).map(str::to_owned);
        let compat = base.join("compat/compatdata").join(artifact);
        let prefix = (entry["platform"] != "Linux").then(|| compat.join("pfx")).filter(|p| p.is_dir()).map(|p| p.to_string_lossy().into_owned());
        let pinned = read_json(&base.join("compat/runtime/pin-cache.json")).and_then(|v| v["version"].as_str().map(str::to_owned));
        let proton = (entry["platform"] != "Linux").then(|| [read(&compat.join("version")), pinned].into_iter().flatten()
            .map(|version| version.trim().to_owned()).filter(|v| safe_id(v)).map(|v| base.join("compat/runtime").join(v))
            .find(|p| p.join("proton").is_file()).map(|p| p.to_string_lossy().into_owned())).flatten();
        games.push(LauncherGame { native_linux: entry["platform"].as_str().is_some_and(|p| p.eq_ignore_ascii_case("linux")), name, path: directory.to_string_lossy().into_owned(), app_id: None, cover_url: cover,
            launcher: LAUNCHER.into(), prefix, proton, store: Some("epic".into()), external_id: Some(id), external_store: Some("epic".into()), flatpak: false, executable: executable(directory, &entry) });
    }
    games
}
pub(super) fn scan(home: &Path) -> Vec<LauncherGame> {
    let mut games = Vec::<LauncherGame>::new();
    for root in roots(home) {
        for game in scan_root(&root) {
            if !games.iter().any(|known| known.external_id == game.external_id) { games.push(game); }
        }
    }
    games
}
pub(super) fn root_for(home: &Path, id: &str) -> Option<PathBuf> {
    roots(home).into_iter().find(|root| scan_root(root).iter().any(|game| game.external_id.as_deref() == Some(id)))
}
pub(super) fn launch_uri(id: &str) -> Result<String, String> {
    parts(id)?;
    Ok(format!("com.epicgames.launcher://apps/{}?action=launch&silent=true", urlencoding::encode(id)))
}
pub(super) fn command(home: &Path, id: &str) -> Result<(String, Vec<String>), String> {
    let uri = launch_uri(id)?;
    for name in ["EpicGamesLauncher", "epic-games-launcher"] {
        if std::env::var_os("PATH").is_some_and(|value| std::env::split_paths(&value).any(|dir| dir.join(name).is_file())) {
            return Ok((name.into(), vec![uri]));
        }
    }
    for folder in [home.join(".local/share/applications"), PathBuf::from("/usr/share/applications"), PathBuf::from("/usr/local/share/applications")] {
        let Ok(files) = fs::read_dir(folder) else { continue };
        for file in files.flatten().take(5000) {
            if file.path().extension().is_none_or(|v| v != "desktop") { continue; }
            let Some(text) = read(&file.path()) else { continue };
            if !text.lines().any(|line| line.strip_prefix("MimeType=").is_some_and(|v| v.split(';').any(|v| v == "x-scheme-handler/com.epicgames.launcher"))) { continue; }
            let Some(exec) = text.lines().find_map(|line| line.strip_prefix("Exec=")) else { continue };
            let Ok(tokens) = crate::core::advanced_settings::wrapper_args(exec) else { continue };
            let mut tokens = tokens.into_iter(); let Some(program) = tokens.next() else { continue };
            let mut args: Vec<String> = tokens.filter(|v| !v.contains('%')).collect(); args.push(uri);
            return Ok((program, args));
        }
    }
    // Unintegrated AppImages have no desktop handler. Search only common application
    // folders, never recurse through game libraries or execute arbitrary matching files.
    for folder in [home.join("Applications"), home.join("Desktop"), home.join("Downloads")] {
        let Ok(files) = fs::read_dir(folder) else { continue };
        let mut paths: Vec<_> = files.flatten().map(|f| f.path()).filter(|p| p.is_file() && p.file_name().is_some_and(|v| {
            let name = v.to_string_lossy().to_ascii_lowercase(); name.starts_with("epicgameslauncher") && name.ends_with(".appimage")
        })).collect(); paths.sort();
        if let Some(path) = paths.pop() { return Ok((path.to_string_lossy().into_owned(), vec![uri])); }
    }
    Err("Epic Games Launcher não encontrado. Integre o AppImage ao menu de aplicativos ou coloque-o em Applications, Desktop ou Downloads.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires installed games in the current user's native Epic launcher"]
    fn epic_local_inventory() {
        let games = scan(&dirs::home_dir().unwrap());
        assert!(!games.is_empty(), "No completed native Epic installations were detected");
        for game in games {
            println!("Epic: {} | prefix={} runner={} executable={} cover={}", game.name, game.prefix.is_some(), game.proton.is_some(), game.executable.is_some(), game.cover_url.is_some());
            assert!(Path::new(&game.path).is_dir());
            assert!(launch_uri(game.external_id.as_deref().unwrap()).is_ok());
            let (program, arguments) = command(&dirs::home_dir().unwrap(), game.external_id.as_deref().unwrap()).unwrap();
            assert!(!program.is_empty());
            assert!(arguments.last().unwrap().starts_with("com.epicgames.launcher://apps/"));
            println!("Epic launch command resolved (not executed)");
        }
    }
    #[test]
    fn journal_uses_last_state_and_ignores_headers_and_torn_lines() {
        let text = r#"{"version":1}
{"installationId":"a","complete":true}
{"installationId":"a","deleted":true}
{"installationId":"b","complete":false}
{"installationId":"c","complete":true}
{"installationId":"b","complete":true}
{"installationId":"c","complete":false}
{"installationId":"broken""#;
        let records = installations(text);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["installationId"], "b");
        assert!(launch_uri("ns:item:game").unwrap().contains("ns%3Aitem%3Agame"));
        assert!(launch_uri("ns:item:../escape").is_err());
    }
    #[test]
    fn installed_game_keeps_its_native_prefix_runner_and_catalog_art() {
        let home = std::env::temp_dir().join(format!("peligames-epic-{}", uuid::Uuid::new_v4()));
        let base = home.join("epic"); let game = home.join("Games/Example");
        let put = |path: PathBuf, text: &str| { fs::create_dir_all(path.parent().unwrap()).unwrap(); fs::write(path, text).unwrap(); };
        put(game.join("bin/game.exe"), "");
        put(base.join("installations.jsonl"), &serde_json::json!({"installationId":"one","complete":true,"namespace":"ns","itemId":"item","artifactId":"game","platform":"Windows","dir":game,"launchExecutable":"bin\\game.exe"}).to_string());
        put(base.join("cache/prod/catalog-cache.json"), r#"{"items":{"item":{"item":{"title":"Example Title","keyImages":[{"type":"DieselGameBoxTall","url":"https://cdn.example/cover.jpg"}]}}}}"#);
        fs::create_dir_all(base.join("compat/compatdata/game/pfx")).unwrap();
        put(base.join("compat/compatdata/game/version"), "GE-Proton10-1\n");
        put(base.join("compat/runtime/GE-Proton10-1/proton"), "");
        let games = scan_root(&base); assert_eq!(games.len(), 1);
        assert_eq!(games[0].name, "Example Title");
        assert!(games[0].prefix.as_ref().unwrap().ends_with("/game/pfx"));
        assert!(games[0].proton.as_ref().unwrap().ends_with("/GE-Proton10-1"));
        assert!(games[0].executable.as_ref().unwrap().ends_with("/bin/game.exe"));
        assert!(executable(&game, &serde_json::json!({"launchExecutable":"../outside.exe"})).is_none());
        put(base.join("compat/compatdata/game/version"), "removed-runner");
        put(base.join("compat/runtime/pin-cache.json"), r#"{"version":"GE-Proton10-1"}"#);
        assert!(scan_root(&base)[0].proton.as_ref().unwrap().ends_with("/GE-Proton10-1"));
        let manifest_path = base.join("installations.jsonl");
        let mut manifest = read_json(&manifest_path).unwrap();
        manifest["platform"] = Value::String("Linux".into());
        put(manifest_path, &manifest.to_string());
        let native = scan_root(&base);
        assert!(native[0].prefix.is_none());
        assert!(native[0].proton.is_none());
        fs::remove_dir_all(home).unwrap();
    }
}
