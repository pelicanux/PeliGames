//! Local Steam preflight for direct launches, which bypass Steam's update handling.
use std::{fs, path::Path};

fn field<'a>(manifest: &'a str, key: &str) -> Option<&'a str> {
    // Only AppState fields: nested depot/user configuration is not installation state.
    let mut depth = 0;
    for line in manifest.lines() {
        let line = line.trim();
        if line == "{" {
            depth += 1;
        }
        if line == "}" {
            depth -= 1;
        }
        if depth == 1 {
            let mut quoted = line.split('"');
            quoted.next();
            if quoted.next() == Some(key) {
                quoted.next();
                return quoted.next();
            }
        }
    }
    None
}

fn problem(manifest: &str) -> Option<&'static str> {
    let flags = field(manifest, "StateFlags")?.parse::<u64>().ok()?;
    // Bit flags, not exclusive values: FullyInstalled + UpdateRequired is still pending.
    // https://github.com/emily33901/SteamStructs/blob/master/EAppState.h
    if flags & 2048 != 0 {
        return Some("A Steam está desinstalando este jogo.");
    }
    if flags & 32768 != 0 {
        return Some("A Steam está movendo a instalação deste jogo.");
    }
    if flags & (256 | 512 | 1024) != 0 {
        return Some("Há uma atualização ou instalação em andamento/pausada na Steam.");
    }
    if flags & (2 | 8) != 0 {
        return Some("Este jogo tem uma atualização pendente na Steam.");
    }
    if flags & (32 | 128) != 0 {
        return Some("A Steam informa arquivos ausentes ou corrompidos neste jogo. Verifique a integridade dos arquivos na Steam.");
    }
    None
}

pub(super) fn check(root: &Path, app_id: Option<&str>) -> Result<(), String> {
    let Some(common) = root
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "common"))
    else {
        return Ok(());
    };
    let Some(steamapps) = common
        .parent()
        .filter(|p| p.file_name().is_some_and(|n| n == "steamapps"))
    else {
        return Ok(());
    };
    let id = app_id.filter(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()));
    let manifests = if let Some(id) = id {
        vec![steamapps.join(format!("appmanifest_{id}.acf"))]
    } else {
        fs::read_dir(steamapps)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|p| {
                p.extension().is_some_and(|ext| ext == "acf")
                    && p.file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with("appmanifest_"))
            })
            .collect()
    };
    for path in manifests {
        // Missing, unreadable or unknown state is not evidence of a pending update.
        if !fs::metadata(&path).is_ok_and(|m| m.len() <= 256 * 1024) {
            continue;
        }
        let Ok(manifest) = fs::read_to_string(&path) else {
            continue;
        };
        if field(&manifest, "installdir") != root.file_name().and_then(|n| n.to_str()) {
            continue;
        }
        if id.is_some_and(|id| field(&manifest, "appid") != Some(id)) {
            continue;
        }
        if let Some(problem) = problem(&manifest) {
            return Err(format!("{problem} Abra a Steam → Downloads e conclua a operação antes de iniciar com mods. Depois, tente novamente no PeliGames."));
        }
        break;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest(flags: &str) -> String {
        format!("\"AppState\"\n{{\n\"appid\" \"3241660\"\n\"installdir\" \"REPO\"\n\"StateFlags\" \"{flags}\"\n\"BytesToDownload\" \"123\"\n\"BytesDownloaded\" \"123\"\n}}")
    }
    #[test]
    fn update_flags_override_installed_bit() {
        for flag in [2, 8, 256, 512, 1024] {
            assert!(problem(&manifest(&(flag | 4).to_string())).is_some());
        }
        for flag in [4, 20, 8196] {
            assert!(problem(&manifest(&flag.to_string())).is_none());
        }
        assert!(problem(&manifest("unknown")).is_none());
    }
    #[test]
    fn checks_matching_installation_even_when_directory_missing() {
        let temp =
            std::env::temp_dir().join(format!("peligames-steam-preflight-{}", std::process::id()));
        let steamapps = temp.join("steamapps");
        fs::create_dir_all(&steamapps).unwrap();
        let path = steamapps.join("appmanifest_3241660.acf");
        fs::write(&path, manifest("6")).unwrap();
        let root = steamapps.join("common/REPO");
        assert!(check(&root, Some("3241660"))
            .unwrap_err()
            .contains("atualização pendente"));
        assert!(check(&root, None).is_err());
        assert!(check(&steamapps.join("common/Other"), Some("3241660")).is_ok());
        fs::write(&path, manifest("4")).unwrap();
        assert!(check(&root, Some("3241660")).is_ok());
        fs::remove_file(path).unwrap();
        assert!(check(&root, Some("3241660")).is_ok());
        fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn ignores_nested_fields() {
        assert_eq!(
            field(
                "\"AppState\"\n{\n\"Depots\"\n{\n\"StateFlags\" \"2\"\n}\n}",
                "StateFlags"
            ),
            None
        );
    }
}
