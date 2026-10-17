//! Identify Steam by its installation, including libraries on other disks.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn roots(home: &Path, flatpak: bool) -> Vec<PathBuf> {
    let paths: &[&str] = if flatpak {
        &[
            ".var/app/com.valvesoftware.Steam/.local/share/Steam",
            ".var/app/com.valvesoftware.Steam/data/Steam",
        ]
    } else {
        &[".local/share/Steam", ".steam/steam", ".steam/root"]
    };
    paths.iter().map(|p| home.join(p)).collect()
}

pub(super) fn command(flatpak: bool, proton: Option<&Path>) -> Command {
    if !flatpak {
        return Command::new("steam");
    }
    let mut command = Command::new("flatpak");
    command.arg("run");
    // Access is limited to this instance, with no persistent Flatpak override.
    if let Some(proton) = proton {
        command.arg(format!("--filesystem={}:ro", proton.display()));
    }
    command.arg("com.valvesoftware.Steam");
    command
}

pub(super) fn running(flatpak: bool) -> bool {
    running_in(Path::new("/proc"), flatpak)
}
fn running_in(proc_root: &Path, flatpak: bool) -> bool {
    let Ok(entries) = fs::read_dir(proc_root) else {
        return false;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
        })
        .any(|entry| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if !entry
                    .metadata()
                    .is_ok_and(|m| m.uid() == unsafe { libc::getuid() })
                {
                    return false;
                }
            }
            let path = entry.path();
            let executable = fs::read_link(path.join("exe")).ok();
            let args = fs::read(path.join("cmdline")).unwrap_or_default();
            let steam = executable
                .as_ref()
                .is_some_and(|p| p.file_name().is_some_and(|s| s == "steam"))
                || args
                    .split(|b| *b == 0)
                    .take(2)
                    .filter_map(|arg| std::str::from_utf8(arg).ok())
                    .any(|arg| Path::new(arg).file_name().is_some_and(|s| s == "steam"));
            if !steam {
                return false;
            }
            match fs::read_to_string(path.join("root/.flatpak-info")) {
                Ok(info) => {
                    flatpak
                        && info
                            .lines()
                            .any(|line| line.trim() == "name=com.valvesoftware.Steam")
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => !flatpak,
                Err(_) => {
                    // If procfs prevents inspection, conservatively block writes.
                    let env = fs::read(path.join("environ")).ok();
                    match env.as_ref().and_then(|env| {
                        env.split(|b| *b == 0)
                            .find(|v| v.starts_with(b"FLATPAK_ID="))
                    }) {
                        Some(id) => flatpak && id == b"FLATPAK_ID=com.valvesoftware.Steam",
                        None => true,
                    }
                }
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_target_only_the_selected_installation() {
        let native = command(false, Some(Path::new("/runner with spaces")));
        assert_eq!(native.get_program(), "steam");
        assert_eq!(native.get_args().count(), 0);
        let sandbox = command(true, Some(Path::new("/runner with spaces")));
        assert_eq!(sandbox.get_program(), "flatpak");
        assert_eq!(
            sandbox.get_args().collect::<Vec<_>>(),
            [
                "run",
                "--filesystem=/runner with spaces:ro",
                "com.valvesoftware.Steam"
            ]
        );
    }
    #[test]
    fn processes_are_scoped_even_when_both_installations_are_running() {
        let proc =
            std::env::temp_dir().join(format!("peligames-steam-proc-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(proc.join("1/root")).unwrap();
        fs::write(proc.join("1/cmdline"), b"/usr/bin/steam\0").unwrap();
        assert!(running_in(&proc, false));
        assert!(!running_in(&proc, true));
        fs::create_dir_all(proc.join("2/root")).unwrap();
        fs::write(proc.join("2/cmdline"), b"/app/extra/steam\0").unwrap();
        fs::write(
            proc.join("2/root/.flatpak-info"),
            "[Application]\nname=com.valvesoftware.Steam\n",
        )
        .unwrap();
        assert!(running_in(&proc, false));
        assert!(running_in(&proc, true));
        fs::remove_dir_all(proc.join("1")).unwrap();
        assert!(!running_in(&proc, false));
        assert!(running_in(&proc, true));
        fs::remove_dir_all(proc).unwrap();
    }
}
