//! Steam owns launch options and Proton. Observe only this user's AppID processes.
use super::*;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

fn belongs(env: &[u8], id: &str) -> bool {
    env.split(|b| *b == 0).any(|value| {
        ["SteamAppId=", "SteamGameId="]
            .iter()
            .any(|key| value == format!("{key}{id}").as_bytes())
    })
}
fn application(command: &[u8], executable: &Path, cwd: &Path, game: &Path) -> bool {
    let first = String::from_utf8_lossy(command.split(|b| *b == 0).next().unwrap_or_default());
    let normalized = first.replace('\\', "/");
    let normalized = normalized
        .strip_prefix("Z:")
        .or_else(|| normalized.strip_prefix("z:"))
        .unwrap_or(&normalized);
    let name = normalized
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let helper = [
        "wine",
        "wine64",
        "wine-preloader",
        "wine64-preloader",
        "wineserver",
        "python",
        "python3",
        "sh",
        "bash",
        "pressure-vessel",
        "reaper",
        "steam",
        "steamwebhelper",
        "steam-runtime-launcher-service",
        "explorer.exe",
        "services.exe",
        "winedevice.exe",
        "rpcss.exe",
        "wineboot.exe",
        "conhost.exe",
        "plugplay.exe",
    ];
    !helper.contains(&name.as_str())
        && (executable.starts_with(game)
            || Path::new(normalized).starts_with(game)
            || (cwd.starts_with(game) && name.ends_with(".exe")))
}
fn snapshot(root: &Path, id: &str, game: &Path) -> Vec<(i32, u64, bool)> {
    let Ok(entries) = fs::read_dir(root) else {
        return vec![];
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse::<i32>().ok()?;
            let dir = entry.path();
            if fs::metadata(&dir).ok()?.uid() != unsafe { libc::geteuid() } {
                return None;
            }
            if !belongs(&fs::read(dir.join("environ")).ok()?, id) {
                return None;
            }
            let command = fs::read(dir.join("cmdline")).ok()?;
            let first = String::from_utf8_lossy(command.split(|b| *b == 0).next()?);
            // Never signal the Steam client, even if it inherited an AppID.
            if ["steam", "steamwebhelper"].contains(&first.rsplit('/').next().unwrap_or_default()) {
                return None;
            }
            let stat = fs::read_to_string(dir.join("stat")).ok()?;
            let fields: Vec<_> = stat.rsplit_once(") ")?.1.split_whitespace().collect();
            if fields.first()? == &"Z" {
                return None;
            }
            let birth = fields.get(19)?.parse().ok()?;
            let executable = fs::read_link(dir.join("exe")).unwrap_or_default();
            let cwd = fs::read_link(dir.join("cwd")).unwrap_or_default();
            Some((pid, birth, application(&command, &executable, &cwd, game)))
        })
        .collect()
}

