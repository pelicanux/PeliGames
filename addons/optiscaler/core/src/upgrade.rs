//! Replace only journal-owned files, retaining game configuration and plugins.
use super::{game_directory, installation as install, read_file, WRITE};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

struct Stage {
    path: PathBuf,
    recovery: bool,
}
impl Drop for Stage {
    fn drop(&mut self) {
        if !self.recovery {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn merge_config(previous: &str, defaults: &str) -> String {
    // Keep existing comments, options and section spelling; only add new defaults.
    let parsed = peligames_mod_config::ini::sections(previous);
    let mut text = previous.to_string();
    for (section, values) in peligames_mod_config::ini::sections(defaults) {
        for (key, value) in values {
            if peligames_mod_config::ini::get(&parsed, &section, &key).is_none() {
                text = peligames_mod_config::ini::put(&text, &section, &key, Some(&value));
            }
        }
    }
    text
}

pub fn upgrade(component: &Path, game: &Path) -> Result<String, String> {
    let _lock = WRITE.lock().map_err(|e| e.to_string())?;
    let game = game_directory(game)?;
    let _directory = peligames_mod_config::DirectoryLock::acquire(&game)?;
    if super::nr_route(&game)?.is_some() {
        return Err("Manage the DLSSNR installation through its own menu".into());
    }
    let previous = install::read_journal(&game)?;
    let owned = install::owned_files(&game, &previous)?;
    let mut stage = Stage {
        path: game.join(format!(
            ".peligames-optiscaler-update-{}",
            uuid::Uuid::new_v4()
        )),
        recovery: false,
    };
    let prepared = stage.path.join("prepared");
    fs::create_dir_all(&prepared).map_err(|e| e.to_string())?;
    let line = install::install_locked(component, &prepared, &previous.proxy, &Default::default())?;
    let mut next = install::read_journal(&prepared)?;
    let old_names: BTreeSet<_> = previous
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    // Verify every old file and every new destination before changing the game.
    for entry in &mut next.files {
        let target = install::safe_target(&game, &entry.path)?;
        if target.exists() && !old_names.contains(entry.path.as_str()) {
            return Err(format!(
                "File already exists; update stopped: {}",
                entry.path
            ));
        }
        if matches!(entry.path.as_str(), "OptiScaler.ini" | "fakenvapi.ini") && target.is_file() {
            let source = install::safe_target(&prepared, &entry.path)?;
            let text = merge_config(&read_file(&target)?, &read_file(&source)?);
            fs::write(&source, text).map_err(|e| e.to_string())?;
            entry.sha256 = install::hash(&source)?;
        }
    }
    let new_journal = serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?;
    fs::write(prepared.join(install::JOURNAL), new_journal).map_err(|e| e.to_string())?;
    let mut backed_up = Vec::new();
    let mut written = Vec::new();
    let result = (|| -> Result<(), String> {
        for path in owned
            .iter()
            .chain(std::iter::once(&game.join(install::JOURNAL)))
        {
            let relative = path.strip_prefix(&game).map_err(|e| e.to_string())?;
            let backup = stage.path.join("backup").join(relative);
            fs::create_dir_all(backup.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::rename(path, &backup).map_err(|e| e.to_string())?;
            backed_up.push((backup, path.clone()));
        }
        for name in next
            .files
            .iter()
            .map(|entry| entry.path.as_str())
            .chain([install::JOURNAL])
        {
            let target = install::safe_target(&game, name)?;
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            // create_new prevents replacing a file added while the update was prepared.
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| e.to_string())?;
            written.push(target);
            let mut input = fs::File::open(prepared.join(name)).map_err(|e| e.to_string())?;
            std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
            output.sync_all().map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        for path in written.iter().rev() {
            let _ = fs::remove_file(path);
        }
        for (backup, target) in backed_up.iter().rev() {
            if fs::rename(backup, target).is_err() {
                stage.recovery = true;
            }
        }
        if stage.recovery {
            return Err(format!(
                "{error}. Recovery files kept in {}",
                stage.path.display()
            ));
        }
        return Err(error);
    }
    Ok(line)
}
