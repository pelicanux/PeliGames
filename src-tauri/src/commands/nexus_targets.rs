//! Resolve declared game and Wine user-data targets without guessing a host path.
use std::{
    fs,
    path::{Path, PathBuf},
};
pub(super) fn root(
    game_root: &Path,
    prefix: &str,
    relative: &str,
) -> Result<(PathBuf, String), String> {
    if let Some(path) = relative.strip_prefix("@msc/") {
        let settings = game_root.join("doorstop_config.ini");
        if !fs::canonicalize(&settings).is_ok_and(|path| path.starts_with(game_root)) {
            return Err("Configure o MSC Mod Loader antes de instalar mods.".into());
        }
        let text = fs::read_to_string(settings).map_err(|e| e.to_string())?;
        if text.len() > 65536 {
            return Err("Configuração MSC grande demais.".into());
        }
        let mut inside = false;
        let mut mode = None;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                inside = line[1..line.len() - 1].eq_ignore_ascii_case("MSCLoader");
            } else if inside {
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim().eq_ignore_ascii_case("mods") {
                        mode = Some(value.trim());
                    }
                }
            }
        }
        return match mode {
            Some("GF") => Ok((game_root.to_path_buf(), format!("Mods/{path}"))),
            Some("MD") => root(
                game_root,
                prefix,
                &format!("@documents/MySummerCar/Mods/{path}"),
            ),
            Some("AD") => root(
                game_root,
                prefix,
                &format!("@low/Amistech/My Summer Car/Mods/{path}"),
            ),
            _ => Err(
                "Pasta de mods MSC não identificada. Configure GF, MD ou AD no MSC Mod Loader."
                    .into(),
            ),
        };
    }
    if let Some(path) = relative.strip_prefix("@msfs/") {
        // Microsoft Flight Simulator: the packages folder the player chose, from
        // `InstalledPackagesPath` in UserCfg.opt, or `Packages` beside it.
        let (base, config) = root(game_root, prefix, "@roaming/Microsoft Flight Simulator/UserCfg.opt")?;
        let config = base.join(config);
        let text = fs::read_to_string(&config).map_err(|_| "Inicie o Microsoft Flight Simulator uma vez no prefixo selecionado para criar o UserCfg.opt.".to_string())?;
        let configured = text.lines().map(str::trim).find_map(|line| line.strip_prefix("InstalledPackagesPath"))
            .map(|value| value.trim().trim_matches('"').to_string()).filter(|value| !value.is_empty());
        let packages = match configured {
            Some(value) => windows_path(prefix, &value)?,
            None => config.parent().ok_or("UserCfg.opt inválido.")?.join("Packages"),
        };
        if !packages.is_dir() {
            return Err(format!("Pasta de pacotes do Microsoft Flight Simulator não encontrada: {}", packages.display()));
        }
        return Ok((packages.clone(), existing_spelling(&packages, path)));
    }
    let (kind, rest) = if let Some(path) = relative.strip_prefix("@local/") {
        ("AppData/Local", path)
    } else if let Some(path) = relative.strip_prefix("@roaming/") {
        ("AppData/Roaming", path)
    } else if let Some(path) = relative.strip_prefix("@low/") {
        ("AppData/LocalLow", path)
    } else if let Some(path) = relative.strip_prefix("@documents/") {
        ("Documents", path)
    } else {
        return Ok((game_root.to_path_buf(), relative.to_string()));
    };
    if prefix.is_empty() {
        return Err("Este mod usa os dados do usuário Windows. Selecione o prefixo do jogo em Configurações.".into());
    }
    let (_, wine) = super::nexus_prefix::resolve(Path::new(prefix))?;
    let users = wine.join("drive_c/users");
    let user = if users.join("steamuser").is_dir() {
        users.join("steamuser")
    } else {
        let names: Vec<_> = fs::read_dir(&users)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().is_dir()
                    && !["public", "default", "default user", "all users"]
                        .contains(&entry.file_name().to_string_lossy().to_lowercase().as_str())
            })
            .collect();
        if names.len() != 1 {
            return Err("Não foi possível identificar um único usuário Windows no prefixo.".into());
        }
        names[0].path()
    };
    let root = user.join(kind);
    // Reject prefix folders pointing outside drive_c, including Documents links.
    // Never silently apply a mod to the launcher's own home directory.
    for parent in root.ancestors().take_while(|p| *p != wine) {
        if fs::symlink_metadata(parent).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err(format!(
                "A pasta do prefixo é um link externo; selecione um prefixo com dados locais: {}",
                parent.display()
            ));
        }
    }
    let mut rest = rest.to_string();
    if rest.starts_with("Electronic Arts/The Sims 4/") {
        let ea = root.join("Electronic Arts");
        let matches = [
            "The Sims 4",
            "Die Sims 4",
            "Los Sims 4",
            "Les\u{a0}Sims\u{a0}4",
            "De Sims 4",
        ]
        .into_iter()
        .filter(|name| ea.join(name).is_dir())
        .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err("Inicie The Sims 4 no prefixo selecionado; é necessária uma única pasta de usuário do jogo.".into());
        }
        rest = rest.replacen(
            "Electronic Arts/The Sims 4/",
            &format!("Electronic Arts/{}/", matches[0]),
            1,
        );
    }
    Ok((root.clone(), existing_spelling(&root, &rest)))
}

