//! Use the current PeliGames entry when a mod view starts a registered game.
use super::nexus_local::NexusGame;
use crate::core::{game_installation, installed_library};
use std::{fs, path::PathBuf};

pub(super) fn refresh(game: &mut NexusGame) -> Result<bool, String> {
    if game.platform != "proton" || game.game["launcher"] != "PeliGames" {
        return Ok(false);
    }
    let entries = installed_library::list(false)?;
    let path = game.game["path"].as_str().unwrap_or("");
    let entry = entries.iter().find(|entry| entry.path == path).ok_or(
        "Este jogo não está mais registrado na biblioteca PeliGames. Atualize a biblioteca.",
    )?;
    let changed = game.compat_data != entry.prefix
        || game.proton != entry.proton
        || game.game["prefix"].as_str() != Some(entry.prefix.as_str())
        || game.game["proton"].as_str() != Some(entry.proton.as_str());
    apply(game, entry);
    Ok(changed)
}
fn apply(game: &mut NexusGame, entry: &installed_library::InstalledEntry) {
    game.compat_data = entry.prefix.clone();
    game.proton = entry.proton.clone();
    game.game["prefix"] = entry.prefix.clone().into();
    game.game["proton"] = entry.proton.clone().into();
}
pub(super) fn runner(game: &NexusGame) -> Result<(PathBuf, PathBuf), String> {
    if game.game["launcher"] != "PeliGames" {
        return super::nexus_deployment::validate_runner(&game.compat_data, &game.proton);
    }
    // Validate both choices before creating the new folder. Wine/UMU initializes it.
    let proton = game_installation::validate_proton(&game.proton)?;
    let prefix = installed_library::validate_replacement_prefix(&game.compat_data)?;
    crate::core::installed_library::ensure_prefix_writable(&prefix)?;
    fs::create_dir_all(&prefix)
        .map_err(|error| format!("Não foi possível criar o prefixo: {error}"))?;
    Ok((prefix, proton))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;
    fn game() -> NexusGame {
        serde_json::from_value(json!({"id":"fixture","game":{"launcher":"PeliGames","path":"fixture.exe"},
            "platform":"proton","mods":[],"profile":null,"running":false,"compat_data":"old","proton":"old",
            "adapter":"","nexus_domain":"","detected_mods":[]})).unwrap()
    }
    #[test]
    fn registered_prefix_refresh_keeps_mod_profile_and_new_uninitialized_path() {
        let mut game = game();
        game.profile = Some("keep-profile".into());
        let entry: installed_library::InstalledEntry=serde_json::from_value(json!({"name":"Game","path":"fixture.exe",
            "directory":"/game","executable":"/game/game.exe","prefix":"/new-prefix","proton":"/new-runner","cover_url":null,"launcher":"PeliGames"})).unwrap();
        apply(&mut game, &entry);
        assert_eq!(game.compat_data, "/new-prefix");
        assert_eq!(game.proton, "/new-runner");
        assert_eq!(game.game["prefix"], "/new-prefix");
        assert_eq!(game.profile.as_deref(), Some("keep-profile"));
    }
    #[test]
    fn registered_launch_creates_only_the_selected_prefix_and_validates_runner_first() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "peligames-nexus-new-prefix-{}",
            uuid::Uuid::new_v4()
        ));
        let proton = root.join("runner");
        fs::create_dir_all(&proton).unwrap();
        fs::write(proton.join("proton"), b"#!/bin/sh\n").unwrap();
        fs::set_permissions(proton.join("proton"), fs::Permissions::from_mode(0o755)).unwrap();
        let mut game = game();
        game.compat_data = root.join("new/prefix").to_string_lossy().into();
        game.proton = proton.to_string_lossy().into();
        let (prefix, _) = runner(&game).unwrap();
        assert_eq!(prefix, Path::new(&game.compat_data));
        assert!(prefix.is_dir());
        assert!(
            fs::read_dir(&prefix).unwrap().next().is_none(),
            "initialization belongs to the runner"
        );
        fs::write(prefix.join("save.dat"), b"keep").unwrap();
        assert!(runner(&game).is_err());
        game.compat_data = root.join("not-created").to_string_lossy().into();
        game.proton = root.join("missing-runner").to_string_lossy().into();
        assert!(runner(&game).is_err());
        assert!(!root.join("not-created").exists());
        assert_eq!(fs::read(prefix.join("save.dat")).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
}
