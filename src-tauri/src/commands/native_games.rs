//! Detect the installed build, never merely whether the store offers Linux support.
use std::{fs, io::Read, path::Path};

pub(super) fn executable_is_native(path: &Path) -> bool {
    let mut header = [0; 4];
    fs::File::open(path).and_then(|mut file| file.read_exact(&mut header)).is_ok() && header == *b"\x7fELF"
}

pub(super) fn installed_native(directory: &Path) -> bool {
    let mut linux = false;
    // A bounded header-only fallback for launchers without platform metadata.
    // Do not mistake bundled libraries or crash reporters for the game's executable.
    for entry in walkdir::WalkDir::new(directory).max_depth(4).follow_links(false).into_iter()
        .filter_entry(|e| !e.file_type().is_dir() || !matches!(e.file_name().to_str(), Some("node_modules" | "redist" | "_CommonRedist" | "compatdata" | "pfx" | "SteamLinuxRuntime")))
        .take(2000).filter_map(Result::ok) {
        if !entry.file_type().is_file() { continue; }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if ["crash", "unins", "steam", "helper", "reporter", "setup", "installer"].iter().any(|word| name.contains(word)) { continue; }
        let extension = entry.path().extension().and_then(|v| v.to_str()).unwrap_or("").to_ascii_lowercase();
        if extension == "exe" {
            let mut header = [0; 2];
            if fs::File::open(entry.path()).and_then(|mut f| f.read_exact(&mut header)).is_ok() && header == *b"MZ" { return false; }
        } else if ["", "x86", "x86_64", "bin"].contains(&extension.as_str()) && executable_is_native(entry.path()) {
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt;
                if entry.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0) { linux = true; }
            }
        }
    }
    linux
}

pub(super) fn steam_native(directory: &Path, id: &str) -> bool {
    if !id.bytes().all(|b| b.is_ascii_digit()) || id.is_empty() { return false; }
    let steamapps = directory.ancestors().find(|p| p.file_name().is_some_and(|v| v == "steamapps"));
    if let Some(text) = steamapps.and_then(|p| fs::read_to_string(p.join(format!("appmanifest_{id}.acf"))).ok()) {
        for line in text.lines() {
            let fields: Vec<_> = line.split('"').collect();
            if fields.get(1) == Some(&"platform_override_dest") {
                match fields.get(3).copied() { Some("windows") => return false, Some("linux") => return true, _ => {} }
            }
        }
    }
    installed_native(directory)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_build_and_steam_override_take_precedence_over_bundled_helpers() {
        let root = std::env::temp_dir().join(format!("peligames-native-{}", uuid::Uuid::new_v4()));
        let apps = root.join("steamapps"); let game = apps.join("common/Game"); fs::create_dir_all(&game).unwrap();
        fs::write(game.join("game.x86_64"), b"\x7fELFfixture").unwrap();
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(game.join("game.x86_64"), fs::Permissions::from_mode(0o755)).unwrap(); }
        assert!(steam_native(&game, "42"));
        fs::write(apps.join("appmanifest_42.acf"), "\"platform_override_dest\" \"windows\"").unwrap();
        assert!(!steam_native(&game, "42"));
        fs::remove_file(apps.join("appmanifest_42.acf")).unwrap();
        fs::write(game.join("game.exe"), b"MZfixture").unwrap(); assert!(!installed_native(&game));
        fs::remove_file(game.join("game.x86_64")).unwrap(); assert!(!installed_native(&game));
        fs::remove_file(game.join("game.exe")).unwrap(); assert!(!installed_native(&game));
        fs::write(game.join("steamhelper"), b"\x7fELFfixture").unwrap(); assert!(!installed_native(&game));
        fs::remove_dir_all(root).unwrap();
    }
}
