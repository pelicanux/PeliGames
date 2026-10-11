//! Bounded, per-game engine diagnostics. Binary dumps are listed, never decoded as text.
use super::nexus_local::NexusGame;
use std::{
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

fn token(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn xml_summary(text: &str) -> String {
    use quick_xml::{events::Event, Reader};
    let mut reader = Reader::from_str(text);
    let mut out = String::new();
    // Deliberately exclude command lines, account IDs, machine IDs and environment dumps.
    const FIELDS: &[&str] = &[
        "CrashType",
        "ErrorMessage",
        "EngineVersion",
        "GameName",
        "PCallStack",
        "CallStack",
    ];
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let field = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if FIELDS.contains(&field.as_str()) {
                    match reader.read_text(e.name()) {
                        Ok(value) => {
                            let value = String::from_utf8_lossy(&value);
                            let value = value
                                .strip_prefix("<![CDATA[")
                                .and_then(|s| s.strip_suffix("]]>"))
                                .unwrap_or(&value);
                            let value =
                                quick_xml::escape::unescape(value).unwrap_or_else(|_| value.into());
                            if !value.trim().is_empty() {
                                let _ = writeln!(
                                    out,
                                    "{field}: {}",
                                    value.chars().take(16000).collect::<String>()
                                );
                            }
                        }
                        Err(_) => {
                            out.push_str("[XML de crash incompleto ou inválido]\n");
                            break;
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => {
                out.push_str("[XML de crash incompleto ou inválido]\n");
                break;
            }
            _ => {}
        }
    }
    out
}

pub(super) fn read(game: &NexusGame) -> String {
    // Without a launch in this launcher session there is nothing to scan.
    match super::nexus_loader_logs::session_started(game) { Some(started) => scan(game, Some(started)), None => String::new() }
}
fn scan(game: &NexusGame, session_started: Option<std::time::SystemTime>) -> String {
    let root = PathBuf::from(game.game["directory"].as_str().unwrap_or(""));
    if !root.is_absolute() {
        return String::new();
    }
    let definition = super::nexus_deployment::definition(game);
    let exe = definition
        .as_ref()
        .map(|d| d.executable.as_str())
        .unwrap_or("");
    let mut names = vec![
        token(game.game["name"].as_str().unwrap_or("")),
        token(root.file_name().and_then(|n| n.to_str()).unwrap_or("")),
    ];
    if let Some(project) = exe
        .split("/Binaries/")
        .next()
        .filter(|_| exe.contains("/Binaries/"))
    {
        names.push(token(project.rsplit('/').next().unwrap_or(project)));
    }
    names.push(token(
        Path::new(exe)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(""),
    ));
    names.retain(|s| s.len() >= 3);
    let matches = |name: &str| names.iter().any(|n| token(name) == *n);
    let engine = if exe.contains("/Binaries/") {
        "Unreal Engine (estrutura Binaries)"
    } else if fs::read_dir(&root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().ends_with("_Data") && e.path().is_dir())
    {
        "Unity (estrutura _Data)"
    } else if names
        .iter()
        .any(|n| n == "cyberpunk2077" || n == "witcher3")
    {
        "REDengine"
    } else if root.join("re_chunk_000.pak").is_file() {
        "RE Engine"
    } else {
        "Motor não identificado pela estrutura local"
    };
    let mut locations = vec![
        (root.clone(), engine),
        (root.join("Saved"), "Unreal Engine"),
    ];
    if exe.contains("/Binaries/") {
        if let Some(project) = exe.split("/Binaries/").next() {
            locations.push((root.join(project).join("Saved"), "Unreal Engine"));
        }
    }
    let prefix = PathBuf::from(&game.compat_data);
    let prefix = if prefix.join("pfx").is_dir() {
        prefix.join("pfx")
    } else {
        prefix
    };
    let users = prefix.join("drive_c/users");
    for user in fs::read_dir(users)
        .into_iter()
        .flatten()
        .take(16)
        .filter_map(Result::ok)
        .filter(|e| !e.path().is_symlink())
    {
        let app = user.path().join("AppData");
        for folder in fs::read_dir(app.join("Local"))
            .into_iter()
            .flatten()
            .take(400)
            .filter_map(Result::ok)
        {
            if matches(&folder.file_name().to_string_lossy()) && !folder.path().is_symlink() {
                locations.push((folder.path().join("Saved"), "Unreal Engine"));
                locations.push((folder.path().join("CrashDumps"), "Crash dumps do jogo"));
            }
        }
        for company in fs::read_dir(app.join("LocalLow"))
            .into_iter()
            .flatten()
            .take(200)
            .filter_map(Result::ok)
            .filter(|e| !e.path().is_symlink())
        {
            for product in fs::read_dir(company.path())
                .into_iter()
                .flatten()
                .take(200)
                .filter_map(Result::ok)
            {
                if matches(&product.file_name().to_string_lossy()) && !product.path().is_symlink() {
                    locations.push((product.path(), "Unity"));
                }
            }
        }
        for report in fs::read_dir(app.join("Local/REDEngine/ReportQueue"))
            .into_iter()
            .flatten()
            .take(200)
            .filter_map(Result::ok)
        {
            let name = token(&report.file_name().to_string_lossy());
            if names.iter().any(|n| name.starts_with(n)) && !report.path().is_symlink() {
                locations.push((report.path(), "REDengine crash report"));
            }
        }
        for folder in fs::read_dir(user.path().join("Documents/My Games"))
            .into_iter()
            .flatten()
            .take(200)
            .filter_map(Result::ok)
        {
            if matches(&folder.file_name().to_string_lossy()) && !folder.path().is_symlink() {
                for extender in ["SKSE", "F4SE", "NVSE", "SFSE", "OBSE", "FOSE"] {
                    locations.push((
                        folder.path().join(extender),
                        "Creation/Gamebryo — script extender / Crash Logger",
                    ));
                }
            }
        }
        locations.push((app.join("Local/CrashDumps"), "Windows/Proton crash dump"));
    }
    // Native Unity uses company/product directories instead of a Proton prefix.
    if game.platform == "native" {
        if let Some(home) = std::env::var_os("HOME") {
            for company in fs::read_dir(PathBuf::from(home).join(".config/unity3d"))
                .into_iter()
                .flatten()
                .take(200)
                .filter_map(Result::ok)
                .filter(|e| !e.path().is_symlink())
            {
                for product in fs::read_dir(company.path())
                    .into_iter()
                    .flatten()
                    .take(200)
                    .filter_map(Result::ok)
                {
                    if matches(&product.file_name().to_string_lossy())
                        && !product.path().is_symlink()
                    {
                        locations.push((product.path(), "Unity nativo"));
                    }
                }
            }
        }
    }
    let started = super::nexus_profile::directory(game)
        .ok()
        .and_then(|p| fs::read_to_string(p.join("logs/session-started.txt")).ok())
        .and_then(|s| s.trim().parse::<u64>().ok());
    let mut candidates = Vec::new();
    for (location, engine) in locations {
        let Ok(base) = fs::canonicalize(&location) else {
            continue;
        };
        for entry in walkdir::WalkDir::new(&location)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                !["SaveGames", ".peligames", ".git", "node_modules"]
                    .contains(&e.file_name().to_string_lossy().as_ref())
            })
            .take(4000)
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy();
            let lower = name.to_ascii_lowercase();
            let dump =
                lower.ends_with(".dmp") || lower.ends_with(".mdmp") || lower.ends_with(".core");
            let xml = lower == "crashcontext.runtime-xml";
            let log = [
                "player.log",
                "player-prev.log",
                "error.log",
                "crash.log",
                "stacktrace.txt",
                "report.txt",
            ]
            .contains(&lower.as_str())
                || (lower.ends_with(".log")
                    && (path.components().any(|c| c.as_os_str() == "Saved")
                        || engine.starts_with("Creation/Gamebryo")));
            if !(dump || xml || log) {
                continue;
            }
            if engine == "Windows/Proton crash dump"
                && !names.iter().any(|n| token(&name).starts_with(n))
            {
                continue;
            }
            if !fs::canonicalize(path).is_ok_and(|p| p.starts_with(&base)) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if !super::nexus_loader_logs::in_session(meta.modified().ok(), session_started) { continue; }
            candidates.push((
                path.to_path_buf(),
                engine,
                dump,
                xml,
                meta.len(),
                meta.modified().ok(),
            ));
        }
    }
    candidates.sort_by_key(|e| std::cmp::Reverse(e.5));
    let mut seen = std::collections::HashSet::new();
    let mut out = format!("\nMotor: {engine}\n--- DIAGNÓSTICOS DE CRASH / MOTOR ---\nColeta por estrutura do jogo e prefixo; presença do dump não identifica sozinha o mod responsável.\n");
    for (path, engine, dump, xml, size, time) in candidates
        .into_iter()
        .filter(|e| seen.insert(e.0.clone()))
        .take(10)
    {
        let stamp = time
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        let age = match (started, stamp) {
            (Some(s), Some(t)) if t < s => "ANTERIOR à última execução",
            (Some(_), Some(_)) => "atualizado após início da última execução",
            _ => "execução não identificada",
        };
        let _ = writeln!(
            out,
            "\n--- {engine} | {} | {size} bytes | modified_unix={} | {age} ---",
            path.display(),
            stamp.unwrap_or(0)
        );
        if dump {
            out.push_str("Dump binário encontrado; conteúdo requer depurador/símbolos e não é incluído no copiar.\n");
        } else if xml && size <= 2 * 1024 * 1024 {
            if let Ok(text) = fs::read_to_string(&path) {
                out.push_str(&xml_summary(&text));
            }
        } else if !xml {
            out.push_str(&super::nexus_profile::read_log_file(&path).unwrap_or_default());
            out.push('\n');
        } else {
            out.push_str("[XML excede o limite de leitura]\n");
        }
        if out.len() > 1024 * 1024 {
            break;
        }
    }
    if seen.is_empty() {
        out.push_str("Nenhum relatório de crash nas localizações reconhecidas. Nem todos os motores geram um relatório.\n");
    }
    super::nexus_reports::sanitize(&out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crash_xml_extracts_failure_without_private_fields() {
        let summary=xml_summary("<root><ErrorMessage>EXCEPTION_ACCESS_VIOLATION &amp; failure</ErrorMessage><LoginId>private-id</LoginId><CommandLine>private-token</CommandLine><PCallStack>UE4SS + 123</PCallStack></root>");
        assert!(summary.contains("EXCEPTION_ACCESS_VIOLATION & failure"));
        assert!(summary.contains("UE4SS + 123"));
        assert!(!summary.contains("private"));
    }
    #[test]
    fn collects_matching_project_crashes_without_saves_or_other_games() {
        let temp = std::env::temp_dir().join(format!("crash-collector-{}", uuid::Uuid::new_v4()));
        let root = temp.join("ExampleGame");
        let prefix = temp.join("prefix");
        fs::create_dir_all(&root).unwrap();
        let local = prefix.join("drive_c/users/steamuser/AppData/Local");
        for (project, message) in [
            ("ExampleGame", "CURRENT_FAILURE"),
            ("OtherGame", "UNRELATED_FAILURE"),
        ] {
            let crash = local.join(project).join("Saved/Crashes/report");
            fs::create_dir_all(&crash).unwrap();
            fs::write(crash.join("CrashContext.runtime-xml"),format!("<root><ErrorMessage>{message}</ErrorMessage><LoginId>private-id</LoginId></root>")).unwrap();
            fs::write(crash.join("UEMinidump.dmp"), "BINARY_NOT_TEXT").unwrap();
        }
        fs::create_dir_all(root.join("SaveGames")).unwrap();
        fs::write(root.join("SaveGames/crash.log"), "SAVE_NOT_LOG").unwrap();
        let game: NexusGame=serde_json::from_value(serde_json::json!({"id":"invalid-test-id","game":{"name":"ExampleGame","directory":root},"platform":"proton","mods":[],"compat_data":prefix})).unwrap();
        // Without a launch since the launcher opened, nothing is scanned.
        assert!(read(&game).is_empty());
        let result = scan(&game, None);
        assert!(result.contains("CURRENT_FAILURE"));
        assert!(result.contains("UEMinidump.dmp"));
        for excluded in [
            "UNRELATED_FAILURE",
            "BINARY_NOT_TEXT",
            "SAVE_NOT_LOG",
            "private-id",
        ] {
            assert!(!result.contains(excluded), "{excluded}");
        }
        fs::remove_dir_all(temp).unwrap();
    }
}
