//! Steam owns launch options and Proton. Observe only this user's AppID processes.
//! Other launchers (Heroic, Lutris…) have no such ID: their game is recognized
//! by its install folder, the same way the Steam game's own processes are.
use super::*;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

fn belongs(env: &[u8], id: &str) -> bool {
    env.split(|b| *b == 0).any(|value| {
        ["SteamAppId=", "SteamGameId=", "STEAM_COMPAT_APP_ID="]
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
/// Steam's reaper/runtime can precede the game and its SteamAppId environment.
/// Match the exact launch marker, never an AppID embedded in another argument.
fn launch_marker(command: &[u8], id: &str) -> bool {
    let args: Vec<_> = command.split(|b| *b == 0).collect();
    args.windows(2)
        .any(|args| args[0] == b"SteamLaunch" && args[1] == format!("AppId={id}").as_bytes())
}
fn client(command: &[u8]) -> bool {
    let first = command.split(|b| *b == 0).next().unwrap_or_default();
    matches!(
        first.rsplit(|b| *b == b'/').next(),
        Some(b"steam" | b"steamwebhelper")
    )
}
fn process_identity(dir: &Path) -> Option<(i32, u64)> {
    if fs::metadata(dir).ok()?.uid() != unsafe { libc::geteuid() } {
        return None;
    }
    let stat = fs::read_to_string(dir.join("stat")).ok()?;
    let fields: Vec<_> = stat.rsplit_once(") ")?.1.split_whitespace().collect();
    if fields.first()? == &"Z" {
        return None;
    }
    Some((fields.get(1)?.parse().ok()?, fields.get(19)?.parse().ok()?))
}
/// Include the launch-specific runtime and its children, even when a child
/// drops Steam's environment. Never include the Steam client or other games.
fn snapshot(root: &Path, id: Option<&str>, game: &Path) -> Vec<(i32, u64, bool)> {
    let Ok(entries) = fs::read_dir(root) else {
        return vec![];
    };
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<i32>().ok())
        else {
            continue;
        };
        let dir = entry.path();
        let Some((parent, birth)) = process_identity(&dir) else {
            continue;
        };
        let Ok(command) = fs::read(dir.join("cmdline")) else {
            continue;
        };
        if client(&command) {
            continue;
        }
        let matched = if let Some(id) = id {
            launch_marker(&command, id)
                || belongs(&fs::read(dir.join("environ")).unwrap_or_default(), id)
        } else {
            let executable = fs::read_link(dir.join("exe")).unwrap_or_default();
            let cwd = fs::read_link(dir.join("cwd")).unwrap_or_default();
            application(&command, &executable, &cwd, game)
        };
        candidates.push((pid, parent, birth, command, matched));
    }
    // Close the launch lineage without crossing PID reuse or a Steam client.
    loop {
        let parents: std::collections::HashMap<_, _> = candidates
            .iter()
            .filter(|p| p.4)
            .map(|p| (p.0, p.2))
            .collect();
        let mut changed = false;
        for process in &mut candidates {
            if !process.4
                && parents
                    .get(&process.1)
                    .is_some_and(|birth| *birth <= process.2)
            {
                process.4 = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    candidates
        .into_iter()
        .filter(|p| p.4)
        .map(|p| {
            let dir = root.join(p.0.to_string());
            let own = application(
                &p.3,
                &fs::read_link(dir.join("exe")).unwrap_or_default(),
                &fs::read_link(dir.join("cwd")).unwrap_or_default(),
                game,
            );
            (p.0, p.2, own)
        })
        .collect()
}

// An empty snapshot before the first launch process is not proof of a stop.
fn cancellation_settled(processes_empty: bool, observed_launch: bool, quiet: Duration) -> bool {
    processes_empty && observed_launch && quiet >= Duration::from_secs(3)
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
    watch(path, Some(id), "Steam", open)
}
/// A game of another launcher (Heroic, Lutris, Bottles…), followed by its install folder.
pub fn start_external(
    path: String,
    launcher: &str,
    open: impl FnOnce() -> Result<std::process::Child, String>,
) -> Result<ExecutionStatus, String> {
    watch(path, None, launcher, open)
}
fn watch(
    path: String,
    id: Option<String>,
    launcher: &str,
    open: impl FnOnce() -> Result<std::process::Child, String>,
) -> Result<ExecutionStatus, String> {
    let launcher = launcher.to_string();
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
        let mut observed_launch = false;
        let mut last_process_seen = None;
        loop {
            if !opener_done {
                match child.try_wait() {
                    Ok(Some(exit)) => {
                        opener_done = true;
                        if !exit.success() && !session.cancel.load(Ordering::Acquire) {
                            failure =
                                Some(format!("{launcher} não aceitou a solicitação de abertura."));
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
            let processes = snapshot(Path::new("/proc"), id.as_deref(), &game);
            if !processes.is_empty() {
                observed_launch = true;
                last_process_seen = Some(Instant::now());
            }
            let cancelled = session.cancel.load(Ordering::Acquire);
            if cancelled {
                let since = stopping.get_or_insert_with(Instant::now);
                for (pid, birth, _) in &processes {
                    let dir = Path::new("/proc").join(pid.to_string());
                    if process_identity(&dir)
                        .is_some_and(|(_, current_birth)| current_birth == *birth)
                        && fs::read(dir.join("cmdline")).is_ok_and(|command| !client(&command))
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
                let quiet = last_process_seen
                    .map(|last| last.max(*since).elapsed())
                    .unwrap_or_default();
                if cancellation_settled(processes.is_empty(), observed_launch, quiet) {
                    break;
                }
                if since.elapsed() >= Duration::from_secs(180) {
                    failure = Some("Não foi possível confirmar o cancelamento na Steam. Cancele a preparação pelo cliente Steam antes de tentar novamente.".into());
                    break;
                }
            } else if processes.iter().any(|p| p.2) {
                playing.get_or_insert_with(|| (std::time::SystemTime::now(), Instant::now()));
                last_seen = Instant::now();
            } else if playing.is_some() && last_seen.elapsed() > Duration::from_secs(3) {
                break;
            } else if playing.is_none() && began.elapsed() > Duration::from_secs(180) {
                failure = Some(format!("Nenhum processo do jogo foi detectado. Confira o {launcher} e tente novamente."));
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
    fn cancellation_waits_for_a_delayed_launch_instead_of_reporting_success() {
        assert!(!cancellation_settled(true, false, Duration::from_secs(10)));
        assert!(!cancellation_settled(false, true, Duration::from_secs(10)));
        assert!(!cancellation_settled(true, true, Duration::from_secs(2)));
        assert!(cancellation_settled(true, true, Duration::from_secs(3)));
    }
    #[test]
    fn preparation_and_token_stripped_children_are_scoped_to_the_app() {
        let root =
            std::env::temp_dir().join(format!("peligames-launch-proc-{}", uuid::Uuid::new_v4()));
        let process = |pid: i32, parent: i32, birth: u64, command: &[u8], env: &[u8]| {
            let dir = root.join(pid.to_string());
            fs::create_dir_all(&dir).unwrap();
            let mut fields = vec!["0".to_string(); 20];
            fields[0] = "S".into();
            fields[1] = parent.to_string();
            fields[19] = birth.to_string();
            fs::write(
                dir.join("stat"),
                format!("{pid} (test) {}", fields.join(" ")),
            )
            .unwrap();
            fs::write(dir.join("cmdline"), command).unwrap();
            fs::write(dir.join("environ"), env).unwrap();
        };
        process(1, 0, 1, b"steam\0", b"SteamAppId=123\0");
        process(
            2,
            1,
            2,
            b"reaper\0SteamLaunch\0AppId=123\0--\0proton\0",
            b"",
        );
        process(3, 2, 3, b"python3\0proton\0", b"");
        process(4, 3, 4, b"wineboot.exe\0", b"");
        process(5, 1, 5, b"reaper\0SteamLaunch\0AppId=1234\0", b"");
        process(6, 1, 6, b"python3\0", b"STEAM_COMPAT_APP_ID=123\0");
        process(7, 2, 7, b"steamwebhelper\0", b"SteamAppId=123\0");
        let mut found: Vec<_> = snapshot(&root, Some("123"), Path::new("/games/example"))
            .into_iter()
            .map(|p| p.0)
            .collect();
        found.sort();
        assert_eq!(found, [2, 3, 4, 6]);
        assert!(!launch_marker(b"reaper\0SteamLaunch\0AppId=1234\0", "123"));
        assert!(!launch_marker(b"game\0--argument=AppId=123\0", "123"));
        fs::remove_dir_all(root).unwrap();
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
