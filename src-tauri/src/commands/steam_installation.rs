//! Identify Steam by its installation, including libraries on other disks.
use std::{
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
    // An unreadable process inventory must never authorize live VDF writes.
    super::steam_processes::clients(Path::new("/proc"), flatpak).map(|clients| !clients.is_empty()).unwrap_or(true)
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
}
