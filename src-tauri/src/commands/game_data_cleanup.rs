//! Complete uninstall removes only data registered to the selected PeliGames entries.
use crate::core::installed_library::InstalledEntry;
use std::{collections::BTreeSet,fs,path::{Path,PathBuf},sync::MutexGuard};

pub(crate) struct Cleanup {
    _backup: MutexGuard<'static,()>,
    _nexus: MutexGuard<'static,()>,
    entries: Vec<InstalledEntry>,
    workspace: super::nexus_local::Workspace,
    ids: BTreeSet<String>,
    directories: BTreeSet<PathBuf>,
    full: bool,
}
fn associated(game:&super::nexus_local::NexusGame, entries:&[InstalledEntry])->bool {
    if game.game["launcher"].as_str()!=Some("PeliGames") {return false;}
    entries.iter().any(|entry|game.game["path"].as_str()==Some(entry.path.as_str())
        || (!game.compat_data.is_empty() && Path::new(&game.compat_data)==Path::new(&entry.prefix)))
}
fn real_tree(path:&Path)->Result<(),String>{
    if !path.is_absolute() || path.parent().is_none(){return Err("Pasta de dados inválida.".into());}
    for ancestor in path.ancestors(){if fs::symlink_metadata(ancestor).is_ok_and(|m|m.file_type().is_symlink()){return Err(format!("Pasta de dados redirecionada: {}",path.display()));}}
    if let Ok(m)=fs::symlink_metadata(path){if !m.is_dir(){return Err("Armazenamento do jogo não é uma pasta.".into());}}
    Ok(())
}
impl Cleanup {
    pub(crate) fn prepare(entries:Vec<InstalledEntry>,full:bool)->Result<Self,String>{
        let backup=super::mod_backup::migration_lock()?;
        let nexus=super::nexus_local::STORAGE.lock().map_err(|e|e.to_string())?;
        let file=super::nexus_local::root()?.join("library.json");
        let workspace=match fs::read(file){Ok(bytes)=>serde_json::from_slice::<super::nexus_local::Workspace>(&bytes).map_err(|e|e.to_string())?,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Default::default(),Err(e)=>return Err(e.to_string())};
        let mut ids=BTreeSet::new();let mut directories=BTreeSet::new();
        for game in workspace.games.iter().filter(|game|associated(game,&entries)){
            super::nexus_profile::idle(game)?;
            ids.insert(game.id.clone());
            if full {
                directories.insert(super::nexus_profile::directory(game)?);
                directories.insert(super::nexus_storage::legacy_directory(game)?);
                directories.insert(super::nexus_durable::portable(game)?);

            } else {directories.insert(super::nexus_profile::directory(game)?.join("logs"));}
        }
        if full {
            for entry in entries.iter().filter(|entry|entry.profile.is_none()) {
                let prefix=Path::new(&entry.prefix);
                let owner=if prefix.join("installation.json").is_file(){Some(prefix)}else{prefix.parent().filter(|parent|parent.join("installation.json").is_file())};
                if let Some(owner)=owner {
                    let record:crate::core::installed_library::InstallationRecord=serde_json::from_slice(&fs::read(owner.join("installation.json")).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
                    if record.prefix==entry.prefix && record.entries.iter().all(|candidate|entries.iter().any(|removed|removed.path==candidate.path)) {
                        directories.insert(crate::core::installation_layout::profiles_dir(owner,prefix)?);
                    }
                }
            }
        }
        for path in &directories{real_tree(path)?;}
        Ok(Self{_backup:backup,_nexus:nexus,entries,workspace,ids,directories,full})
    }
    pub(crate) fn finish(mut self)->Result<(),String>{
        for entry in &self.entries{crate::core::game_logs::remove(&entry.path)?;}
        for id in &self.ids {
            if !self.full{continue;}
            let alias=super::nexus_local::root()?.join("games").join(id);
            if fs::symlink_metadata(&alias).is_ok_and(|meta|meta.file_type().is_symlink()) {
                let target=fs::read_link(&alias).map_err(|e|e.to_string())?;
                if target.components().count()==1 && target.file_name().and_then(|name|name.to_str()).is_some_and(|name|name.ends_with(&format!("--{id}"))) {fs::remove_file(alias).map_err(|e|e.to_string())?;}
            }
        }
        for directory in &self.directories{
            if directory.exists(){
                crate::core::bounded_logs::remove_directory(directory).map_err(|e|format!("Não foi possível remover dados do jogo em {}: {e}",directory.display()))?;
                if directory.file_name().is_some_and(|name|name=="profiles") {if let Some(parent)=directory.parent(){let _=fs::remove_dir(parent);}}
            }
        }
        if self.full {
            super::mod_backup::forget_uninstalled(&self.ids)?;
            self.workspace.games.retain(|game|!self.ids.contains(&game.id));
            if !self.ids.is_empty(){super::nexus_local::save(&self.workspace)?;}
            let remaining=crate::core::installed_library::list(false)?;
            for entry in &self.entries{
                if !remaining.iter().any(|other|other.directory==entry.directory){super::launch_profiles::remove_installation(Path::new(&entry.directory))?;}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cleanup_matches_only_the_owned_entry_and_never_a_store_game_or_another_copy() {
        let entries:Vec<InstalledEntry>=serde_json::from_value(serde_json::json!([{"path":"peligames:one","name":"Game","directory":"/games/game","executable":"/games/game/game.exe","prefix":"/prefix/one","proton":"/runner","cover_url":null,"launcher":"PeliGames"}])).unwrap();
        let mut game:super::super::nexus_local::NexusGame=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4(),"game":{"path":"peligames:one","launcher":"PeliGames"},"platform":"proton","mods":[],"profile":null,"running":false,"compat_data":"/prefix/one","proton":"/runner","adapter":"","nexus_domain":"","detected_mods":[]})).unwrap();
        assert!(associated(&game,&entries));game.game["launcher"]="Steam".into();assert!(!associated(&game,&entries));
        game.game["launcher"]="PeliGames".into();game.game["path"]="peligames:two".into();game.compat_data="/prefix/two".into();assert!(!associated(&game,&entries));
    }
}

/// Copy recoveries outside a prefix before its recursive removal.
pub(crate) fn preserve_backups(entries:&[InstalledEntry])->Result<(),String>{
    let _backup=super::mod_backup::migration_lock()?;
    let _nexus=super::nexus_local::STORAGE.lock().map_err(|e|e.to_string())?;
    let file=super::nexus_local::root()?.join("library.json");
    let workspace:super::nexus_local::Workspace=match fs::read(file){Ok(bytes)=>serde_json::from_slice(&bytes).map_err(|e|e.to_string())?,Err(e)if e.kind()==std::io::ErrorKind::NotFound=>Default::default(),Err(e)=>return Err(e.to_string())};
    for game in workspace.games.iter().filter(|game|associated(game,entries)) {
        super::mod_backup::preserve_recovery(game)?;
    }
    Ok(())
}
