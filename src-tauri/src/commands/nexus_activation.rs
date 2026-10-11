//! Merge managed activation state; preserve unrelated entries and use deployment rollback.
use super::{
    nexus_local::NexusGame,
    nexus_modules::{ActivationKind, Definition},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
fn plugin(name: &str) -> bool {
    [".esm", ".esp", ".esl"]
        .iter()
        .any(|ext| name.to_ascii_lowercase().ends_with(ext))
}
fn members(paths: impl Iterator<Item = String>, data: &str) -> BTreeSet<String> {
    paths
        .filter_map(|path| {
            path.strip_prefix(&format!("{data}/"))
                .filter(|tail| !tail.contains('/') && plugin(tail))
                .map(str::to_string)
        })
        .collect()
}
fn read(path: &Path) -> Result<String, String> {
    match fs::read(path) {
        Ok(bytes) if bytes.len() > 2 * 1024 * 1024 => {
            Err("Configuração de mods excede 2 MiB.".into())
        }
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| {
            "A configuração existente não está em UTF-8; preservada sem alterações.".into()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}
fn ini(original: &str, section: &str, lines: &[String], remove: impl Fn(&str) -> bool) -> String {
    let mut result = Vec::new();
    let mut inside = false;
    let mut found = false;
    for line in original.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if inside {
                result.extend(lines.iter().cloned());
            }
            inside = trimmed[1..trimmed.len() - 1].eq_ignore_ascii_case(section);
            found |= inside;
        }
        if !inside || !remove(line) {
            result.push(line.to_string());
        }
    }
    if inside {
        result.extend(lines.iter().cloned());
    }
    if !found {
        result.push(format!("[{section}]"));
        result.extend(lines.iter().cloned());
    }
    result.join("\r\n") + "\r\n"
}
fn remove_owned(original: &str, old: &BTreeSet<String>) -> Vec<String> {
    original
        .lines()
        .filter(|line| {
            !old.iter()
                .any(|name| name.eq_ignore_ascii_case(line.trim().trim_start_matches('*')))
        })
        .map(str::to_string)
        .collect()
}
/// Read TES4 master dependencies without loading an entire plugin into memory.
fn masters(path: &Path, legacy: bool) -> Result<Vec<String>, String> {
    use std::io::Read;
    let mut input = fs::File::open(path).map_err(|e| e.to_string())?;
    let header_size = if legacy { 20 } else { 24 };
    let mut header = vec![0; header_size];
    input
        .read_exact(&mut header)
        .map_err(|_| "Cabeçalho de plugin incompleto.")?;
    if &header[..4] != b"TES4" {
        return Err(format!("Plugin Bethesda inválido: {}", path.display()));
    }
    let size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    if size > 2 * 1024 * 1024
        || u32::from_le_bytes(header[8..12].try_into().unwrap()) & 0x40000 != 0
    {
        return Err("Cabeçalho de plugin comprimido ou grande demais.".into());
    }
    let mut records = vec![0; size];
    input
        .read_exact(&mut records)
        .map_err(|_| "Plugin truncado.")?;
    let mut offset = 0;
    let mut result = Vec::new();
    while offset < records.len() {
        if offset + 6 > records.len() {
            return Err("Subregistro de plugin truncado.".into());
        }
        let count =
            u16::from_le_bytes(records[offset + 4..offset + 6].try_into().unwrap()) as usize;
        if offset + 6 + count > records.len() {
            return Err("Subregistro de plugin inválido.".into());
        }
        if &records[offset..offset + 4] == b"MAST" {
            let name = String::from_utf8(records[offset + 6..offset + 6 + count].to_vec())
                .map_err(|_| "Nome de dependência inválido.")?
                .trim_end_matches('\0')
                .to_string();
            if !super::nexus_modules::relative(&name) || name.contains('/') || !plugin(&name) {
                return Err("Dependência de plugin insegura.".into());
            }
            result.push(name);
        }
        offset += 6 + count;
    }
    Ok(result)
}
fn ordered(
    game: &NexusGame,
    module: &Definition,
    runtime: &Path,
    data: &str,
    names: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let mut dependencies = BTreeMap::new();
    for name in names {
        let deps = masters(
            &runtime.join(data).join(name),
            ["oblivion", "oblivionremastered"].contains(&module.domain.as_str()),
        )?;
        for dep in &deps {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(dep))
                && !fs::read_dir(root.join(data))
                    .map_err(|e| e.to_string())?
                    .filter_map(Result::ok)
                    .any(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(dep)
                    })
            {
                return Err(format!(
                    "O plugin {name} depende de {dep}, que não está instalado."
                ));
            }
        }
        dependencies.insert(name.clone(), deps);
    }
    let mut pending = names.clone();
    let mut result = Vec::<String>::new();
    while !pending.is_empty() {
        let next = pending
            .iter()
            .filter(|name| {
                dependencies[*name]
                    .iter()
                    .all(|dep| !pending.iter().any(|n| n.eq_ignore_ascii_case(dep)))
            })
            .min_by_key(|name| {
                (
                    !name.to_ascii_lowercase().ends_with(".esm"),
                    name.to_ascii_lowercase(),
                )
            })
            .cloned()
            .ok_or("Dependências circulares entre plugins.")?;
        pending.remove(&next);
        result.push(next);
    }
    Ok(result)
}
pub(super) fn generated_config(module: &Definition, path: &str) -> bool {
    module
        .activation
        .as_ref()
        .is_some_and(|rule| rule.path == path)
        || module.ue4ss().is_some_and(|layout| layout.mod_folders().iter().any(|folder| path == format!("{folder}/mods.txt")))
        || module.domain == "thesims4"
            && path == "@documents/Electronic Arts/The Sims 4/Options.ini"
        || module.ini_settings.iter().any(|setting| setting.file == path)
}
pub(super) fn prepare(
    game: &NexusGame,
    module: &Definition,
    runtime: &Path,
    old: &BTreeMap<String, String>,
) -> Result<(), String> {
    if let Some(layout) = module.ue4ss() {
        let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
        let folders = layout.mod_folders();
        // Validate live paths before reading; no writes to the game occur during preparation.
        for folder in &folders { super::nexus_deployment::destination(game, &format!("{folder}/mods.txt"))?; }
        let visible = super::nexus_deploy_methods::view(game)?.unwrap_or_else(|| root.to_path_buf());
        prepare_ue4ss(&visible, runtime, old, &folders)?;
    }
    if module.domain == "oblivionremastered" {
        let files = walkdir::WalkDir::new(runtime)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .collect::<Vec<_>>();
        for path in files {
            if path.ends_with("Scripts/main.lua") && path.to_string_lossy().contains("/ue4ss/Mods/")
            {
                let folder = path
                    .parent()
                    .and_then(Path::parent)
                    .ok_or("Pasta UE4SS inválida.")?;
                fs::write(folder.join("enabled.txt"), b"").map_err(|e| e.to_string())?;
            }
        }
    }
    prepare_main(game, module, runtime, old)?;
    prepare_ini(game, module, runtime, old)
}
/// Applies the module's INI values to a copy of the live file. Once managed, the
/// file stays in every later profile (unchanged when no mod remains), otherwise
/// deployment would remove the player's INI with the last mod.
fn prepare_ini(
    game: &NexusGame,
    module: &Definition,
    runtime: &Path,
    old: &BTreeMap<String, String>,
) -> Result<(), String> {
    if module.ini_settings.is_empty() {
        return Ok(());
    }
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let files: BTreeSet<&str> = module.ini_settings.iter().map(|s| s.file.as_str()).collect();
    let mut mods = false;
    for entry in walkdir::WalkDir::new(runtime) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() { continue; }
        let path = entry.path().strip_prefix(runtime).unwrap().to_string_lossy().into_owned();
        if !files.contains(path.as_str()) && module.activation.as_ref().is_none_or(|rule| rule.path != path) {
            mods = true;
            break;
        }
    }
    for file in files {
        if !mods && !old.contains_key(file) { continue; }
        let (base, relative) = super::nexus_targets::root(root, &game.compat_data, file)?;
        let live = base.join(relative);
        if !mods && fs::symlink_metadata(&live).is_err() { continue; }
        let text = read(&live)?;
        let settings: Vec<_> = module.ini_settings.iter().filter(|s| s.file == file).collect();
        let output = if !mods {
            text
        } else {
            if text.trim().is_empty() && !settings.iter().any(|s| s.create) {
                let name = file.rsplit('/').next().unwrap_or(file);
                return Err(format!("Inicie {} uma vez no prefixo selecionado para criar {name} antes de instalar mods.", module.name));
            }
            let mut text = text;
            let sections: BTreeSet<String> = settings.iter().map(|s| s.section.to_ascii_lowercase()).collect();
            for section in sections {
                let chosen: Vec<_> = settings.iter().filter(|s| s.section.eq_ignore_ascii_case(&section)).collect();
                let lines: Vec<String> = chosen.iter().map(|s| format!("{}={}", s.key, s.value)).collect();
                text = ini(&text, &chosen[0].section, &lines, |line| {
                    line.split_once('=').is_some_and(|(key, _)| chosen.iter().any(|s| key.trim().eq_ignore_ascii_case(&s.key)))
                });
            }
            text
        };
        let target = runtime.join(file);
        fs::create_dir_all(target.parent().ok_or("Pasta de ativação ausente.")?).map_err(|e| e.to_string())?;
        fs::write(target, output).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn ue4ss_script_names(paths: impl Iterator<Item = String>, folder: &str) -> Result<BTreeMap<String, String>, String> {
    let mut names = BTreeMap::new();
    let prefix = format!("{}/", folder.to_ascii_lowercase());
    for path in paths {
        let lower = path.to_ascii_lowercase();
        let Some(tail) = lower.strip_prefix(&prefix) else { continue; };
        let Some((name, entry)) = tail.split_once('/') else { continue; };
        if !matches!(entry, "scripts/main.lua" | "dlls/main.dll") { continue; }
        let original = &path[prefix.len()..prefix.len() + name.len()];
        if original.trim().is_empty() || original.contains([':', ';', '\n', '\r']) { return Err("Nome de script UE4SS inválido para mods.txt.".into()); }
        names.insert(name.to_string(), original.to_string());
    }
    Ok(names)
}
fn prepare_ue4ss(root: &Path, runtime: &Path, old: &BTreeMap<String, String>, folders: &[String]) -> Result<(), String> {
    let paths = walkdir::WalkDir::new(runtime).into_iter().map(|entry| {
        let entry = entry.map_err(|e| e.to_string())?;
        Ok(entry.file_type().is_file().then(|| entry.path().strip_prefix(runtime).unwrap().to_string_lossy().into_owned()))
    }).collect::<Result<Vec<_>, String>>()?.into_iter().flatten().collect::<Vec<_>>();
    for folder in folders {
        let previous = ue4ss_script_names(old.keys().cloned(), folder)?;
        let next = ue4ss_script_names(paths.iter().cloned(), folder)?;
        let config = format!("{folder}/mods.txt");
        // Once managed, keep this shared list in the next profile even when the
        // last owned script is removed; otherwise deployment would delete manual entries.
        if previous.is_empty() && next.is_empty() && !old.contains_key(&config) { continue; }
        let live = root.join(&config);
        let original = read(&live)?;
        // A loader's supplied defaults can seed a new config, but never overwrite a live list.
        let original = if original.is_empty() { read(&runtime.join(&config))? } else { original };
        let mut lines: Vec<String> = original.lines().filter(|line| {
            let name = line.split(';').next().unwrap_or_default().split_once(':').map(|(name, _)| name.trim().to_ascii_lowercase());
            !name.is_some_and(|name| previous.contains_key(&name) || next.contains_key(&name))
        }).map(str::to_string).collect();
        for (key, name) in &previous { if !next.contains_key(key) { lines.push(format!("{name} : 0")); } }
        for name in next.values() { lines.push(format!("{name} : 1")); }
        let target = runtime.join(config);
        fs::create_dir_all(target.parent().ok_or("Pasta de ativação ausente.")?).map_err(|e| e.to_string())?;
        fs::write(target, lines.join("\r\n") + "\r\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn prepare_main(
    game: &NexusGame,
    module: &Definition,
    runtime: &Path,
    old: &BTreeMap<String, String>,
) -> Result<(), String> {
    let Some(rule) = &module.activation else {
        return Ok(());
    };
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let paths = walkdir::WalkDir::new(runtime)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            e.path()
                .strip_prefix(runtime)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    let names = members(paths.iter().cloned(), &rule.data);
    let previous = members(old.keys().cloned(), &rule.data);
    if matches!(
        rule.kind,
        ActivationKind::Bethesda | ActivationKind::Morrowind
    ) && names.is_empty()
        && previous.is_empty()
        && !(rule.kind == ActivationKind::Morrowind
            && paths.iter().chain(old.keys()).any(|p| {
                p.starts_with(&format!("{}/", rule.data))
                    && p.to_ascii_lowercase().ends_with(".bsa")
            }))
    {
        return Ok(());
    }
    if rule.kind == ActivationKind::Elden
        && !paths
            .iter()
            .any(|p| p.starts_with(&format!("{}/", rule.data)))
        && !old.contains_key(&rule.path)
    {
        return Ok(());
    }
    if rule.kind == ActivationKind::BaldursGate3
        && !paths.iter().chain(old.keys()).any(|p| {
            p.starts_with(&format!("{}/", rule.data)) && p.to_ascii_lowercase().ends_with(".pak")
        })
    {
        return Ok(());
    }
    let (config_root, relative) = super::nexus_targets::root(root, &game.compat_data, &rule.path)?;
    let config = config_root.join(relative);
    let original = read(&config)?;
    let content = match rule.kind {
        ActivationKind::Bethesda => {
            if names.is_empty() && previous.is_empty() {
                return Ok(());
            }
            let mut lines = remove_owned(&original, &previous);
            for name in ordered(game, module, runtime, &rule.data, &names)? {
                lines.retain(|line| {
                    !line
                        .trim()
                        .trim_start_matches('*')
                        .eq_ignore_ascii_case(&name)
                });
                lines.push(if rule.starred {
                    format!("*{name}")
                } else {
                    name
                });
            }
            // Opt-in LOOT order (masterlist downloaded); the plugins.txt folder is the game's local path.
            let local = config.parent().ok_or("Destino de ativação inválido.")?;
            let lines = super::nexus_loot::sort(game, module, runtime, &rule.data, local, lines);
            lines.join("\r\n") + "\r\n"
        }
        ActivationKind::Sims4 => {
            if !paths
                .iter()
                .chain(old.keys())
                .any(|p| p.starts_with(&format!("{}/", rule.data)))
            {
                return Ok(());
            }
            let mut lines = original
                .lines()
                .filter(|line| {
                    !line.contains("# PeliGames")
                        && !line.trim().starts_with("PackedFile PeliGames/")
                })
                .map(str::to_string)
                .collect::<Vec<_>>();
            if paths
                .iter()
                .any(|p| p.starts_with(&format!("{}/", rule.data)))
            {
                lines.extend([
                    "# PeliGames".to_string(),
                    "Priority 500".to_string(),
                    "PackedFile PeliGames/*.package".to_string(),
                ]);
                let options = "@documents/Electronic Arts/The Sims 4/Options.ini";
                let (base, relative) =
                    super::nexus_targets::root(root, &game.compat_data, options)?;
                let text = read(&base.join(relative))?;
                if !text
                    .lines()
                    .any(|line| line.trim().eq_ignore_ascii_case("[options]"))
                {
                    return Err(
                        "Inicie The Sims 4 uma vez no prefixo selecionado antes de instalar mods."
                            .into(),
                    );
                }
                let output = runtime.join(options);
                fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
                fs::write(
                    output,
                    ini(
                        &text,
                        "options",
                        &["scriptmodsenabled=1".into(), "modsdisabled=0".into()],
                        |line| {
                            line.split_once('=').is_some_and(|(key, _)| {
                                ["scriptmodsenabled", "modsdisabled"]
                                    .contains(&key.trim().to_ascii_lowercase().as_str())
                            })
                        },
                    ),
                )
                .map_err(|e| e.to_string())?;
            }
            if !paths
                .iter()
                .any(|p| p.starts_with(&format!("{}/", rule.data)))
            {
                let options = "@documents/Electronic Arts/The Sims 4/Options.ini";
                if old.contains_key(options) {
                    let (base, relative) =
                        super::nexus_targets::root(root, &game.compat_data, options)?;
                    let output = runtime.join(options);
                    fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
                    fs::write(output, read(&base.join(relative))?).map_err(|e| e.to_string())?;
                }
            }
            if !paths
                .iter()
                .any(|p| p.starts_with(&format!("{}/", rule.data)))
            {
                let options = "@documents/Electronic Arts/The Sims 4/Options.ini";
                if old.contains_key(options) {
                    let (base, relative) =
                        super::nexus_targets::root(root, &game.compat_data, options)?;
                    let output = runtime.join(options);
                    fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
                    fs::write(output, read(&base.join(relative))?).map_err(|e| e.to_string())?;
                }
            }
            lines.join("\r\n") + "\r\n"
        }
        ActivationKind::Morrowind => {
            let mut active = original
                .lines()
                .filter_map(|line| line.split_once('='))
                .filter(|(key, _)| key.trim().to_lowercase().starts_with("gamefile"))
                .map(|(_, name)| name.trim().to_string())
                .filter(|name| !previous.iter().any(|old| old.eq_ignore_ascii_case(name)))
                .collect::<Vec<_>>();
            for name in &names {
                if !active.iter().any(|n| n.eq_ignore_ascii_case(name)) {
                    active.push(name.clone());
                }
            }
            active.sort_by_key(|name| {
                (
                    !name.to_ascii_lowercase().ends_with(".esm"),
                    name.to_ascii_lowercase(),
                )
            });
            let configured = ini(
                &original,
                "Game Files",
                &active
                    .iter()
                    .enumerate()
                    .map(|(i, n)| format!("GameFile{i}={n}"))
                    .collect::<Vec<_>>(),
                |line| {
                    line.split_once('=')
                        .is_some_and(|(key, _)| key.trim().to_lowercase().starts_with("gamefile"))
                },
            );
            let archives = |paths: Vec<String>| {
                paths
                    .into_iter()
                    .filter_map(|p| {
                        p.strip_prefix(&format!("{}/", rule.data))
                            .filter(|name| {
                                !name.contains('/') && name.to_ascii_lowercase().ends_with(".bsa")
                            })
                            .map(str::to_string)
                    })
                    .collect::<BTreeSet<_>>()
            };
            let old_archives = archives(old.keys().cloned().collect());
            let current_archives = archives(paths.clone());
            if old_archives.is_empty() && current_archives.is_empty() {
                configured
            } else {
                let mut loaded = original
                    .lines()
                    .filter_map(|line| {
                        line.split_once('=')
                            .filter(|(key, _)| key.trim().to_lowercase().starts_with("archive"))
                    })
                    .map(|(_, value)| value.trim().to_string())
                    .filter(|name| {
                        !old_archives
                            .iter()
                            .any(|old| old.eq_ignore_ascii_case(name))
                    })
                    .collect::<Vec<_>>();
                for name in current_archives {
                    if !loaded.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                        loaded.push(name);
                    }
                }
                ini(
                    &configured,
                    "Archives",
                    &loaded
                        .iter()
                        .enumerate()
                        .map(|(i, n)| format!("Archive {i}={n}"))
                        .collect::<Vec<_>>(),
                    |line| {
                        line.split_once('=').is_some_and(|(key, _)| {
                            key.trim().to_lowercase().starts_with("archive")
                        })
                    },
                )
            }
        }
        ActivationKind::Memoria => {
            let folders: BTreeSet<_> = paths
                .iter()
                .filter_map(|p| {
                    p.strip_prefix(&format!("{}/", rule.data))
                        .and_then(|tail| tail.split('/').next())
                })
                .collect();
            if folders
                .iter()
                .any(|folder| folder.contains([',', '"']) || folder.chars().any(char::is_control))
            {
                return Err("Nome de pasta incompatível com FolderNames do Memoria.".into());
            }
            let mut inside = false;
            let mut value = None;
            for line in original.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') && trimmed.ends_with(']') {
                    inside = trimmed[1..trimmed.len() - 1].eq_ignore_ascii_case("Mod");
                } else if inside {
                    if let Some((key, text)) = line.split_once('=') {
                        if key.trim().eq_ignore_ascii_case("FolderNames") {
                            value = Some((key, text));
                            break;
                        }
                    }
                }
            }
            if let Some((_, text)) = value {
                let mut quoted = false;
                for character in text.chars() {
                    if character == '"' {
                        quoted = !quoted;
                    }
                    if character == ',' && quoted {
                        return Err("FolderNames contém vírgula dentro de um nome; configuração preservada.".into());
                    }
                }
                if quoted {
                    return Err(
                        "FolderNames contém aspas incompletas; configuração preservada.".into(),
                    );
                }
            }
            let mut existing = value
                .map(|(_, v)| {
                    v.split(',')
                        .map(|name| name.trim().trim_matches('"').to_string())
                        .filter(|name| {
                            !name.is_empty() && !name.starts_with(&format!("{}/", rule.data))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            existing.extend(
                folders
                    .iter()
                    .map(|folder| format!("{}/{folder}", rule.data)),
            );
            let values = existing
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(", ");
            ini(
                &original,
                "Mod",
                &[format!("FolderNames={values}")],
                |line| {
                    line.split_once('=')
                        .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("FolderNames"))
                },
            )
        }
        ActivationKind::Elden => {
            let baseline = super::nexus_profile::directory(game)?
                .join("settings/activation-original")
                .join(&rule.path);
            if !old.contains_key(&rule.path) && config.is_file() && !baseline.exists() {
                fs::create_dir_all(baseline.parent().ok_or("Backup inválido.")?)
                    .map_err(|e| e.to_string())?;
                fs::write(&baseline, &original).map_err(|e| e.to_string())?;
            }
            if !paths
                .iter()
                .any(|p| p.starts_with(&format!("{}/", rule.data)))
            {
                use sha2::{Digest, Sha256};
                let unchanged = old.get(&rule.path).is_some_and(|hash| {
                    *hash == format!("{:x}", Sha256::digest(original.as_bytes()))
                });
                if !unchanged {
                    original.clone()
                } else if baseline.is_file() {
                    read(&baseline)?
                } else {
                    return Ok(());
                }
            } else {
                // The configuration is a sibling of Game, so the relative asset path is stable.
                format!("[modengine]\ndebug = false\nexternal_dlls = []\n[extension.mod_loader]\nenabled = true\nloose_params = false\nmods = [{{ enabled = true, name = \"PeliGames\", path = \"{}\" }}]\n",rule.data)
            }
        }
        ActivationKind::BaldursGate3 => {
            super::nexus_bg3::activation(game, runtime, &rule.data, &original, old)?
        }
        ActivationKind::Reloaded => reloaded(game, module, runtime, &rule.data, &original, old)?,
    };
    let output = runtime.join(&rule.path);
    fs::create_dir_all(output.parent().ok_or("Destino inválido.")?).map_err(|e| e.to_string())?;
    fs::write(output, content).map_err(|e| e.to_string())
}

/// These engines order plugins by modification time. Only managed files receive
/// timestamps; external plugins are left untouched and deployment rollback is armed.
pub(super) fn timestamps(
    game: &NexusGame,
    module: &Definition,
    runtime: &Path,
) -> Result<(), String> {
    if !["oblivion", "fallout3", "newvegas", "morrowind"].contains(&module.domain.as_str()) {
        return Ok(());
    }
    let Some(rule) = &module.activation else {
        return Ok(());
    };
    if !runtime.join(&rule.path).is_file() {
        return Ok(());
    }
    let names = read(&runtime.join(&rule.path))?
        .lines()
        .filter_map(|line| {
            if rule.kind == ActivationKind::Morrowind {
                line.split_once('=')
                    .filter(|(key, _)| key.trim().to_lowercase().starts_with("gamefile"))
                    .map(|(_, name)| name.trim().to_string())
            } else {
                plugin(line.trim()).then(|| line.trim().to_string())
            }
        })
        .collect::<Vec<_>>();
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let managed: BTreeSet<_> = names
        .iter()
        .filter(|name| runtime.join(&rule.data).join(name).is_file())
        .cloned()
        .collect();
    let mut base = std::time::SystemTime::now();
    for name in names.iter().filter(|name| !managed.contains(*name)) {
        if !super::nexus_modules::relative(name) || name.contains('/') {
            return Err("Nome de plugin externo inseguro; configuração preservada.".into());
        }
        if let Ok(time) = fs::metadata(root.join(&rule.data).join(name)).and_then(|m| m.modified())
        {
            base = base.max(time);
        }
    }
    for (i, name) in names
        .iter()
        .filter(|name| managed.contains(*name))
        .enumerate()
    {
        let time = base
            .checked_add(std::time::Duration::from_secs(60 * (i as u64 + 1)))
            .ok_or("Data de plugin inválida.")?;
        fs::OpenOptions::new()
            .write(true)
            .open(root.join(&rule.data).join(name))
            .and_then(|file| file.set_modified(time))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn reloaded(
    game: &NexusGame,
    module: &Definition,
    runtime: &Path,
    data: &str,
    original: &str,
    old: &BTreeMap<String, String>,
) -> Result<String, String> {
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let load = |path: &Path| -> Result<serde_json::Value, String> {
        serde_json::from_str(read(path)?.trim_start_matches('\u{feff}'))
            .map_err(|e| format!("ModConfig Reloaded-II inválido: {e}"))
    };
    let mut owned = BTreeSet::new();
    for path in old
        .keys()
        .filter(|p| p.starts_with(&format!("{data}/")) && p.ends_with("/ModConfig.json"))
    {
        let config = load(&root.join(path))?;
        if let Some(id) = config["ModId"].as_str() {
            owned.insert(id.to_string());
        }
    }
    let mut configs = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut apps: Option<BTreeSet<String>> = None;
    for (base, managed) in [(root.join(data), false), (runtime.join(data), true)] {
        for entry in walkdir::WalkDir::new(base)
            .max_depth(4)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file() && e.file_name() == "ModConfig.json")
            .take(1000)
        {
            let config = load(entry.path())?;
            let id = config["ModId"]
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 200 && !id.chars().any(char::is_control))
                .ok_or("ModId Reloaded-II inválido.")?
                .to_string();
            if !managed && owned.contains(&id) {
                continue;
            }
            if managed {
                if !active.insert(id.clone()) {
                    return Err("ModId duplicado nos pacotes Reloaded-II.".into());
                }
                if let Some(ids) = config["SupportedAppId"]
                    .as_array()
                    .filter(|a| !a.is_empty())
                {
                    let ids = ids
                        .iter()
                        .filter_map(|id| id.as_str().map(str::to_string))
                        .collect::<BTreeSet<_>>();
                    apps = Some(
                        apps.map(|previous| previous.intersection(&ids).cloned().collect())
                            .unwrap_or(ids),
                    );
                }
            }
            configs.insert(id, config);
        }
    }
    let executable = module.executable.rsplit('/').next().unwrap().to_lowercase();
    let app_id=match apps {
        Some(ids) if ids.iter().any(|id|id.eq_ignore_ascii_case(&executable))=>ids.into_iter().find(|id|id.eq_ignore_ascii_case(&executable)).unwrap(),
        Some(ids) if ids.len()==1=>ids.into_iter().next().unwrap(),
        Some(_)=>return Err("Os pacotes Reloaded-II não identificam um único aplicativo compatível com este executável.".into()),
        None=>executable,
    };
    // Hard dependencies are included, optional dependencies stay untouched.
    loop {
        let mut extra = Vec::new();
        for id in &active {
            if let Some(deps) = configs[id]["ModDependencies"].as_array() {
                for dep in deps {
                    let dep = dep.as_str().ok_or("Dependência Reloaded-II inválida.")?;
                    if !configs.contains_key(dep) {
                        return Err(format!(
                            "Instale a dependência Reloaded-II {dep} antes deste mod."
                        ));
                    }
                    if !active.contains(dep) {
                        extra.push(dep.to_string());
                    }
                }
            }
        }
        if extra.is_empty() {
            break;
        }
        active.extend(extra);
    }
    let mut config: serde_json::Value = if original.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(original).map_err(|e| e.to_string())?
    };
    if !config.is_object() {
        return Err("AppConfig Reloaded-II inválido.".into());
    }
    if let Some(previous) = config["PeliGamesManagedMods"].as_array() {
        owned.extend(
            previous
                .iter()
                .filter_map(|id| id.as_str().map(str::to_string)),
        );
    }
    let mut enabled = config["EnabledMods"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    enabled.retain(|id| !id.as_str().is_some_and(|id| owned.contains(id)));
    config["PeliGamesManagedMods"] = active
        .iter()
        .filter(|id| {
            !enabled
                .iter()
                .any(|value| value.as_str() == Some(id.as_str()))
        })
        .cloned()
        .collect::<Vec<_>>()
        .into();
    for id in active {
        if !enabled.iter().any(|v| v.as_str() == Some(&id)) {
            enabled.push(id.into());
        }
    }
    config["AppId"] = app_id.into();
    config["AppName"] = module.name.clone().into();
    config["AppLocation"] = format!("Z:{}", root.join(&module.executable).display())
        .replace('/', "\\")
        .into();
    config["WorkingDirectory"] = format!("Z:{}", root.display()).replace('/', "\\").into();
    config["AutoInject"] = true.into();
    config["EnabledMods"] = enabled.clone().into();
    config["SortedMods"] = enabled.into();
    // This is a game-local Reloaded installation; portable mode keeps Apps/Mods
    // beside it and does not rewrite the global ReloadedII.json in AppData.
    if runtime.join(data).is_dir() && !root.join("Reloaded/portable.txt").exists() {
        fs::create_dir_all(runtime.join("Reloaded")).map_err(|e| e.to_string())?;
        fs::write(runtime.join("Reloaded/portable.txt"), []).map_err(|e| e.to_string())?;
    }
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())
}

#[cfg(test)]
mod palworld_tests {
    use super::*;
    pub(super) const PALWORLD_MOD_FOLDERS: [&str; 2] = ["Pal/Binaries/Win64/ue4ss/Mods", "Pal/Binaries/Win64/Mods"];
    pub(super) fn prepare_palworld(root: &Path, runtime: &Path, old: &BTreeMap<String, String>) -> Result<(), String> {
        prepare_ue4ss(root, runtime, old, &PALWORLD_MOD_FOLDERS.map(String::from))
    }
    #[test]
    fn activation_merge_enables_and_disables_owned_scripts_preserving_manual_entries() {
        for folder in PALWORLD_MOD_FOLDERS {
            let root = std::env::temp_dir().join(format!("palworld-activation-{}", uuid::Uuid::new_v4()));
            let runtime = root.join("stage");
            let config = format!("{folder}/mods.txt");
            fs::create_dir_all(root.join(folder)).unwrap();
            fs::write(root.join(&config), "; user list\r\nManual : 1\r\nOther : 0\r\nMenu : 0 ; old state\r\n").unwrap();
            let script = format!("{folder}/Menu/Scripts/main.lua");
            fs::create_dir_all(runtime.join(&script).parent().unwrap()).unwrap();
            fs::write(runtime.join(&script), "-- fixture").unwrap();
            prepare_palworld(&root, &runtime, &BTreeMap::new()).unwrap();
            let enabled = fs::read_to_string(runtime.join(&config)).unwrap();
            assert!(enabled.contains("Manual : 1") && enabled.contains("Other : 0"));
            assert!(enabled.contains("; user list") && enabled.contains("Menu : 1"));
            assert!(!enabled.contains("Menu : 0"));
            // Preparation only touches the stage, not the real game config.
            assert!(fs::read_to_string(root.join(&config)).unwrap().contains("Menu : 0"));
            fs::write(root.join(&config), enabled).unwrap();
            fs::remove_file(runtime.join(&script)).unwrap();
            prepare_palworld(&root, &runtime, &BTreeMap::from([(script, "previous".into())])).unwrap();
            let disabled = fs::read_to_string(runtime.join(&config)).unwrap();
            assert!(disabled.contains("Menu : 0") && !disabled.contains("Menu : 1"));
            assert!(disabled.contains("Manual : 1") && disabled.contains("Other : 0"));
            fs::write(root.join(&config), &disabled).unwrap();
            fs::remove_file(runtime.join(&config)).unwrap();
            prepare_palworld(&root, &runtime, &BTreeMap::from([(config.clone(), "shared config".into())])).unwrap();
            assert_eq!(fs::read_to_string(runtime.join(config)).unwrap(), disabled);
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[cfg(test)]
mod palworld_archive_test {
    use super::*;
    use super::palworld_tests::*;
    #[test]
    #[ignore = "Read-only validation of an actual user-provided mod archive"]
    fn actual_mixed_archive_routes_all_files_for_both_loader_layouts() {
        let path = std::env::var("PELIGAMES_TEST_MOD_ARCHIVE").expect("Provide PELIGAMES_TEST_MOD_ARCHIVE");
        let mut zip = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let entries: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().replace('\\', "/")).collect();
        let module = super::super::nexus_modules::registry().definitions.into_iter().find(|m|m.domain == "palworld").unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        for flat in [false, true] {
            let root = std::env::temp_dir().join(format!("palworld-archive-{}", uuid::Uuid::new_v4()));
            let runtime = root.join("stage");
            let loader = if flat { "Pal/Binaries/Win64/UE4SS.dll" } else { "Pal/Binaries/Win64/ue4ss/UE4SS.dll" };
            fs::create_dir_all(root.join(loader).parent().unwrap()).unwrap();
            fs::write(root.join(loader), "fixture").unwrap();
            let mut targets = Vec::new();
            for index in 0..zip.len() {
                let mut entry = zip.by_index(index).unwrap();
                if entry.is_dir() { continue; }
                let source = entry.name().replace('\\', "/");
                let canonical = module.route_package(&source, id, &entries).unwrap().expect("Every file in this mixed package must be recognized");
                let target = super::super::nexus_scan::ue4ss_mod_target(&root, &module.ue4ss().unwrap(), canonical).unwrap();
                let output = runtime.join(&target);
                fs::create_dir_all(output.parent().unwrap()).unwrap();
                std::io::copy(&mut entry, &mut fs::File::create(output).unwrap()).unwrap();
                targets.push(target);
            }
            assert_eq!(targets.len(), 3);
            assert!(targets.iter().any(|p|p.ends_with("Scripts/main.lua")));
            assert!(targets.iter().any(|p|p.ends_with("enabled.txt")));
            assert!(targets.iter().any(|p|p == "Pal/Content/Paks/LogicMods/DekModConfigMenu_P.pak"));
            prepare_palworld(&root, &runtime, &BTreeMap::new()).unwrap();
            let folder = if flat { PALWORLD_MOD_FOLDERS[1] } else { PALWORLD_MOD_FOLDERS[0] };
            assert!(fs::read_to_string(runtime.join(folder).join("mods.txt")).unwrap().contains("DekModConfigMenu : 1"));
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[cfg(test)]
mod ini_settings_tests {
    use super::*;
    fn module(domain: &str) -> Definition {
        super::super::nexus_modules::registry().definitions.into_iter().find(|m| m.domain == domain).unwrap()
    }
    #[test]
    fn archive_invalidation_follows_mods_and_keeps_the_managed_ini() {
        let base = std::env::temp_dir().join(format!("bethesda-ini-{}", uuid::Uuid::new_v4()));
        let documents = base.join("prefix/pfx/drive_c/users/steamuser/Documents/My Games");
        fs::create_dir_all(&documents).unwrap();
        fs::write(base.join("prefix/pfx/system.reg"), "").unwrap();
        fs::create_dir_all(base.join("game")).unwrap();
        let game: NexusGame = serde_json::from_value(serde_json::json!({
            "id": "test", "platform": "proton", "mods": [],
            "game": {"directory": base.join("game")}, "compat_data": base.join("prefix"),
        })).unwrap();
        let runtime = base.join("stage");
        fs::create_dir_all(runtime.join("Data/textures")).unwrap();
        fs::write(runtime.join("Data/textures/loose.dds"), "fixture").unwrap();

        // Game-written INI absent: the player must start the game once.
        let skyrim = module("skyrimspecialedition");
        assert!(prepare_ini(&game, &skyrim, &runtime, &BTreeMap::new()).unwrap_err().contains("Skyrim.ini"));

        let file = "@documents/My Games/Skyrim Special Edition/Skyrim.ini";
        let live = documents.join("Skyrim Special Edition/Skyrim.ini");
        fs::create_dir_all(live.parent().unwrap()).unwrap();
        fs::write(&live, "[General]\r\nsLanguage=ENGLISH\r\n[Archive]\r\nbInvalidateOlderFiles=0\r\nSResourceArchiveList=Skyrim - Misc.bsa\r\n").unwrap();
        prepare_ini(&game, &skyrim, &runtime, &BTreeMap::new()).unwrap();
        let staged = fs::read_to_string(runtime.join(file)).unwrap();
        assert!(staged.contains("sLanguage=ENGLISH") && staged.contains("SResourceArchiveList=Skyrim - Misc.bsa"));
        assert!(staged.contains("bInvalidateOlderFiles=1") && staged.contains("sResourceDataDirsFinal=\r\n"));
        assert!(!staged.contains("bInvalidateOlderFiles=0"));
        assert!(generated_config(&skyrim, file) && super::super::nexus_deployment::config(&skyrim, file) && skyrim.allowed(file));

        // No mod left: an unmanaged INI is untouched, a managed one is kept as is.
        fs::remove_dir_all(&runtime).unwrap();
        fs::create_dir_all(&runtime).unwrap();
        prepare_ini(&game, &skyrim, &runtime, &BTreeMap::new()).unwrap();
        assert!(!runtime.join(file).exists());
        fs::write(&live, &staged).unwrap();
        prepare_ini(&game, &skyrim, &runtime, &BTreeMap::from([(file.to_string(), "previous".into())])).unwrap();
        assert_eq!(fs::read_to_string(runtime.join(file)).unwrap(), staged);

        // `StarfieldCustom.ini` is created like the Vortex Starfield extension does.
        fs::remove_dir_all(&runtime).unwrap();
        fs::create_dir_all(runtime.join("Data")).unwrap();
        fs::write(runtime.join("Data/loose.nif"), "fixture").unwrap();
        prepare_ini(&game, &module("starfield"), &runtime, &BTreeMap::new()).unwrap();
        let custom = fs::read_to_string(runtime.join("@documents/My Games/Starfield/StarfieldCustom.ini")).unwrap();
        assert_eq!(custom, "[Archive]\r\nbInvalidateOlderFiles=1\r\nsResourceDataDirsFinal=\r\n");
        fs::remove_dir_all(base).unwrap();
    }
}
