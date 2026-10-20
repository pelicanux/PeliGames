//! Per-process MangoHUD configuration, changed only on focus transitions.
//! The worker performs filesystem I/O; input callbacks only publish a state.
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Arc, OnceLock,
    },
    time::Duration,
};
const MAX_CONFIG: u64 = 256 * 1024;
struct Profile {
    _lock: fs::File,
    path: PathBuf,
    foreground: String,
    background: String,
    cap: u32,
}
struct Service {
    desired: Arc<AtomicU8>,
    wake: mpsc::SyncSender<()>,
    cap: u32,
}
static PROFILE: OnceLock<Option<Profile>> = OnceLock::new();
static SERVICE: OnceLock<Option<Service>> = OnceLock::new();
fn private_dir(path: &Path) -> Result<(), String> {
    match fs::create_dir(path) {
        Ok(()) => fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_dir()
        || m.file_type().is_symlink()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
    {
        return Err("Pasta de sessão do MangoHUD insegura".into());
    }
    Ok(())
}
fn limits(value: &str) -> Vec<u32> {
    value
        .split([',', '+'])
        .filter_map(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v <= 1000)
        .collect()
}
fn configured_limits(text: &str) -> Vec<u32> {
    text.lines()
        .filter_map(|l| l.split('#').next()?.trim().split_once('='))
        .filter(|(k, _)| k.trim() == "fps_limit")
        .last()
        .map(|(_, v)| limits(v))
        .unwrap_or_default()
}
fn background_config(foreground: &str, requested: u32) -> (String, u32) {
    let values = configured_limits(foreground);
    let cap = if requested == 0 {
        values
            .iter()
            .copied()
            .filter(|v| *v > 0)
            .min()
            .unwrap_or(30)
    } else {
        requested
    };
    let cap = values
        .first()
        .copied()
        .filter(|v| *v > 0)
        .map(|v| v.min(cap))
        .unwrap_or(cap);
    let mut text = foreground
        .lines()
        .filter(|line| {
            line.split_once('=')
                .is_none_or(|(key, _)| key.trim() != "fps_limit")
        })
        .collect::<Vec<_>>()
        .join("\n");
    text.push_str(&format!("\nfps_limit={cap}\n"));
    (text, cap)
}
fn write_config(path: &Path, text: &str) -> Result<(), String> {
    let temporary = path.with_extension("pending");
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn cleanup(root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten().take(512) {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.mode() & 0o077 != 0
        {
            continue;
        }
        let Ok(lock) = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path.join("session.lock"))
        else {
            continue;
        };
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            continue;
        }
        for name in ["MangoHud.conf", "MangoHud.pending", "session.lock"] {
            let file = path.join(name);
            if fs::symlink_metadata(&file).is_ok_and(|m| {
                m.is_file() && !m.file_type().is_symlink() && m.uid() == unsafe { libc::geteuid() }
            }) {
                let _ = fs::remove_file(file);
            }
        }
        let _ = fs::remove_dir(path);
    }
}
fn prepare() -> Result<Profile, String> {
    let requested = std::env::var("PELI_OVERLAY_BACKGROUND_FPS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v <= 1000)
        .ok_or("FPS sem foco inválido")?;
    let source = std::env::var_os("PELI_OVERLAY_HUD_SOURCE")
        .or_else(|| std::env::var_os("MANGOHUD_CONFIGFILE"))
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or("Configuração do MangoHUD ausente")?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&source)
        .map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } || meta.len() > MAX_CONFIG {
        return Err("Configuração do MangoHUD inválida".into());
    }
    let mut foreground = String::new();
    Read::take(&mut file, MAX_CONFIG + 1)
        .read_to_string(&mut foreground)
        .map_err(|e| e.to_string())?;
    if foreground.len() as u64 > MAX_CONFIG {
        return Err("Configuração do MangoHUD grande demais".into());
    }
    let overrides = std::env::var("MANGOHUD_CONFIG").unwrap_or_default();
    let saved_fps = std::env::var("PELI_OVERLAY_HUD_FPS").ok();
    let foreground_override = saved_fps.as_deref().or_else(|| {
        overrides
            .split(',')
            .filter_map(|s| s.split_once('='))
            .filter(|(k, _)| *k == "fps_limit")
            .last()
            .map(|(_, v)| v)
    });
    if let Some(value) = foreground_override {
        if !limits(value).is_empty() {
            foreground.push_str(&format!("\nfps_limit={}\n", value.replace('+', ",")));
        }
    }
    let root = source
        .parent()
        .ok_or("Pasta MangoHUD ausente")?
        .join("overlay-sessions");
    private_dir(&root)?;
    let root_lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join(".sessions.lock"))
        .map_err(|e| e.to_string())?;
    use std::os::fd::AsRawFd;
    // Optional monitoring must never delay launching a game behind another
    // process preparing its session (including Steam runtime helpers).
    if unsafe { libc::flock(root_lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("Não foi possível reservar a sessão do MangoHUD".into());
    }
    cleanup(&root);
    let name: String = source
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .take(80)
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let namespace = fs::metadata("/proc/self/ns/pid")
        .map(|m| m.ino())
        .unwrap_or(0);
    let session = root.join(format!("{name}--{}-{namespace}", std::process::id()));
    private_dir(&session)?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(session.join("session.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("Sessão do MangoHUD já está ativa".into());
    }
    let path = session.join("MangoHud.conf");
    write_config(&path, &foreground)?;
    // Executed by the preload constructor before the application's threads and
    // before MangoHUD is loaded as a Vulkan layer. Each exec gets its own file.
    std::env::set_var("PELI_OVERLAY_HUD_SOURCE", &source);
    std::env::set_var("PELI_OVERLAY_HUD_FPS", foreground_override.unwrap_or(""));
    std::env::set_var("MANGOHUD_CONFIGFILE", &path);
    let mut retained = overrides
        .split(',')
        .filter(|part| !part.is_empty() && *part != "read_cfg" && !part.starts_with("fps_limit="))
        .collect::<Vec<_>>();
    retained.insert(0, "read_cfg");
    std::env::set_var("MANGOHUD_CONFIG", retained.join(","));
    let (background, cap) = background_config(&foreground, requested);
    Ok(Profile {
        _lock: lock,
        path,
        foreground,
        background,
        cap,
    })
}
extern "C" fn initialize() {
    if std::env::var_os("PELI_OVERLAY_BACKGROUND_FPS").is_none()
        || std::env::var("PELI_OVERLAY_DISABLED").ok().as_deref() == Some("1")
    {
        return;
    }
    PROFILE.get_or_init(|| match prepare() {
        Ok(profile) => Some(profile),
        Err(error) => {
            eprintln!("PeliGames: FPS sem foco indisponível: {error}");
            None
        }
    });
}
#[used]
#[cfg_attr(target_os = "linux", link_section = ".init_array")]
static INITIALIZE: extern "C" fn() = initialize;
fn service() -> Option<&'static Service> {
    SERVICE
        .get_or_init(|| {
            let profile = PROFILE.get()?.as_ref()?;
            let (wake, receiver) = mpsc::sync_channel(1);
            let desired = Arc::new(AtomicU8::new(0));
            let state = desired.clone();
            std::thread::Builder::new()
                .name("peli-focus-fps".into())
                .spawn(move || {
                    let mut last = 0;
                    let mut failed = false;
                    while receiver.recv().is_ok() {
                        std::thread::sleep(Duration::from_millis(100));
                        while receiver.try_recv().is_ok() {}
                        let next = state.load(Ordering::Acquire);
                        if next == last {
                            continue;
                        }
                        let text = if next == 1 {
                            &profile.foreground
                        } else {
                            &profile.background
                        };
                        match write_config(&profile.path, text) {
                            Ok(()) => {
                                last = next;
                                failed = false;
                            }
                            Err(error) => {
                                if !failed {
                                    crate::report(&format!(
                                        "Falha ao atualizar FPS sem foco: {error}"
                                    ));
                                    failed = true;
                                }
                            }
                        }
                    }
                })
                .ok()?;
            Some(Service {
                desired,
                wake,
                cap: profile.cap,
            })
        })
        .as_ref()
}
pub(crate) fn focus(focused: bool) {
    if let Some(service) = service() {
        let next = if focused { 1 } else { 2 };
        if service.desired.swap(next, Ordering::AcqRel) != next {
            let _ = service.wake.try_send(());
        }
    }
}
pub(crate) fn status() -> String {
    match SERVICE.get().and_then(Option::as_ref) {
        Some(s) => format!("MangoHUD · sem foco: {} FPS", s.cap),
        None => "Limite sem foco não ativado nesta execução.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lowest_positive_and_foreground_settings_are_preserved() {
        let original = "cpu_stats=1\nfps_limit=0,90,30,60\ntime=1\n";
        let (background, cap) = background_config(original, 0);
        assert_eq!(cap, 30);
        assert_eq!(configured_limits(&background), vec![30]);
        assert!(background.contains("time=1"));
        assert_eq!(configured_limits(original), vec![0, 90, 30, 60]);
    }
    #[test]
    fn background_never_raises_foreground_limit() {
        assert_eq!(background_config("fps_limit=20\n", 60).1, 20);
        assert_eq!(background_config("fps_limit=0\n", 0).1, 30);
    }
    #[test]
    fn separate_sessions_restore_exact_original_bytes() {
        let root = std::env::temp_dir().join(format!(
            "peli-focus-test-{}-{}",
            std::process::id(),
            crate::nonce()
        ));
        fs::create_dir(&root).unwrap();
        let a = root.join("one.conf");
        let b = root.join("two.conf");
        let original = "fps_limit=144,60,30\ncpu_stats=1\n";
        write_config(&a, original).unwrap();
        write_config(&b, original).unwrap();
        write_config(&a, &background_config(original, 30).0).unwrap();
        assert_eq!(fs::read_to_string(&b).unwrap(), original);
        write_config(&a, original).unwrap();
        assert_eq!(fs::read_to_string(a).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn child_focus_roundtrip() {
        if std::env::var_os("PELI_FOCUS_TEST_CHILD").is_none() {
            return;
        }
        let profile = PROFILE
            .get()
            .and_then(Option::as_ref)
            .expect("preload constructor created a private profile");
        let source = PathBuf::from(std::env::var_os("PELI_OVERLAY_HUD_SOURCE").unwrap());
        assert_ne!(profile.path, source);
        assert_eq!(configured_limits(&profile.foreground), vec![90, 30]);
        focus(false);
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(
            configured_limits(&fs::read_to_string(&profile.path).unwrap()),
            vec![30]
        );
        focus(true);
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(
            fs::read_to_string(&profile.path).unwrap(),
            profile.foreground
        );
        focus(false);
        focus(true);
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(
            fs::read_to_string(&profile.path).unwrap(),
            profile.foreground
        );
        assert_eq!(
            configured_limits(&fs::read_to_string(source).unwrap()),
            vec![144, 60, 30]
        );
    }
    #[test]
    fn two_processes_have_independent_profiles_and_live_sessions_survive_cleanup() {
        let root = std::env::temp_dir().join(format!(
            "peli-focus-processes-{}-{}",
            std::process::id(),
            crate::nonce()
        ));
        fs::create_dir(&root).unwrap();
        let source = root.join("ExampleGame.conf");
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&source)
            .unwrap();
        f.write_all(b"fps_limit=144,60,30\ncpu_stats=1\n").unwrap();
        drop(f);
        let child = || {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "focus_fps::lifecycle_tests::child_focus_roundtrip",
                ])
                .env("PELI_FOCUS_TEST_CHILD", "1")
                .env("PELI_OVERLAY_BACKGROUND_FPS", "0")
                .env("PELI_OVERLAY_DISABLED", "0")
                .env("MANGOHUD_CONFIGFILE", &source)
                .env("MANGOHUD_CONFIG", "read_cfg,fps_limit=90+30,no_display=1")
                .env_remove("PELI_OVERLAY_HUD_SOURCE")
                .env_remove("PELI_OVERLAY_HUD_FPS")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        };
        let first = child();
        std::thread::sleep(Duration::from_millis(150));
        let second = child();
        for child in [first, second] {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let sessions = root.join("overlay-sessions");
        assert_eq!(
            fs::read_dir(&sessions)
                .unwrap()
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .count(),
            2
        );
        cleanup(&sessions);
        assert_eq!(
            fs::read_dir(&sessions)
                .unwrap()
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .count(),
            0
        );
        fs::remove_dir_all(root).unwrap();
    }
}
