//! Organization and bounded reading of launcher-owned benchmark CSVs.
use std::{fs, io::Read, path::{Path, Component}};
use serde::Serialize;
const MAX_BYTES: u64 = 16 * 1024 * 1024;
#[derive(Serialize)]
pub struct BenchmarkFile { pub name: String, pub size: u64, pub modified: u64 }
fn logs() -> Result<std::path::PathBuf, String> {
    let root = super::mangohud_config::directory()?.join("logs");
    match fs::symlink_metadata(&root) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => Ok(root),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(root),
        _ => Err("Pasta de benchmarks inválida.".into()),
    }
}
fn valid_name(name: &str) -> bool {
    let parts: Vec<_> = Path::new(name).components().collect();
    (1..=2).contains(&parts.len()) && parts.iter().all(|part| matches!(part, Component::Normal(_)))
        && !name.contains('\\') && !name.contains("//") && !name.ends_with('/')
        && !name.chars().any(|c| c.is_control()) && name.ends_with(".csv") && !name.ends_with("_summary.csv")
}
/// MangoHUD names files executable_YYYY-MM-DD_HH-MM-SS.csv.
fn executable_folder(name: &str) -> Option<String> {
    let stem = name.strip_suffix("_summary.csv").or_else(||name.strip_suffix(".csv"))?;
    let split = stem.len().checked_sub(20)?;
    if !stem.is_char_boundary(split) { return None; }
    let (program, suffix) = stem.split_at(split);
    let bytes = suffix.as_bytes();
    if program.is_empty() || bytes.len()!=20 || !bytes.iter().enumerate().all(|(i,b)| match i { 0|11 => *b==b'_', 5|8|14|17 => *b==b'-', _ => b.is_ascii_digit() }) { return None; }
    let program = program.strip_suffix(".exe").unwrap_or(program);
    Some(super::mangohud_config::game_folder(program))
}
fn list_in(root: &Path) -> Result<Vec<BenchmarkFile>, String> {
    if !root.exists() { return Ok(Vec::new()); }
    // On demand only. Existing MangoHUD writers can finish writing through an
    // open handle after the rename; the eventual summary is grouped next time.
    for entry in fs::read_dir(root).map_err(|e|e.to_string())?.take(5000).flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(folder) = executable_folder(&name) else { continue; };
        let Ok(meta) = fs::symlink_metadata(entry.path()) else { continue; };
        if !meta.is_file() || meta.file_type().is_symlink() { continue; }
        let destination = root.join(folder);
        if super::mangohud_config::safe_directory(&destination).is_err() { continue; }
        // A hard link plus unlink never overwrites an existing recording.
        if fs::hard_link(entry.path(), destination.join(&name)).is_ok() { let _ = fs::remove_file(entry.path()); }
    }
    let mut files = Vec::new();
    let mut remaining = 5000usize;
    for entry in fs::read_dir(root).map_err(|e|e.to_string())?.take(5000).flatten() {
        if remaining == 0 { break; }
        let Ok(meta) = fs::symlink_metadata(entry.path()) else { continue; };
        if meta.file_type().is_symlink() { continue; }
        if meta.is_dir() {
            if let Ok(children) = fs::read_dir(entry.path()) {
                for child in children.take(remaining).flatten() {
                    remaining = remaining.saturating_sub(1);
                    let name = format!("{}/{}",entry.file_name().to_string_lossy(),child.file_name().to_string_lossy());
                    append_file(&mut files, child.path(), name);
                }
            }
        } else {
            remaining = remaining.saturating_sub(1);
            append_file(&mut files, entry.path(), entry.file_name().to_string_lossy().into_owned());
        }
    }
    files.sort_by(|a,b| b.modified.cmp(&a.modified).then_with(|| a.name.cmp(&b.name)));
    Ok(files)
}
fn append_file(files: &mut Vec<BenchmarkFile>, path: std::path::PathBuf, name: String) {
    if !valid_name(&name) { return; }
    let Ok(meta) = fs::symlink_metadata(path) else { return; };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_BYTES { return; }
    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|t| t.as_secs()).unwrap_or(0);
    files.push(BenchmarkFile { name, size: meta.len(), modified });
}
fn game_aliases(name: &str, executable: Option<&str>, directory: Option<&str>) -> Vec<String> {
    let mut aliases = vec![super::mangohud_config::game_folder(name)];
    for value in [executable,directory].into_iter().flatten() {
        let normalized = value.replace('\\', "/");
        if let Some(base) = normalized.trim_end_matches('/').rsplit('/').next().filter(|s| !s.is_empty()) {
            let base = if base.to_ascii_lowercase().ends_with(".exe") { &base[..base.len()-4] } else { base };
            aliases.push(super::mangohud_config::game_folder(base));
        }
    }
    aliases
}
fn matches_game(file: &BenchmarkFile, aliases: &[String]) -> bool {
    file.name.split_once('/').is_some_and(|(folder,_)| aliases.iter().any(|alias| alias.eq_ignore_ascii_case(folder)))
}
pub fn list(game_name: Option<&str>, executable: Option<&str>, directory: Option<&str>) -> Result<Vec<BenchmarkFile>, String> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK.lock().map_err(|_| "Registros ocupados.")?;
    let mut files = list_in(&logs()?)?;
    if let Some(name) = game_name {
        let aliases = game_aliases(name,executable,directory);
        files.retain(|file| matches_game(file,&aliases));
    }
    files.truncate(200);
    Ok(files)
}
pub fn read(name: &str) -> Result<String, String> {
    read_in(&logs()?, name)
}
fn read_in(root: &Path, name: &str) -> Result<String, String> {
    if !valid_name(name) { return Err("Nome de benchmark inválido.".into()); }
    if let Some(parent) = Path::new(name).parent().filter(|p| !p.as_os_str().is_empty()) {
        let meta = fs::symlink_metadata(root.join(parent)).map_err(|_| "Pasta de benchmark indisponível.")?;
        if !meta.is_dir() || meta.file_type().is_symlink() { return Err("Pasta de benchmark inválida.".into()); }
    }
    read_path(&root.join(name)).map(|(csv,_)|csv)
}
fn read_path(path: &Path) -> Result<(String,fs::Metadata), String> {
    let mut options = fs::OpenOptions::new(); options.read(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK); }
    let file = options.open(path).map_err(|_| "Não foi possível abrir esse benchmark.".to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_BYTES { return Err("Benchmark inválido ou maior que 16 MiB.".into()); }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Benchmark maior que 16 MiB.".into()); }
    let csv=String::from_utf8(bytes).map_err(|_| "O benchmark não é um CSV de texto válido.".to_string())?;
    Ok((csv,meta))
}
#[derive(Serialize)]
pub struct SelectedBenchmark { pub name:String, pub csv:String, pub size:u64, pub modified:u64 }
/// Called only with the native file dialog's selection, never an IPC path.
pub fn selected(path: &Path) -> Result<SelectedBenchmark,String> {
    let name=path.file_name().and_then(|name|name.to_str()).ok_or("Nome do registro inválido.")?;
    let lower=name.to_lowercase();
    if !lower.ends_with(".csv") || lower.ends_with("_summary.csv") {return Err("Selecione o CSV completo do MangoHUD, não o arquivo summary.".into());}
    let (csv,meta)=read_path(path)?;
    let modified=meta.modified().ok().and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|t|t.as_secs()).unwrap_or(0);
    Ok(SelectedBenchmark {name:name.into(),csv,size:meta.len(),modified})
}
#[cfg(test)] mod tests {
    #[test] fn manual_selection_reads_external_csv_without_moving_it() {
        let root=std::env::temp_dir().join(format!("peligames-external-benchmark-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let file=root.join("Steam Game.CSV");std::fs::write(&file,"fps,frametime\n60,16.67\n").unwrap();
        let selected=super::selected(&file).unwrap();
        assert_eq!(selected.name,"Steam Game.CSV");assert!(selected.csv.contains("16.67"));assert!(file.exists());
        assert!(super::selected(&root.join("game_summary.csv")).is_err());
        #[cfg(unix)] {std::os::unix::fs::symlink(&file,root.join("link.csv")).unwrap();assert!(super::selected(&root.join("link.csv")).is_err());}
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn reads_only_bounded_regular_csv_files() {
        let root = std::env::temp_dir().join(format!("peligames-benchmarks-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("game.csv"),"fps,frametime\n60,16.67\n").unwrap();
        assert!(super::read_in(&root,"game.csv").unwrap().contains("16.67"));
        assert!(super::read_in(&root,"../game.csv").is_err());
        #[cfg(unix)] {
            std::os::unix::fs::symlink(root.join("game.csv"),root.join("link.csv")).unwrap();
            assert!(super::read_in(&root,"link.csv").is_err());
        }
        let large = std::fs::File::create(root.join("large.csv")).unwrap();
        large.set_len(super::MAX_BYTES+1).unwrap();
        assert!(super::read_in(&root,"large.csv").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn groups_and_reads_recordings_without_overwriting_or_following_links() {
        let root = std::env::temp_dir().join(format!("peligames-benchmark-folders-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let name = "Cyberpunk2077.exe_2026-10-17_21-30-05.csv";
        std::fs::write(root.join(name), "fps,frametime\n60,16.67\n").unwrap();
        let summary = name.replace(".csv", "_summary.csv");
        std::fs::write(root.join(&summary), "summary").unwrap();
        let files = super::list_in(&root).unwrap();
        assert_eq!(files.len(),1);
        assert_eq!(files[0].name,format!("Cyberpunk2077/{name}"));
        assert!(super::read_in(&root,&files[0].name).unwrap().contains("16.67"));
        assert!(root.join("Cyberpunk2077").join(summary).exists());
        std::fs::write(root.join(name),"second recording").unwrap();
        super::list_in(&root).unwrap();
        assert_eq!(std::fs::read_to_string(root.join(name)).unwrap(),"second recording");
        #[cfg(unix)] {
            std::os::unix::fs::symlink(root.join("Cyberpunk2077"),root.join("link")).unwrap();
            assert!(super::read_in(&root,&format!("link/{name}")).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test] fn game_filter_matches_title_folder_and_windows_executable_without_other_games() {
        let aliases = super::game_aliases("Cyberpunk 2077", Some("C:\\Games\\Cyberpunk2077.exe"),Some("/mnt/Games/Cyberpunk"));
        for folder in ["Cyberpunk 2077","Cyberpunk2077","cyberpunk"] {
            assert!(super::matches_game(&super::BenchmarkFile{name:format!("{folder}/test.csv"),size:1,modified:1},&aliases));
        }
        assert!(!super::matches_game(&super::BenchmarkFile{name:"Other Game/test.csv".into(),size:1,modified:1},&aliases));
        assert!(!super::matches_game(&super::BenchmarkFile{name:"test.csv".into(),size:1,modified:1},&aliases));
    }
    #[test] fn extracts_only_standard_recording_names() {
        assert_eq!(super::executable_folder("vkcube_2026-10-17_21-30-05.csv").as_deref(),Some("vkcube"));
        assert!(super::executable_folder("arbitrary.csv").is_none());
        assert_eq!(super::super::mangohud_config::game_folder("../Game/Name"),"_Game_Name");
    }
    #[test] fn rejects_paths_and_summaries() {
        for name in ["../game.csv", "/tmp/game.csv", "dir/sub/game.csv", "dir\\game.csv", "game_summary.csv", "game.txt"] { assert!(!super::valid_name(name), "{name}"); }
        assert!(super::valid_name("game_2026-10-17.csv"));
        assert!(super::valid_name("Cyberpunk 2077/game.csv"));
    }
}