pub fn start(path: String, id: String, uri: String) -> Result<ExecutionStatus, String> {
    start_with(path, id, || {
        std::process::Command::new("xdg-open")
            .arg(uri)
            .env_remove("LD_LIBRARY_PATH")
            .spawn()
            .map_err(|e| format!("Falha ao abrir Steam: {e}"))
    })
}
pub fn start_with(
    path: String,
    id: String,
    open: impl FnOnce() -> Result<std::process::Child, String>,
) -> Result<ExecutionStatus, String> {
    let id = id
        .parse::<u32>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or("Steam AppID inválido.")?
        .to_string();
    let game = PathBuf::from(&path)
        .canonicalize()
        .map_err(|e| format!("Pasta do jogo indisponível: {e}"))?;
    let mut slot = SESSION.lock().map_err(|e| e.to_string())?;
    let registry = super::super::execution_registry::directory()?;
    if let Some(state) = slot
        .as_ref()
        .and_then(|s| s.status.lock().ok().map(|s| s.clone()))
        .filter(|s| s.active())
        .or_else(|| super::super::execution_registry::status_at(&registry))
    {
        if state.path == path {
            return Ok(state);
        }
        return Err("Um jogo ou programa já está em execução.".into());
    }
    let session = Arc::new(Session {
        status: Mutex::new(ExecutionStatus {
            id: uuid::Uuid::new_v4().to_string(),
            path,
            state: "starting".into(),
            log: String::new(),
            error: None,
        }),
        cancel: AtomicBool::new(false),
    });
    let initial = session.status.lock().map_err(|e| e.to_string())?.clone();
    super::super::execution_registry::publish(&registry, &initial)?;
    let child = open();
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            super::super::execution_registry::finish(&registry, &initial.id);
            return Err(e);
        }
    };
    *slot = Some(session.clone());
    let session_id = initial.id.clone();
    std::thread::spawn(move || {
        let mut published_state = String::from("starting");
        let began = Instant::now();
        let mut playing = None;
        let mut last_seen = Instant::now();
        let mut stopping = None;
        let mut opener_done = false;
        let mut failure = None;
        loop {
            if !opener_done {
                match child.try_wait() {
                    Ok(Some(exit)) => {
                        opener_done = true;
                        if !exit.success() && !session.cancel.load(Ordering::Acquire) {
                            failure = Some("A Steam não aceitou a solicitação de abertura.".into());
                            break;
                        }
                    }
                    Err(e) => {
                        failure = Some(e.to_string());
                        break;
                    }
                    _ => {}
                }
            }
            if super::super::execution_registry::cancellation_requested(&registry, &session_id) {
                session.cancel.store(true, Ordering::Release);
            }
            let processes = snapshot(Path::new("/proc"), &id, &game);
            let cancelled = session.cancel.load(Ordering::Acquire);
            if cancelled {
                let since = stopping.get_or_insert_with(Instant::now);
                for (pid, birth, _) in &processes {
                    if snapshot(Path::new("/proc"), &id, &game)
                        .iter()
                        .any(|p| p.0 == *pid && p.1 == *birth)
                    {
                        unsafe {
                            libc::kill(
                                *pid,
                                if since.elapsed() < Duration::from_secs(3) {
                                    libc::SIGTERM
                                } else {
                                    libc::SIGKILL
                                },
                            );
                        }
                    }
                }
                if processes.is_empty() && since.elapsed() > Duration::from_secs(3) {
                    break;
                }
            } else if processes.iter().any(|p| p.2) {
                playing.get_or_insert_with(|| (std::time::SystemTime::now(), Instant::now()));
                last_seen = Instant::now();
            } else if playing.is_some() && last_seen.elapsed() > Duration::from_secs(3) {
                break;
            } else if playing.is_none() && began.elapsed() > Duration::from_secs(180) {
                failure = Some("Nenhum processo do jogo foi detectado. Confira a janela da Steam e tente novamente.".into());
                break;
            }
            if let Ok(mut state) = session.status.lock() {
                state.state = if cancelled {
                    "stopping"
                } else if playing.is_some() {
                    "running"
                } else {
                    "starting"
                }
                .into();
                if published_state != state.state {
                    if super::super::execution_registry::publish(&registry, &state).is_ok() {
                        published_state = state.state.clone();
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        if !opener_done {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        if let Ok(mut state) = session.status.lock() {
            state.state = if failure.is_some() {
                "failed"
            } else if session.cancel.load(Ordering::Acquire) {
                "cancelled"
            } else {
                "exited"
            }
            .into();
            state.error = failure;
            super::super::execution_registry::finish(&registry, &state.id);
            if let Some((started, since)) = playing {
                super::super::playtime::record(&state.path, started, since.elapsed());
            }
        }
    });
    Ok(initial)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_id_is_exact_not_a_substring() {
        assert!(belongs(b"SteamAppId=123\0Other=x\0", "123"));
        assert!(belongs(b"SteamGameId=123\0", "123"));
        assert!(!belongs(b"SteamAppId=1234\0", "123"));
        assert!(!belongs(b"Other=SteamAppId=123\0", "123"));
    }
    #[test]
    fn preparation_is_not_gameplay() {
        let game = Path::new("/games/example");
        assert!(!application(
            b"python3\0/game/proton\0",
            Path::new("/usr/bin/python3"),
            Path::new("/"),
            game
        ));
        assert!(!application(
            b"wine64\0/games/example/Game.exe\0",
            Path::new("/usr/bin/wine64"),
            Path::new("/"),
            game
        ));
        assert!(application(
            b"Z:\\games\\example\\Game.exe\0",
            Path::new("/usr/bin/wine64"),
            Path::new("/"),
            game
        ));
        assert!(application(
            b"./game\0",
            Path::new("/games/example/game"),
            Path::new("/"),
            game
        ));
        assert!(application(
            b"Game.exe\0",
            Path::new("/usr/bin/wine64"),
            game,
            game
        ));
        assert!(!application(
            b"wineboot.exe\0",
            Path::new("/usr/bin/wine64"),
            game,
            game
        ));
        assert!(!application(
            b"/games/example-other/game\0",
            Path::new("/games/example-other/game"),
            Path::new("/"),
            game
        ));
    }
}
