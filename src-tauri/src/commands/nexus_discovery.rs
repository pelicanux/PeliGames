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
pub(super) fn at(root: &Path) -> Option<Definition> {
    let matches: Vec<_> = registry().definitions
        .into_iter()
        .filter(|definition| {
            definition
                .required
                .iter()
                .all(|relative| root.join(relative).is_file()
                    && fs::canonicalize(root.join(relative)).is_ok_and(|file| file.starts_with(root)))
                && file_header(&root.join(&definition.executable), &definition.header)
        })
        .collect();
    if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
    }
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
}
