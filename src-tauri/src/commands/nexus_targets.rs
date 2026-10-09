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
    let (kind, rest) = if let Some(path) = relative.strip_prefix("@local/") {
        ("AppData/Local", path)
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
    Ok((root, rest))
}
