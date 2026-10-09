//! Human-readable per-game storage, with legacy path compatibility.
use std::{fs, path::{Path,PathBuf}, sync::Mutex};
static MIGRATION: Mutex<()> = Mutex::new(());
fn label(name: &str) -> String {
    let value: String = name.chars().take(48).map(|c|if c.is_alphanumeric(){c.to_ascii_lowercase()}else{'-'}).collect();
    let value=value.trim_matches('-');if value.is_empty(){"jogo".into()}else{value.into()}
}
fn regular_directory(path: &Path) -> Result<(),String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => Ok(()),
        Ok(_) => Err("Pasta de mods inválida ou simbólica.".into()),
        Err(error) if error.kind()==std::io::ErrorKind::NotFound => fs::create_dir(path).map_err(|e|e.to_string()),
        Err(error) => Err(error.to_string()),
    }
}
pub(super) fn directory(game: &super::nexus_local::NexusGame) -> Result<PathBuf,String> {
    directory_at(&super::nexus_local::root()?,&game.id,game.game["name"].as_str().unwrap_or("jogo"))
}
fn directory_at(base: &Path,id: &str,name: &str) -> Result<PathBuf,String> {
    uuid::Uuid::parse_str(id).map_err(|_|"Identificador de jogo inválido.")?;
    let _lock=MIGRATION.lock().map_err(|e|e.to_string())?;
    let parent=base.join("games");regular_directory(&parent)?;
    let old=parent.join(id);let suffix=format!("--{id}");
    if fs::symlink_metadata(&old).is_ok_and(|m|m.file_type().is_symlink()) {
        let link=fs::read_link(&old).map_err(|e|e.to_string())?;
        if link.components().count()!=1 || !link.file_name().and_then(|n|n.to_str()).is_some_and(|n|n.ends_with(&suffix)) {return Err("Link legado de mods inválido.".into());}
        let target=parent.join(link);
        if !fs::symlink_metadata(&target).is_ok_and(|m|m.is_dir() && !m.file_type().is_symlink()) {return Err("Pasta legada de mods indisponível.".into());}
        return Ok(target);
    }
    let mut matches=fs::read_dir(&parent).map_err(|e|e.to_string())?.filter_map(Result::ok).filter(|e|e.file_name().to_str().is_some_and(|n|n.ends_with(&suffix))).collect::<Vec<_>>();
    if matches.len()>1 {return Err("Há mais de uma pasta para este jogo. Nenhuma foi alterada.".into());}
    let target=matches.pop().map(|e|e.path()).unwrap_or_else(||parent.join(format!("{}{suffix}",label(name))));
    if let Ok(meta)=fs::symlink_metadata(&target) {if !meta.is_dir() || meta.file_type().is_symlink(){return Err("Pasta do jogo inválida ou simbólica.".into());}}
    match fs::symlink_metadata(&old) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {
            // A running process may reference the old profile in its environment.
            if fs::read_to_string(old.join("session.pid")).ok().and_then(|s|s.trim().parse::<u32>().ok()).is_some_and(|pid|pid>0 && Path::new("/proc").join(pid.to_string()).exists()) {return Ok(old);}
            if target.exists(){return Err("Pastas antiga e nova coexistem. Nenhuma foi sobrescrita.".into());}
            #[cfg(unix)] {
                fs::rename(&old,&target).map_err(|e|e.to_string())?;
                if let Err(error)=std::os::unix::fs::symlink(target.file_name().ok_or("Nome de pasta inválido.")?,&old) {
                    fs::rename(&target,&old).map_err(|rollback|format!("{error}; restauração: {rollback}"))?;
                    return Err(error.to_string());
                }
            }
            #[cfg(not(unix))] {return Ok(old);}
        },
        Ok(_) => return Err("Pasta legada de mods inválida.".into()),
        Err(error) if error.kind()==std::io::ErrorKind::NotFound => {},
        Err(error) => return Err(error.to_string()),
    }
    Ok(target)
}
#[cfg(test)] mod tests {
    use super::*;
    fn base()->PathBuf {let p=std::env::temp_dir().join(format!("peligames-storage-{}",uuid::Uuid::new_v4()));fs::create_dir(&p).unwrap();p}
    #[test] fn readable_unique_and_stable_after_title_change() {
        let base=base();let id=uuid::Uuid::new_v4().to_string();let p=directory_at(&base,&id,"Stardew Valley").unwrap();assert!(p.file_name().unwrap().to_str().unwrap().starts_with("stardew-valley--"));fs::create_dir(&p).unwrap();assert_eq!(directory_at(&base,&id,"Outro título").unwrap(),p);assert_ne!(directory_at(&base,&uuid::Uuid::new_v4().to_string(),"Stardew Valley").unwrap(),p);fs::remove_dir_all(base).unwrap();
    }
    #[test] fn invalid_id_and_path_characters() {let base=base();assert!(directory_at(&base,"../outside","Game").is_err());let p=directory_at(&base,&uuid::Uuid::new_v4().to_string(),"../../Game/Name").unwrap();assert_eq!(p.parent().unwrap(),base.join("games"));fs::remove_dir_all(base).unwrap();}
    #[cfg(unix)] #[test] fn migration_keeps_files_and_legacy_references() {let base=base();let id=uuid::Uuid::new_v4().to_string();let old=base.join("games").join(&id);fs::create_dir_all(old.join("archives")).unwrap();fs::write(old.join("archives/mod.zip"),"data").unwrap();let p=directory_at(&base,&id,"Palworld").unwrap();assert_eq!(fs::read(p.join("archives/mod.zip")).unwrap(),b"data");assert_eq!(fs::canonicalize(&old).unwrap(),p);assert_eq!(directory_at(&base,&id,"Changed name").unwrap(),p);fs::remove_dir_all(base).unwrap();}
    #[cfg(unix)] #[test] fn rejects_external_symlink() {let base=base();let id=uuid::Uuid::new_v4().to_string();fs::create_dir(base.join("games")).unwrap();std::os::unix::fs::symlink("/tmp",base.join("games").join(&id)).unwrap();assert!(directory_at(&base,&id,"Game").is_err());fs::remove_dir_all(base).unwrap();}
    #[cfg(unix)] #[test] fn defers_migration_while_session_process_exists() {let base=base();let id=uuid::Uuid::new_v4().to_string();let old=base.join("games").join(&id);fs::create_dir_all(&old).unwrap();fs::write(old.join("session.pid"),std::process::id().to_string()).unwrap();assert_eq!(directory_at(&base,&id,"Game").unwrap(),old);fs::remove_file(old.join("session.pid")).unwrap();assert_ne!(directory_at(&base,&id,"Game").unwrap(),old);fs::remove_dir_all(base).unwrap();}
    #[test] fn conflicting_folders_are_not_merged_or_overwritten() {let base=base();let id=uuid::Uuid::new_v4().to_string();let named=directory_at(&base,&id,"Game").unwrap();fs::create_dir(&named).unwrap();let old=base.join("games").join(&id);fs::create_dir(&old).unwrap();fs::write(old.join("old.txt"),"old").unwrap();fs::write(named.join("new.txt"),"new").unwrap();assert!(directory_at(&base,&id,"Game").is_err());assert!(old.join("old.txt").is_file());assert!(named.join("new.txt").is_file());fs::remove_dir_all(base).unwrap();}

}
