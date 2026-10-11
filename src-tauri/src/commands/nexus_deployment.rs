//! Module-driven file deployment for native/Proton games, with ownership and rollback.
use super::{
    nexus_local::NexusGame,
    nexus_profile::{directory, profile},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn definition(game: &NexusGame) -> Option<super::nexus_modules::Definition> {
    game.game["directory"].as_str().and_then(|root| super::nexus_discovery::at(Path::new(root)))
        .filter(|definition| definition.platform == game.platform && definition.engine == super::nexus_modules::Engine::Deployment)
}
pub(super) fn supports(game: &NexusGame) -> bool { definition(game).is_some() }
// Crimson Desert plugins and proxy loaders must be Windows x64 DLL images.
// This checks architecture/structure, not compatibility with a game update.
fn crimson_plugin_header(mut reader: impl Read, name: &str) -> Result<(), String> {
    let invalid = || format!("Plugin/carregador de Crimson Desert inválido ou não x64: {name}. É necessário um DLL PE x64; nenhum arquivo foi aplicado ao jogo.");
    let mut dos = [0u8; 64];
    reader.read_exact(&mut dos).map_err(|_| invalid())?;
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if &dos[..2] != b"MZ" || !(64..=1024 * 1024).contains(&offset) {return Err(invalid());}
    if std::io::copy(&mut reader.by_ref().take(offset - 64), &mut std::io::sink()).map_err(|_| invalid())? != offset - 64 {return Err(invalid());}
    let mut pe = [0u8; 26];
    reader.read_exact(&mut pe).map_err(|_| invalid())?;
    if &pe[..4] != b"PE\0\0" || u16::from_le_bytes([pe[4],pe[5]]) != 0x8664
        || u16::from_le_bytes([pe[22],pe[23]]) & 0x2000 == 0
        || u16::from_le_bytes([pe[24],pe[25]]) != 0x20b {return Err(invalid());}
    Ok(())
}
pub(super) fn unmapped_file(name: &str) -> Result<(), String> {
    // Thunderstore metadata is not a game configuration. Other unmapped
    // configuration/content files must not disappear from a mixed package.
    if matches!(name.to_ascii_lowercase().as_str(), "manifest.json" | "thunderstore.toml") { return Ok(()); }
    let extension = Path::new(name).extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
    if name.to_ascii_lowercase().contains(".patch_") || ["dll", "asi", "lua", "exe", "pak", "utoc", "ucas", "archive", "esp", "esm", "esl", "bsa", "ba2", "vbf", "fbmod", "fbpack", "smp", "dazip", "daimod", "mem", "ts4script", "package", "dl_bin",
        "json", "ini", "cfg", "xml", "bin", "dat", "dds", "repobundle", "assetbundle", "vbuild", "toml", "yaml", "yml", "bnk", "bank", "wem", "paz", "pamt", "modpatch", "cdmod", "py", "pyc", "pyo"].contains(&extension.as_str()) {
        return Err(format!("O pacote contém conteúdo que este módulo não sabe instalar: {name}. Nenhum arquivo foi aplicado ao jogo."));
    }
    Ok(())
}
/// Folders present in an archive, for the manual root selector (bounded).
pub(super) fn package_folders(entries: &[String]) -> Vec<String> {
    let mut folders = BTreeSet::new();
    folders.insert(String::new());
    for entry in entries {
        let parts: Vec<&str> = entry.trim_end_matches('/').split('/').collect();
        let depth = if entry.ends_with('/') { parts.len() } else { parts.len().saturating_sub(1) };
        for end in 1..=depth.min(8) {
            folders.insert(format!("{}/", parts[..end].join("/")));
        }
        if folders.len() > 2000 { break; }
    }
    folders.into_iter().collect()
}

/// Files of a package installed from a user-chosen root. Content outside the
/// root is not silently dropped when it is functional (see `unmapped_file`).
pub(super) fn manual_files(module: &super::nexus_modules::Definition, mod_id: &str, root: &super::nexus_local::ManualRoot, entries: &[String]) -> Result<Vec<(String, String)>, String> {
    let source = root.source.trim_matches('/');
    if !source.is_empty() && !super::nexus_modules::relative(source) { return Err("Pasta raiz inválida.".into()); }
    let prefix = if source.is_empty() { String::new() } else { format!("{source}/") };
    let inside: Vec<(&String, String)> = entries.iter().filter(|e| !e.ends_with('/'))
        .filter_map(|e| e.strip_prefix(&prefix).map(|rel| (e, rel.to_string()))).collect();
    if inside.is_empty() { return Err(format!("A pasta escolhida não contém arquivos: {}", if source.is_empty() { "(raiz)" } else { source })); }
    let relative_entries: Vec<String> = inside.iter().map(|(_, rel)| rel.clone()).collect();
    let mut files = Vec::new();
    for (original, rel) in &inside {
        let target = if root.destination.is_empty() { module.route_package(rel, mod_id, &relative_entries)? } else { Some(module.manual_target(rel, &root.destination, mod_id)?) };
        match target { Some(target) => files.push(((*original).clone(), target)), None => unmapped_file(original)? }
    }
    for entry in entries.iter().filter(|e| !e.ends_with('/') && !e.starts_with(&prefix)) { unmapped_file(entry)?; }
    Ok(files)
}

/// A file supplied by several enabled mods: `winner` is deployed.
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct Override {
    pub target: String,
    pub winner: String,
    pub overridden: Vec<String>,
}

fn save_overrides(game: &NexusGame, overrides: Vec<Override>) -> Result<(), String> {
    let path = directory(game)?.join("conflicts.json");
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&overrides).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    fs::rename(&temporary, &path).map_err(|e| e.to_string())
}

