//! Discord IPC for Proton's Discord bridge (PROTON_DISCORD_BRIDGE). The Steam
//! Runtime container shares the discord-ipc-N sockets at the top of
//! XDG_RUNTIME_DIR, not those that Flatpak or Snap clients create in their own
//! folders. With no top-level socket, link the sandboxed client's socket there.
//! The runtime directory is a per-login tmpfs: the link ends with the session
//! and is recreated at the next launch.
use std::{
    fs,
    io::ErrorKind,
    os::unix::fs::{symlink, FileTypeExt},
    path::{Path, PathBuf},
};

/// Folders, relative to XDG_RUNTIME_DIR, where sandboxed clients serve IPC.
const SANDBOXED: [&str; 6] = [
    "app/com.discordapp.Discord",
    ".flatpak/com.discordapp.Discord/xdg-run",
    "app/dev.vencord.Vesktop",
    ".flatpak/dev.vencord.Vesktop/xdg-run",
    "snap.discord",
    "snap.discord-canary",
];
const SLOTS: u8 = 10;

fn socket(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.file_type().is_socket())
}

/// The runtime directory of this login session.
pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::geteuid() })))
}

/// Read-only endpoints for native and sandboxed desktop clients.
pub fn candidates(runtime: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for folder in std::iter::once("").chain(SANDBOXED) {
        for slot in 0..SLOTS { paths.push(runtime.join(folder).join(format!("discord-ipc-{slot}"))); }
    }
    paths
}

/// Links a sandboxed client's socket as `discord-ipc-N` in `runtime` when no
/// top-level socket answers. Returns the link created, if any. Never replaces a
/// real socket or a link it did not make (one pointing outside `SANDBOXED`).
pub fn link_sandboxed(runtime: &Path) -> Result<Option<PathBuf>, String> {
    let ours = |target: &Path| SANDBOXED.iter().any(|folder| target.starts_with(runtime.join(folder)));
    let mut free = None;
    for slot in 0..SLOTS {
        let top = runtime.join(format!("discord-ipc-{slot}"));
        match fs::symlink_metadata(&top) {
            // A native client, or a link that still reaches a running client.
            Ok(_) if socket(&top) => return Ok(None),
            Ok(meta) if meta.file_type().is_symlink() => {
                // A link left by a previous session of a closed client.
                if fs::read_link(&top).is_ok_and(|target| ours(&target)) {
                    fs::remove_file(&top).map_err(|e| format!("Não foi possível renovar {}: {e}", top.display()))?;
                    free = free.or(Some(top));
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => free = free.or(Some(top)),
            Err(error) => return Err(error.to_string()),
        }
    }
    let Some(link) = free else { return Ok(None) };
    let found = SANDBOXED.iter().flat_map(|folder| (0..SLOTS).map(move |slot| runtime.join(folder).join(format!("discord-ipc-{slot}"))))
        .find(|candidate| socket(candidate));
    let Some(target) = found else { return Ok(None) };
    symlink(&target, &link).map_err(|e| format!("Não foi possível ligar o Discord em {}: {e}", link.display()))?;
    Ok(Some(link))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    fn runtime() -> PathBuf {
        let path = std::env::temp_dir().join(format!("peli-discord-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn flatpak_client_is_linked_where_the_container_looks() {
        let root = runtime();
        let flatpak = root.join("app/com.discordapp.Discord");
        fs::create_dir_all(&flatpak).unwrap();
        let _client = UnixListener::bind(flatpak.join("discord-ipc-0")).unwrap();
        let link = link_sandboxed(&root).unwrap().unwrap();
        assert_eq!(link, root.join("discord-ipc-0"));
        assert_eq!(fs::read_link(&link).unwrap(), flatpak.join("discord-ipc-0"));
        // A working link is kept, not recreated.
        assert_eq!(link_sandboxed(&root).unwrap(), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_clients_and_foreign_links_are_left_alone() {
        let root = runtime();
        fs::create_dir_all(root.join("app/com.discordapp.Discord")).unwrap();
        let _flatpak = UnixListener::bind(root.join("app/com.discordapp.Discord/discord-ipc-0")).unwrap();
        // A native client already answers: nothing to do.
        let native = UnixListener::bind(root.join("discord-ipc-0")).unwrap();
        assert_eq!(link_sandboxed(&root).unwrap(), None);
        drop(native);
        fs::remove_file(root.join("discord-ipc-0")).unwrap();
        // A link made by someone else stays, even if broken; the next free slot is used.
        symlink("/elsewhere/socket", root.join("discord-ipc-0")).unwrap();
        assert_eq!(link_sandboxed(&root).unwrap(), Some(root.join("discord-ipc-1")));
        assert_eq!(fs::read_link(root.join("discord-ipc-0")).unwrap(), PathBuf::from("/elsewhere/socket"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stale_links_are_renewed_and_nothing_is_linked_without_a_client() {
        let root = runtime();
        let flatpak = root.join("app/com.discordapp.Discord");
        fs::create_dir_all(&flatpak).unwrap();
        // Left by a previous session: the client was closed and its socket removed.
        symlink(flatpak.join("discord-ipc-3"), root.join("discord-ipc-0")).unwrap();
        assert_eq!(link_sandboxed(&root).unwrap(), None);
        assert!(fs::symlink_metadata(root.join("discord-ipc-0")).is_err(), "stale link removed");
        let _client = UnixListener::bind(flatpak.join("discord-ipc-0")).unwrap();
        assert_eq!(link_sandboxed(&root).unwrap(), Some(root.join("discord-ipc-0")));
        assert!(socket(&root.join("discord-ipc-0")));
        fs::remove_dir_all(root).unwrap();
    }
}
