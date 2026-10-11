//! Bounded loader logs, renewed per execution; enabled is not proof of loading.
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
fn collect(game: &NexusGame) -> Result<Vec<PathBuf>, String> {
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta do jogo ausente.")?);
    let mut roots = vec![root.to_path_buf()];
    if let Some(view) = super::nexus_deploy_methods::view(game)? { roots.insert(0,view); }
    if let Ok(profile) = super::nexus_profile::profile(game) {
        if !roots.contains(&profile) { roots.push(profile); }
    }
    let module = super::nexus_deployment::definition(game);
    let mut files = Vec::new();
    for base in &roots {
        for (location, depth) in [
            ("BepInEx/LogOutput.log",1), ("MelonLoader/Latest.log",1), ("reframework/log.txt",1), ("red4ext/logs",1), ("red4ext/plugins",3), ("r6/logs",1),
            ("bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.log",1),
            ("Pal/Binaries/Win64/ue4ss/UE4SS.log",1), ("Pal/Binaries/Win64/UE4SS.log",1), ("ue4ss/UE4SS.log",1),
        ] { for file in files_at(base,location,depth) { if !files.contains(&file) {files.push(file);} } }
        // Script extender logs: game root (xNVSE, xOBSE) or Documents inside the prefix (SKSE, F4SE, SFSE).
        if let Some(extender) = module.as_ref().and_then(|module| module.script_extender.as_ref()) {
            for log in &extender.logs {
                let located = if log.starts_with('@') { super::nexus_targets::root(base, &game.compat_data, log).ok() } else { Some((base.to_path_buf(), log.clone())) };
                if let Some((folder, local)) = located {
                    for file in files_at(&folder, &local, 1) { if !files.contains(&file) { files.push(file); } }
                }
            }
        }
        if let Some(layout) = module.as_ref().and_then(|module| module.ue4ss()) {
            for location in [format!("{}/ue4ss/UE4SS.log", layout.binaries), format!("{}/UE4SS.log", layout.binaries)] {
                for file in files_at(base,&location,1) { if !files.contains(&file) {files.push(file);} }
            }
        }
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
    Ok(files)
}

pub(super) const NO_SESSION: &str = "Nenhuma execução desde que o launcher foi aberto. Os logs começam quando você clica em Iniciar.";
pub(super) fn session_started(game: &NexusGame) -> Option<std::time::SystemTime> {
    let logs = super::nexus_profile::directory(game).ok()?.join("logs");
    if let Ok(value) = fs::read_to_string(logs.join("session-started-ms.txt")) {
        return value.trim().parse::<u64>().ok().map(|ms| UNIX_EPOCH + std::time::Duration::from_millis(ms));
    }
    fs::read_to_string(logs.join("session-started.txt")).ok()?.trim().parse::<u64>().ok()
        .map(|seconds| UNIX_EPOCH + std::time::Duration::from_secs(seconds))
}

pub(super) fn in_session(modified: Option<std::time::SystemTime>, started: Option<std::time::SystemTime>) -> bool {
    started.is_none_or(|start| modified.is_some_and(|modified| modified >= start))
}

fn reset_files(files: Vec<PathBuf>) -> Result<(), String> {
    for file in files {
        let mut options = fs::OpenOptions::new();
        options.write(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW); }
        let output = options.open(&file).map_err(|e| format!("Não foi possível renovar o log {}: {e}", file.display()))?;
        if !output.metadata().map_err(|e| e.to_string())?.is_file() { continue; }
        output.set_len(0).map_err(|e| format!("Não foi possível renovar o log {}: {e}", file.display()))?;
    }
    Ok(())
}

pub(super) fn reset(game: &NexusGame) -> Result<(), String> {
    // Only known text logs, resolved inside the game/profile/prefix. Never remove
    // save data, crash dumps, launcher logs, or arbitrary files in the game root.
    reset_files(collect(game)?)
}

pub(super) fn read(game: &NexusGame) -> Result<String, String> {
    // No launch since the launcher opened: nothing belongs to this session.
    let Some(started) = session_started(game) else { return Ok(NO_SESSION.into()); };
    let started = Some(started);
    let files = collect(game)?;
    let mut output = String::new();
    for file in files.into_iter().filter(|file| {
        fs::metadata(file).is_ok_and(|meta| meta.len() > 0 && in_session(meta.modified().ok(), started))
    }).take(16) {
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
    fn new_session_clears_only_selected_text_logs() {
        let root = std::env::temp_dir().join(format!("loader-reset-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let log = root.join("UE4SS.log");
        fs::write(&log, "previous failure").unwrap();
        fs::write(root.join("save.sav"), "save data").unwrap();
        fs::write(root.join("crash.dmp"), "dump data").unwrap();
        reset_files(vec![log.clone()]).unwrap();
        assert_eq!(fs::metadata(&log).unwrap().len(), 0);
        assert_eq!(fs::read_to_string(root.join("save.sav")).unwrap(), "save data");
        assert_eq!(fs::read_to_string(root.join("crash.dmp")).unwrap(), "dump data");
        #[cfg(unix)] {
            let link = root.join("link.log");
            std::os::unix::fs::symlink(root.join("save.sav"), &link).unwrap();
            assert!(reset_files(vec![link]).is_err());
            assert_eq!(fs::read_to_string(root.join("save.sav")).unwrap(), "save data");
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn older_diagnostics_do_not_leak_into_the_current_session() {
        let start = UNIX_EPOCH + std::time::Duration::from_millis(1500);
        assert!(!in_session(Some(UNIX_EPOCH + std::time::Duration::from_millis(1499)), Some(start)));
        assert!(in_session(Some(start), Some(start)));
        assert!(in_session(Some(start + std::time::Duration::from_millis(1)), Some(start)));
        assert!(!in_session(None, Some(start)));
        assert!(in_session(Some(start), None));
    }
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