/// Host path of a Windows path inside the prefix, through its drive letters
/// (`dosdevices/c:` → `drive_c`, `z:` → `/`).
fn windows_path(prefix: &str, value: &str) -> Result<PathBuf, String> {
    let (_, wine) = super::nexus_prefix::resolve(Path::new(prefix))?;
    let (drive, rest) = value.split_once(':').filter(|(drive, _)| drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic()))
        .ok_or_else(|| format!("Caminho do Windows inválido: {value}"))?;
    let device = fs::canonicalize(wine.join("dosdevices").join(format!("{}:", drive.to_ascii_lowercase())))
        .map_err(|_| format!("Unidade {drive}: não existe no prefixo."))?;
    let mut path = device;
    for part in rest.split(['\\', '/']).filter(|part| !part.is_empty()) {
        if part == ".." || part == "." { return Err(format!("Caminho do Windows inválido: {value}")); }
        path.push(part);
    }
    Ok(path)
}
/// Windows ignores letter case, so a game and a tool may spell the same file
/// differently (`Plugins.txt` / `plugins.txt`). Use the spelling already on disk
/// for every existing segment (only when exactly one entry matches); the rest is kept.
fn existing_spelling(root: &Path, relative: &str) -> String {
    let mut current = root.to_path_buf();
    let mut parts = Vec::new();
    let mut resolving = true;
    for part in relative.split('/') {
        let mut chosen = part.to_string();
        if resolving {
            if !current.join(part).exists() {
                let mut found = fs::read_dir(&current).into_iter().flatten().flatten().take(20_000)
                    .filter(|entry| entry.file_name().to_string_lossy().eq_ignore_ascii_case(part))
                    .map(|entry| entry.file_name().to_string_lossy().into_owned());
                match (found.next(), found.next()) {
                    (Some(name), None) => chosen = name,
                    _ => resolving = false,
                }
            }
            current.push(&chosen);
        }
        parts.push(chosen);
    }
    parts.join("/")
}

#[cfg(test)]
mod spelling_tests {
    use super::*;
    #[test]
    fn existing_files_keep_the_spelling_on_disk() {
        let root = std::env::temp_dir().join(format!("peligames-spelling-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("Skyrim Special Edition")).unwrap();
        fs::write(root.join("Skyrim Special Edition/Plugins.txt"), "").unwrap();
        assert_eq!(existing_spelling(&root, "Skyrim Special Edition/plugins.txt"), "Skyrim Special Edition/Plugins.txt");
        assert_eq!(existing_spelling(&root, "skyrim special edition/New.txt"), "Skyrim Special Edition/New.txt");
        assert_eq!(existing_spelling(&root, "Other/plugins.txt"), "Other/plugins.txt");
        // Two spellings on disk: ambiguous, left to the deployment's own refusal.
        fs::write(root.join("Skyrim Special Edition/plugins.txt"), "").unwrap();
        assert_eq!(existing_spelling(&root, "Skyrim Special Edition/PLUGINS.TXT"), "Skyrim Special Edition/PLUGINS.TXT");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn flight_simulator_packages_follow_user_cfg() {
        let base = std::env::temp_dir().join(format!("msfs-{}", uuid::Uuid::new_v4()));
        let pfx = base.join("prefix/pfx");
        let roaming = pfx.join("drive_c/users/steamuser/AppData/Roaming/Microsoft Flight Simulator");
        fs::create_dir_all(&roaming).unwrap();
        fs::write(pfx.join("system.reg"), "").unwrap();
        fs::create_dir_all(pfx.join("dosdevices")).unwrap();
        std::os::unix::fs::symlink("../drive_c", pfx.join("dosdevices/c:")).unwrap();
        let prefix = base.join("prefix").to_string_lossy().into_owned();
        // Not started yet: no UserCfg.opt.
        assert!(root(&base, &prefix, "@msfs/Community/x").is_err());
        // Default: Packages beside UserCfg.opt.
        fs::write(roaming.join("UserCfg.opt"), "Version 63\n").unwrap();
        fs::create_dir_all(roaming.join("Packages/Community")).unwrap();
        let (dir, rest) = root(&base, &prefix, "@msfs/Community/pkg/manifest.json").unwrap();
        assert_eq!(dir.join(rest), roaming.join("Packages/Community/pkg/manifest.json"));
        // Player's folder, as a Windows path inside the prefix.
        let custom = pfx.join("drive_c/MSFS");
        fs::create_dir_all(custom.join("Community")).unwrap();
        fs::write(roaming.join("UserCfg.opt"), "Version 63\nInstalledPackagesPath \"C:\\MSFS\"\n").unwrap();
        let (dir, rest) = root(&base, &prefix, "@msfs/Community/pkg/manifest.json").unwrap();
        assert_eq!(fs::canonicalize(dir).unwrap().join(rest), fs::canonicalize(&custom).unwrap().join("Community/pkg/manifest.json"));
        fs::write(roaming.join("UserCfg.opt"), "InstalledPackagesPath \"C:\\..\\x\"\n").unwrap();
        assert!(root(&base, &prefix, "@msfs/Community/x").is_err());
        fs::remove_dir_all(base).unwrap();
    }
}
