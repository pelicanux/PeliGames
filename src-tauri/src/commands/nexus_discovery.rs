//! Game definitions identify local installations independently of their title/store.
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use super::nexus_modules::{registry, Definition};
fn file_header(path: &Path, expected: &[u8]) -> bool {
    let mut header = vec![0; expected.len()];
    fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .is_ok()
        && header == expected
}
/// Resolves `relative` below `root`, ignoring letter case only when the exact
/// path does not exist (module paths come from Windows, which ignores case).
/// Each segment must match exactly one entry; the result stays inside `root`.
pub(super) fn locate(root: &Path, relative: &str) -> Option<PathBuf> {
    let exact = root.join(relative);
    let located = if exact.exists() { exact } else {
        let mut current = root.to_path_buf();
        for part in relative.split('/').filter(|part| !part.is_empty()) {
            let mut found = fs::read_dir(&current).ok()?.flatten().take(20_000)
                .filter(|entry| entry.file_name().to_string_lossy().eq_ignore_ascii_case(part));
            let entry = found.next()?;
            if found.next().is_some() { return None; }
            current = entry.path();
        }
        current
    };
    let root = fs::canonicalize(root).ok()?;
    fs::canonicalize(&located).ok().filter(|path| path.starts_with(&root)).map(|_| located)
}

/// `locate`, returned relative to `root` with the on-disk spelling.
pub(super) fn locate_relative(root: &Path, relative: &str) -> Option<String> {
    locate(root, relative)?.strip_prefix(root).ok().map(|path| path.to_string_lossy().into_owned())
}

/// Steam App ID of an installation in `<library>/steamapps/common/<dir>`, read
/// from the appmanifest whose `installdir` is that folder.
pub(super) fn steam_app_id(root: &Path) -> Option<String> {
    let common = root.parent().filter(|p| p.file_name().is_some_and(|n| n == "common"))?;
    let steamapps = common.parent()?;
    let folder = root.file_name()?.to_string_lossy().into_owned();
    fs::read_dir(steamapps).ok()?.flatten().take(5000)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "acf"))
        .find_map(|path| super::scanner::parse_acf_file(&path).filter(|(_, _, installdir)| *installdir == folder).map(|(id, _, _)| id))
}

/// Proton prefix of a Steam game: the library owning the installation first,
/// then every other library (Steam can keep compatdata in another library).
pub(super) fn steam_prefix(directory: &Path, app_id: &str, libraries: &[PathBuf]) -> Option<PathBuf> {
    if app_id.is_empty() || !app_id.bytes().all(|b| b.is_ascii_digit()) { return None; }
    let owner = directory.parent().and_then(Path::parent).map(Path::to_path_buf);
    owner.into_iter().chain(libraries.iter().map(|library| library.join("steamapps")))
        .map(|steamapps| steamapps.join("compatdata").join(app_id))
        .find(|prefix| prefix.join("pfx/system.reg").is_file())
}

fn matches(root: &Path, definition: &Definition) -> bool {
    definition.required.iter().all(|relative| locate(root, relative).is_some_and(|file| file.is_file()))
        && locate(root, &definition.executable).is_some_and(|exe| file_header(&exe, &definition.header))
}

