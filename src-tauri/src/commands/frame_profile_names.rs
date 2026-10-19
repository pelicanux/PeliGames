//! Readable filenames, with legacy compatibility and collision protection.
use std::{
    fs,
    path::{Path, PathBuf},
};

fn hex(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn valid(id: &str) -> bool {
    if id.len() == 64 && hex(id) {
        return true;
    }
    let Some((name, digest)) = id.rsplit_once('-') else {
        return false;
    };
    !name.is_empty()
        && name.len() <= 40
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
        && (8..=64).contains(&digest.len())
        && hex(digest)
}

fn slug(name: &str) -> String {
    let mut result = String::new();
    for ch in name.chars() {
        if result.len() >= 40 {
            break;
        }
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else if !result.is_empty() && !result.ends_with('-') {
            result.push('-');
        }
    }
    let result = result.trim_end_matches('-');
    if result.is_empty() {
        "jogo".into()
    } else {
        result.into()
    }
}

pub fn allocate(profiles: &Path, name: &str, digest: &str) -> Result<PathBuf, String> {
    if digest.len() != 64 || !hex(digest) {
        return Err("Identificador de perfil inválido.".into());
    }
    fs::create_dir_all(profiles).map_err(|e| e.to_string())?;
    let name = slug(name);
    // Never infer ownership from an eight-character prefix alone.
    for length in (8..=64).step_by(4) {
        let base = profiles.join(format!("{name}-{}", &digest[..length]));
        let identity = base.with_extension("identity");
        if fs::read_to_string(&identity).is_ok_and(|value| value == digest) {
            return Ok(base);
        }
        if ["sh", "json", "toml", "launch.conf", "live.json", "identity"]
            .iter()
            .any(|ext| base.with_extension(ext).symlink_metadata().is_ok())
        {
            continue;
        }
        use std::io::Write;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(identity)
        {
            Ok(mut file) => {
                file.write_all(digest.as_bytes())
                    .map_err(|e| e.to_string())?;
                return Ok(base);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Não foi possível criar um nome exclusivo para o perfil.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_legacy_and_readable_names_but_rejects_paths() {
        assert!(valid(&"a".repeat(64)));
        assert!(valid("tomb-raider-a1b2c3d4"));
        for id in [
            "../tomb-raider-a1b2c3d4",
            "a/other-a1b2c3d4",
            "a--b-a1b2c3d4",
            "a-123",
            "a-ABCDEF12",
        ] {
            assert!(!valid(id));
        }
        assert_eq!(slug("Tomb Raider™ / 2013"), "tomb-raider-2013");
        assert_eq!(slug("遊戲"), "jogo");
        assert!(slug(&"long name ".repeat(30)).len() <= 40);
    }
    #[test]
    fn collisions_preserve_files_and_same_identity_reuses_name() {
        let dir = std::env::temp_dir().join(format!("peli-profile-names-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let first = format!("12345678{}", "a".repeat(56));
        let second = format!("12345678{}", "b".repeat(56));
        let a = allocate(&dir, "Tomb Raider", &first).unwrap();
        fs::write(a.with_extension("sh"), "original").unwrap();
        assert_eq!(a.file_name().unwrap(), "tomb-raider-12345678");
        assert_eq!(allocate(&dir, "Tomb Raider", &first).unwrap(), a);
        let b = allocate(&dir, "Tomb Raider", &second).unwrap();
        assert_eq!(b.file_name().unwrap(), "tomb-raider-12345678bbbb");
        assert_eq!(
            fs::read_to_string(a.with_extension("sh")).unwrap(),
            "original"
        );
        fs::write(dir.join("other-12345678.toml"), "untracked").unwrap();
        assert_ne!(
            allocate(&dir, "Other", &first).unwrap(),
            dir.join("other-12345678")
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
