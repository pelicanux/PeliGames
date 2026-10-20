//! Two retained execution records per library entry, including Proton's own output.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Seek, SeekFrom}, path::{Path, PathBuf}};
#[derive(Serialize)]
pub struct GameLogs { pub current: String, pub previous: String }
#[derive(Serialize, Deserialize)]
struct Identity {
    name: String,
    path: String,
    identity: String,
    executable: String,
    prefix: String,
}
fn digest(path: &str) -> String { format!("{:x}", Sha256::digest(path.as_bytes())) }
fn slug(name: &str) -> String {
    let mut result = String::new();
    for ch in name.chars() {
        if result.len() + ch.len_utf8() > 64 {break;}
        if ch.is_alphanumeric() {result.extend(ch.to_lowercase());}
        else if !result.is_empty() && !result.ends_with('-') {result.push('-');}
    }
    let value = result.trim_end_matches('-');
    if value.is_empty() {"jogo".into()} else {value.into()}
}
fn plain_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta|meta.is_dir() && !meta.file_type().is_symlink())
}
fn identity_at(root: &Path) -> Option<Identity> {
    if !plain_directory(root) {return None;}
    let mut options = fs::OpenOptions::new(); options.read(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);}
    let mut bytes = Vec::new();
    let file = options.open(root.join("game.json")).ok()?;
    if !file.metadata().ok()?.is_file() {return None;}
    file.take(64 * 1024).read_to_end(&mut bytes).ok()?;
    serde_json::from_slice(&bytes).ok()
}
fn belongs(root: &Path, path: &str, id: &str) -> bool {
    identity_at(root).is_some_and(|meta|meta.path == path && meta.identity == id)
}
/// Read-only lookup: never move a directory while Proton may still be writing to it.
fn existing_at(base: &Path, path: &str) -> Result<Option<PathBuf>, String> {
    let id = digest(path);
    let mut directories: Vec<_> = match fs::read_dir(base) {
        Ok(entries) => entries.flatten().map(|entry|entry.path()).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    directories.sort();
    if let Some(root) = directories.into_iter().find(|root|belongs(root,path,&id)) {return Ok(Some(root));}
    let legacy = base.join(id);
    Ok(plain_directory(&legacy).then_some(legacy))
}
fn save_identity(root: &Path, identity: &Identity) -> Result<(), String> {
    use std::io::Write;
    let temporary = root.join(format!(".game-{}.tmp",uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();options.write(true).create_new(true);
        #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let mut file = options.open(&temporary).map_err(|e|e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(identity).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        fs::rename(&temporary,root.join("game.json")).map_err(|e|e.to_string())
    })();
    if result.is_err() {let _=fs::remove_file(temporary);}
    result
}
fn rename_exclusive(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(target_os="linux")] {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(from.as_os_str().as_bytes())?;
        let to = std::ffi::CString::new(to.as_os_str().as_bytes())?;
        if unsafe {libc::renameat2(libc::AT_FDCWD,from.as_ptr(),libc::AT_FDCWD,to.as_ptr(),libc::RENAME_NOREPLACE)} == 0 {Ok(())}
        else {Err(std::io::Error::last_os_error())}
    }
    #[cfg(not(target_os="linux"))] {
        if to.symlink_metadata().is_ok() {return Err(std::io::ErrorKind::AlreadyExists.into());}
        fs::rename(from,to)
    }
}
/// Called under the prefix lease, before launch: safe to rename an older log directory.
fn prepare_root_at(base: &Path, path: &str, name: &str, executable: &Path, prefix: &Path) -> Result<PathBuf,String> {
    fs::create_dir_all(base).map_err(|e|e.to_string())?;
    let id = digest(path);
    let previous = existing_at(base,path)?;
    let identity = Identity {name:name.into(),path:path.into(),identity:id.clone(),executable:executable.to_string_lossy().into_owned(),prefix:prefix.to_string_lossy().into_owned()};
    let name = slug(name);
    for length in (8..=64).step_by(4) {
        let root = base.join(format!("{name}-{}",&id[..length]));
        if previous.as_ref() == Some(&root) || belongs(&root,path,&id) {
            save_identity(&root,&identity)?;return Ok(root);
        }
        if root.symlink_metadata().is_ok() {continue;}
        if let Some(previous) = &previous {
            // The identity remains discoverable even if a subsequent rename fails.
            save_identity(previous,&identity)?;
            match rename_exclusive(previous,&root) {
                Ok(()) => return Ok(root),
                Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        match fs::create_dir(&root) {
            Ok(()) => {save_identity(&root,&identity)?;return Ok(root);}
            Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Não foi possível reservar uma pasta exclusiva para os logs.".into())
}
fn rotate_at(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root).map_err(|e|e.to_string())?;
    let current = root.join("current"); let previous = root.join("previous");
    // Called only while holding the prefix lease, before spawning the new process.
    if current.exists() {
        if previous.exists() { fs::remove_dir_all(&previous).map_err(|e|e.to_string())?; }
        fs::rename(&current, &previous).map_err(|e|e.to_string())?;
    }
    fs::create_dir_all(&current).map_err(|e|e.to_string())?;
    Ok(current)
}
pub(crate) fn prepare(path: &str, run: &mut super::game_installation::PreparedRun) -> Result<(), String> {
    let base = super::paths::app_root()?.join("game-logs");
    let root = prepare_root_at(&base,path,&run.name,&run.executable,&run.prefix)?;
    let directory = rotate_at(&root)?;
    let file = fs::File::create(directory.join("execution.log")).map_err(|e|e.to_string())?;
    use std::io::Write;
    writeln!(&file, "PeliGames\nGame: {}\nExecutable: {}\nPrefix: {}", run.name, run.executable.display(), run.prefix.display()).map_err(|e|e.to_string())?;
    let stderr = file.try_clone().map_err(|e|e.to_string())?;
    run.command.stdout(file).stderr(stderr).env("PROTON_LOG_DIR", &directory);
    let old = std::mem::replace(&mut run.log, directory.join("execution.log"));
    let _ = fs::remove_file(old);
    Ok(())
}
fn read_at(directory: &Path) -> Result<String, String> {
    let entries = match fs::read_dir(directory) { Ok(entries) => entries, Err(e) if e.kind()==std::io::ErrorKind::NotFound => return Ok(String::new()), Err(e) => return Err(e.to_string()) };
    let mut paths: Vec<_> = entries.flatten().filter(|e|e.file_type().is_ok_and(|t|t.is_file()) && e.path().extension().is_some_and(|x|x=="log")).map(|e|e.path()).collect();
    paths.sort();
    let mut output = String::new();
    let mut budget = 1024 * 1024;
    for path in paths.into_iter().take(16) {
        if budget == 0 { break; }
        let mut options = fs::OpenOptions::new(); options.read(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW); }
        let mut file = options.open(&path).map_err(|e|e.to_string())?;
        let size = file.metadata().map_err(|e|e.to_string())?.len();
        if size == 0 { continue; }
        let count = (size as usize).min(budget);
        file.seek(SeekFrom::Start(size.saturating_sub(count as u64))).map_err(|e|e.to_string())?;
        let mut bytes = Vec::new(); file.take(count as u64).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
        output.push_str(&format!("--- {} ---\n{}\n", path.file_name().unwrap_or_default().to_string_lossy(), String::from_utf8_lossy(&bytes)));
        budget -= bytes.len();
    }
    Ok(output)
}
/// Resolve both readable names and the legacy identity without moving active logs.
pub fn folder(path: &str) -> Result<PathBuf, String> {
    if !super::installed_library::list(false)?.iter().any(|entry| entry.path == path) {
        return Err("Jogo não registrado no PeliGames.".into());
    }
    existing_at(&super::paths::app_root()?.join("game-logs"), path)?
        .ok_or_else(|| "Ainda não há pasta de logs para este jogo. Execute-o primeiro.".into())
}
pub fn read(path: &str) -> Result<GameLogs, String> {
    if !super::installed_library::list(false)?.iter().any(|entry| entry.path==path) { return Err("Jogo não registrado no PeliGames.".into()); }
    let base = super::paths::app_root()?.join("game-logs");
    let Some(directory) = existing_at(&base,path)? else {return Ok(GameLogs {current:String::new(),previous:String::new()});};
    Ok(GameLogs { current: read_at(&directory.join("current"))?, previous: read_at(&directory.join("previous"))? })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {fn new()->Self {let path=std::env::temp_dir().join(format!("peli-log-layout-{}",uuid::Uuid::new_v4()));fs::create_dir_all(&path).unwrap();Self(path)}}
    impl Drop for Temp {fn drop(&mut self){let _=fs::remove_dir_all(&self.0);}}
    fn prepare(base:&Path,path:&str,name:&str)->PathBuf {prepare_root_at(base,path,name,Path::new("/game/main.exe"),Path::new("/game/prefix")).unwrap()}
    #[test]
    fn readable_names_have_distinct_installation_identities() {
        let dir=Temp::new();
        let first=prepare(&dir.0,"installation-one","Real MU");
        let second=prepare(&dir.0,"installation-two","Real MU");
        assert!(first.file_name().unwrap().to_string_lossy().starts_with("real-mu-"));
        assert_eq!(first.file_name().unwrap().len(),"real-mu-".len()+8);
        assert_ne!(first,second);
        assert_eq!(identity_at(&first).unwrap().path,"installation-one");
        assert_eq!(prepare(&dir.0,"installation-one","Real MU"),first);
    }
    #[test]
    fn legacy_logs_remain_readable_and_migrate_without_losing_history() {
        let dir=Temp::new();let path="legacy-game";let old=dir.0.join(digest(path));
        fs::create_dir_all(old.join("current")).unwrap();fs::create_dir_all(old.join("previous")).unwrap();
        fs::write(old.join("current/execution.log"),"current record").unwrap();
        fs::write(old.join("previous/execution.log"),"previous record").unwrap();
        assert_eq!(existing_at(&dir.0,path).unwrap(),Some(old.clone()));
        assert!(!old.join("game.json").exists());
        let new=prepare(&dir.0,path,"Real MU");
        assert!(!old.exists());
        assert_eq!(existing_at(&dir.0,path).unwrap(),Some(new.clone()));
        assert!(read_at(&new.join("current")).unwrap().contains("current record"));
        assert!(read_at(&new.join("previous")).unwrap().contains("previous record"));
    }
    #[test]
    fn title_changes_keep_the_same_history_and_update_the_mapping() {
        let dir=Temp::new();let old=prepare(&dir.0,"game-id","Old name");
        fs::create_dir(old.join("current")).unwrap();fs::write(old.join("current/execution.log"),"keep").unwrap();
        let new=prepare(&dir.0,"game-id","New name");
        assert_ne!(old,new);assert!(!old.exists());
        assert!(read_at(&new.join("current")).unwrap().contains("keep"));
        assert_eq!(identity_at(&new).unwrap().name,"New name");
    }
    #[test]
    fn occupied_short_names_are_preserved_and_the_identifier_extends() {
        let dir=Temp::new();let id=digest("new-game");let occupied=dir.0.join(format!("real-mu-{}",&id[..8]));
        fs::create_dir(&occupied).unwrap();fs::write(occupied.join("do-not-touch"),"old data").unwrap();
        let allocated=prepare(&dir.0,"new-game","Real MU");
        assert_eq!(allocated.file_name().unwrap().to_string_lossy(),format!("real-mu-{}",&id[..12]));
        assert_eq!(fs::read_to_string(occupied.join("do-not-touch")).unwrap(),"old data");
    }
    #[test]
    fn unsafe_or_long_titles_stay_inside_the_log_directory() {
        for name in ["../../Outside", "", "...", "A very long name ".repeat(80).as_str()] {
            let value=slug(name);assert!(!value.contains('/'));assert!(!value.contains('.'));assert!(value.len()<=64);assert!(!value.is_empty());
        }
        let dir=Temp::new();let source=dir.0.join("source");let target=dir.0.join("target");
        fs::create_dir(&source).unwrap();fs::create_dir(&target).unwrap();
        assert!(rename_exclusive(&source,&target).is_err());assert!(source.exists());assert!(target.exists());
    }
}
