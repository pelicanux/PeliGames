//! Tie a managed capture to the game process, not to Wine/Steam helpers.
use std::{
    fs,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub(super) fn identity(root: &Path, pid: u32) -> Option<u64> {
    let path = root.join(pid.to_string());
    if fs::metadata(&path).ok()?.uid() != unsafe { libc::geteuid() } {
        return None;
    }
    let stat = fs::read_to_string(path.join("stat")).ok()?;
    let fields: Vec<_> = stat.rsplit_once(") ")?.1.split_whitespace().collect();
    if matches!(*fields.first()?, "Z" | "X") {
        return None;
    }
    fields.get(19)?.parse().ok()
}
struct Process {
    pid: u32,
    started: u64,
    fd: Option<OwnedFd>,
}
impl Process {
    fn open(root: &Path, pid: u32) -> Option<Self> {
        if pid <= 1 {
            return None;
        }
        let started = identity(root, pid)?;
        let fd = if root == Path::new("/proc") {
            let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) } as i32;
            (raw >= 0).then(|| unsafe { OwnedFd::from_raw_fd(raw) })
        } else {
            None
        };
        // A PID may have exited/recycled between reading stat and opening pidfd.
        if identity(root, pid) != Some(started) {
            return None;
        }
        Some(Self { pid, started, fd })
    }
    fn alive(&self, root: &Path) -> bool {
        identity(root, self.pid) == Some(self.started)
    }
}
#[derive(Default)]
struct Target {
    directory: PathBuf,
    executable: PathBuf,
    steam: String,
}
fn absolute(value: &str, cwd: &Path) -> PathBuf {
    let value = value.replace('\\', "/");
    let value = value
        .strip_prefix("Z:")
        .or_else(|| value.strip_prefix("z:"))
        .unwrap_or(&value);
    let path = PathBuf::from(value);
    let path = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    path.canonicalize().unwrap_or(path)
}
impl Target {
    fn new(directory: &str, executable: &str, steam: &str) -> Self {
        let cwd = Path::new(directory);
        Self {
            directory: if directory.is_empty() {
                PathBuf::new()
            } else {
                absolute(directory, Path::new("/"))
            },
            executable: if executable.is_empty() {
                PathBuf::new()
            } else {
                absolute(executable, cwd)
            },
            steam: if !steam.is_empty() && steam != "0" && steam.bytes().all(|v| v.is_ascii_digit())
            {
                steam.into()
            } else {
                String::new()
            },
        }
    }
    fn matches(&self, root: &Path, pid: u32) -> bool {
        let path = root.join(pid.to_string());
        let args = fs::read(path.join("cmdline")).unwrap_or_default();
        let Some(program) = args.split(|b| *b == 0).next().filter(|v| !v.is_empty()) else {
            return false;
        };
        let program = String::from_utf8_lossy(program).replace('\\', "/");
        let name = program
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(
            name.as_str(),
            "wine"
                | "wine64"
                | "wine-preloader"
                | "wine64-preloader"
                | "wineserver"
                | "services.exe"
                | "explorer.exe"
                | "wineboot.exe"
                | "winedevice.exe"
                | "rpcss.exe"
                | "plugplay.exe"
                | "conhost.exe"
                | "start.exe"
        ) {
            return false;
        }
        let env = fs::read(path.join("environ")).unwrap_or_default();
        let ids: Vec<_> = env
            .split(|v| *v == 0)
            .filter_map(|v| std::str::from_utf8(v).ok())
            .filter_map(|v| {
                v.strip_prefix("SteamAppId=")
                    .or_else(|| v.strip_prefix("SteamGameId="))
            })
            .filter(|v| *v != "0")
            .collect();
        if !self.steam.is_empty() && !ids.is_empty() && ids.iter().any(|id| *id != self.steam) {
            return false;
        }
        // Wine replaces argv[0] with the actual PE executable after startup.
        // Do not match wrappers just because a later argument contains its path.
        if !self.steam.is_empty() && !ids.is_empty() && name.ends_with(".exe") {
            return true;
        }
        let cwd = fs::read_link(path.join("cwd")).unwrap_or_default();
        let prefix = env
            .split(|v| *v == 0)
            .filter_map(|v| std::str::from_utf8(v).ok())
            .find_map(|v| v.strip_prefix("WINEPREFIX="));
        let program = if program.as_bytes().get(1) == Some(&b':')
            && !program.starts_with("Z:/")
            && !program.starts_with("z:/")
        {
            // Epic/Lutris and managed Wine games need no Steam App ID. Resolve
            // their Windows drive through the prefix's actual drive mapping.
            prefix.map(|prefix| {
                let drive = program[..1].to_ascii_lowercase();
                let base = if drive == "c" {
                    Path::new(prefix).join("drive_c")
                } else {
                    Path::new(prefix)
                        .join("dosdevices")
                        .join(format!("{drive}:"))
                };
                let path = base.join(program[2..].trim_start_matches('/'));
                path.canonicalize().unwrap_or(path)
            })
        } else {
            (program.starts_with('/')
                || program.starts_with("Z:/")
                || program.starts_with("z:/")
                || program.starts_with("./"))
            .then(|| absolute(&program, &cwd))
        };
        let exe = fs::read_link(path.join("exe")).ok();
        [program, exe].into_iter().flatten().any(|p| {
            (!self.executable.as_os_str().is_empty() && p == self.executable)
                || (self.directory.components().count() > 2 && p.starts_with(&self.directory))
        })
    }
}

