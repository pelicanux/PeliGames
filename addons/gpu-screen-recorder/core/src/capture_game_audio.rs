//! Resolve a game's audio on demand; never choose the whole desktop as fallback.
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Default)]
pub struct Target {
    pub directory: String,
    pub executable: String,
    pub steam_app_id: String,
}
#[derive(Debug)]
struct Stream {
    name: String,
    pid: Option<u32>,
}
fn number(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|v| v.try_into().ok())
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| *v > 1)
}
fn streams(objects: &[Value]) -> Vec<Stream> {
    objects
        .iter()
        .filter_map(|node| {
            if node["type"] != "PipeWire:Interface:Node" {
                return None;
            }
            let props = &node["info"]["props"];
            if props["media.class"] != "Stream/Output/Audio"
                || props["node.virtual"] == true
                || props["node.virtual"] == "true"
            {
                return None;
            }
            // Same name and case-insensitive comparison as GSR's PipeWire app filter.
            let name = props["application.name"]
                .as_str()
                .or_else(|| props["node.name"].as_str())?
                .to_owned();
            let client = number(&props["client.id"]).and_then(|id| {
                objects.iter().find(|v| {
                    v["type"] == "PipeWire:Interface:Client" && v["id"].as_u64() == Some(id.into())
                })
            });
            let pid = number(&props["application.process.id"]).or_else(|| {
                client.and_then(|v| number(&v["info"]["props"]["application.process.id"]))
            });
            Some(Stream { name, pid })
        })
        .collect()
}
fn snapshot() -> Result<Vec<Value>, String> {
    let mut command = Command::new("pw-dump");
    super::super::env_utils::host_environment(&mut command);
    let mut child=command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        .map_err(|_|"A detecção automática precisa do pw-dump (ferramentas do PipeWire). Escolha a fonte de áudio manualmente.".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Não foi possível consultar as fontes de áudio.")?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes);
        (result, bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(
                    "O PipeWire não respondeu à detecção. Escolha a fonte de áudio manualmente."
                        .into(),
                );
            }
        }
    };
    let (read, bytes) = reader.join().map_err(|_| "Falha ao consultar o áudio.")?;
    if !status.success() || read.is_err() || bytes.len() > 4 * 1024 * 1024 {
        return Err(
            "Não foi possível listar o áudio do PipeWire. Escolha a fonte manualmente.".into(),
        );
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "Resposta de áudio inválida. Escolha a fonte manualmente.".into())
}
fn normalized_path(value: &str, prefix: Option<&str>) -> Option<PathBuf> {
    let value = value.replace('\\', "/");
    let path = if value.starts_with("Z:/") || value.starts_with("z:/") {
        PathBuf::from(&value[2..])
    } else if value.starts_with("C:/") || value.starts_with("c:/") {
        Path::new(prefix?).join("drive_c").join(&value[3..])
    } else {
        PathBuf::from(value)
    };
    path.is_absolute()
        .then(|| path.canonicalize().unwrap_or(path))
}
fn belongs(root: &Path, mut pid: u32, target: &Target) -> bool {
    let directory = Path::new(&target.directory)
        .canonicalize()
        .ok()
        .filter(|p| p.is_dir() && p.components().count() > 2);
    let executable = normalized_path(&target.executable, None);
    let steam = (!target.steam_app_id.is_empty()
        && target.steam_app_id.bytes().all(|c| c.is_ascii_digit())
        && target.steam_app_id != "0")
        .then_some(target.steam_app_id.as_str());
    let mut visited = BTreeSet::new();
    for _ in 0..64 {
        if pid <= 1 || !visited.insert(pid) {
            break;
        }
        let path = root.join(pid.to_string());
        if fs::metadata(&path).ok().map(|m| m.uid()) != Some(unsafe { libc::geteuid() }) {
            break;
        }
        let env = fs::read(path.join("environ")).unwrap_or_default();
        let values: Vec<_> = env
            .split(|v| *v == 0)
            .filter_map(|v| std::str::from_utf8(v).ok())
            .collect();
        if let Some(id) = steam {
            let ids: Vec<_> = values
                .iter()
                .filter_map(|v| {
                    v.strip_prefix("SteamAppId=")
                        .or_else(|| v.strip_prefix("SteamGameId="))
                })
                .filter(|v| *v != "0")
                .collect();
            if !ids.is_empty() {
                return ids.iter().all(|v| *v == id);
            }
        }
        let prefix = values.iter().find_map(|v| v.strip_prefix("WINEPREFIX="));
        let command = fs::read(path.join("cmdline")).unwrap_or_default();
        let paths = command
            .split(|v| *v == 0)
            .filter_map(|v| std::str::from_utf8(v).ok())
            .filter_map(|v| normalized_path(v, prefix));
        let exe = fs::read_link(path.join("exe")).ok();
        if paths.chain(exe).any(|p| {
            executable.as_ref() == Some(&p)
                || directory.as_ref().is_some_and(|dir| p.starts_with(dir))
        }) {
            return true;
        }
        if directory
            .as_ref()
            .is_some_and(|dir| fs::read_link(path.join("cwd")).is_ok_and(|p| p.starts_with(dir)))
        {
            return true;
        }
        // Stop at the desktop launcher: siblings launched by it are not the game.
        let stat = fs::read_to_string(path.join("stat")).unwrap_or_default();
        let Some(fields) = stat.rsplit_once(") ").map(|v| v.1) else {
            break;
        };
        let Some(parent) = fields
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
        else {
            break;
        };
        pid = parent;
    }
    false
}
fn select(
    streams: &[Stream],
    available: &[String],
    mut matches: impl FnMut(u32) -> bool,
) -> Result<String, String> {
    let matching: Vec<_> = streams
        .iter()
        .map(|s| s.pid.is_some_and(&mut matches))
        .collect();
    let mut names = BTreeSet::new();
    for (stream, is_game) in streams.iter().zip(&matching) {
        if !is_game {
            continue;
        }
        let name = stream.name.to_lowercase();
        if name.is_empty() || name.len() > 512 || name.chars().any(|c| c.is_control() || c == '|') {
            return Err("Nome de áudio inválido. Escolha a fonte manualmente.".into());
        }
        if streams
            .iter()
            .zip(&matching)
            .any(|(s, matched)| !matched && s.name.eq_ignore_ascii_case(&stream.name))
        {
            return Err("Outro aplicativo usa o mesmo nome de áudio do jogo. Escolha a fonte manualmente ou feche o outro aplicativo.".into());
        }
        let Some(available) = available
            .iter()
            .find(|s| s.eq_ignore_ascii_case(&stream.name))
        else {
            return Err("O gravador ainda não reconheceu o áudio do jogo. Aguarde o jogo emitir som e tente novamente.".into());
        };
        names.insert(format!("app:{available}"));
    }
    if names.is_empty() {
        return Err("Áudio deste jogo não identificado. Abra o jogo, aguarde ele emitir som e tente novamente, ou escolha a fonte manualmente.".into());
    }
    if names.len() > 32 {
        return Err("Muitas fontes de áudio para este jogo. Escolha a fonte manualmente.".into());
    }
    Ok(names.into_iter().collect::<Vec<_>>().join("|"))
}
pub fn resolve(target: &Target, available: &[String]) -> Result<String, String> {
    let objects = snapshot()?;
    select(&streams(&objects), available, |pid| {
        belongs(Path::new("/proc"), pid, target)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wine_names_must_belong_only_to_the_selected_game() {
        let streams = vec![
            Stream {
                name: "Wine64 Preloader".into(),
                pid: Some(10),
            },
            Stream {
                name: "Wine Preloader".into(),
                pid: Some(11),
            },
            Stream {
                name: "Discord".into(),
                pid: Some(12),
            },
        ];
        let available = vec![
            "Wine64 Preloader".into(),
            "Wine Preloader".into(),
            "Discord".into(),
        ];
        assert_eq!(
            select(&streams, &available, |pid| pid == 10 || pid == 11).unwrap(),
            "app:Wine Preloader|app:Wine64 Preloader"
        );
        let mut ambiguous = streams;
        ambiguous.push(Stream {
            name: "wine64 preloader".into(),
            pid: Some(99),
        });
        assert!(select(&ambiguous, &available, |pid| pid == 10 || pid == 11)
            .unwrap_err()
            .contains("mesmo nome"));
        ambiguous.last_mut().unwrap().pid = None;
        assert!(select(&ambiguous, &available, |pid| pid == 10 || pid == 11).is_err());
        assert!(select(&ambiguous, &available, |_| false)
            .unwrap_err()
            .contains("não identificado"));
    }
    #[test]
    fn pipewire_client_pid_and_native_game_audio_are_recognized() {
        let objects = serde_json::json!([
            {"id":40,"type":"PipeWire:Interface:Client","info":{"props":{"application.process.id":"321"}}},
            {"type":"PipeWire:Interface:Node","info":{"props":{"client.id":40,"media.class":"Stream/Output/Audio","application.name":"Native Game"}}},
            {"type":"PipeWire:Interface:Node","info":{"props":{"media.class":"Audio/Sink","node.name":"Speakers"}}}
        ]);
        let streams = streams(objects.as_array().unwrap());
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].pid, Some(321));
        assert_eq!(
            select(&streams, &["Native Game".into()], |pid| pid == 321).unwrap(),
            "app:Native Game"
        );
        assert!(select(&streams, &[], |_| true).is_err());
    }
    #[test]
    fn wine_windows_paths_use_the_prefix_and_preserve_spaces() {
        let root = std::env::temp_dir().join(format!("peli-audio-path-{}", uuid::Uuid::new_v4()));
        let game = root.join("prefix/drive_c/Games/Game Name");
        fs::create_dir_all(&game).unwrap();
        let exe = game.join("Game.exe");
        fs::write(&exe, []).unwrap();
        let process = root.join("21");
        fs::create_dir(&process).unwrap();
        fs::write(
            process.join("environ"),
            format!("WINEPREFIX={}\0", root.join("prefix").display()),
        )
        .unwrap();
        fs::write(
            process.join("cmdline"),
            b"wine64-preloader\0C:\\Games\\Game Name\\Game.exe\0",
        )
        .unwrap();
        let target = Target {
            directory: game.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert!(belongs(&root, 21, &target));
        assert!(!belongs(&root, 21, &Target::default()));
        assert_eq!(
            normalized_path(&format!("Z:{}", exe.display()), None),
            Some(exe)
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn process_folder_steam_id_and_children_match_without_sibling_games() {
        let root = std::env::temp_dir().join(format!("peli-audio-{}", uuid::Uuid::new_v4()));
        let game = root.join("game");
        fs::create_dir_all(&game).unwrap();
        for (pid, parent, cwd, env) in [
            (10, 1, game.clone(), "SteamAppId=123"),
            (11, 10, root.clone(), ""),
            (12, 1, root.clone(), "SteamAppId=999"),
        ] {
            let dir = root.join(pid.to_string());
            fs::create_dir(&dir).unwrap();
            fs::write(dir.join("environ"), env).unwrap();
            fs::write(dir.join("cmdline"), b"wine64-preloader\0").unwrap();
            fs::write(dir.join("stat"), format!("{pid} (game) S {parent} 0")).unwrap();
            std::os::unix::fs::symlink(cwd, dir.join("cwd")).unwrap();
        }
        let target = Target {
            directory: game.to_string_lossy().into_owned(),
            steam_app_id: "123".into(),
            ..Default::default()
        };
        assert!(belongs(&root, 10, &target));
        assert!(belongs(&root, 11, &target));
        assert!(!belongs(&root, 12, &target));
        let native = Target {
            directory: game.to_string_lossy().into_owned(),
            ..Default::default()
        };
        assert!(belongs(&root, 10, &native));
        assert!(belongs(&root, 11, &native));
        assert!(!belongs(&root, 12, &native));
        fs::remove_dir_all(root).unwrap();
    }
}
