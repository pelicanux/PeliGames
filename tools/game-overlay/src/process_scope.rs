//! Keep inherited input hooks inert in Wine's system helper processes.
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    Pending,
    Game,
    Helper,
}

fn basename(path: &[u8]) -> &[u8] {
    path.rsplit(|c| *c == b'/' || *c == b'\\')
        .next()
        .unwrap_or(path)
}
// Basename-only argv is used by some Wine versions. A full executable path
// outside Wine's Windows system directories wins over the helper basename:
// a game named steam.exe in its own directory must keep its overlay.
const HELPERS: &[&[u8]] = &[
    b"steam.exe",
    b"tabtip.exe",
    b"wineboot.exe",
    b"services.exe",
    b"winedevice.exe",
    b"plugplay.exe",
    b"rpcss.exe",
    b"explorer.exe",
    b"conhost.exe",
    b"winemenubuilder.exe",
    b"rundll32.exe",
];
fn helper(exe: &[u8]) -> bool {
    let name = basename(exe);
    if !HELPERS.iter().any(|known| name.eq_ignore_ascii_case(known)) {
        return false;
    }
    if name.len() == exe.len() {
        return true;
    }
    let parent = &exe[..exe.len() - name.len() - 1];
    [b"system32".as_slice(), b"syswow64", b"windows"]
        .iter()
        .any(|directory| basename(parent).eq_ignore_ascii_case(directory))
}
fn classify(comm: &[u8], command: &[u8]) -> Scope {
    let comm = comm.strip_suffix(b"\n").unwrap_or(comm);
    // The first PE is the current process, not a game passed as a later argument
    // to Wine's steam.exe launcher. Wine can register services before renaming comm.
    if let Some(exe) = command
        .split(|c| *c == 0)
        .find(|arg| arg.len() >= 4 && arg[arg.len() - 4..].eq_ignore_ascii_case(b".exe"))
    {
        return if helper(exe) {
            Scope::Helper
        } else {
            Scope::Game
        };
    }
    if helper(comm) {
        return Scope::Helper;
    }
    if comm.is_empty()
        || [
            b"wine".as_slice(),
            b"wine64",
            b"wine-preloader",
            b"wine64-preloader",
            b"wine64-preloade",
        ]
        .iter()
        .any(|name| comm.eq_ignore_ascii_case(name))
    {
        Scope::Pending
    } else {
        Scope::Game
    }
}

pub(crate) fn enabled() -> bool {
    static SCOPE: AtomicU8 = AtomicU8::new(0);
    match SCOPE.load(Ordering::Acquire) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    let comm = std::fs::read("/proc/self/comm").unwrap_or_default();
    let command = std::fs::read("/proc/self/cmdline").unwrap_or_default();
    match classify(&comm, &command) {
        Scope::Game => {
            SCOPE.store(1, Ordering::Release);
            true
        }
        Scope::Helper => {
            SCOPE.store(2, Ordering::Release);
            false
        }
        // Do not cache a Wine loader's identity before the PE target is known.
        Scope::Pending => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helper_is_excluded_before_and_after_wine_renames_the_process() {
        for comm in [b"wine64-preloade\n".as_slice(), b"tabtip.exe\n"] {
            assert_eq!(
                classify(
                    comm,
                    b"wine64-preloader\0C:\\windows\\system32\\TabTip.EXE\0"
                ),
                Scope::Helper
            );
        }
        assert_eq!(classify(b"tabtip.exe\n", b""), Scope::Helper);
    }
    #[test]
    fn real_game_keeps_its_hooks_even_if_a_later_argument_names_the_helper() {
        assert_eq!(
            classify(
                b"wine64-preloade\n",
                b"wine64-preloader\0Z:\\games\\Diablo IV.exe\0tabtip.exe\0"
            ),
            Scope::Game
        );
        assert_eq!(
            classify(b"Palworld-Win64-S\n", b"/games/Palworld.exe\0"),
            Scope::Game
        );
        assert_eq!(
            classify(b"native-game\n", b"/games/native-game\0"),
            Scope::Game
        );
    }
    #[test]
    fn steam_launcher_is_not_the_game_it_receives_as_an_argument() {
        assert_eq!(
            classify(
                b"steam.exe\n",
                b"c:\\windows\\system32\\steam.exe\0/mnt/games/Diablo IV.exe\0"
            ),
            Scope::Helper
        );
    }
    #[test]
    fn windows_system_helpers_are_excluded_without_excluding_same_named_games() {
        for name in HELPERS {
            let mut path = b"wine64-preloader\0C:\\WINDOWS\\SYSTEM32\\".to_vec();
            path.extend_from_slice(name);
            path.push(0);
            assert_eq!(classify(b"wine64-preloade", &path), Scope::Helper);
            assert_eq!(classify(name, b""), Scope::Helper);
            let mut game = b"Z:\\Games\\".to_vec();
            game.extend_from_slice(name);
            game.push(0);
            assert_eq!(classify(name, &game), Scope::Game);
        }
        assert_eq!(
            classify(b"explorer.exe", b"C:\\windows\\explorer.exe\0"),
            Scope::Helper
        );
        assert_eq!(
            classify(b"wine64-preloade", b"C:/windows/syswow64/steam.exe\0"),
            Scope::Helper
        );
    }
    #[test]
    fn unresolved_wine_loader_is_not_treated_as_a_game() {
        assert_eq!(
            classify(b"wine64-preloade\n", b"wine64-preloader\0"),
            Scope::Pending
        );
        assert_eq!(classify(b"", b""), Scope::Pending);
    }
}
