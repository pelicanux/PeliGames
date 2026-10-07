//! Persistent library of destinations installed through PeliGames.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
static LIBRARY_LOCK: Mutex<()> = Mutex::new(());
// The launcher and Pelinstall are separate processes sharing the same manifests.
fn process_lock(index: &Path) -> Result<fs::File, String> {
    fs::create_dir_all(index.parent().ok_or("Registro inválido.")?).map_err(|e|e.to_string())?;
    let file=fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(index.with_extension("lock")).map_err(|e|e.to_string())?;
    #[cfg(unix)] { use std::os::fd::AsRawFd; if unsafe {libc::flock(file.as_raw_fd(),libc::LOCK_EX)} != 0 { return Err(std::io::Error::last_os_error().to_string()); } }
    Ok(file)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct InstalledEntry {
    #[serde(default)]
    pub discovery_source: Option<String>,
    #[serde(default)]
    pub launch_arguments: Vec<String>,
    #[serde(default)]
    pub advanced: super::advanced_settings::AdvancedSettings,
    pub name: String,
    pub path: String,
    pub directory: String,
    pub executable: String,
    pub prefix: String,
    pub proton: String,
    pub cover_url: Option<String>,
    pub launcher: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EntryOverride {
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub advanced: Option<super::advanced_settings::AdvancedSettings>,
    #[serde(default)]
    pub prefix: Option<String>,
    pub source: String,
    pub name: String,
    pub proton: String,
    pub executable: String,
}
#[derive(Deserialize)]
pub struct EntrySettings {
    #[serde(default)]
    pub advanced: Option<super::advanced_settings::AdvancedSettings>,
    #[serde(default)]
    pub prefix: Option<String>,
    pub path: String,
    pub name: String,
    pub proton: String,
    pub executable: String,
}
#[derive(Serialize, Deserialize)]
pub struct InstallationRecord {
    pub name: String,
    pub directory: String,
    pub prefix: String,
    pub proton: String,
    pub installer: String,
    pub cover_url: Option<String>,
    pub entries: Vec<InstalledEntry>,
    #[serde(default)]
    pub overrides: Vec<EntryOverride>,
}
fn library_file() -> Result<PathBuf, String> {
    Ok(dirs::config_dir()
        .ok_or("Pasta de configuração indisponível.")?
        .join("peligames/installed-destinations.json"))
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("Destino inválido.")?).map_err(|e| e.to_string())?;
    let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(
        &temp,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(temp);
        return Err(error.to_string());
    }
    Ok(())
}
fn destinations(index: &Path) -> Result<BTreeSet<String>, String> {
    match fs::read(index) {
        Ok(data) => serde_json::from_slice(&data)
            .map_err(|e| format!("Registro de instalações inválido: {e}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(error) => Err(error.to_string()),
    }
}
fn excluded_file(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "setup",
        "unins",
        "uninstall",
        "vc_redist",
        "vcredist",
        "dxsetup",
        "crashreport",
        "unitycrashhandler",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}
fn walk(
    root: &Path,
    installer: &Path,
    files: &mut BTreeSet<PathBuf>,
    depth: usize,
    visited: &mut usize,
) -> Result<(), String> {
    if depth > 24 {
        return Err("A instalação excede o limite de profundidade da varredura.".into());
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("Não foi possível examinar {}: {e}", root.display())),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        *visited += 1;
        if *visited > 100_000 {
            return Err("A instalação excede o limite de arquivos da varredura.".into());
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        // Never follow prefix links to home, mounts or another installation.
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if [
                ".peligames",
                "prefix",
                "logs",
                "windows",
                "$recycle.bin",
                "system volume information",
                "_commonredist",
                "redist",
                "redistributables",
            ]
            .contains(&name.as_str())
            {
                continue;
            }
            walk(&path, installer, files, depth + 1, visited)?;
        } else if kind.is_file() && name.ends_with(".exe") && !excluded_file(&name) {
            let canonical = path.canonicalize().map_err(|e| e.to_string())?;
            if canonical != installer {
                files.insert(canonical);
            }
        }
    }
    Ok(())
}
fn scan(record: &mut InstallationRecord) -> Result<(), String> {
    // Explicit registrations have independent identities, even when they share
    // an executable and prefix. Only discovered executables are deduplicated.
    let added: Vec<_> = record.entries.iter()
        .filter(|entry| entry.path.starts_with("peligames:") && Path::new(&entry.executable).is_file())
        .cloned().collect();
    let mut files = BTreeSet::new();
    let mut visited = 0;
    let installer = PathBuf::from(&record.installer);
    // Added entries have no installer: rescan only their explicitly chosen executable.
    if !record.installer.is_empty() {
        walk(
            Path::new(&record.directory),
            &installer,
            &mut files,
            0,
            &mut visited,
        )?;
        walk(
            &Path::new(&record.prefix).join("drive_c"),
            &installer,
            &mut files,
            0,
            &mut visited,
        )?;
    }
    for item in &record.overrides {
        if !item.source.starts_with("peligames:") && Path::new(&item.executable).is_file() {
            files.insert(PathBuf::from(&item.source));
        }
    }
    files.retain(|file| {
        if record
            .overrides
            .iter()
            .any(|item| item.executable.is_empty() && Path::new(&item.source) == file)
        {
            return false;
        }
        !record.overrides.iter().any(|item| {
            item.executable != item.source
                && Path::new(&item.executable) == file
                && !record
                    .entries
                    .iter()
                    .any(|entry| Path::new(&entry.path) == file)
        })
    });
    let hints = if record.installer.is_empty() { Default::default() } else { super::installation_targets::discover(Path::new(&record.prefix)) };
    let multiple = files.len() > 1;
    record.entries = files
        .into_iter()
        .map(|file| {
            let executable = file.to_string_lossy().into_owned();
            let stem = file.file_stem().unwrap_or_default().to_string_lossy();
            let custom = record
                .overrides
                .iter()
                .find(|item| item.source == executable);
            let target = custom
                .map(|item| Path::new(&item.executable))
                .unwrap_or(&file);
            let hint = hints.get(target).filter(|_| custom.and_then(|c| c.prefix.as_deref()).is_none_or(|p| p == record.prefix));
            InstalledEntry {
                discovery_source: hint.map(|h| h.source.clone()),
                launch_arguments: hint.map(|h| h.arguments.clone()).unwrap_or_default(),
                advanced: custom
                    .and_then(|item| item.advanced.clone())
                    .unwrap_or_default(),
                name: custom.map(|item| item.name.clone()).unwrap_or_else(|| {
                    if multiple {
                        format!("{} · {stem}", record.name)
                    } else {
                        record.name.clone()
                    }
                }),
                path: executable.clone(),
                directory: target
                    .parent()
                    .unwrap_or(Path::new(&record.directory))
                    .to_string_lossy()
                    .into_owned(),
                executable: target.to_string_lossy().into_owned(),
                prefix: custom
                    .and_then(|item| item.prefix.clone())
                    .unwrap_or_else(|| record.prefix.clone()),
                proton: custom
                    .map(|item| item.proton.clone())
                    .unwrap_or_else(|| record.proton.clone()),
                cover_url: custom.and_then(|item| item.cover_url.clone()).or_else(|| record.cover_url.clone()),
                launcher: "PeliGames".into(),
            }
        })
        .collect();
    record.entries.sort_by_key(|entry| match entry.discovery_source.as_deref() { Some("Atalho do Windows") => 0, Some("Registro: App Paths") => 1, Some("Registro: launcher") => 2, Some(_) => 3, None => 4 });
    record.entries.extend(added);
    Ok(())
}
pub fn register(mut record: InstallationRecord) -> Result<usize, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let index = library_file()?;
    let _process_guard = process_lock(&index)?;
    let manifest = Path::new(&record.directory).join("installation.json");
    if manifest.is_file() {
        let previous: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        record.entries = previous.entries;
        record.overrides = previous.overrides;
    }
    scan(&mut record)?;
    let count = record.entries.len();
    write_json(
        &Path::new(&record.directory).join("installation.json"),
        &record,
    )?;
    let mut roots = destinations(&index)?;
    roots.insert(record.directory.clone());
    write_json(&index, &roots)?;
    Ok(count)
}
pub fn list(rescan: bool) -> Result<Vec<InstalledEntry>, String> {
    list_at(&library_file()?, rescan)
}
fn list_at(index: &Path, rescan: bool) -> Result<Vec<InstalledEntry>, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for directory in destinations(index)? {
        super::installation_layout::migrate(Path::new(&directory))?;
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.exists() {
            continue;
        }
        let mut record: InstallationRecord =
            serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| format!("Registro inválido em {directory}: {e}"))?;
        let old_prefix = Path::new(&directory).join(".peligames/prefix");
        let moved = Path::new(&record.prefix) == old_prefix;
        if moved {
            let prefix = Path::new(&directory).join("prefix");
            if let Ok(relative) = Path::new(&record.installer).strip_prefix(&old_prefix) {
                record.installer = prefix.join(relative).to_string_lossy().into_owned();
            }
            record.prefix = prefix.to_string_lossy().into_owned();
        }
        if rescan || moved {
            scan(&mut record)?;
            write_json(&manifest, &record)?;
        }
        entries.extend(record.entries.into_iter().filter(|entry| {
            Path::new(&entry.executable).is_file() && seen.insert(entry.path.clone())
        }));
    }
    Ok(entries)
}

