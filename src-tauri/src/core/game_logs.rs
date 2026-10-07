//! Two retained execution records per library entry, including Proton's own output.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Seek, SeekFrom}, path::{Path, PathBuf}};
#[derive(Serialize)]
pub struct GameLogs { pub current: String, pub previous: String }
fn root(path: &str) -> Result<PathBuf, String> {
    Ok(dirs::config_dir().ok_or("Configuração indisponível.")?.join("peligames/game-logs").join(format!("{:x}", Sha256::digest(path.as_bytes()))))
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
    let directory = rotate_at(&root(path)?)?;
    let file = fs::File::create(directory.join("execution.log")).map_err(|e|e.to_string())?;
    use std::io::Write;
    writeln!(&file, "PeliGames\nExecutable: {}\nPrefix: {}", run.executable.display(), run.prefix.display()).map_err(|e|e.to_string())?;
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
pub fn read(path: &str) -> Result<GameLogs, String> {
    if !super::installed_library::list(false)?.iter().any(|entry| entry.path==path) { return Err("Jogo não registrado no PeliGames.".into()); }
    let directory = root(path)?;
    Ok(GameLogs { current: read_at(&directory.join("current"))?, previous: read_at(&directory.join("previous"))? })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_current_and_exactly_one_previous_execution() {
        let root = std::env::temp_dir().join(format!("peli-logs-{}",uuid::Uuid::new_v4()));
        for n in 1..=3 { let current=rotate_at(&root).unwrap(); fs::write(current.join("execution.log"),format!("run {n}")).unwrap(); fs::write(current.join("steam.log"),format!("proton {n}")).unwrap(); }
        assert!(read_at(&root.join("current")).unwrap().contains("run 3"));
        let previous=read_at(&root.join("previous")).unwrap(); assert!(previous.contains("run 2")); assert!(previous.contains("proton 2")); assert!(!previous.contains("run 1"));
        assert_eq!(fs::read_dir(&root).unwrap().count(),2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_and_large_logs_are_bounded() {
        let root=std::env::temp_dir().join(format!("peli-log-tail-{}",uuid::Uuid::new_v4())); assert_eq!(read_at(&root).unwrap(),"");
        fs::create_dir(&root).unwrap(); fs::write(root.join("execution.log"),vec![b'x';2*1024*1024]).unwrap();
        assert!(read_at(&root).unwrap().len()<1024*1024+100); fs::remove_dir_all(root).unwrap();
    }
}
