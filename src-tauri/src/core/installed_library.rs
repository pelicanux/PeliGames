//! Persistent library of destinations installed through PeliGames.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
fn deserialize_proton<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    Ok(super::paths::migrated_runner_path(&value))
}
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
    #[serde(deserialize_with = "deserialize_proton")]
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
    #[serde(deserialize_with = "deserialize_proton")]
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
    #[serde(deserialize_with = "deserialize_proton")]
    pub proton: String,
    pub executable: String,
}
#[derive(Serialize, Deserialize)]
pub struct InstallationRecord {
    pub name: String,
    pub directory: String,
    pub prefix: String,
    #[serde(deserialize_with = "deserialize_proton")]
    pub proton: String,
    pub installer: String,
    pub cover_url: Option<String>,
    pub entries: Vec<InstalledEntry>,
    #[serde(default)]
    pub overrides: Vec<EntryOverride>,
}
fn library_file() -> Result<PathBuf, String> {
    Ok(super::paths::app_root()?.join("installed-destinations.json"))
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