/// Files replaced by mod priority in the current profile.
pub(super) fn overrides(game: &NexusGame) -> Result<Vec<Override>, String> {
    match fs::read(directory(game)?.join("conflicts.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn validate_companions(files: &[(String, String)]) -> Result<(), String> {
    let targets: BTreeSet<_> = files.iter().map(|(_, target)| target.to_ascii_lowercase()).collect();
    for (_, target) in files {
        let lower = target.to_ascii_lowercase();
        for (suffix, companion) in [(".utoc", ".ucas"), (".ucas", ".utoc")] {
            if let Some(stem) = lower.strip_suffix(suffix) {
                if !targets.contains(&format!("{stem}{companion}")) {
                    return Err(format!("Pacote IoStore incompleto: falta o arquivo {stem}{companion}."));
                }
            }
        }
    }
    Ok(())
}
fn folder(game: &NexusGame) -> Result<PathBuf, String> {
    definition(game).ok_or("A pasta não corresponde a um módulo de instalação de arquivos.")?;
    fs::canonicalize(game.game["directory"].as_str().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())
}
pub(super) fn detect(game: &mut NexusGame) {
    if !supports(game) || game.platform != "proton" {
        return;
    }
    if game.compat_data.is_empty() {
        if let Ok(dir) = folder(game) {
            let id = super::nexus_discovery::steam_app_id(&dir).or_else(|| definition(game).and_then(|module| module.steam_id)).unwrap_or_default();
            let libraries = dirs::home_dir().map(|home| super::scanner::steam_libraries(&home)).unwrap_or_default();
            if let Some(prefix) = super::nexus_discovery::steam_prefix(&dir, &id, &libraries) {
                game.compat_data = prefix.to_string_lossy().into();
            }
        }
    }
    if game.proton.is_empty() {
        if let Ok(info) = fs::read_to_string(Path::new(&game.compat_data).join("config_info")) {
            if let Some(line) = info.lines().nth(1) {
                if let Some(base) = line.trim().strip_suffix("/files/share/fonts/") {
                    if let Ok(path) = fs::canonicalize(base) {
                        if path.join("proton").is_file() {
                            game.proton = path.to_string_lossy().into();
                        }
                    }
                }
            }
        }
    }
}
pub(super) fn validate_runner(prefix: &str, proton: &str) -> Result<(PathBuf, PathBuf), String> {
    let (prefix, _) = super::nexus_prefix::resolve(Path::new(prefix))?;
    let proton = fs::canonicalize(proton).map_err(|e| format!("Proton: {e}"))?;
    if !proton.join("proton").is_file() {
        return Err("Selecione uma instalação do Proton.".into());
    }
    Ok((prefix, proton))
}
fn processes(game: &NexusGame, managed: bool) -> Vec<i32> {
    let Ok(dir) = folder(game) else {
        return vec![];
    };
    let Some(module) = definition(game) else { return vec![]; };
    let mut targets = vec![dir.join(&module.executable)];
    if let Some(executable) = &module.launch_executable { targets.push(dir.join(executable)); }
    if let Some(executable) = &module.mod_launch_executable { targets.push(dir.join(executable)); }
    if module.domain == "eldenring" {targets.push(dir.join("modengine2_launcher.exe"));}
    if module.domain == "finalfantasy12" {targets.push(dir.join("x64/ff12-launcher.exe"));}
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return found;
    };
    let marker = directory(game)
        .map(|p| format!("PELIGAMES_NEXUS_PROFILE={}", p.display()))
        .unwrap_or_default();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|v| *v > 1)
        else {
            continue;
        };
        let Ok(cmdline) = fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let matches = cmdline.split(|b| *b == 0).any(|arg| {
            let arg = String::from_utf8_lossy(arg).replace('\\', "/");
            targets.iter().any(|p| {
                arg.strip_prefix("Z:")
                    .unwrap_or(&arg)
                    .eq_ignore_ascii_case(&p.to_string_lossy())
            })
        });
        if !matches {
            continue;
        }
        if managed {
            let Ok(env) = fs::read(entry.path().join("environ")) else {
                continue;
            };
            if !env.split(|b| *b == 0).any(|v| v == marker.as_bytes()) {
                continue;
            }
        }
        found.push(pid);
    }
    found
}
pub(super) fn running(game: &NexusGame) -> bool {
    !processes(game, true).is_empty()
}
pub(super) fn idle(game: &NexusGame) -> Result<(), String> {
    if !processes(game, false).is_empty() {
        Err("Feche o jogo antes de alterar os mods.".into())
    } else {
        Ok(())
    }
}
fn checksum(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
#[derive(Default, Serialize, Deserialize)]
struct Deployment {
    #[serde(default)]
    method: super::nexus_deploy_methods::Method,
    #[serde(default)]
    sources: BTreeMap<String, String>,
    files: BTreeMap<String, String>,
    #[serde(default)]
    targets: BTreeMap<String, String>,
}
fn verify_targets(game:&NexusGame,old:&Deployment)->Result<(),String>{
    for(relative,previous)in &old.targets {
        let dest = destination(game, relative)?;
        if fs::symlink_metadata(&dest).is_ok_and(|meta| meta.file_type().is_symlink()) {
            let expected = old.sources.get(relative).ok_or("Link externo preservado; restaure os arquivos do perfil.")?;
            let source = fs::read_link(&dest).map_err(|e| e.to_string())?;
            if old.method != super::nexus_deploy_methods::Method::Symlink || source != Path::new(expected)
                || !fs::canonicalize(&source).is_ok_and(|path| path.starts_with(directory(game).unwrap_or_default().join("profiles"))) {
                return Err(format!("Link alterado fora do gerenciador; preservado: {relative}"));
            }
        }
        if old.files.contains_key(relative) && destination(game,relative)?.to_string_lossy()!=previous.as_str(){return Err("O destino dos mods mudou no prefixo/carregador. Restaure o destino anterior e desative os mods antes de mudar a configuração.".into());}
    }
    Ok(())
}
fn manifest(game: &NexusGame) -> Result<PathBuf, String> {
    Ok(directory(game)?.join("deployment.json"))
}
fn load_deployment(game: &NexusGame) -> Result<Deployment, String> {
    match fs::read(manifest(game)?) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Deployment::default()),
        Err(e) => Err(e.to_string()),
    }
}
pub(super) fn config(module: &super::nexus_modules::Definition, path: &str) -> bool {
    if module.config_files.iter().any(|file| file == path) { return true; }
    if module.domain == "repo" && path.starts_with("BepInEx/config/") && path.ends_with(".cfg") { return true; }
    // Loader configuration is rewritten by the game; it is preserved, not verified.
    if let Some(layout) = module.unity() {
        let bepinex = layout.loaders.contains(&super::nexus_modules::UnityLoader::Bepinex) && path.starts_with("BepInEx/config/") && path.ends_with(".cfg");
        let melon = layout.loaders.contains(&super::nexus_modules::UnityLoader::Melonloader) && path.starts_with("UserData/");
        if bepinex || melon { return true; }
    }
    module.ue4ss().is_some_and(|layout| path.ends_with("UE4SS-settings.ini")
        || path.ends_with("MemberVariableLayout.ini")
        || path.ends_with("/Mods/mods.txt")
        || path.ends_with("/Mods/mods.json")
        || (path.starts_with(&format!("{}/", layout.logic_mods))
            && path.ends_with(".modconfig.json")))
}
pub(super) fn destination(game: &NexusGame, path: &str) -> Result<PathBuf, String> {
    let module = definition(game).ok_or("Módulo do jogo não identificado.")?;
    destination_in(&folder(game)?, &module, path, &game.compat_data)
}
fn destination_in(root: &Path, module: &super::nexus_modules::Definition, path: &str, prefix: &str) -> Result<PathBuf, String> {
    destination_managed(root, module, path, prefix, &BTreeSet::new())
}
/// `managed`: files of the previous deployment, replaced by this one. Only such
/// an exact file may differ in letter case from the new path (a priority change
/// between mods spelling the same file differently).
fn destination_managed(root: &Path, module: &super::nexus_modules::Definition, path: &str, prefix: &str, managed: &BTreeSet<PathBuf>) -> Result<PathBuf, String> {
    if !module.allowed(path) {
        return Err(format!("Destino de mod inválido: {path}"));
    }
    let (folder, local) = super::nexus_targets::root(root, prefix, path)?;
    let target = folder.join(&local);
    // Wine treats names without regard to case; reject ambiguous Linux paths.
    let mut current = folder.clone();
    for component in local.split('/') {
        if current.is_dir() {
            for entry in fs::read_dir(&current).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.eq_ignore_ascii_case(component) && name != component && !(entry.file_type().is_ok_and(|t| t.is_file()) && managed.contains(&entry.path())) {
                    return Err(format!(
                        "Conflito de maiúsculas/minúsculas; preservado: {}",
                        entry.path().display()
                    ));
                }
            }
        }
        current.push(component);
    }
    for parent in target.parent().into_iter().flat_map(Path::ancestors).take_while(|p| *p != folder) {
        if let Ok(meta) = fs::symlink_metadata(parent) {
            if meta.file_type().is_symlink() {
                return Err(format!("Link recusado: {}", parent.display()));
            }
        }
    }
    Ok(target)
}
pub(super) fn build(game: &NexusGame) -> Result<String, String> { build_inner(game, true) }
// Removing a loader must remain possible even when enabled mods depend on it.
pub(super) fn build_after_removal(game: &NexusGame) -> Result<String, String> { build_inner(game, false) }
fn build_inner(game: &NexusGame, require_dependencies: bool) -> Result<String, String> {
    idle(game)?;
    let module = definition(game).ok_or("Módulo do jogo não identificado.")?;
    let revision = uuid::Uuid::new_v4().to_string();
    let dir = directory(game)?.join("profiles").join(&revision);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut overrides: BTreeMap<String, Override> = BTreeMap::new();
    let result = (|| {
        // Lower-case target → (spelling written, owning mod). Mod order is priority:
        // a later mod replaces an earlier mod's file, as in MO2.
        let mut written: BTreeMap<String, (String, String)> = BTreeMap::new();
        let mut total = 0u64;
        let mut patches = super::nexus_patches::Planner::new(game, &module)?;
        for item in game.mods.iter().filter(|m| m.enabled) {
            uuid::Uuid::parse_str(&item.id).map_err(|_| "Mod inválido.")?;
            let mut zip = super::nexus_archive::open(game,item)?;
            let selected = if let Some(files) = item.installed_files.as_ref().or(item.fomod_files.as_ref()) {
                super::nexus_fomod::validate_installation_files(&mut zip,&module,files)?;
                Some(files.clone())
            } else {
                let resolution = super::nexus_fomod::resolve(&mut zip, &module, &folder(game)?, &item.id, item.fomod_selection.as_deref())?;
                if !resolution.issues.is_empty() {return Err(resolution.issues.join("\n"));}
                resolution.files
            };
            let entries = (0..zip.len()).map(|i| zip.by_index(i).map(|entry| entry.name().replace('\\', "/")).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?;
            let patch_map = patches.map(&entries)?;
            // Manual roots and Unity packages are planned as a whole (DLL contents decide the loader folder).
            let unity_targets: Option<BTreeMap<String, String>> = match (&item.manual_root, module.unity()) {
                (Some(root), layout) if selected.is_none() => {
                    let mut files = manual_files(&module, &item.id, root, &entries)?;
                    if let Some(layout) = layout { super::nexus_unity::retarget_from_archive(&module, &layout, &mut zip, &mut files, &item.id, &folder(game)?)?; }
                    Some(files.into_iter().collect())
                }
                (None, Some(layout)) if selected.is_none() => Some(super::nexus_unity::plan_package(&module, &layout, &mut zip, &entries, &item.id, &folder(game)?)?.into_iter().collect()),
                _ => None,
            };
            let mut recognized = false;
            for i in 0..zip.len() {
                let entry = zip.by_index(i).map_err(|e| e.to_string())?;
                let name = entry.name().replace('\\', "/");
                if entry.is_dir() {continue;}
                drop(entry);
                let targets = if let Some(selected) = &selected { selected.iter().filter(|(source,_)|source == &name).map(|(_,target)|target.clone()).collect::<Vec<_>>() } else if let Some(planned) = &unity_targets { planned.get(&name).cloned().into_iter().collect() } else {module.route_package(&name,&item.id,&entries)?.into_iter().collect()};
                if targets.is_empty() && selected.is_none() {unmapped_file(&name)?;}
                if module.domain == "crimsondesert" && !targets.is_empty() {
                    module.route_package(&name,&item.id,&entries)?;
                }
                for target in targets {
                let target = match module.ue4ss() { Some(layout) if item.installed_files.is_none() => super::nexus_scan::ue4ss_mod_target(&folder(game)?, &layout, target)?, _ => target };
                let target = if module.domain == "helldivers2" {patch_map.get(&name).cloned().unwrap_or(target)} else {target};
                let entry = zip.by_index(i).map_err(|e|e.to_string())?;
                total = total
                    .checked_add(entry.size())
                    .ok_or("Arquivo grande demais.")?;
                if entry.size() > 512 * 1024 * 1024 || total > 2 * 1024 * 1024 * 1024 {
                    return Err("O perfil excede 2 GB de extração.".into());
                }
                if let Some((previous, owner)) = written.get(&target.to_lowercase()).cloned() {
                    // The same package mapping two files to one place is ambiguous, not a priority.
                    if owner == item.id { return Err(format!("Conflito de arquivos dentro do mesmo pacote: {target}")); }
                    if previous != target { let _ = fs::remove_file(dir.join(&previous)); }
                    let entry = overrides.entry(target.to_lowercase()).or_insert_with(|| Override { target: target.clone(), winner: String::new(), overridden: Vec::new() });
                    entry.overridden.push(owner);
                    entry.winner = item.id.clone();
                    entry.target = target.clone();
                }
                written.insert(target.to_lowercase(), (target.clone(), item.id.clone()));
                recognized = true;
                let path = dir.join(&target);
                fs::create_dir_all(path.parent().ok_or("Caminho inválido.")?)
                    .map_err(|e| e.to_string())?;
                let declared = entry.size();
                let copied = std::io::copy(
                    &mut entry.take(declared + 1),
                    &mut fs::File::create(&path).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?;
                if copied != declared {
                    return Err("Tamanho inválido no ZIP.".into());
                }
                if module.domain == "crimsondesert" && (target.to_ascii_lowercase().ends_with(".asi") || target.to_ascii_lowercase().ends_with(".dll")) {
                    crimson_plugin_header(fs::File::open(&path).map_err(|e|e.to_string())?, &name)?;
                }
                }
            }
            if !recognized {
                return Err(format!(
                    "{} não contém arquivos reconhecidos pelo módulo deste jogo.",
                    item.name
                ));
            }
        }
        let old = load_deployment(game)?;
        verify_targets(game,&old)?;
        let files: Vec<_> = walkdir::WalkDir::new(&dir).into_iter().filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file()).map(|entry| entry.path().strip_prefix(&dir).unwrap().to_string_lossy().into_owned()).collect();
        validate_companions(&files.iter().map(|target| (target.clone(), target.clone())).collect::<Vec<_>>())?;
        let missing = missing_frameworks(&module, &folder(game)?, &files, Some(&dir), &old.files);
        if require_dependencies && !missing.is_empty() { return Err(format!("Instale as dependências do módulo antes deste mod: {}", missing.iter().map(|item| item.name.as_str()).collect::<Vec<_>>().join(", "))); }
        for path in old.files.keys().filter(|p| config(&module, p)) {
            let source = if old.method == super::nexus_deploy_methods::Method::Vfs {
                super::nexus_deploy_methods::view(game)?.map(|view| view.join(path)).unwrap_or(destination(game,path)?)
            } else {destination(game,path)?};
            let cached = directory(game)?.join("settings").join(path);
            if source.is_file() {
                fs::create_dir_all(cached.parent().ok_or("Configuração inválida.")?)
                    .map_err(|e| e.to_string())?;
                fs::copy(&source, &cached).map_err(|e| e.to_string())?;
            }
        }
        for entry in walkdir::WalkDir::new(&dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let path = entry
                .path()
                .strip_prefix(&dir)
                .map_err(|e| e.to_string())?
                .to_string_lossy();
            if config(&module, &path) {
                let cached = directory(game)?.join("settings").join(path.as_ref());
                if cached.is_file() {
                    fs::copy(cached, entry.path()).map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    })();
    if result.is_ok() {
        // Recorded beside (never inside) the profile so it is not deployed to the game.
        save_overrides(game, overrides.into_values().collect())?;
    }
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok(revision)
}
/// Rollback remains armed until library.json is committed.
pub(super) struct Transaction {
    game: NexusGame,
    module: super::nexus_modules::Definition,
    root: PathBuf,
    backup: PathBuf,
    paths: Vec<String>,
    old_manifest: Option<Vec<u8>>,
    committed: bool,
}
impl Transaction {
    pub(super) fn commit(mut self) {
        self.committed = true;
        // SMAPI reports empty mod directories as invalid mods. Clean only the
        // parents of managed files, after the library commit makes rollback unnecessary.
        if self.module.domain == "stardewvalley" {
            let mods = self.root.join("Mods");
            for path in &self.paths {
                if let Ok(dest) = destination_in(&self.root, &self.module, path, &self.game.compat_data) {
                    prune_empty_mod_parents(&mods, &dest);
                }
            }
        }
        let _ = fs::remove_dir_all(&self.backup);
    }
}
fn prune_empty_mod_parents(mods: &Path, file: &Path) {
    if !file.starts_with(mods) { return; }
    let Ok(boundary) = fs::canonicalize(mods) else { return; };
    let mut parent = file.parent();
    while let Some(dir) = parent.filter(|dir| *dir != mods && dir.starts_with(mods)) {
        // Never follow directory links or remove user-generated/foreign content.
        if fs::symlink_metadata(dir).map_or(true, |m| m.file_type().is_symlink())
            || !fs::canonicalize(dir).is_ok_and(|p| p.starts_with(&boundary))
            || fs::remove_dir(dir).is_err() { break; }
        parent = dir.parent();
    }
}
impl Drop for Transaction {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for path in self.paths.iter().rev() {
            if let Ok(dest) = destination_in(&self.root, &self.module, path, &self.game.compat_data) {
                let _ = fs::remove_file(&dest);
                let old = self.backup.join(path);
                if fs::symlink_metadata(&old).is_ok() {
                    let _ = super::nexus_deploy_methods::backup(&old, &dest);
                }
            }
        }
        if let Ok(m) = manifest(&self.game) {
            if let Some(b) = &self.old_manifest {
                let _ = fs::write(m, b);
            } else {
                let _ = fs::remove_file(m);
            }
        }
        let _ = fs::remove_dir_all(&self.backup);
    }
}
pub(super) fn apply(game: &NexusGame) -> Result<Transaction, String> {
    idle(game)?;
    let module = definition(game).ok_or("Módulo do jogo não identificado.")?;
    let root = folder(game)?;
    let old = load_deployment(game)?;
    verify_targets(game,&old)?;
    let runtime = profile(game)?;
    super::nexus_activation::prepare(game, &module, &runtime, &old.files)?;
    super::nexus_deploy_methods::unmount(game)?;
    let mut next = Deployment { method: game.deploy_method, ..Default::default() };
    let managed: BTreeSet<PathBuf> = if old.method == super::nexus_deploy_methods::Method::Vfs { BTreeSet::new() } else { old.targets.values().map(PathBuf::from).collect() };
    for entry in walkdir::WalkDir::new(&runtime) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .strip_prefix(&runtime)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .to_string();
        let target=destination_managed(&root, &module, &path, &game.compat_data, &managed)?;
        if game.deploy_method == super::nexus_deploy_methods::Method::Vfs && path.starts_with('@') {
            return Err("VFS ainda não suporta destinos de mods dentro do prefixo. Selecione cópia ou links para este jogo.".into());
        }
        next.sources.insert(path.clone(), entry.path().to_string_lossy().into_owned());
        next.targets.insert(path.clone(),target.to_string_lossy().into_owned());
        next.files.insert(path, checksum(entry.path())?);
    }
    // Files written by this deployment are managed too while the old spelling is removed.
    let managed: BTreeSet<PathBuf> = managed.into_iter().chain(next.targets.values().map(PathBuf::from)).collect();
    let old_physical: BTreeSet<_> = if old.method == super::nexus_deploy_methods::Method::Vfs { BTreeSet::new() } else { old.files.keys().cloned().collect() };
    let all: BTreeSet<_> = old_physical.iter().chain(next.files.keys()).cloned().collect();
    for path in &all {
        let dest = destination_managed(&root, &module, path, &game.compat_data, &managed)?;
        if fs::symlink_metadata(&dest).is_err() {
            continue;
        }
        if fs::symlink_metadata(&dest).is_ok_and(|m|m.file_type().is_symlink()) && !old_physical.contains(path) { return Err(format!("Link externo preservado: {path}")); }
        let hash = checksum(&dest)?;
        match old.files.get(path).filter(|_| old_physical.contains(path)) {
            Some(expected) if expected != &hash && !config(&module, path) => {
                return Err(format!(
                    "Arquivo modificado fora do gerenciador; preservado: {path}"
                ))
            }
            None if !super::nexus_activation::generated_config(&module,path) => {
                return Err(format!(
                    "Arquivo existente não pertence ao perfil; preservado: {path}"
                ))
            }
            _ => {}
        }
    }
    let mut tx = Transaction {
        game: game.clone(),
        module: module.clone(),
        root: root.clone(),
        backup: directory(game)?.join(format!("rollback-{}", uuid::Uuid::new_v4())),
        paths: Vec::new(),
        old_manifest: fs::read(manifest(game)?).ok(),
        committed: false,
    };
    fs::create_dir_all(&tx.backup).map_err(|e| e.to_string())?;
    for path in all {
        if game.deploy_method == super::nexus_deploy_methods::Method::Vfs && !old_physical.contains(&path) {continue;}
        let dest = destination_managed(&root, &module, &path, &game.compat_data, &managed)?;
        if fs::symlink_metadata(&dest).is_ok() {
            let backup = tx.backup.join(&path);
            fs::create_dir_all(backup.parent().ok_or("Backup inválido.")?)
                .map_err(|e| e.to_string())?;
            super::nexus_deploy_methods::backup(&dest, &backup)?;
        }
        tx.paths.push(path.clone());
        if fs::symlink_metadata(&dest).is_ok() {
            fs::remove_file(&dest).map_err(|e| e.to_string())?;
        }
        if next.files.contains_key(&path) && game.deploy_method != super::nexus_deploy_methods::Method::Vfs {
            fs::create_dir_all(dest.parent().ok_or("Destino inválido.")?)
                .map_err(|e| e.to_string())?;
            // Mutable settings stay independent of profile files and link methods.
            let method = if config(&module,&path) || super::nexus_activation::generated_config(&module,&path) {super::nexus_deploy_methods::Method::Copy} else {game.deploy_method};
            method.place(&runtime.join(&path), &dest)?;
        }
    }
    if game.deploy_method != super::nexus_deploy_methods::Method::Vfs {super::nexus_activation::timestamps(game, &module, &runtime)?;}
    let temporary = tx.backup.join("manifest.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temporary, manifest(game)?).map_err(|e| e.to_string())?;
    Ok(tx)
}
pub(super) fn launch(game: &NexusGame) -> Result<(), String> { launch_with(game, None) }
pub(super) fn launch_tool(game: &NexusGame, name: &str) -> Result<(), String> {
    super::steam_readiness::check(Path::new(game.game["directory"].as_str().unwrap_or("")), game.game["app_id"].as_str())?;
    let module = definition(game).ok_or("Módulo não identificado.")?;
    let tool = module.tools.iter().find(|tool| tool.name == name).ok_or("Ferramenta não registrada para este jogo.")?;
    launch_with(game, Some(tool))
}
fn launch_with(game: &NexusGame, tool: Option<&super::nexus_modules::GameTool>) -> Result<(), String> {
    idle(game)?;
    let module = definition(game).ok_or("Módulo do jogo não identificado.")?;
    let dir = folder(game)?;
    let installed = load_deployment(game)?;
    verify_targets(game,&installed)?;
    if tool.is_none() {
        let files = installed.files.keys().cloned().collect::<Vec<_>>();
        let staged = profile(game).ok();
        let missing = missing_frameworks(&module, &dir, &files, staged.as_deref(), &installed.files);
        if !missing.is_empty() { return Err(format!("Instale as dependências antes de iniciar: {}", missing.iter().map(|item|item.name.as_str()).collect::<Vec<_>>().join(", "))); }
    }
    let virtual_view = if installed.method == super::nexus_deploy_methods::Method::Vfs {
        Some(super::nexus_deploy_methods::mount(game, &profile(game)?, &dir)?)
    } else {None};
    let visible = virtual_view.as_deref().unwrap_or(&dir);
    for (path, hash) in &installed.files {
        if !config(&module, path) && checksum(&if virtual_view.is_some() {visible.join(path)} else {destination(game,path)?})? != *hash {
            return Err(format!(
                "Arquivo do perfil alterado: {path}. Reinstale o mod."
            ));
        }
    }
    let ue4ss = module.ue4ss();
    if let Some(layout) = &ue4ss {
        let lua = installed.files.keys().any(|p| p.ends_with(".lua"));
        let logic = installed.files.keys().any(|p| p.starts_with(&format!("{}/", layout.logic_mods)));
        if (lua || logic) && !super::nexus_scan::ue4ss_installed(visible, layout) {
            return Err("Os scripts Lua/LogicMods exigem UE4SS ativo.".into());
        }
    }
    let modengine = module.domain == "eldenring" && installed.files.keys().any(|path| path.starts_with("Game/mod/"));
    if modengine && !visible.join("modengine2_launcher.exe").is_file() {return Err("Instale o Mod Engine 2 antes de iniciar este perfil.".into());}
    let reloaded = module.activation.as_ref().is_some_and(|a| a.kind == super::nexus_modules::ActivationKind::Reloaded) && installed.files.keys().any(|p|p.starts_with("Reloaded/Mods/"));
    let launcher = if reloaded { "Reloaded/Reloaded-II.exe" } else if modengine { "modengine2_launcher.exe" } else if module.domain == "finalfantasy12" && installed.files.contains_key("x64/ff12-launcher.exe") && installed.files.contains_key("x64/launcher.dll") && !visible.join("x64/dinput8.dll").is_file() && !visible.join("x64/dxgi.dll").is_file() {
        "x64/ff12-launcher.exe"
    } else if let Some(executable) = module.mod_launch_executable.as_deref().filter(|path| visible.join(path).is_file()) { executable
    } else {module.launch_executable.as_deref().unwrap_or(&module.executable)};
    if ["dragonageinquisition","starwarsbattlefront22017"].contains(&module.domain.as_str()) && !installed.files.is_empty() && tool.is_none() {
        return Err("Estes pacotes precisam ser aplicados no Frosty/DAI Mod Manager. Use o botão do gerenciador na aba Mods.".into());
    }
    let launcher = tool.map(|tool| tool.executable.as_str()).unwrap_or(launcher);
    // Module paths come from Windows; use the on-disk spelling of the executable.
    let located = super::nexus_discovery::locate_relative(visible, launcher);
    let launcher = located.as_deref().unwrap_or(launcher);
    if !visible.join(launcher).is_file() || !fs::canonicalize(visible.join(launcher)).is_ok_and(|path|path.starts_with(visible) || installed.sources.get(launcher).is_some_and(|source| Path::new(source) == path)) {return Err(format!("Ferramenta não instalada: {launcher}"));}
    let mut cmd = if game.platform == "native" {
        Command::new(dir.join(launcher))
    } else {
    let (prefix, proton) = validate_runner(&game.compat_data, &game.proton)?;
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let steam = [".local/share/Steam", ".steam/steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam", ".var/app/com.valvesoftware.Steam/data/Steam"]
        .iter().map(|relative| home.join(relative)).find(|path| path.is_dir());
    // Steam-style prefixes (also Heroic/Faugus with Proton) run through Proton when a
    // Steam client folder exists; otherwise umu runs the same compatdata folder.
    let mut command = if let Some(steam) = steam.filter(|_| prefix.join("pfx/system.reg").is_file()) {
        let mut command = Command::new(proton.join("proton"));
        command.arg("run").env("STEAM_COMPAT_CLIENT_INSTALL_PATH", steam)
            .env("STEAM_COMPAT_DATA_PATH", &prefix);
        command
    } else {
        let mut command = Command::new(crate::core::game_installation::find_umu()?);
        command.env("WINEPREFIX", &prefix).env("PROTONPATH", &proton).env("GAMEID", "umu-default");
        command
    };
    command.arg(dir.join(launcher));
    command
    };
    if let Some(tool) = tool {cmd.args(&tool.arguments);}
    if tool.is_none() && reloaded {cmd.arg("--launch").arg(format!("Z:{}",dir.join(&module.executable).display()).replace('/',"\\")).arg("--working-directory").arg(format!("Z:{}",dir.display()).replace('/',"\\"));}
    if tool.is_none() && modengine {cmd.args(["-t", "er", "-c", "PeliGamesModEngine.toml"]);}
    cmd.current_dir(if module.domain == "finalfantasy12" {dir.join("x64")} else {dir.clone()})
        .env("PELIGAMES_NEXUS_PROFILE", directory(game)?)
        .env("WINEDEBUG", "-all")
        .env_remove("PROTON_LOG");
    if let Some(id) = &module.steam_id {
        cmd.env("STEAM_COMPAT_APP_ID", id).env("SteamAppId", id).env("SteamGameId", id);
    }
    if tool.is_none() { super::nexus_launch_options::apply(game, &mut cmd, visible)?; }
    else { super::nexus_launch_options::apply_tool(game, &mut cmd, visible); }
    let cmd = if let Some(view) = virtual_view {super::nexus_deploy_methods::wrap(cmd, &view, &dir)?} else {cmd};
    if tool.is_some() { super::nexus_profile::spawn_tool(game, cmd) }
    else { super::nexus_profile::spawn(game, cmd) }
}
pub(super) fn stop(game: &NexusGame) -> Result<(), String> {
    let targets = processes(game, true);
    if targets.is_empty() {
        return Err("Este perfil não está em execução.".into());
    }
    // Signal only the Wine game processes, never the shared wineserver.
    for pid in targets {
        #[cfg(unix)]
        if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}
#[derive(Clone, Serialize)]
pub struct FrameworkStatus { pub name: String, pub mod_id: u64, pub url: String, pub installed: bool, pub kind: String }
pub(super) fn framework_status(module: &super::nexus_modules::Definition, root: &Path) -> Vec<FrameworkStatus> {
    module.frameworks.iter().map(|item| FrameworkStatus {
        name: item.name.clone(), mod_id: item.mod_id, installed: item.installed(root),
        url: item.url.clone().unwrap_or_else(|| format!("https://www.nexusmods.com/{}/mods/{}", module.domain, item.mod_id)),
        kind: if item.mod_id == 0 { "external" } else { "nexus" }.into(),
    }).collect()
}
fn missing_frameworks(module: &super::nexus_modules::Definition, root: &Path, targets: &[String], staged: Option<&Path>, owned: &BTreeMap<String,String>) -> Vec<FrameworkStatus> {
    module.frameworks.iter().filter(|item| targets.iter().any(|target| item.for_paths.iter().any(|prefix| target.starts_with(prefix)) && (item.for_extensions.is_empty() || item.for_extensions.iter().any(|ext| target.to_ascii_lowercase().ends_with(ext)))))
        .filter(|item| {
            let present = |file: &String| staged.map(|stage| stage.join(file).is_file()).unwrap_or_else(|| targets.contains(file))
                || !owned.contains_key(file) && root.join(file).is_file() && fs::canonicalize(root.join(file)).is_ok_and(|path| path.starts_with(root));
            !item.required_files.iter().all(present) || !item.any_files.is_empty() && !item.any_files.iter().any(present)
        }).map(|item| FrameworkStatus { name:item.name.clone(), mod_id:item.mod_id, url:item.url.clone().unwrap_or_else(|| format!("https://www.nexusmods.com/{}/mods/{}",module.domain,item.mod_id)), kind: if item.mod_id == 0 { "external" } else { "nexus" }.into(), installed:false }).collect()
}
#[derive(Serialize)]
pub struct InstallationPlan { pub overrides: Vec<(String, String)>, pub manual_root: Option<super::nexus_local::ManualRoot>, pub resolved_files: Vec<(String,String)>, pub activation_file: Option<String>, pub module: String, pub files: Vec<(String,String)>, pub missing: Vec<FrameworkStatus>, pub notes: Vec<String>, pub fomod: Option<super::nexus_fomod::Installer>, pub selection: Vec<String>, pub issues: Vec<String> }
pub(super) fn plan(game: &NexusGame, mod_id: &str) -> Result<InstallationPlan,String> {
    let module = super::nexus_discovery::at(Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?)).ok_or("Módulo do jogo não identificado.")?;
    let item = game.mods.iter().find(|item| item.id == mod_id).ok_or("Mod não encontrado.")?;
    uuid::Uuid::parse_str(mod_id).map_err(|_| "Mod inválido.")?;
    let mut archive = super::nexus_archive::open(game,item)?;
    let resolution = super::nexus_fomod::resolve(&mut archive,&module,Path::new(game.game["directory"].as_str().unwrap()),mod_id,item.fomod_selection.as_deref())?;
    let entries = (0..archive.len()).map(|i| archive.by_index(i).map(|entry| entry.name().replace('\\', "/")).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?;
    let unity = module.unity();
    let mut files = if let Some(selected) = resolution.files {selected} else if let Some(root) = &item.manual_root {
        let mut files = manual_files(&module, mod_id, root, &entries)?;
        if let Some(layout) = &unity { super::nexus_unity::retarget_from_archive(&module, layout, &mut archive, &mut files, mod_id, &folder(game)?)?; }
        files
    } else if let Some(layout) = &unity {
        let files = super::nexus_unity::plan_package(&module, layout, &mut archive, &entries, mod_id, &folder(game)?)?;
        let planned: BTreeSet<&String> = files.iter().map(|(source, _)| source).collect();
        for name in entries.iter().filter(|e| !e.ends_with('/') && !planned.contains(e)) { unmapped_file(name)?; }
        files
    } else {
        let mut files = Vec::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(|error|error.to_string())?;
            if entry.is_dir() {continue;}
            let name = entry.name().replace('\\', "/");
            if let Some(target) = module.route_package(&name,mod_id,&entries)? {files.push((name,target));} else {unmapped_file(&name)?;}
        }
        files
    };
    if module.domain == "helldivers2" {
        let mut patches = super::nexus_patches::Planner::new(game, &module)?;
        for previous in game.mods.iter().take_while(|previous| previous.id != item.id).filter(|previous| previous.enabled) {
            let mut archive = super::nexus_archive::open(game,previous)?;
            let entries = (0..archive.len()).map(|i| archive.by_index(i).map(|e|e.name().replace('\\',"/")).map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
            patches.map(&entries)?;
        }
        let patch_map = patches.map(&entries)?;
        for (source,target) in &mut files {if let Some(path) = patch_map.get(source) {*target=path.clone();}}
    }
    if let Some(layout) = module.ue4ss() {
        let root = folder(game)?;
        for (_, target) in &mut files { *target = super::nexus_scan::ue4ss_mod_target(&root, &layout, target.clone())?; }
    }
    files.sort();
    if module.domain == "crimsondesert" {
        for (source, target) in &files {
            module.route_package(source, mod_id, &entries)?;
            if target.to_ascii_lowercase().ends_with(".asi") || target.to_ascii_lowercase().ends_with(".dll") {
                let index = (0..archive.len()).find(|index| archive.by_index(*index).is_ok_and(|entry| entry.name().replace('\\', "/") == *source)).ok_or("Plugin não encontrado no pacote.")?;
                crimson_plugin_header(archive.by_index(index).map_err(|e|e.to_string())?, source)?;
            }
        }
    }
    let mut destinations = BTreeSet::new();
    for (_,target) in &files {if !destinations.insert(target.to_ascii_lowercase()) {return Err(format!("O pacote contém alternativas para o mesmo destino: {target}. Selecione uma versão do mod."));}}
    if files.is_empty() && resolution.issues.is_empty() {return Err("O pacote não contém arquivos reconhecidos pelo módulo do jogo.".into());}
    if resolution.issues.is_empty() {validate_companions(&files)?;}
    let mut issues = resolution.issues;
    for (_, target) in &files {
        if target.starts_with('@') {
            if let Err(error) = super::nexus_targets::root(Path::new(game.game["directory"].as_str().unwrap()), &game.compat_data, target) {if !issues.contains(&error) {issues.push(error);}}
        }
    }
    let mut resolved_files=Vec::new();
    for(source,target)in &files {
        match destination_in(Path::new(game.game["directory"].as_str().unwrap()),&module,target,&game.compat_data) {
            Ok(path)=>resolved_files.push((source.clone(),path.to_string_lossy().into_owned())),
            Err(error)=>if !issues.contains(&error){issues.push(error);},
        }
    }
    let targets = files.iter().map(|(_,target)|target.clone()).collect::<Vec<_>>();
    let mut activation_file = None;
    if let Some(rule) = &module.activation {
        let needs_activation = targets.iter().any(|target| target.starts_with(&format!("{}/", rule.data)) && match rule.kind {
            super::nexus_modules::ActivationKind::Bethesda => [".esp", ".esm", ".esl"].iter().any(|ext| target.to_ascii_lowercase().ends_with(ext)),
            super::nexus_modules::ActivationKind::Morrowind => [".esp", ".esm", ".bsa"].iter().any(|ext| target.to_ascii_lowercase().ends_with(ext)),
            super::nexus_modules::ActivationKind::BaldursGate3 => target.to_ascii_lowercase().ends_with(".pak"),
            _ => true,
        });
        if needs_activation {
            match destination_in(Path::new(game.game["directory"].as_str().unwrap()), &module, &rule.path, &game.compat_data) {
                Ok(path) => activation_file = Some(path.to_string_lossy().into_owned()),
                Err(error) => if !issues.contains(&error) {issues.push(error);},
            }
        }
    }

    let current_profile = profile(game).ok();
    let owned = load_deployment(game)?;
    verify_targets(game,&owned)?;
    let missing = missing_frameworks(&module, Path::new(game.game["directory"].as_str().unwrap()), &targets, current_profile.as_deref(), &owned.files);
    // Files this mod will replace in enabled mods above it (it is installed at the end of the list).
    let planned: BTreeMap<String, &String> = files.iter().map(|(_, target)| (target.to_ascii_lowercase(), target)).collect();
    let mut overrides = Vec::new();
    for other in game.mods.iter().filter(|other| other.id != item.id && other.installed && other.enabled) {
        for (_, target) in other.installed_files.iter().flatten() {
            if let Some(mine) = planned.get(&target.to_ascii_lowercase()) { overrides.push(((*mine).clone(), other.name.clone())); }
        }
    }
    overrides.sort(); overrides.dedup(); overrides.truncate(500);
    Ok(InstallationPlan {overrides, manual_root: item.manual_root.clone(), resolved_files, activation_file, module:module.name, files, missing, notes:module.notes, fomod:resolution.installer, selection:resolution.selection, issues})
}

#[cfg(test)]
mod package_completeness_tests {
    use super::*;
    #[test]
    #[ignore = "Requires a real CET archive and an isolated XDG_CONFIG_HOME"]
    fn real_cet_survives_disabling_another_mod_and_preserves_foreign_files() {
        use std::io::Write;
        let config = PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap());
        assert!(config.starts_with("/tmp") && config.file_name().unwrap().to_string_lossy().starts_with("peligames-"));
        let archive = PathBuf::from(std::env::var("PELIGAMES_TEST_CET_ARCHIVE").unwrap());
        let root = config.join(format!("cyberpunk-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("bin/x64")).unwrap();
        fs::write(root.join("bin/x64/Cyberpunk2077.exe"), b"MZ fixture").unwrap();
        fs::write(root.join("bin/x64/unrelated.ini"), b"original config").unwrap();
        let cet = uuid::Uuid::new_v4().to_string();
        let other = uuid::Uuid::new_v4().to_string();
        let mut game: NexusGame = serde_json::from_value(serde_json::json!({
            "id":uuid::Uuid::new_v4().to_string(),"game":{"directory":root,"name":"Cyberpunk 2077"},"platform":"proton",
            "mods":[{"id":cet,"name":"CET","archive":archive,"size":0,"installed":true,"enabled":true},
            {"id":other,"name":"Other","archive":"other.zip","size":0,"installed":true,"enabled":true}]
        })).unwrap();
        let archives = directory(&game).unwrap().join("archives");
        fs::create_dir_all(&archives).unwrap();
        fs::copy(archive, archives.join(format!("{cet}.zip"))).unwrap();
        let redscript = std::env::var("PELIGAMES_TEST_REDSCRIPT_ARCHIVE").ok();
        let mut zip = zip::ZipWriter::new(fs::File::create(archives.join(format!("{other}.zip"))).unwrap());
        zip.start_file("archive/pc/mod/other.archive", zip::write::FileOptions::default()).unwrap();
        zip.write_all(b"fixture archive").unwrap(); zip.finish().unwrap();
        if let Some(archive) = &redscript { fs::copy(archive, archives.join(format!("{other}.zip"))).unwrap(); }
        let approved = plan(&game, &cet).unwrap();
        assert!(approved.issues.is_empty());
        game.mods[0].installed_files = Some(approved.files);
        game.profile = Some(build(&game).unwrap());
        drop(apply(&game).unwrap());
        assert!(!root.join("bin/x64/global.ini").exists());
        apply(&game).unwrap().commit();
        if redscript.is_some() { assert!(root.join("engine/config/base/scripts.ini").is_file()); }
        let cet_config = fs::read(root.join("bin/x64/global.ini")).unwrap();
        game.mods[1].enabled = false;
        game.profile = Some(build_after_removal(&game).unwrap());
        apply(&game).unwrap().commit();
        assert_eq!(fs::read(root.join("bin/x64/global.ini")).unwrap(), cet_config);
        assert!(root.join("bin/x64/plugins/cyber_engine_tweaks.asi").is_file());
        assert!(!root.join("archive/pc/mod/other.archive").exists());
        assert!(!root.join("engine/config/base/scripts.ini").exists());
        game.mods[0].enabled = false;
        game.profile = Some(build_after_removal(&game).unwrap());
        apply(&game).unwrap().commit();
        assert!(!root.join("bin/x64/global.ini").exists());
        assert_eq!(fs::read(root.join("bin/x64/unrelated.ini")).unwrap(), b"original config");
        assert!(!root.join("bin/x64/plugins/cyber_engine_tweaks.asi").exists());
    }
    #[test]
    fn cet_global_ini_is_installed_without_allowing_arbitrary_bin_configs() {
        let module = super::super::nexus_modules::registry().definitions.into_iter().find(|m| m.domain == "cyberpunk2077").unwrap();
        let source = "bin/x64/global.ini";
        assert_eq!(module.route_package(source, "00000000-0000-4000-8000-000000000001", &[source.into()]).unwrap().as_deref(), Some(source));
        assert!(module.allowed(source));
        let redscript_config = "engine/config/base/scripts.ini";
        assert_eq!(module.route_package(redscript_config, "00000000-0000-4000-8000-000000000001", &[redscript_config.into()]).unwrap().as_deref(), Some(redscript_config));
        assert_eq!(module.route_package("bin/x64/unknown.ini", "00000000-0000-4000-8000-000000000001", &[]).unwrap(), None);
        assert!(unmapped_file("bin/x64/unknown.ini").is_err());
    }
    #[test]
    fn mixed_packages_cannot_silently_discard_configuration_or_content() {
        let modules = super::super::nexus_modules::registry().definitions;
        for module in modules.iter().filter(|m| m.engine == super::super::nexus_modules::Engine::Deployment) {
            for source in ["unsupported/config/settings.json", "unsupported/config.ini", "unsupported/content.dds", "unsupported/model.assetbundle", "unsupported/bundle.repobundle"] {
                if module.route_package(source, "00000000-0000-4000-8000-000000000001", &[source.into()]).is_ok_and(|target| target.is_none()) {
                    assert!(unmapped_file(source).is_err(), "{} silently discards {source}", module.domain);
                }
            }
        }
        for source in ["README.md", "LICENSE.txt", "icon.png", "manifest.json", "thunderstore.toml"] { assert!(unmapped_file(source).is_ok(), "{source}"); }
        assert!(unmapped_file("AnotherMod/manifest.json").is_err());
    }
}

#[cfg(test)]
mod crimson_tests {
    use super::*;
    #[test]
    fn plugin_validation_rejects_32bit_executables_and_truncated_headers() {
        let mut bytes = vec![0u8; 154];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&128u32.to_le_bytes());
        bytes[128..132].copy_from_slice(b"PE\0\0");
        bytes[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
        bytes[150..152].copy_from_slice(&0x2000u16.to_le_bytes());
        bytes[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
        assert!(crimson_plugin_header(bytes.as_slice(),"plugin.asi").is_ok());
        bytes[132..134].copy_from_slice(&0x14cu16.to_le_bytes());
        assert!(crimson_plugin_header(bytes.as_slice(),"plugin.asi").is_err());
        bytes[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
        bytes[150..152].fill(0);
        assert!(crimson_plugin_header(bytes.as_slice(),"plugin.asi").is_err());
        assert!(crimson_plugin_header(&bytes[..70],"version.dll").is_err());
        bytes[60..64].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(crimson_plugin_header(bytes.as_slice(),"plugin.asi").is_err());
    }
    #[test]
    fn crimson_requires_loader_for_asi_but_accepts_loader_in_same_package() {
        let module = super::super::nexus_modules::registry().definitions.into_iter().find(|m|m.domain=="crimsondesert").unwrap();
        let root = std::env::temp_dir().join(format!("crimson-requirements-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("bin64")).unwrap();
        let plugins = vec!["bin64/autoloot.asi".into()];
        assert_eq!(missing_frameworks(&module,&root,&plugins,None,&BTreeMap::new()).len(),1);
        let bundled = vec!["bin64/autoloot.asi".into(),"bin64/winmm.dll".into()];
        assert!(missing_frameworks(&module,&root,&bundled,None,&BTreeMap::new()).is_empty());
        fs::write(root.join("bin64/version.dll"),b"fixture").unwrap();
        assert!(missing_frameworks(&module,&root,&plugins,None,&BTreeMap::new()).is_empty());
        let owned = BTreeMap::from([("bin64/version.dll".into(),"previous loader".into())]);
        assert_eq!(missing_frameworks(&module,&root,&plugins,None,&owned).len(),1);
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod cyberpunk_tests {
    use super::*;
    #[test]
    fn cet_script_requires_active_loader_and_preserves_all_files() {
        let module = super::super::nexus_modules::registry().definitions.into_iter().find(|m|m.domain=="cyberpunk2077").unwrap();
        let root = std::env::temp_dir().join(format!("cet-requirements-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("bin/x64/plugins")).unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        let sources: Vec<String> = vec!["bin/x64/plugins/cyber_engine_tweaks/mods/Vehicle Customizer/init.lua".into(),
            "bin/x64/plugins/cyber_engine_tweaks/mods/Vehicle Customizer/config/settings.json".into()];
        let files: Vec<_> = sources.iter().map(|source| module.route_package(source,id,&sources).unwrap().unwrap()).collect();
        assert_eq!(files, sources);
        let missing = missing_frameworks(&module,&root,&files,None,&BTreeMap::new());
        assert_eq!(missing.len(),1);
        assert_eq!(missing[0].mod_id,107);
        let loader = "bin/x64/plugins/cyber_engine_tweaks.asi";
        fs::write(root.join(loader),b"fixture").unwrap();
        assert!(missing_frameworks(&module,&root,&files,None,&BTreeMap::new()).is_empty());
        // Previously deployed, disabled loader files cannot satisfy the requirement.
        let owned = BTreeMap::from([(loader.into(),"previous CET deployment".into())]);
        assert_eq!(missing_frameworks(&module,&root,&files,None,&owned).len(),1);
        let mut bundled = files.clone();bundled.push(loader.into());
        assert!(missing_frameworks(&module,&root,&bundled,None,&owned).is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod palworld_lifecycle_test {
    use super::*;
    #[test]
    #[ignore = "Requires a real mod archive and an isolated XDG_CONFIG_HOME"]
    fn actual_dekmcm_install_disable_remove_and_rollback() {
        let config_root = PathBuf::from(std::env::var("XDG_CONFIG_HOME").expect("Isolated config required"));
        assert!(config_root.starts_with("/tmp") && config_root.file_name().unwrap().to_string_lossy().starts_with("peligames-"));
        let archive = std::env::var("PELIGAMES_TEST_MOD_ARCHIVE").expect("Real archive required");
        for flat in [true, false] {
            let root = config_root.join(format!("game-{}",uuid::Uuid::new_v4()));
            let binaries = root.join("Pal/Binaries/Win64");
            fs::create_dir_all(&binaries).unwrap();
            fs::write(root.join("Palworld.exe"), b"MZ fixture").unwrap();
            fs::write(binaries.join("Palworld-Win64-Shipping.exe"), b"MZ fixture").unwrap();
            fs::write(binaries.join("dwmapi.dll"), b"manual proxy").unwrap();
            let loader = if flat { binaries.join("UE4SS.dll") } else { binaries.join("ue4ss/UE4SS.dll") };
            fs::create_dir_all(loader.parent().unwrap()).unwrap();
            fs::write(&loader, b"manual loader").unwrap();
            let mods = if flat { binaries.join("Mods") } else { binaries.join("ue4ss/Mods") };
            fs::create_dir_all(&mods).unwrap();
            let list = mods.join("mods.txt");
            fs::write(&list, "; manual\r\nManual : 1\r\n").unwrap();
            let id = uuid::Uuid::new_v4().to_string();
            let mut game: NexusGame = serde_json::from_value(serde_json::json!({
                "id":uuid::Uuid::new_v4().to_string(),"game":{"directory":root,"name":"Palworld","path":root.join("Palworld.exe")},
                "platform":"proton","mods":[{"id":id,"name":"DekMCM","archive":archive,"size":11800471,"installed":true,"enabled":true}]
            })).unwrap();
            let managed = directory(&game).unwrap().join("archives");
            fs::create_dir_all(&managed).unwrap();
            fs::copy(&archive, managed.join(format!("{id}.zip"))).unwrap();
            let plan = plan(&game, &id).unwrap();
            assert_eq!(plan.files.len(),3);
            assert!(plan.issues.is_empty());
            game.profile = Some(build(&game).unwrap());
            // Dropping an uncommitted transaction restores the pre-install game.
            drop(apply(&game).unwrap());
            assert!(!mods.join("DekModConfigMenu/Scripts/main.lua").exists());
            assert_eq!(fs::read_to_string(&list).unwrap(), "; manual\r\nManual : 1\r\n");
            apply(&game).unwrap().commit();
            assert!(mods.join("DekModConfigMenu/Scripts/main.lua").is_file());
            assert!(mods.join("DekModConfigMenu/enabled.txt").is_file());
            let pak = root.join("Pal/Content/Paks/LogicMods/DekModConfigMenu_P.pak");
            assert!(pak.is_file());
            assert!(fs::read_to_string(&list).unwrap().contains("DekModConfigMenu : 1"));
            game.mods[0].enabled=false;
            game.profile=Some(build_after_removal(&game).unwrap());
            apply(&game).unwrap().commit();
            assert!(!mods.join("DekModConfigMenu/Scripts/main.lua").exists() && !pak.exists());
            assert!(fs::read_to_string(&list).unwrap().contains("Manual : 1"));
            game.mods.clear();
            game.profile=Some(build_after_removal(&game).unwrap());
            apply(&game).unwrap().commit();
            assert!(fs::read_to_string(&list).unwrap().contains("Manual : 1"));
            assert_eq!(fs::read(&loader).unwrap(), b"manual loader");
        }
    }
}

#[cfg(test)]
mod manual_root_tests {
    use super::*;
    use super::super::nexus_local::ManualRoot;
    const ID: &str = "00000000-0000-4000-8000-000000000001";

    fn module() -> super::super::nexus_modules::Definition {
        super::super::nexus_modules::parse(br#"{"schema":1,"version":"1.0.0","domain":"manualtest","name":"Manual Test","platform":"proton","engine":"deployment",
            "executable":"Game.exe","required":["Game.exe"],"header":[77,90],"destinations":["Data","Mods"],
            "routes":[{"kind":"extension","extension":".esp","target":"Data/","root_only":true}],
            "rejected_extensions":[".exe"],"protected_files":["Data/Game.esm"]}"#).unwrap()
    }
    fn entries(items: &[&str]) -> Vec<String> { items.iter().map(|s| s.to_string()).collect() }
    fn root(source: &str, destination: &str) -> ManualRoot { ManualRoot { source: source.into(), destination: destination.into() } }

    #[test]
    fn chosen_root_feeds_module_routes_or_an_allowed_destination() {
        let m = module();
        let pack = entries(&["Author Pack/Main/Plugin.esp", "Author Pack/Main/textures/a.dds", "Author Pack/readme.txt"]);
        // Automatic routes from the chosen folder: unknown loose content is refused, not dropped.
        assert!(manual_files(&m, ID, &root("Author Pack/Main", ""), &pack).is_err());
        let only_plugin = entries(&["Author Pack/Main/Plugin.esp", "Author Pack/readme.txt"]);
        assert_eq!(manual_files(&m, ID, &root("Author Pack/Main/", ""), &only_plugin).unwrap(), vec![("Author Pack/Main/Plugin.esp".to_string(), "Data/Plugin.esp".to_string())]);
        // Explicit destination: the whole folder keeps its structure below it.
        let files = manual_files(&m, ID, &root("Author Pack/Main", "Data"), &pack).unwrap();
        assert_eq!(files, vec![
            ("Author Pack/Main/Plugin.esp".to_string(), "Data/Plugin.esp".to_string()),
            ("Author Pack/Main/textures/a.dds".to_string(), "Data/textures/a.dds".to_string()),
        ]);
    }

    #[test]
    fn manual_root_keeps_every_module_protection() {
        let m = module();
        let pack = entries(&["Main/textures/a.dds"]);
        assert!(manual_files(&m, ID, &root("Main", "Elsewhere"), &pack).is_err(), "destination outside the module");
        assert!(manual_files(&m, ID, &root("../Main", "Data"), &pack).is_err(), "unsafe root");
        assert!(manual_files(&m, ID, &root("Missing", "Data"), &pack).is_err(), "empty root");
        assert!(manual_files(&m, ID, &root("Main", "Data"), &entries(&["Main/tool.exe"])).is_err(), "rejected extension");
        assert!(manual_files(&m, ID, &root("Main", "Data"), &entries(&["Main/Game.esm"])).is_err(), "protected original");
        assert!(manual_files(&m, ID, &root("Main", "Data"), &entries(&["Main/a.dds", "Other/plugin.dll"])).is_err(), "functional file outside the root");
        assert!(manual_files(&m, ID, &root("Main", "Data"), &entries(&["Main/a.dds", "Other/readme.txt"])).is_ok());
    }

    #[test]
    fn package_folders_lists_bounded_archive_folders() {
        let folders = package_folders(&entries(&["A/B/c.txt", "A/d.txt", "E/", "f.txt"]));
        assert_eq!(folders, vec!["", "A/", "A/B/", "E/"]);
    }
}

#[cfg(test)]
mod priority_tests {
    use super::*;
    use std::io::Write;

    fn zip(path: &Path, files: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes) in files {
            writer.start_file(*name, zip::write::FileOptions::default()).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    #[ignore = "Requires an isolated XDG_CONFIG_HOME (/tmp/peligames-*)"]
    fn later_mod_overrides_earlier_files_and_order_changes_winner() {
        let config_root = PathBuf::from(std::env::var("XDG_CONFIG_HOME").expect("Isolated config required"));
        assert!(config_root.starts_with("/tmp") && config_root.file_name().unwrap().to_string_lossy().starts_with("peligames-"));
        let root = config_root.join(format!("game-{}", uuid::Uuid::new_v4()));
        let binaries = root.join("SB/Binaries/Win64");
        fs::create_dir_all(&binaries).unwrap();
        fs::write(binaries.join("SB-Win64-Shipping.exe"), b"MZ fixture").unwrap();
        let (first, second) = (uuid::Uuid::new_v4().to_string(), uuid::Uuid::new_v4().to_string());
        let mut game: NexusGame = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "game": {"directory": root, "name": "Stellar Blade", "path": binaries.join("SB-Win64-Shipping.exe")},
            "platform": "proton", "mods": [
                {"id": first, "name": "Base", "archive": "base.zip", "size": 1, "installed": true, "enabled": true},
                {"id": second, "name": "Patch", "archive": "patch.zip", "size": 1, "installed": true, "enabled": true}]
        })).unwrap();
        assert_eq!(definition(&game).unwrap().domain, "stellarblade");
        let managed = directory(&game).unwrap().join("archives");
        fs::create_dir_all(&managed).unwrap();
        zip(&managed.join(format!("{first}.zip")), &[("Outfit_P.pak", b"base outfit"), ("Shared_P.pak", b"from base")]);
        // Same file with another letter case: on Linux both spellings must not survive.
        zip(&managed.join(format!("{second}.zip")), &[("shared_p.PAK", b"from patch")]);
        let paks = root.join("SB/Content/Paks/~mods");

        game.profile = Some(build(&game).unwrap());
        apply(&game).unwrap().commit();
        assert_eq!(fs::read(paks.join("shared_p.PAK")).unwrap(), b"from patch");
        assert!(!paks.join("Shared_P.pak").exists(), "a grafia antiga foi substituída");
        assert_eq!(fs::read(paks.join("Outfit_P.pak")).unwrap(), b"base outfit");
        let recorded = overrides(&game).unwrap();
        assert_eq!(recorded, vec![Override { target: "SB/Content/Paks/~mods/shared_p.PAK".into(), winner: second.clone(), overridden: vec![first.clone()] }]);

        // Moving the patch above the base gives the base priority.
        game.mods.swap(0, 1);
        game.profile = Some(build(&game).unwrap());
        apply(&game).unwrap().commit();
        assert_eq!(fs::read(paks.join("Shared_P.pak")).unwrap(), b"from base");
        assert!(!paks.join("shared_p.PAK").exists());
        assert_eq!(overrides(&game).unwrap()[0].winner, first);

        // Disabling the winner restores the other mod's file.
        game.mods[1].enabled = false;
        game.profile = Some(build_after_removal(&game).unwrap());
        apply(&game).unwrap().commit();
        assert_eq!(fs::read(paks.join("shared_p.PAK")).unwrap(), b"from patch");
        assert!(overrides(&game).unwrap().is_empty());

        // Two files of one package for the same destination stay an error.
        zip(&managed.join(format!("{second}.zip")), &[("a/Dup_P.pak", b"a"), ("b/dup_p.pak", b"b")]);
        // After the swap the patch is mods[0]; only the duplicated package is enabled.
        game.mods[0].enabled = true;
        game.mods[1].enabled = false;
        assert!(build(&game).unwrap_err().contains("mesmo pacote"));
        fs::remove_dir_all(&root).unwrap();
    }
}