pub(super) fn matches(
    root: &Path,
    pid: u32,
    directory: &str,
    executable: &str,
    steam: &str,
) -> bool {
    identity(root, pid).is_some() && Target::new(directory, executable, steam).matches(root, pid)
}

pub(super) struct Game {
    root: PathBuf,
    target: Target,
    processes: Vec<Process>,
    bound: bool,
    next_scan: Instant,
}
impl Game {
    pub fn new(directory: &str, executable: &str, steam: &str, peer: Option<u32>) -> Self {
        let mut game = Self {
            root: "/proc".into(),
            target: Target::new(directory, executable, steam),
            processes: Vec::new(),
            bound: false,
            next_scan: Instant::now(),
        };
        // SO_PEERCRED gives the real host PID, including inside Steam's namespace.
        // An overlay request is tied to that exact process, not to its helpers.
        if let Some(pid) = peer.filter(|pid| *pid != std::process::id()) {
            // A request from an already-exited game must be canceled, not
            // rebound to a different process or left waiting for another run.
            game.bound = true;
            if let Some(process) = Process::open(&game.root, pid) {
                game.processes.push(process);
            }
        }
        game.ended();
        game
    }
    pub fn ended(&mut self) -> bool {
        if self.bound {
            self.processes.retain(|p| p.alive(&self.root));
            return self.processes.is_empty();
        }
        // Starting capture in the launcher before the game is allowed. Bind when
        // it appears; absence before its first launch must not cancel capture.
        if Instant::now() < self.next_scan {
            return false;
        }
        self.next_scan = Instant::now() + Duration::from_secs(1);
        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let Some(pid) = entry.file_name().to_str().and_then(|v| v.parse().ok()) else {
                    continue;
                };
                if pid == std::process::id()
                    || identity(&self.root, pid).is_none()
                    || !self.target.matches(&self.root, pid)
                {
                    continue;
                }
                if let Some(process) = Process::open(&self.root, pid) {
                    self.processes.push(process);
                }
                if self.processes.len() == 16 {
                    break;
                }
            }
        }
        self.bound = !self.processes.is_empty();
        false
    }
    pub fn wait(&self) {
        let mut fds: Vec<_> = self
            .processes
            .iter()
            .filter_map(|p| p.fd.as_ref())
            .map(|fd| libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();
        // pidfd wakes immediately on exit. A short fallback covers old kernels.
        if fds.is_empty() || unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, 100) } < 0 {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "capture-lifetime-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn process(&self, pid: u32, program: &str, steam: &str, start: u64) {
            let p = self.0.join(pid.to_string());
            fs::create_dir_all(&p).unwrap();
            let mut fields = vec!["0".to_owned(); 20];
            fields[0] = "S".into();
            fields[19] = start.to_string();
            fs::write(
                p.join("stat"),
                format!("{pid} (name with ) spaces) {}", fields.join(" ")),
            )
            .unwrap();
            fs::write(p.join("cmdline"), format!("{program}\0")).unwrap();
            fs::write(p.join("environ"), format!("SteamAppId={steam}\0")).unwrap();
        }
        fn game(&self) -> Game {
            Game {
                root: self.0.clone(),
                target: Target {
                    steam: "123".into(),
                    ..Default::default()
                },
                processes: vec![],
                bound: false,
                next_scan: Instant::now(),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn wine_helpers_do_not_keep_a_closed_games_replay_alive() {
        let f = Fixture::new();
        f.process(501, "Z:\\games\\Example.exe", "123", 10);
        f.process(502, "C:\\windows\\system32\\services.exe", "123", 11);
        f.process(503, "Z:\\games\\Other.exe", "456", 12);
        let mut game = f.game();
        assert!(!game.ended());
        assert_eq!(game.processes.len(), 1);
        fs::remove_dir_all(f.0.join("501")).unwrap();
        assert!(game.ended());
    }
    #[test]
    fn starting_before_the_game_binds_on_its_first_launch() {
        let f = Fixture::new();
        let mut game = f.game();
        assert!(!game.ended());
        assert!(!game.bound);
        f.process(501, "/games/example/run", "123", 10);
        game.target.directory = "/games/example".into();
        game.next_scan = Instant::now();
        assert!(!game.ended());
        assert!(game.bound);
        fs::remove_dir_all(f.0.join("501")).unwrap();
        assert!(game.ended());
    }
    #[test]
    fn recycled_pid_and_zombies_are_not_treated_as_the_original_game() {
        let f = Fixture::new();
        f.process(501, "Z:\\games\\Example.exe", "123", 10);
        let mut game = f.game();
        assert!(!game.ended());
        f.process(501, "Z:\\games\\Example.exe", "123", 20);
        assert!(game.ended());
        let mut game = f.game();
        assert!(!game.ended());
        let p = f.0.join("501/stat");
        let stat = fs::read_to_string(&p).unwrap().replace(") S ", ") Z ");
        fs::write(p, stat).unwrap();
        assert!(game.ended());
    }
    #[test]
    fn executable_in_a_wrappers_arguments_does_not_bind_the_wrapper() {
        let f = Fixture::new();
        f.process(
            501,
            "/usr/bin/python3\0/proton\0/games/example/Example.exe",
            "123",
            10,
        );
        let mut game = f.game();
        game.target.directory = "/games/example".into();
        assert!(!game.ended());
        assert!(!game.bound);
    }
    #[test]
    fn wine_game_without_steam_is_bound_through_its_prefix_drive_mapping() {
        let f = Fixture::new();
        let prefix = f.0.join("prefix");
        let directory = prefix.join("drive_c/games/example");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("Example.exe"), b"fixture").unwrap();
        f.process(501, "C:\\games\\example\\Example.exe", "0", 10);
        fs::write(
            f.0.join("501/environ"),
            format!("WINEPREFIX={}\0", prefix.display()),
        )
        .unwrap();
        let mut game = f.game();
        game.target.steam.clear();
        game.target.directory = directory;
        assert!(!game.ended());
        assert!(game.bound);
        fs::remove_dir_all(f.0.join("501")).unwrap();
        assert!(game.ended());
    }
    #[test]
    fn an_overlay_request_from_an_exited_game_is_already_ended() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let peer = child.id();
        child.kill().unwrap();
        child.wait().unwrap();
        let mut game = Game::new("", "", "", Some(peer));
        assert!(game.ended());
    }
    #[test]
    fn basename_only_wrapper_is_not_assumed_to_reside_in_the_game_directory() {
        let f = Fixture::new();
        f.process(
            501,
            "python3\0/proton\0/games/example/Example.exe",
            "123",
            10,
        );
        let mut game = f.game();
        game.target.directory = "/games/example".into();
        assert!(!game.ended());
        assert!(!game.bound);
    }
    #[test]
    fn exact_overlay_peer_exit_is_observed_by_pidfd() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let mut game = Game::new("", "", "", Some(child.id()));
        assert!(!game.ended());
        assert_eq!(game.processes.len(), 1);
        child.kill().unwrap();
        child.wait().unwrap();
        game.wait();
        assert!(game.ended());
    }
}