pub(super) fn at(root: &Path) -> Option<Definition> {
    let mut found: Vec<_> = registry().definitions.into_iter().filter(|definition| matches(root, definition)).collect();
    if found.len() > 1 {
        // Editions sharing executables are told apart by the Steam ID first.
        if let Some(id) = steam_app_id(root) {
            let mut by_id: Vec<_> = found.iter().filter(|definition| definition.steam_id.as_deref() == Some(id.as_str())).cloned().collect();
            if by_id.len() == 1 { return by_id.pop().map(|definition| definition.for_store(root)); }
        }
        // A derived game (Enderal) has its base game's identity plus its own files:
        // the match whose identity strictly contains every other one wins.
        let keys: Vec<_> = found.iter().map(identity).collect();
        found = found.into_iter().enumerate()
            .filter(|(i, _)| keys.iter().enumerate().all(|(j, other)| *i == j || (other.is_subset(&keys[*i]) && other != &keys[*i])))
            .map(|(_, definition)| definition).collect();
    }
    // GOG/Epic editions keep their settings in other folders.
    if found.len() == 1 { found.pop().map(|definition| definition.for_store(root)) } else { None }
}
/// Files that identify a module's installation, ignoring letter case.
pub(super) fn identity(definition: &Definition) -> std::collections::BTreeSet<String> {
    definition.required.iter().chain(std::iter::once(&definition.executable)).map(|path| path.to_ascii_lowercase()).collect()
}
pub(super) fn resolve(source: &Path) -> Result<(PathBuf, Option<Definition>), String> {
    let path = fs::canonicalize(source).map_err(|error| format!("Pasta do jogo: {error}"))?;
    let initial = if path.is_file() {
        path.parent().ok_or("Pasta inválida.")?.to_path_buf()
    } else {
        path
    };
    if !initial.is_dir() {
        return Err("Escolha uma pasta existente.".into());
    }
    // Check the selected directory and nearby parents, never guess from its name.
    for root in initial.ancestors().take(6) {
        if let Some(definition) = at(root) {
            return Ok((root.to_path_buf(), Some(definition)));
        }
    }
    Ok((initial, None))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locate_ignores_case_only_when_unique_and_inside_root() {
        let root = std::env::temp_dir().join(format!("locate-{}", uuid::Uuid::new_v4()));
        put(&root, "SB/Binaries/Win64/SB-Win64-Shipping.exe", b"MZ");
        assert_eq!(locate(&root, "sb/binaries/win64/sb-win64-shipping.exe"), Some(root.join("SB/Binaries/Win64/SB-Win64-Shipping.exe")));
        assert_eq!(locate_relative(&root, "sb/BINARIES/win64/SB-Win64-Shipping.EXE").as_deref(), Some("SB/Binaries/Win64/SB-Win64-Shipping.exe"));
        assert_eq!(locate(&root, "SB/missing.exe"), None);
        put(&root, "sb/other.txt", b"x");
        assert_eq!(locate(&root, "Sb/Binaries/Win64/SB-Win64-Shipping.exe"), None, "duas grafias: ambíguo");
        assert!(locate(&root, "SB/Binaries/Win64/SB-Win64-Shipping.exe").is_some(), "o caminho exato continua valendo");
        #[cfg(unix)] {
            let outside = std::env::temp_dir().join(format!("locate-outside-{}", uuid::Uuid::new_v4()));
            put(&outside, "Game.exe", b"MZ");
            std::os::unix::fs::symlink(outside.join("Game.exe"), root.join("game.exe")).unwrap();
            assert_eq!(locate(&root, "GAME.EXE"), None, "link para fora da pasta");
            fs::remove_dir_all(outside).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn steam_manifest_identifies_app_and_prefix_across_libraries() {
        let base = std::env::temp_dir().join(format!("steam-libs-{}", uuid::Uuid::new_v4()));
        let game = base.join("lib1/steamapps/common/My Game");
        put(&game, "Game.exe", b"MZ");
        put(&base, "lib1/steamapps/appmanifest_123.acf", b"\"AppState\"\n{\n\t\"appid\"\t\t\"123\"\n\t\"name\"\t\t\"My Game\"\n\t\"installdir\"\t\t\"My Game\"\n}\n");
        put(&base, "lib1/steamapps/appmanifest_999.acf", b"\"AppState\"\n{\n\t\"appid\"\t\t\"999\"\n\t\"name\"\t\t\"Other\"\n\t\"installdir\"\t\t\"Other\"\n}\n");
        assert_eq!(steam_app_id(&game).as_deref(), Some("123"));
        assert_eq!(steam_app_id(&base), None);
        let libraries = vec![base.join("lib1"), base.join("lib2")];
        assert_eq!(steam_prefix(&game, "123", &libraries), None);
        put(&base, "lib2/steamapps/compatdata/123/pfx/system.reg", b"WINE");
        assert_eq!(steam_prefix(&game, "123", &libraries), Some(base.join("lib2/steamapps/compatdata/123")));
        put(&base, "lib1/steamapps/compatdata/123/pfx/system.reg", b"WINE");
        assert_eq!(steam_prefix(&game, "123", &libraries), Some(base.join("lib1/steamapps/compatdata/123")), "biblioteca dona primeiro");
        assert_eq!(steam_prefix(&game, "../123", &libraries), None);
        fs::remove_dir_all(base).unwrap();
    }
    fn put(root: &Path, path: &str, bytes: &[u8]) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }
    #[test]
    fn crimson_recognizes_nested_executable_and_rejects_wrong_header_or_external_link() {
        let root = std::env::temp_dir().join(format!("crimson-discovery-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        assert!(at(&root).is_none());
        put(&root,"bin64/CrimsonDesert.exe",b"not PE");
        assert!(at(&root).is_none());
        put(&root,"bin64/CrimsonDesert.exe",b"MZfixture");
        let (resolved,module) = resolve(&root.join("bin64/CrimsonDesert.exe")).unwrap();
        assert_eq!(resolved,root.canonicalize().unwrap());
        assert_eq!(module.unwrap().domain,"crimsondesert");
        #[cfg(unix)] {
            let outside = root.with_extension("exe"); fs::write(&outside,b"MZfixture").unwrap();
            fs::remove_file(root.join("bin64/CrimsonDesert.exe")).unwrap();
            std::os::unix::fs::symlink(&outside,root.join("bin64/CrimsonDesert.exe")).unwrap();
            assert!(at(&root).is_none()); fs::remove_file(outside).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn repo_requires_a_real_windows_executable_and_resolves_its_folder() {
        let root = std::env::temp_dir().join(format!("peligames-repo-discovery-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        assert!(at(&root).is_none());
        put(&root, "REPO.exe", b"invalid");
        assert!(at(&root).is_none());
        put(&root, "REPO.exe", b"MZfixture");
        let (resolved, module) = resolve(&root.join("REPO.exe")).unwrap();
        assert_eq!(resolved, fs::canonicalize(&root).unwrap());
        let module = module.unwrap();
        assert_eq!(module.domain, "repo");
        assert_eq!(module.platform, "proton");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn validates_files_and_resolves_nested_executable_to_game_root() {
        let root = std::env::temp_dir().join(format!("peligames-discovery-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        put(&root, "Palworld.exe", b"MZfixture");
        assert!(at(&root).is_none());
        put(&root, "Pal/Binaries/Win64/Palworld-Win64-Shipping.exe", b"MZfixture");
        assert_eq!(at(&root).unwrap().domain, "palworld");
        let (resolved, found) = resolve(&root.join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe")).unwrap();
        assert_eq!(resolved, fs::canonicalize(&root).unwrap());
        assert_eq!(found.unwrap().platform, "proton");
        put(&root, "Pal/Binaries/Win64/Palworld-Win64-Shipping.exe", b"not an executable");
        assert!(at(&root).is_none());
        put(&root, "valheim.x86_64", b"\x7fELFfixture");
        assert!(at(&root).is_none());
        put(&root, "valheim_Data/globalgamemanagers", b"fixture");
        assert_eq!(at(&root).unwrap().domain, "valheim");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn enderal_special_edition_is_told_apart_from_skyrim_special_edition() {
        let library = std::env::temp_dir().join(format!("peligames-enderal-{}", uuid::Uuid::new_v4()));
        let common = library.join("steamapps/common");
        let (enderal, skyrim) = (common.join("Enderal Special Edition"), common.join("Skyrim Special Edition"));
        for root in [&enderal, &skyrim] { put(root, "SkyrimSE.exe", b"MZfixture"); }
        put(&enderal, "Enderal Launcher.exe", b"MZfixture");
        assert_eq!(at(&skyrim).unwrap().name, "Skyrim Special Edition");
        // Both modules match the Enderal folder; its launcher makes it the more specific one.
        assert_eq!(at(&enderal).unwrap().domain, "enderalspecialedition");
        put(&library, "steamapps/appmanifest_976620.acf", b"\"AppState\"\n{\n\t\"appid\"\t\t\"976620\"\n\t\"name\"\t\t\"Enderal: Forgotten Stories (Special Edition)\"\n\t\"installdir\"\t\t\"Enderal Special Edition\"\n}\n");
        assert_eq!(at(&enderal).unwrap().domain, "enderalspecialedition");
        // A manifest naming the base game wins over the file layout.
        fs::remove_file(library.join("steamapps/appmanifest_976620.acf")).unwrap();
        put(&library, "steamapps/appmanifest_489830.acf", b"\"AppState\"\n{\n\t\"appid\"\t\t\"489830\"\n\t\"name\"\t\t\"Skyrim Special Edition\"\n\t\"installdir\"\t\t\"Enderal Special Edition\"\n}\n");
        assert_eq!(at(&enderal).unwrap().domain, "skyrimspecialedition");
        fs::remove_dir_all(library).unwrap();
    }
    #[test]
    fn store_editions_use_their_own_settings_folders() {
        let root = std::env::temp_dir().join(format!("peligames-store-{}", uuid::Uuid::new_v4()));
        put(&root, "SkyrimSE.exe", b"MZfixture");
        let steam = at(&root).unwrap();
        assert_eq!(steam.activation.as_ref().unwrap().path, "@local/Skyrim Special Edition/plugins.txt");
        put(&root, "Galaxy64.dll", b"MZ");
        let gog = at(&root).unwrap();
        assert_eq!(gog.activation.as_ref().unwrap().path, "@local/Skyrim Special Edition GOG/plugins.txt");
        assert!(gog.config_files.contains(&"@local/Skyrim Special Edition GOG/plugins.txt".to_string()));
        assert!(gog.allowed("@local/Skyrim Special Edition GOG/plugins.txt") && !gog.allowed("@local/Skyrim Special Edition/plugins.txt"));
        assert!(gog.ini_settings.iter().all(|s| s.file == "@documents/My Games/Skyrim Special Edition GOG/Skyrim.ini"));
        assert_eq!(gog.script_extender.unwrap().logs, ["@documents/My Games/Skyrim Special Edition GOG/SKSE"]);
        // GOG wins over Epic when both markers exist, as in libloadorder.
        put(&root, "EOSSDK-Win64-Shipping.dll", b"MZ");
        assert_eq!(at(&root).unwrap().activation.unwrap().path, "@local/Skyrim Special Edition GOG/plugins.txt");
        fs::remove_file(root.join("Galaxy64.dll")).unwrap();
        assert_eq!(at(&root).unwrap().activation.unwrap().path, "@local/Skyrim Special Edition EPIC/plugins.txt");
        fs::remove_dir_all(root).unwrap();
    }
}
