//! Keep Steam's client directory available after UMU prepares its environment.
//! The adapter delegates to the original Proton; it never modifies that runner,
//! Steam's library, or the game's prefix itself.
use std::{fs, path::{Path, PathBuf}};
use sha2::{Digest, Sha256};

fn client_directory(home: &Path) -> Result<PathBuf, String> {
    [".local/share/Steam", ".steam/steam", ".steam/root", ".var/app/com.valvesoftware.Steam/.local/share/Steam", ".var/app/com.valvesoftware.Steam/data/Steam"]
        .into_iter().map(|relative| home.join(relative))
        .find(|root| root.join("legacycompat/steamclient64.dll").is_file() && root.join("legacycompat/GameOverlayRenderer64.dll").is_file())
        .and_then(|root| root.canonicalize().ok())
        .ok_or_else(|| "Não foi possível localizar os componentes do cliente Steam. Abra e atualize a Steam antes de tentar novamente.".into())
}
fn quote(value: &Path) -> String { format!("'{}'", value.to_string_lossy().replace('\'', "'\\''")) }

#[cfg(unix)]
fn adapter(base: &Path, proton: &Path, steam: &Path) -> Result<PathBuf, String> {
    use std::os::unix::{fs::{symlink, PermissionsExt}, ffi::OsStrExt};
    let proton = proton.canonicalize().map_err(|e| e.to_string())?;
    let manifest = fs::read_to_string(proton.join("toolmanifest.vdf")).map_err(|_| "O Proton selecionado não possui um manifesto compatível com o UMU.".to_string())?;
    let entry = manifest.lines().find_map(|line| {
        let fields: Vec<_> = line.split('"').collect();
        (fields.get(1) == Some(&"commandline")).then(|| fields.get(3).copied()).flatten()
    });
    if !entry.is_some_and(|entry| entry == "/proton" || entry.starts_with("/proton ")) {
        return Err("O ponto de entrada deste Proton não é compatível com a comunicação Steam do PeliGames.".into());
    }
    fs::create_dir_all(base).map_err(|e| e.to_string())?;
    if base.is_symlink() { return Err("A pasta do adaptador Steam não pode ser um link simbólico.".into()); }
    fs::set_permissions(base, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    // A separate adapter per build avoids stale links when a runner updates in place.
    let mut hash = Sha256::new(); hash.update(proton.as_os_str().as_bytes()); hash.update([0]); hash.update(steam.as_os_str().as_bytes());
    hash.update(&manifest);
    let mut entries = fs::read_dir(&proton).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in &entries {
        if let Ok(meta) = entry.metadata() {
            hash.update(entry.file_name().as_os_str().as_bytes());
            hash.update(meta.len().to_le_bytes());
            if let Ok(at) = meta.modified().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).map_err(std::io::Error::other)) { hash.update(at.as_nanos().to_le_bytes()); }
        }
    }
    let target = base.join(format!("{:x}", hash.finalize()));
    let script = format!("#!/bin/sh\nexport STEAM_COMPAT_CLIENT_INSTALL_PATH={}\nexport PROTONPATH={}\nexport UMU_USE_STEAM=1\nexec {} \"$@\"\n", quote(steam), quote(&proton), quote(&proton.join("proton")));
    if target.is_dir() && !target.is_symlink() && fs::read_to_string(target.join("proton")).ok().as_deref() == Some(&script) { return Ok(target); }
    if target.symlink_metadata().is_ok() { return Err("O adaptador Steam está inconsistente. Remova apenas a pasta de adaptadores do PeliGames e tente novamente.".into()); }
    let staging = base.join(format!(".new-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        for entry in &entries {
            if entry.file_name() != "proton" { symlink(entry.path(), staging.join(entry.file_name())).map_err(|e| e.to_string())?; }
        }
        fs::write(staging.join("proton"), script).map_err(|e| e.to_string())?;
        fs::set_permissions(staging.join("proton"), fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        fs::rename(&staging, &target).map_err(|e| e.to_string())?;
        Ok(target)
    })();
    if result.is_err() { let _ = fs::remove_dir_all(staging); }
    result
}

pub(crate) fn runner(proton: &Path) -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let steam = client_directory(&home)?;
    #[cfg(unix)] { adapter(&super::paths::app_path("runners/steam-client-adapters")?, proton, &steam) }
    #[cfg(not(unix))] { let _ = (proton, steam); Err("A integração Steam está disponível apenas no Linux.".into()) }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn steam_adapter_restores_the_client_path_after_umu_without_modifying_the_runner() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("peligames-steam-client-{}", uuid::Uuid::new_v4()));
        let proton = root.join("Proton's runner"); let steam = root.join("Steam client");
        fs::create_dir_all(&proton).unwrap(); fs::create_dir_all(&steam).unwrap();
        let original = "#!/bin/sh\nprintf '%s\\n' \"$STEAM_COMPAT_CLIENT_INSTALL_PATH\" \"$PROTONPATH\" \"$UMU_USE_STEAM\" \"$@\"\n";
        fs::write(proton.join("proton"), original).unwrap();
        fs::set_permissions(proton.join("proton"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(proton.join("toolmanifest.vdf"), "\"manifest\"\n{\n\"commandline\" \"/proton %verb%\"\n\"require_tool_appid\" \"4183110\"\n\"compatmanager_layer_name\" \"proton\"\n}").unwrap();
        let wrapped = adapter(&root.join("adapters"), &proton, &steam).unwrap();
        assert!(wrapped.join("toolmanifest.vdf").is_symlink());
        let output = std::process::Command::new(wrapped.join("proton")).env("STEAM_COMPAT_CLIENT_INSTALL_PATH", "").env("UMU_USE_STEAM", "0").args(["waitforexitandrun", "a game.exe", "--flag"]).output().unwrap();
        assert!(output.status.success());
        let values = String::from_utf8(output.stdout).unwrap();
        assert_eq!(values.lines().collect::<Vec<_>>(), [steam.to_str().unwrap(), proton.to_str().unwrap(), "1", "waitforexitandrun", "a game.exe", "--flag"]);
        assert_eq!(fs::read_to_string(proton.join("proton")).unwrap(), original);
        assert_eq!(adapter(&root.join("adapters"), &proton, &steam).unwrap(), wrapped);
        if std::env::var_os("PELIGAMES_TEST_LOCAL_UMU").is_some() {
            // A harmless stand-in for Proton verifies the real installed UMU/runtime
            // boundary. This never executes Wine, a game, or Steam's binaries.
            let game = root.join("a game.exe"); fs::write(&game, b"fixture").unwrap();
            let output = std::process::Command::new("/usr/bin/umu-run").arg(&game).arg("--flag")
                .env("WINEPREFIX", root.join("prefix")).env("PROTONPATH", &wrapped)
                .env("GAMEID", "umu-default").env("UMU_USE_STEAM", "1")
                .env("UMU_RUNTIME_UPDATE", "0").env("UMU_LOG", "info")
                .env("UMU_HTTP_TIMEOUT", "3").env("UMU_HTTP_RETRIES", "0")
                .env_remove("UMU_NO_PROTON").env_remove("UMU_NO_RUNTIME").output().unwrap();
            let text = String::from_utf8_lossy(&output.stdout);
            assert!(output.status.success(), "Local UMU adapter failed: {}", String::from_utf8_lossy(&output.stderr));
            assert!(text.contains(steam.to_str().unwrap()));
            assert!(text.contains(proton.to_str().unwrap()));
            println!("Real UMU/runtime preserved the restored Steam directory and delegated arguments.");
        }
        assert!(client_directory(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