#[derive(Clone, Serialize)]
pub struct ExecutableMatch {
    pub shortcuts: Option<Vec<String>>,
    pub entry: InstalledEntry,
    pub destination: String,
    pub installer: Option<String>,
}
pub fn find_executable(executable: &str) -> Result<Vec<ExecutableMatch>, String> {
    find_executable_at(&library_file()?, executable)
}
fn find_executable_at(index: &Path, executable: &str) -> Result<Vec<ExecutableMatch>, String> {
    let selected = super::game_installation::validate_executable(executable)?;
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let mut matches = vec![];
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.is_file() { continue; }
        let record: InstallationRecord = serde_json::from_slice(&fs::read(manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let installer = Path::new(&record.installer).canonicalize().ok().filter(|path| path.is_file());
        for entry in record.entries {
            if !Path::new(&entry.executable).is_file() { continue; }
            if Path::new(&entry.executable).canonicalize().ok().as_ref() == Some(&selected) || installer.as_ref() == Some(&selected) {
                matches.push(ExecutableMatch { shortcuts: super::pelinstall::shortcut_paths(&entry.path).ok(), entry, destination: directory.clone(), installer: installer.as_ref().map(|path| path.to_string_lossy().into_owned()) });
            }
        }
    }
    Ok(matches)
}

pub(crate) fn validate_selected_prefix(path: &str) -> Result<PathBuf, String> {
    let path = super::game_installation::absolute(path)?;
    let metadata = fs::symlink_metadata(&path).map_err(|e| format!("Prefixo indisponível: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Escolha uma pasta real de prefixo Wine/Proton.".into());
    }
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if canonical.parent().is_none()
        || !fs::symlink_metadata(canonical.join("drive_c"))
            .is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
        || !fs::symlink_metadata(canonical.join("system.reg"))
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
    {
        return Err("Selecione o prefixo Wine/Proton que contém drive_c e system.reg.".into());
    }
    if super::game_execution::prefix_in_use(&canonical) {
        return Err("Encerre os programas deste prefixo antes de alterá-lo.".into());
    }
    Ok(canonical)
}

pub fn set_cover(path: &str, cover: &str) -> Result<(), String> { set_cover_at(&library_file()?,path,cover) }
fn set_cover_at(index: &Path, path: &str, cover: &str) -> Result<(), String> {
    if !cover.starts_with("https://") || cover.len() > 8192 || cover.contains(['\n','\r','\0']) { return Err("Capa inválida.".into()); }
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?; let _process_guard = process_lock(index)?;
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json"); if !manifest.is_file() { continue; }
        let mut record: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if let Some(entry) = record.entries.iter_mut().find(|entry| entry.path == path) {
            entry.cover_url = Some(cover.into());
            if let Some(custom) = record.overrides.iter_mut().find(|item| item.source == path) { custom.cover_url = Some(cover.into()); }
            else { record.overrides.push(EntryOverride { source: entry.path.clone(), name: entry.name.clone(), proton: entry.proton.clone(), executable: entry.executable.clone(), prefix: Some(entry.prefix.clone()), advanced: Some(entry.advanced.clone()), cover_url: Some(cover.into()) }); }
            return write_json(&manifest,&record);
        }
    }
    Err("Entrada não registrada na biblioteca PeliGames.".into())
}
pub fn update_settings(settings: EntrySettings) -> Result<InstalledEntry, String> {
    update_at(&library_file()?, settings)
}
fn update_at(index: &Path, settings: EntrySettings) -> Result<InstalledEntry, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let name = settings.name.trim();
    if name.is_empty() {
        return Err("Informe o nome do jogo ou programa.".into());
    }
    let executable = super::game_installation::validate_executable(&settings.executable)?;
    let proton = super::game_installation::validate_proton(&settings.proton)?;
    for directory in destinations(index)? {
        let manifest = Path::new(&directory).join("installation.json");
        if !manifest.is_file() {
            continue;
        }
        let mut record: InstallationRecord =
            serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if let Some(entry) = record
            .entries
            .iter_mut()
            .find(|entry| entry.path == settings.path)
        {
            if let Some(prefix) = settings.prefix.as_deref() {
                if prefix != entry.prefix {
                    entry.prefix = validate_selected_prefix(prefix)?
                        .to_string_lossy()
                        .into_owned();
                }
            }
            if let Some(advanced) = &settings.advanced {
                advanced.validate_runner(&proton)?;
                entry.advanced = advanced.clone();
            }
            entry.name = name.into();
            entry.proton = proton.to_string_lossy().into_owned();
            if Path::new(&entry.executable) != executable { entry.launch_arguments.clear(); entry.discovery_source = None; }
            entry.executable = executable.to_string_lossy().into_owned();
            entry.directory = executable
                .parent()
                .ok_or("Diretório inválido.")?
                .to_string_lossy()
                .into_owned();
            let updated = entry.clone();
            record.overrides.retain(|item| item.source != settings.path);
            record.overrides.push(EntryOverride {
                cover_url: updated.cover_url.clone(),
                advanced: Some(updated.advanced.clone()),
                prefix: Some(updated.prefix.clone()),
                source: settings.path,
                name: updated.name.clone(),
                proton: updated.proton.clone(),
                executable: updated.executable.clone(),
            });
            write_json(&manifest, &record)?;
            return Ok(updated);
        }
    }
    Err("Entrada não registrada na biblioteca PeliGames.".into())
}

