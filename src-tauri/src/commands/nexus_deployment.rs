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
fn unmapped_file(name: &str) -> Result<(), String> {
    let extension = Path::new(name).extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase();
    if name.to_ascii_lowercase().contains(".patch_") || ["dll", "asi", "lua", "exe", "pak", "utoc", "ucas", "archive", "esp", "esm", "esl", "bsa", "ba2", "vbf", "fbmod", "fbpack", "smp", "dazip", "daimod", "mem", "ts4script", "package", "dl_bin"].contains(&extension.as_str()) {
        return Err(format!("O pacote contém conteúdo que este módulo não sabe instalar: {name}. Nenhum arquivo foi aplicado ao jogo."));
    }
    Ok(())
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
            if let Some(steamapps) = dir.parent().and_then(Path::parent) {
                let Some(id) = definition(game).and_then(|module| module.steam_id) else { return; };
                let prefix = steamapps.join("compatdata").join(id);
                if prefix.join("pfx/system.reg").is_file() {
                    game.compat_data = prefix.to_string_lossy().into();
                }
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
    module.domain == "palworld" && (path.ends_with("UE4SS-settings.ini")
        || path.ends_with("MemberVariableLayout.ini")
        || path.ends_with("/Mods/mods.txt")
        || path.ends_with("/Mods/mods.json")
        || (path.starts_with("Pal/Content/Paks/LogicMods/")
            && path.ends_with(".modconfig.json")))
}
pub(super) fn destination(game: &NexusGame, path: &str) -> Result<PathBuf, String> {
    let module = definition(game).ok_or("Módulo do jogo não identificado.")?;
    destination_in(&folder(game)?, &module, path, &game.compat_data)
}
fn destination_in(root: &Path, module: &super::nexus_modules::Definition, path: &str, prefix: &str) -> Result<PathBuf, String> {
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
                if name.eq_ignore_ascii_case(component) && name != component {
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
    let result = (|| {
        let mut written = BTreeSet::new();
        let mut total = 0u64;
        let mut patches = super::nexus_patches::Planner::new(game, &module)?;
        for item in game.mods.iter().filter(|m| m.enabled) {
            uuid::Uuid::parse_str(&item.id).map_err(|_| "Mod inválido.")?;
            let mut zip = super::nexus_archive::open(game,item)?;
            let selected = if let Some(files) = &item.fomod_files {
                super::nexus_fomod::validate_files(&mut zip,&module,files)?;
                Some(files.clone())
            } else {
                let resolution = super::nexus_fomod::resolve(&mut zip, &module, &folder(game)?, &item.id, item.fomod_selection.as_deref())?;
                if !resolution.issues.is_empty() {return Err(resolution.issues.join("\n"));}
                resolution.files
            };
            let entries = (0..zip.len()).map(|i| zip.by_index(i).map(|entry| entry.name().replace('\\', "/")).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?;
            let patch_map = patches.map(&entries)?;
            let mut recognized = false;
            for i in 0..zip.len() {
                let entry = zip.by_index(i).map_err(|e| e.to_string())?;
                let name = entry.name().replace('\\', "/");
                if entry.is_dir() {continue;}
                drop(entry);
                let targets = if let Some(selected) = &selected { selected.iter().filter(|(source,_)|source == &name).map(|(_,target)|target.clone()).collect::<Vec<_>>() } else {module.route_package(&name,&item.id,&entries)?.into_iter().collect()};
                if targets.is_empty() && selected.is_none() {unmapped_file(&name)?;}
                if module.domain == "crimsondesert" && !targets.is_empty() {
                    module.route_package(&name,&item.id,&entries)?;
                }
                for target in targets {
                let target = if module.domain == "helldivers2" {patch_map.get(&name).cloned().unwrap_or(target)} else {target};
                let entry = zip.by_index(i).map_err(|e|e.to_string())?;
                total = total
                    .checked_add(entry.size())
                    .ok_or("Arquivo grande demais.")?;
                if entry.size() > 512 * 1024 * 1024 || total > 2 * 1024 * 1024 * 1024 {
                    return Err("O perfil excede 2 GB de extração.".into());
                }
                if !written.insert(target.to_lowercase()) {
                    return Err(format!("Conflito de arquivos: {target}"));
                }
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
        let target=destination_in(&root, &module, &path, &game.compat_data)?;
        if game.deploy_method == super::nexus_deploy_methods::Method::Vfs && path.starts_with('@') {
            return Err("VFS ainda não suporta destinos de mods dentro do prefixo. Selecione cópia ou links para este jogo.".into());
        }
        next.sources.insert(path.clone(), entry.path().to_string_lossy().into_owned());
        next.targets.insert(path.clone(),target.to_string_lossy().into_owned());
        next.files.insert(path, checksum(entry.path())?);
    }
    let old_physical: BTreeSet<_> = if old.method == super::nexus_deploy_methods::Method::Vfs { BTreeSet::new() } else { old.files.keys().cloned().collect() };
    let all: BTreeSet<_> = old_physical.iter().chain(next.files.keys()).cloned().collect();
    for path in &all {
        let dest = destination_in(&root, &module, path, &game.compat_data)?;
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
        let dest = destination_in(&root, &module, &path, &game.compat_data)?;
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
    let lua = module.domain == "palworld" && installed.files.keys().any(|p| p.ends_with(".lua"));
    let loader = module.domain == "palworld" && super::nexus_scan::ue4ss_installed(visible);
    if module.domain == "palworld" && (lua || installed.files.keys().any(|p| p.contains("/LogicMods/"))) && !loader {
        return Err("Os scripts Lua/LogicMods exigem UE4SS ativo.".into());
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
    if !visible.join(launcher).is_file() || !fs::canonicalize(visible.join(launcher)).is_ok_and(|path|path.starts_with(visible) || installed.sources.get(launcher).is_some_and(|source| Path::new(source) == path)) {return Err(format!("Ferramenta não instalada: {launcher}"));}
    let mut cmd = if game.platform == "native" {
        Command::new(dir.join(launcher))
    } else {
    let (prefix, proton) = validate_runner(&game.compat_data, &game.proton)?;
    let mut command = if prefix.join("pfx/system.reg").is_file() {
        let steam = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?.join(".local/share/Steam");
        if !steam.exists() { return Err("Instalação nativa da Steam não encontrada.".into()); }
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
    if let Some(tool) = tool {cmd.args(&tool.arguments);} else {cmd.args(&module.launch_arguments);}
    if tool.is_none() && reloaded {cmd.arg("--launch").arg(format!("Z:{}",dir.join(&module.executable).display()).replace('/',"\\")).arg("--working-directory").arg(format!("Z:{}",dir.display()).replace('/',"\\"));}
    if tool.is_none() && modengine {cmd.args(["-t", "er", "-c", "PeliGamesModEngine.toml"]);}
    cmd.current_dir(if module.domain == "finalfantasy12" {dir.join("x64")} else {dir.clone()})
        .env("PELIGAMES_NEXUS_PROFILE", directory(game)?)
        .env("WINEDEBUG", "-all")
        .env_remove("PROTON_LOG");
    if let Some(id) = &module.steam_id {
        cmd.env("STEAM_COMPAT_APP_ID", id).env("SteamAppId", id).env("SteamGameId", id);
    }
    if loader {
        let inherited = std::env::var("WINEDLLOVERRIDES").unwrap_or_default();
        cmd.env(
            "WINEDLLOVERRIDES",
            if inherited.is_empty() {
                "dwmapi=n,b".into()
            } else {
                format!("{inherited};dwmapi=n,b")
            },
        );
    }
    for item in &module.dll_overrides {
        if module.domain == "finalfantasy12" && item.name == "dxgi" && visible.join("x64/dinput8.dll").is_file() { continue; }
        if visible.join(&item.file).is_file() {
            let current = cmd.get_envs().find(|(key, _)| *key == "WINEDLLOVERRIDES")
                .and_then(|(_, value)| value).map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_else(|| std::env::var("WINEDLLOVERRIDES").unwrap_or_default());
            cmd.env("WINEDLLOVERRIDES", if current.is_empty() { format!("{}=n,b", item.name) } else { format!("{current};{}=n,b", item.name) });
        }
    }
    // UE4SS rotates one current log; retain its predecessor beside the launcher logs.
    let ue4ss = visible.join("Pal/Binaries/Win64/ue4ss/UE4SS.log");
    let logs = directory(game)?.join("logs");
    fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    if module.domain == "palworld" && ue4ss.exists() {
        use std::io::Write;
        let content = super::nexus_profile::read_log_file(&ue4ss)?;
        let mut output = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(logs.join("current.log"))
            .map_err(|e| e.to_string())?;
        writeln!(output, "\n--- UE4SS ---\n{content}").map_err(|e| e.to_string())?;
        fs::remove_file(ue4ss).map_err(|e| e.to_string())?;
    }
    let cmd = if let Some(view) = virtual_view {super::nexus_deploy_methods::wrap(cmd, &view, &dir)?} else {cmd};
    super::nexus_profile::spawn(game, cmd)
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
pub(super) fn log_path(game: &NexusGame) -> Result<PathBuf, String> {
    if definition(game).is_some_and(|module| module.domain == "palworld") {
        Ok(folder(game)?.join("Pal/Binaries/Win64/ue4ss/UE4SS.log"))
    } else { Err("Este módulo não utiliza o log UE4SS do Palworld.".into()) }
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
pub struct InstallationPlan { pub resolved_files: Vec<(String,String)>, pub activation_file: Option<String>, pub module: String, pub files: Vec<(String,String)>, pub missing: Vec<FrameworkStatus>, pub notes: Vec<String>, pub fomod: Option<super::nexus_fomod::Installer>, pub selection: Vec<String>, pub issues: Vec<String> }
pub(super) fn plan(game: &NexusGame, mod_id: &str) -> Result<InstallationPlan,String> {
    let module = super::nexus_discovery::at(Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?)).ok_or("Módulo do jogo não identificado.")?;
    let item = game.mods.iter().find(|item| item.id == mod_id).ok_or("Mod não encontrado.")?;
    uuid::Uuid::parse_str(mod_id).map_err(|_| "Mod inválido.")?;
    let mut archive = super::nexus_archive::open(game,item)?;
    let resolution = super::nexus_fomod::resolve(&mut archive,&module,Path::new(game.game["directory"].as_str().unwrap()),mod_id,item.fomod_selection.as_deref())?;
    let entries = (0..archive.len()).map(|i| archive.by_index(i).map(|entry| entry.name().replace('\\', "/")).map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?;
    let mut files = if let Some(selected) = resolution.files {selected} else {
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
    Ok(InstallationPlan {resolved_files, activation_file, module:module.name, files, missing, notes:module.notes, fomod:resolution.installer, selection:resolution.selection, issues})
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
