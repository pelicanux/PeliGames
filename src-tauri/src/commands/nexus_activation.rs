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
        || module.domain == "thesims4"
            && path == "@documents/Electronic Arts/The Sims 4/Options.ini"
}
pub(super) fn prepare(
    game: &NexusGame,
    module: &Definition,
    runtime: &Path,
    old: &BTreeMap<String, String>,
) -> Result<(), String> {
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
    prepare_main(game, module, runtime, old)
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