/// Register an existing executable without running an installer or moving its files.
pub fn add(
    request: super::game_installation::InstallationRequest,
) -> Result<InstalledEntry, String> {
    add_at(&library_file()?, request)
}
fn add_at(
    index: &Path,
    request: super::game_installation::InstallationRequest,
) -> Result<InstalledEntry, String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err("Confirme o nome do jogo ou programa.".into());
    }
    let executable = super::game_installation::validate_executable(&request.executable)?;
    let proton = super::game_installation::validate_proton(&request.proton)?;
    let directory = super::game_installation::absolute(&request.directory)?;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    if directory.parent().is_none() {
        return Err("Escolha uma pasta dedicada ao jogo.".into());
    }
    let prefix = directory.join("prefix");
    if fs::symlink_metadata(&prefix).is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()) {
        return Err("O prefixo deve ser uma pasta real dentro do destino.".into());
    }
    fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;
    let entry = InstalledEntry {
        discovery_source: None,
        launch_arguments: Vec::new(),
        advanced: Default::default(),
        name: name.into(),
        path: format!("peligames:{}", uuid::Uuid::new_v4()),
        directory: executable
            .parent()
            .ok_or("Executável sem diretório.")?
            .to_string_lossy()
            .into_owned(),
        executable: executable.to_string_lossy().into_owned(),
        prefix: prefix.to_string_lossy().into_owned(),
        proton: proton.to_string_lossy().into_owned(),
        cover_url: request.cover_url,
        launcher: "PeliGames".into(),
    };
    let manifest = directory.join("installation.json");
    let mut record = if manifest.is_file() {
        let existing: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Registro de instalação inválido: {e}"))?;
        if Path::new(&existing.directory) != directory {
            return Err("O destino registrado foi redirecionado.".into());
        }
        existing
    } else {
        InstallationRecord {
            name: entry.name.clone(), directory: directory.to_string_lossy().into_owned(),
            prefix: entry.prefix.clone(), proton: entry.proton.clone(), installer: String::new(),
            cover_url: entry.cover_url.clone(), entries: vec![], overrides: vec![],
        }
    };
    record.entries.push(entry.clone());
    record.overrides.push(EntryOverride {
        cover_url: entry.cover_url.clone(),
        advanced: None, prefix: None, source: entry.path.clone(), name: entry.name.clone(),
        proton: entry.proton.clone(), executable: entry.executable.clone(),
    });
    write_json(&directory.join("installation.json"), &record)?;
    let mut roots = destinations(index)?;
    roots.insert(record.directory);
    write_json(index, &roots)?;
    Ok(entry)
}
/// The UI supplies only a registered entry identity, never a directory to erase.
pub fn uninstall(path: String) -> Result<(), String> {
    uninstall_at(&library_file()?, &path)
}
fn uninstall_at(index: &Path, path: &str) -> Result<(), String> {
    uninstall_with_shortcuts_at(index, path, &super::pelinstall::shortcut_directories())
}
fn uninstall_with_shortcuts_at(index: &Path, path: &str, shortcut_directories: &[PathBuf]) -> Result<(), String> {
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process_guard = process_lock(index)?;
    let mut roots = destinations(index)?;
    for directory in roots.clone() {
        let root = Path::new(&directory);
        let manifest = root.join("installation.json");
        if !manifest.exists() {
            continue;
        }
        let record: InstallationRecord =
            serde_json::from_slice(&fs::read(&manifest).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if directory != path && !record.entries.iter().any(|entry| entry.path == path) {
            continue;
        }
        let canonical = root.canonicalize().map_err(|e| e.to_string())?;
        if Path::new(&record.directory) != canonical {
            return Err("O destino registrado foi redirecionado.".into());
        }
        let prefix = PathBuf::from(
            record
                .entries
                .iter()
                .find(|entry| entry.path == path)
                .map(|entry| entry.prefix.as_str())
                .unwrap_or(&record.prefix),
        );
        let mut affected = Vec::new();
        let mut shortcut_entries = BTreeSet::new();
        let mut managed = false;
        for destination in &roots {
            let other_root = Path::new(destination);
            let other_manifest = other_root.join("installation.json");
            if !other_manifest.is_file() {
                continue;
            }
            let mut other: InstallationRecord =
                serde_json::from_slice(&fs::read(&other_manifest).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let same_prefix = |entry: &InstalledEntry| Path::new(&entry.prefix) == prefix;
            let any = other.entries.iter().any(same_prefix);
            if any || (destination == &directory && other.entries.is_empty()) {
                let other_canonical = other_root.canonicalize().map_err(|e| e.to_string())?;
                if Path::new(&other.directory) != other_canonical {
                    return Err("Destino registrado inválido.".into());
                }
                managed |= prefix == other_canonical.join("prefix");
                let removed: BTreeSet<String> = other
                    .entries
                    .iter()
                    .filter(|entry| same_prefix(entry))
                    .map(|entry| entry.path.clone())
                    .collect();
                shortcut_entries.extend(removed.iter().cloned());
                other.entries.retain(|entry| !same_prefix(entry));
                other
                    .overrides
                    .retain(|item| !removed.contains(&item.source));
                for source in removed {
                    other.overrides.push(EntryOverride {
                        cover_url: None,
                        advanced: None,
                        source,
                        name: String::new(),
                        proton: String::new(),
                        executable: String::new(),
                        prefix: Some(prefix.to_string_lossy().into_owned()),
                    });
                }
                affected.push((destination.clone(), other_manifest, other));
            }
        }
        match fs::symlink_metadata(&prefix) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("Não é permitido remover um prefixo redirecionado.".into());
                }
                if !managed
                    && validate_selected_prefix(
                        &record
                            .entries
                            .iter()
                            .find(|entry| entry.path == path)
                            .map(|entry| entry.prefix.clone())
                            .unwrap_or(record.prefix.clone()),
                    )? != prefix
                {
                    return Err("Prefixo registrado inválido.".into());
                }
                if super::game_execution::prefix_in_use(&prefix) {
                    return Err("Encerre os programas deste prefixo antes de desinstalar.".into());
                }
                fs::remove_dir_all(&prefix)
                    .map_err(|e| format!("Não foi possível remover o prefixo: {e}"))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        for entry in shortcut_entries {
            super::pelinstall::remove_shortcuts_at(shortcut_directories, &entry)
                .map_err(|error| format!("O prefixo foi removido, mas não foi possível remover todos os atalhos. Tente desinstalar novamente: {error}"))?;
        }
        for (destination, manifest, remaining) in affected {
            if remaining.entries.is_empty() {
                fs::remove_file(manifest).map_err(|e| e.to_string())?;
                roots.remove(&destination);
            } else {
                write_json(&manifest, &remaining)?;
            }
        }
        write_json(index, &roots)?;
        return Ok(());
    }
    Err("Entrada não registrada na biblioteca PeliGames.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn added_fixture() -> (
        PathBuf,
        PathBuf,
        super::super::game_installation::InstallationRequest,
    ) {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("peligames-added-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("runner")).unwrap();
        let runner = root.join("runner/proton");
        fs::write(&runner, "#!/bin/sh\nexit 0").unwrap();
        fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
        let exe = root.join("External.exe");
        fs::write(&exe, b"MZ test").unwrap();
        let request = super::super::game_installation::InstallationRequest {
            name: "Existing game".into(),
            directory: root.join("Game").to_string_lossy().into_owned(),
            executable: exe.to_string_lossy().into_owned(),
            proton: root.join("runner").to_string_lossy().into_owned(),
            cover_url: Some("cover".into()),
        };
        let index = root.join("config/index.json");
        (root, index, request)
    }
    #[test]
    fn per_entry_covers_survive_rescans_and_settings_without_changing_duplicates() {
        let (root,index,request) = added_fixture();
        let first = add_at(&index,request.clone()).unwrap(); let second = add_at(&index,request).unwrap();
        let manifest = root.join("Game/installation.json");
        let mut record: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        record.entries[0].path = first.executable.clone(); record.overrides[0].source = first.executable.clone(); write_json(&manifest,&record).unwrap();
        let cover = "https://example.com/game-cover.png";
        set_cover_at(&index,&first.executable,cover).unwrap();
        let listed = list_at(&index,true).unwrap();
        assert_eq!(listed.iter().find(|entry|entry.path == first.executable).unwrap().cover_url.as_deref(),Some(cover));
        assert_eq!(listed.iter().find(|entry|entry.path == second.path).unwrap().cover_url,second.cover_url);
        let updated = update_at(&index,EntrySettings { path: first.executable.clone(),name:"Renamed".into(),proton:first.proton,executable:first.executable.clone(),prefix:None,advanced:None }).unwrap();
        assert_eq!(updated.cover_url.as_deref(),Some(cover));
        assert_eq!(list_at(&index,true).unwrap().iter().find(|entry|entry.path == first.executable).unwrap().cover_url.as_deref(),Some(cover));
        assert!(set_cover_at(&index,"unknown",cover).is_err()); assert!(set_cover_at(&index,&second.path,"file:///private").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn advanced_options_survive_rescanning_and_other_field_updates() {
        let (root, index, request) = added_fixture();
        let entry = add_at(&index, request).unwrap();
        let mut advanced = entry.advanced.clone();
        advanced.options.insert("wayland".into(), false);
        advanced.dll_overrides = "dxgi=n,b;dinput8=n,b".into();
        update_at(
            &index,
            EntrySettings {
                advanced: Some(advanced),
                prefix: None,
                path: entry.path.clone(),
                name: entry.name.clone(),
                proton: entry.proton.clone(),
                executable: entry.executable.clone(),
            },
        )
        .unwrap();
        update_at(
            &index,
            EntrySettings {
                advanced: None,
                prefix: None,
                path: entry.path.clone(),
                name: "Renamed".into(),
                proton: entry.proton.clone(),
                executable: entry.executable.clone(),
            },
        )
        .unwrap();
        let loaded = list_at(&index, true).unwrap();
        assert_eq!(loaded[0].advanced.options["wayland"], false);
        assert_eq!(loaded[0].advanced.dll_overrides, "dxgi=n,b;dinput8=n,b");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn changing_prefix_persists_without_moving_files_and_uninstall_targets_new_prefix() {
        let (root, index, request) = added_fixture();
        let entry = add_at(&index, request).unwrap();
        let selected = root.join("OtherPrefix");
        fs::create_dir_all(selected.join("drive_c")).unwrap();
        fs::write(selected.join("system.reg"), "registry").unwrap();
        fs::write(Path::new(&entry.prefix).join("keep.txt"), "old prefix").unwrap();
        let settings = |prefix: &Path| EntrySettings {
            advanced: None,
            path: entry.path.clone(),
            name: entry.name.clone(),
            executable: entry.executable.clone(),
            proton: entry.proton.clone(),
            prefix: Some(prefix.to_string_lossy().into_owned()),
        };
        let before = fs::read(root.join("Game/installation.json")).unwrap();
        assert!(update_at(&index, settings(&root)).is_err());
        assert_eq!(
            fs::read(root.join("Game/installation.json")).unwrap(),
            before
        );
        let updated = update_at(&index, settings(&selected)).unwrap();
        assert_eq!(updated.prefix, selected.to_string_lossy());
        assert_eq!(list_at(&index, true).unwrap()[0].prefix, updated.prefix);
        assert!(Path::new(&entry.prefix).join("keep.txt").is_file());
        assert!(Path::new(&entry.executable).is_file());
        uninstall_at(&index, &entry.path).unwrap();
        assert!(!selected.exists());
        assert!(Path::new(&entry.prefix).join("keep.txt").is_file());
        assert!(Path::new(&entry.executable).is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn shared_selected_prefix_removes_all_users_but_keeps_other_prefixes() {
        let (root, index, request) = added_fixture();
        let first = add_at(&index, request.clone()).unwrap();
        let mut second_request = request;
        second_request.directory = root.join("Second").to_string_lossy().into_owned();
        second_request.executable = root.join("Second.exe").to_string_lossy().into_owned();
        fs::write(&second_request.executable, b"MZ second").unwrap();
        let second = add_at(&index, second_request).unwrap();
        let selected = root.join("SharedPrefix");
        fs::create_dir_all(selected.join("drive_c")).unwrap();
        fs::write(selected.join("system.reg"), "registry").unwrap();
        for entry in [&first, &second] {
            update_at(
                &index,
                EntrySettings {
                    advanced: None,
                    path: entry.path.clone(),
                    name: entry.name.clone(),
                    executable: entry.executable.clone(),
                    proton: entry.proton.clone(),
                    prefix: Some(selected.to_string_lossy().into_owned()),
                },
            )
            .unwrap();
        }
        uninstall_at(&index, &first.path).unwrap();
        assert!(list_at(&index, true).unwrap().is_empty());
        assert!(!selected.exists());
        assert!(Path::new(&first.prefix).is_dir());
        assert!(Path::new(&second.prefix).is_dir());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn added_entry_survives_rescan_and_uninstall_only_erases_its_prefix() {
        let (root, index, request) = added_fixture();
        let entry = add_at(&index, request.clone()).unwrap();
        assert!(Path::new(&entry.prefix).is_dir());
        let duplicate = add_at(&index, request).unwrap();
        assert_ne!(duplicate.path, entry.path);
        fs::create_dir_all(root.join("Game/logs")).unwrap();
        fs::write(root.join("Game/logs/keep.log"), "log").unwrap();
        fs::write(root.join("Game/Other.exe"), b"MZ other").unwrap();
        fs::write(Path::new(&entry.prefix).join("save.dat"), "save").unwrap();
        let entries = list_at(&index, true).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|item| item.path == entry.path));
        uninstall_at(&index, &entry.path).unwrap();
        assert!(!Path::new(&entry.prefix).exists());
        assert!(root.join("External.exe").exists());
        assert!(root.join("Game/Other.exe").exists());
        assert!(root.join("Game/logs/keep.log").exists());
        assert!(list_at(&index, true).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn uninstall_removes_shortcuts_of_all_shared_prefix_entries_and_preserves_other_games() {
        let (root,index,request) = added_fixture();
        let first = add_at(&index,request.clone()).unwrap(); let second = add_at(&index,request.clone()).unwrap();
        let mut other_request = request; other_request.directory = root.join("Other").to_string_lossy().into_owned();
        let other = add_at(&index,other_request).unwrap();
        let desktop = root.join("Desktop"); let apps = root.join("applications");
        fs::create_dir_all(&desktop).unwrap(); fs::create_dir_all(&apps).unwrap();
        let make = |directory: &Path, file: &str, entry: &InstalledEntry| {
            let path = directory.join(file);
            fs::write(&path,format!("[Desktop Entry]\nComment=Executar com PeliGames\nExec=\"/tmp/PeliGames\" --launch-game \"{}\"\n",entry.path)).unwrap(); path
        };
        let first_shortcut = make(&desktop,"first.desktop",&first);
        let copy = make(&apps,"copy.desktop",&first);
        let second_shortcut = make(&apps,"second.desktop",&second);
        let preserved = make(&desktop,"other.desktop",&other);
        // Failed validation must not remove any shortcut.
        fs::remove_dir_all(&first.prefix).unwrap();
        std::os::unix::fs::symlink(&other.prefix,&first.prefix).unwrap();
        assert!(uninstall_with_shortcuts_at(&index,&first.path,&[desktop.clone(),apps.clone()]).is_err());
        assert!(first_shortcut.exists() && copy.exists() && second_shortcut.exists());
        fs::remove_file(&first.prefix).unwrap(); fs::create_dir_all(&first.prefix).unwrap();
        uninstall_with_shortcuts_at(&index,&first.path,&[desktop,apps]).unwrap();
        assert!(!first_shortcut.exists() && !copy.exists() && !second_shortcut.exists());
        assert!(preserved.exists() && Path::new(&other.prefix).exists());
        assert!(Path::new(&first.executable).exists());
        let remaining = list_at(&index,false).unwrap(); assert_eq!(remaining.len(),1); assert_eq!(remaining[0].path,other.path);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn duplicate_registrations_keep_independent_settings_and_survive_rescans() {
        let (root, index, request) = added_fixture();
        let first = add_at(&index, request.clone()).unwrap();
        let second = add_at(&index, request.clone()).unwrap();
        let mut elsewhere = request;
        elsewhere.directory = root.join("Another registration").to_string_lossy().into_owned();
        let third = add_at(&index, elsewhere).unwrap();
        assert_ne!(first.path, second.path);
        assert_ne!(second.path, third.path);
        assert_eq!(first.executable, second.executable);
        assert_eq!(first.prefix, second.prefix);
        assert_ne!(first.prefix, third.prefix);
        let mut advanced = second.advanced.clone();
        advanced.dll_overrides = "dinput8=n,b".into();
        update_at(&index, EntrySettings {
            path: second.path.clone(), name: "My other configuration".into(),
            executable: second.executable.clone(), proton: second.proton.clone(),
            prefix: None, advanced: Some(advanced),
        }).unwrap();
        let entries = list_at(&index, true).unwrap();
        assert_eq!(entries.len(), 3);
        let original = entries.iter().find(|entry| entry.path == first.path).unwrap();
        assert_eq!(original.name, first.name);
        assert_eq!(original.advanced.dll_overrides, first.advanced.dll_overrides);
        let changed = entries.iter().find(|entry| entry.path == second.path).unwrap();
        assert_eq!(changed.name, "My other configuration");
        assert_eq!(changed.advanced.dll_overrides, "dinput8=n,b");
        assert_eq!(list_at(&index, true).unwrap().len(), 3);
        // Uninstall still removes all entries sharing the erased prefix.
        uninstall_at(&index, &first.path).unwrap();
        let remaining = list_at(&index, true).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].path, third.path);
        assert!(Path::new(&third.prefix).is_dir());
        assert!(Path::new(&third.executable).is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn adding_to_an_installed_or_legacy_destination_preserves_existing_entries() {
        let (root, index, request) = added_fixture();
        let first = add_at(&index, request.clone()).unwrap();
        let manifest = root.join("Game/installation.json");
        let mut record: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        // Previous versions used the executable path as identity.
        record.entries[0].path = first.executable.clone();
        record.overrides[0].source = first.executable.clone();
        record.installer = root.join("setup.exe").to_string_lossy().into_owned();
        write_json(&manifest, &record).unwrap();
        let duplicate = add_at(&index, request).unwrap();
        let entries = list_at(&index, true).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|entry| entry.path == first.executable));
        assert!(entries.iter().any(|entry| entry.path == duplicate.path));
        let record: InstallationRecord = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        assert!(!record.installer.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn uninstall_rejects_unregistered_and_redirected_prefixes() {
        let (root, index, request) = added_fixture();
        let entry = add_at(&index, request).unwrap();
        assert!(uninstall_at(&index, "/unregistered").is_err());
        fs::remove_dir_all(&entry.prefix).unwrap();
        let keep = root.join("keep");
        fs::create_dir_all(&keep).unwrap();
        fs::write(keep.join("save"), "keep").unwrap();
        std::os::unix::fs::symlink(&keep, &entry.prefix).unwrap();
        assert!(uninstall_at(&index, &entry.path).is_err());
        assert!(keep.join("save").exists());
        assert!(root.join("Game/installation.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn edited_settings_survive_rescan_without_changing_prefix_or_other_entries() {
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("peligames-settings-test-{}", uuid::Uuid::new_v4()));
        let prefix = root.join("prefix");
        let program = prefix.join("drive_c/Game");
        fs::create_dir_all(&program).unwrap();
        let original = program.join("Game.exe");
        let sibling = program.join("Editor.exe");
        fs::write(&original, "MZ").unwrap();
        fs::write(&sibling, "MZ").unwrap();
        let runner = root.join("runner");
        fs::create_dir_all(&runner).unwrap();
        fs::write(runner.join("proton"), "runner").unwrap();
        fs::set_permissions(runner.join("proton"), fs::Permissions::from_mode(0o755)).unwrap();
        let mut record = InstallationRecord {
            name: "Original".into(),
            directory: root.display().to_string(),
            prefix: prefix.display().to_string(),
            proton: "/previous-runner".into(),
            installer: "/setup.exe".into(),
            cover_url: Some("cover".into()),
            entries: vec![],
            overrides: vec![],
        };
        scan(&mut record).unwrap();
        let index = root.join("library.json");
        write_json(&index, &BTreeSet::from([record.directory.clone()])).unwrap();
        write_json(&root.join("installation.json"), &record).unwrap();
        let target = program.join("Changed.exe");
        fs::write(&target, "MZ").unwrap();
        let updated = update_at(
            &index,
            EntrySettings {
                advanced: None,
                prefix: None,
                path: original.display().to_string(),
                name: "Custom title".into(),
                proton: runner.display().to_string(),
                executable: target.display().to_string(),
            },
        )
        .unwrap();
        assert_eq!(updated.path, original.display().to_string());
        assert_eq!(updated.prefix, record.prefix);
        assert_eq!(updated.cover_url, record.cover_url);
        fs::remove_file(&original).unwrap();
        let entries = list_at(&index, true).unwrap();
        assert_eq!(entries.len(), 2);
        let own = entries
            .iter()
            .find(|entry| entry.path == updated.path)
            .unwrap();
        assert_eq!(own.name, "Custom title");
        assert_eq!(own.executable, target.display().to_string());
        assert_eq!(own.proton, runner.display().to_string());
        assert_eq!(
            entries
                .iter()
                .find(|entry| entry.path == sibling.display().to_string())
                .unwrap()
                .proton,
            "/previous-runner"
        );
        let before = fs::read(root.join("installation.json")).unwrap();
        assert!(update_at(
            &index,
            EntrySettings {
                advanced: None,
                prefix: None,
                path: own.path.clone(),
                name: "Fail".into(),
                proton: "/missing".into(),
                executable: target.display().to_string()
            }
        )
        .is_err());
        assert_eq!(fs::read(root.join("installation.json")).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn restores_old_installation_with_visible_paths() {
        let root =
            std::env::temp_dir().join(format!("peligames-migration-test-{}", uuid::Uuid::new_v4()));
        let prefix = root.join(".peligames/prefix");
        let program = prefix.join("drive_c/Game/Game.exe");
        fs::create_dir_all(program.parent().unwrap()).unwrap();
        fs::create_dir_all(root.join(".peligames/logs")).unwrap();
        fs::write(&program, "MZ").unwrap();
        fs::write(root.join(".peligames/logs/previous.log"), "saved log").unwrap();
        let mut record = InstallationRecord {
            name: "Meu Jogo".into(),
            directory: root.display().to_string(),
            prefix: prefix.display().to_string(),
            proton: "/runner".into(),
            installer: prefix.join("drive_c/setup.exe").display().to_string(),
            cover_url: Some("https://example.com/cover.png".into()),
            entries: vec![],
            overrides: vec![],
        };
        scan(&mut record).unwrap();
        let index = root.join("library.json");
        write_json(&index, &BTreeSet::from([record.directory.clone()])).unwrap();
        write_json(&root.join(".peligames/installation.json"), &record).unwrap();
        let entries = list_at(&index, false).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].prefix, root.join("prefix").display().to_string());
        assert_eq!(
            entries[0].executable,
            root.join("prefix/drive_c/Game/Game.exe")
                .display()
                .to_string()
        );
        assert_eq!(entries[0].cover_url, record.cover_url);
        assert_eq!(entries[0].proton, record.proton);
        let saved: InstallationRecord =
            serde_json::from_slice(&fs::read(root.join("installation.json")).unwrap()).unwrap();
        assert_eq!(
            saved.installer,
            root.join("prefix/drive_c/setup.exe").display().to_string()
        );
        assert!(root.join("logs/previous.log").is_file());
        assert!(!root.join(".peligames").exists());
        assert_eq!(list_at(&index, false).unwrap().len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scan_excludes_system_installers_and_home_links_and_restores_library() {
        let root =
            std::env::temp_dir().join(format!("peligames-library-test-{}", uuid::Uuid::new_v4()));
        let prefix = root.join("Game/prefix");
        let program = prefix.join("drive_c/Program Files/Game");
        fs::create_dir_all(&program).unwrap();
        fs::write(program.join("Game.exe"), "MZ").unwrap();
        fs::write(program.join("Editor.EXE"), "MZ").unwrap();
        fs::write(program.join("unins000.exe"), "MZ").unwrap();
        let windows = prefix.join("drive_c/windows/system32");
        fs::create_dir_all(&windows).unwrap();
        fs::write(windows.join("notepad.exe"), "MZ").unwrap();
        let installer = root.join("Game/Installer.exe");
        fs::write(&installer, "MZ").unwrap();
        let other = root.join("Other");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("Other.exe"), "MZ").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&other, program.join("outside")).unwrap();
        let mut record = InstallationRecord {
            name: "Meu Jogo".into(),
            directory: root.join("Game").display().to_string(),
            prefix: prefix.display().to_string(),
            proton: "/runner".into(),
            installer: installer.display().to_string(),
            cover_url: Some("https://example.com/cover.png".into()),
            entries: vec![],
            overrides: vec![],
        };
        scan(&mut record).unwrap();
        assert_eq!(record.entries.len(), 2);
        assert!(record
            .entries
            .iter()
            .all(|e| e.launcher == "PeliGames" && e.cover_url == record.cover_url));
        let index = root.join("library.json");
        write_json(&index, &BTreeSet::from([record.directory.clone()])).unwrap();
        write_json(&root.join("Game/installation.json"), &record).unwrap();
        assert_eq!(list_at(&index, false).unwrap().len(), 2);
        fs::remove_file(program.join("Editor.EXE")).unwrap();
        assert_eq!(list_at(&index, false).unwrap().len(), 1);
        fs::write(program.join("New.exe"), "MZ").unwrap();
        assert_eq!(list_at(&index, true).unwrap().len(), 2);
        fs::remove_file(program.join("New.exe")).unwrap();
        fs::remove_file(program.join("Game.exe")).unwrap();
        assert!(list_at(&index, true).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
