//! Human-readable managed filenames. Identities live in metadata, never in names.
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};
static ALLOCATION: Mutex<()> = Mutex::new(());
static TEMP: AtomicU64 = AtomicU64::new(0);
pub fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars() {
        if out.len() >= 64 {
            break;
        }
        let c = match c.to_ascii_lowercase() {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "game".into()
    } else {
        out.into()
    }
}
pub fn valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}
// Repeated gallery/status reads reuse one small index per namespace. Creating,
// removing or atomically replacing an identity invalidates its directory stamp.
// Hashing is never needed for the filename, and lookups do not rescan on every frame.
type Stamp = (u64, Option<std::time::SystemTime>);
type Index = BTreeMap<String, PathBuf>;
static INDEXES: OnceLock<Mutex<BTreeMap<PathBuf, (Stamp, Index)>>> = OnceLock::new();
pub fn find(folder: &Path, identity: &str) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(folder).ok()?;
    let stamp = (meta.ino(), meta.modified().ok());
    let indexes = INDEXES.get_or_init(Default::default);
    let cached = indexes.lock().ok().and_then(|all| {
        all.get(folder)
            .filter(|(known, _)| *known == stamp)
            .map(|(_, entries)| entries.get(identity).cloned())
    });
    if let Some(path) = cached {
        match path {
            None => return None,
            Some(path)
                if fs::symlink_metadata(path.with_extension("identity"))
                    .is_ok_and(|m| m.is_file())
                    && fs::read_to_string(path.with_extension("identity"))
                        .is_ok_and(|value| value == identity) =>
            {
                return Some(path)
            }
            _ => {}
        }
    }
    let mut entries = Index::new();
    for entry in fs::read_dir(folder).ok()?.take(131072).flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "identity")
            || !entry.file_type().is_ok_and(|t| t.is_file())
        {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if !valid(stem) || entry.metadata().map_or(true, |m| m.len() > 16384) {
            continue;
        }
        if let Ok(value) = fs::read_to_string(&path) {
            entries.insert(value, path.with_extension(""));
        }
    }
    let answer = entries.get(identity).cloned();
    if let Ok(mut all) = indexes.lock() {
        if all.len() >= 64 && !all.contains_key(folder) {
            if let Some(key) = all.keys().next().cloned() {
                all.remove(&key);
            }
        }
        all.insert(folder.to_owned(), (stamp, entries));
    }
    answer
}
/// Local date and time for names (`2026-10-28-153012`).
pub fn stamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as libc::time_t;
    let mut time: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&now, &mut time) }.is_null() {
        return now.to_string();
    }
    format!(
        "{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        time.tm_year + 1900,
        time.tm_mon + 1,
        time.tm_mday,
        time.tm_hour,
        time.tm_min,
        time.tm_sec
    )
}
/// Creates `<label>-<date>` (a folder, or a file with `extension`), adding `-2`,
/// `-3`… when that second already has one. For copies made on each operation.
pub fn dated(folder: &Path, label: &str, extension: Option<&str>) -> Result<PathBuf, String> {
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let base = format!("{}-{}", slug(label), stamp());
    for number in 1..=16384 {
        let stem = if number == 1 { base.clone() } else { format!("{base}-{number}") };
        let path = match extension {
            Some(extension) => folder.join(format!("{stem}.{extension}")),
            None => folder.join(stem),
        };
        let created = match extension {
            Some(_) => fs::OpenOptions::new().write(true).create_new(true).open(&path).map(|_| ()),
            None => fs::create_dir(&path),
        };
        match created {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Não foi possível reservar um nome para esta cópia.".into())
}
/// Creates the folder `<label>` (`-2`, `-3`… when taken) and returns it.
pub fn unique_directory(folder: &Path, label: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let base = slug(label);
    for number in 1..=16384 {
        let path = folder.join(if number == 1 { base.clone() } else { format!("{base}-{number}") });
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Não foi possível reservar um nome para esta pasta.".into())
}
pub fn allocate(folder: &Path, label: &str, identity: &str) -> Result<PathBuf, String> {
    if identity.len() > 16384 || identity.is_empty() {
        return Err("Identidade de arquivo inválida.".into());
    }
    let _guard = ALLOCATION.lock().map_err(|e| e.to_string())?;
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(folder)
        .map_err(|e| e.to_string())?;
    use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(folder.join(".names.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if let Some(path) = find(folder, identity) {
        return Ok(path);
    }
    let name = slug(label);
    for number in 1..=16384 {
        let stem = if number == 1 {
            name.clone()
        } else {
            format!("{name}-{number}")
        };
        let base = folder.join(&stem);
        let occupied = fs::read_dir(folder)
            .map_err(|e| e.to_string())?
            .flatten()
            .any(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n == stem || n.starts_with(&format!("{stem}."))
            });
        if occupied {
            continue;
        }
        let temporary = folder.join(format!(
            ".{stem}-{}-{}.tmp",
            std::process::id(),
            TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let target = base.with_extension("identity");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        if let Err(e) = file
            .write_all(identity.as_bytes())
            .and_then(|_| file.sync_all())
        {
            let _ = fs::remove_file(&temporary);
            return Err(e.to_string());
        }
        let result = fs::hard_link(&temporary, &target);
        let _ = fs::remove_file(temporary);
        match result {
            Ok(()) => return Ok(base),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if let Some(path) = find(folder, identity) {
                    return Ok(path);
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Não foi possível reservar um nome para este arquivo.".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readable_names_preserve_collisions_identity_and_existing_files() {
        let folder =
            std::env::temp_dir().join(format!("peli-readable-names-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let a = allocate(&folder, "Tomb Raider", "steam:203160").unwrap();
        assert_eq!(a.file_name().unwrap(), "tomb-raider");
        fs::write(a.with_extension("json"), b"original").unwrap();
        assert_eq!(
            allocate(&folder, "Changed title", "steam:203160").unwrap(),
            a
        );
        let b = allocate(&folder, "Tomb Raider", "other:203160").unwrap();
        assert_eq!(b.file_name().unwrap(), "tomb-raider-2");
        fs::write(folder.join("untracked.json"), b"keep").unwrap();
        assert_ne!(
            allocate(&folder, "untracked", "new").unwrap(),
            folder.join("untracked")
        );
        assert_eq!(fs::read(a.with_extension("json")).unwrap(), b"original");
        assert!(find(&folder, "added-by-other-process").is_none());
        fs::write(folder.join("external.identity"), "added-by-other-process").unwrap();
        assert_eq!(
            find(&folder, "added-by-other-process"),
            Some(folder.join("external"))
        );
        fs::remove_file(folder.join("external.identity")).unwrap();
        assert!(find(&folder, "added-by-other-process").is_none());

        assert_eq!(slug("Geração de Quadros"), "geracao-de-quadros");
        assert!(!valid("../game"));
        assert!(!valid("a--b"));
        fs::remove_dir_all(folder).unwrap();
    }
}
