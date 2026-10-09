//! Bounded, read-only collection of loader logs; enabled is not proof of loading.
use std::{fs, path::{Path, PathBuf}, time::UNIX_EPOCH};
use super::nexus_local::NexusGame;

fn files_at(root: &Path, relative: &str, depth: usize) -> Vec<PathBuf> {
    let Ok(root) = root.canonicalize() else { return Vec::new(); };
    let start = root.join(relative);
    if start.is_file() { return start.canonicalize().ok().filter(|file| file.starts_with(&root)).into_iter().collect(); }
    let mut files = walkdir::WalkDir::new(start).follow_links(false).max_depth(depth).into_iter().take(512)
        .filter_map(Result::ok).filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "log"))
        .filter_map(|entry| entry.path().canonicalize().ok()).filter(|file| file.starts_with(&root)).collect::<Vec<_>>();
    files.sort_by_key(|file| std::cmp::Reverse(fs::metadata(file).and_then(|m|m.modified()).ok()));
    files.truncate(8); files
}
pub(super) fn read(game: &NexusGame) -> Result<String, String> {
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta do jogo ausente.")?);
    let mut roots = vec![root.to_path_buf()];
    if let Some(view) = super::nexus_deploy_methods::view(game)? { roots.insert(0,view); }
    let mut files = Vec::new();
    for base in &roots {
        for (location, depth) in [
            ("BepInEx/LogOutput.log",1), ("red4ext/logs",1), ("red4ext/plugins",3), ("r6/logs",1),
            ("bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.log",1),
            ("Pal/Binaries/Win64/ue4ss/UE4SS.log",1), ("Pal/Binaries/Win64/UE4SS.log",1), ("ue4ss/UE4SS.log",1),
        ] { for file in files_at(base,location,depth) { if !files.contains(&file) {files.push(file);} } }
        if game.adapter == "crimsondesert" {
            for file in files_at(base, "bin64", 3) { if !files.contains(&file) {files.push(file);} }
        }
    }
    if game.adapter == "stardewvalley" {
        if let Ok((_,prefix)) = super::nexus_prefix::resolve(Path::new(&game.compat_data)) {
            let users=prefix.join("drive_c/users");
            if let Ok(entries)=fs::read_dir(&users) {
                for entry in entries.flatten().take(32) {
                    files.extend(files_at(&prefix,&format!("drive_c/users/{}/AppData/Roaming/StardewValley/ErrorLogs/SMAPI-latest.txt",entry.file_name().to_string_lossy()),1));
                }
            }
        }
        if game.platform == "native" {
            if let Some(home)=std::env::var_os("HOME") { files.extend(files_at(&PathBuf::from(home),".config/StardewValley/ErrorLogs/SMAPI-latest.txt",1)); }
        }
    }
    let mut output = String::new();
    for file in files.into_iter().take(16) {
        let updated=fs::metadata(&file).and_then(|m|m.modified()).ok().and_then(|time|time.duration_since(UNIX_EPOCH).ok()).map(|time|time.as_secs()).unwrap_or(0);
        output.push_str(&format!("--- {} | modified_unix={} ---\n",file.display(),updated));
        match super::nexus_profile::read_log_file(&file) { Ok(content)=>output.push_str(&content), Err(error)=>output.push_str(&error) }
        output.push_str("\n\n");
        if output.len() > 2 * 1024 * 1024 {break;}
    }
    if output.is_empty() { output="Nenhum log de carregador encontrado. Inicie o jogo com mods. Alguns mods não registram confirmação de carregamento.".into(); }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loader_files_are_bounded_and_do_not_follow_external_links() {
        let root=std::env::temp_dir().join(format!("loader-logs-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("red4ext/logs")).unwrap();
        for n in 0..12 {fs::write(root.join(format!("red4ext/logs/{n}.log")),"loaded").unwrap();}
        fs::write(root.join("red4ext/logs/not-a-log.dll"),"binary").unwrap();
        fs::write(root.join("red4ext/logs/README.txt"),"documentation").unwrap();
        #[cfg(unix)] std::os::unix::fs::symlink("/etc/passwd",root.join("red4ext/logs/escape.log")).unwrap();
        let files=files_at(&root,"red4ext/logs",1);
        assert_eq!(files.len(),8);assert!(files.iter().all(|file|file.starts_with(&root)&&file.extension().unwrap()=="log"));
        #[cfg(unix)] assert!(files_at(&root,"red4ext/logs/escape.log",1).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
