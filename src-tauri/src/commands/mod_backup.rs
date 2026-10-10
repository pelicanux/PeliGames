//! Portable copies of mod data. Account keys and browser profiles never enter this format.
use super::nexus_local::{self, NexusGame, Workspace};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

static OPERATIONS: Mutex<()> = Mutex::new(());
static PENDING: Mutex<Option<Vec<NexusGame>>> = Mutex::new(None);
static WORKER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[derive(Default, Serialize, Deserialize)]
pub struct BackupSettings {
    pub per_game: bool,
    #[serde(default)]
    pub game_overrides: BTreeMap<String, bool>,
    #[serde(default)]
    pub last_error: String,
    #[serde(default, skip_deserializing)]
    pub synchronizing: bool,
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    format: u32,
    game: NexusGame,
    storage: String,
    files: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
struct Backup {
    format: u32,
    games: Vec<String>,
}
#[derive(Default, Serialize)]
pub struct Report {
    restored: usize,
    skipped: usize,
    warnings: Vec<String>,
    path: String,
}
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn game_enabled(settings: &BackupSettings, id: &str) -> bool {
    settings
        .game_overrides
        .get(id)
        .copied()
        .unwrap_or(settings.per_game)
}
fn has_installed_mods(game: &NexusGame) -> bool {
    game.mods.iter().any(|m| m.installed)
}
fn any_enabled(settings: &BackupSettings) -> bool {
    settings.per_game || settings.game_overrides.values().any(|enabled| *enabled)
}
fn settings_path() -> Result<PathBuf, String> {
    Ok(crate::core::paths::app_root()?.join("mod-backup.json"))
}
fn settings() -> Result<BackupSettings, String> {
    match fs::read(settings_path()?) {
        Ok(b) => {
            let mut value: BackupSettings = serde_json::from_slice(&b).map_err(error)?;
            value.synchronizing = WORKER.load(std::sync::atomic::Ordering::SeqCst);
            Ok(value)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Default::default()),
        Err(e) => Err(error(e)),
    }
}
fn json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temporary, serde_json::to_vec_pretty(value).map_err(error)?).map_err(error)?;
    fs::rename(&temporary, path).map_err(error)
}
fn directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(format!("Pasta inválida ou simbólica: {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path).map_err(error),
        Err(e) => Err(error(e)),
    }
}
fn existing_directory(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(error)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(format!("Pasta inválida no backup: {}", path.display()));
    }
    Ok(())
}
fn relative(value: &str) -> Result<&Path, String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains('\\')
        || !path.components().all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("Caminho inválido no backup.".into());
    }
    Ok(path)
}
fn hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(error)?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = file.read(&mut b).map_err(error)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn keep(entry: &walkdir::DirEntry) -> bool {
    let n = entry.file_name().to_string_lossy();
    // Never walk mounted game views, live process state, rollback transactions or logs.
    !matches!(
        n.as_ref(),
        "merged" | "work" | "session.pid" | "session.json"
    ) && !n.starts_with("rollback-")
        && !n.ends_with(".log")
}
fn copy_tree(source: &Path, target: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::new();
    directory(target)?;
    if !source.exists() {
        return Ok(files);
    }
    existing_directory(source)?;
    for entry in walkdir::WalkDir::new(source)
        .follow_links(false)
        .into_iter()
        .filter_entry(keep)
    {
        let entry = entry.map_err(error)?;
        let p = entry.path().strip_prefix(source).map_err(error)?;
        if p.as_os_str().is_empty() {
            continue;
        }
        if entry.file_type().is_symlink() {
            return Err(format!(
                "Link não portátil no armazenamento: {}",
                entry.path().display()
            ));
        }
        let dest = target.join(p);
        if entry.file_type().is_dir() {
            directory(&dest)?;
        } else if entry.file_type().is_file() {
            fs::copy(entry.path(), &dest).map_err(error)?;
            let checksum = hash(&dest)?;
            if hash(entry.path())? != checksum {
                return Err("Os dados mudaram durante a cópia; a recuperação anterior foi preservada. Tente novamente com o jogo fechado.".into());
            }
            files.insert(p.to_string_lossy().into_owned(), checksum);
        } else {
            return Err("Arquivo especial não suportado no backup.".into());
        }
    }
    Ok(files)
}
fn validate_files(root: &Path, files: &BTreeMap<String, String>) -> Result<(), String> {
    if files.len() > 200_000 {
        return Err("Backup contém arquivos demais.".into());
    }
    existing_directory(root)?;
    for (name, expected) in files {
        let rel = relative(name)?;
        let mut path = root.to_path_buf();
        for component in rel.components() {
            path.push(component.as_os_str());
            let m = fs::symlink_metadata(&path).map_err(error)?;
            if m.file_type().is_symlink() {
                return Err("Link não permitido no backup.".into());
            }
        }
        if !path.is_file() || hash(&path)? != *expected {
            return Err(format!("Backup incompleto ou alterado: {name}"));
        }
    }
    Ok(())
}
fn read_snapshot(root: &Path) -> Result<Snapshot, String> {
    existing_directory(root)?;
    let file = root.join("snapshot.json");
    if fs::symlink_metadata(&file)
        .map_err(error)?
        .file_type()
        .is_symlink()
        || fs::metadata(&file).map_err(error)?.len() > 8 * 1024 * 1024
    {
        return Err("Registro de recuperação inválido.".into());
    }
    let snapshot: Snapshot =
        serde_json::from_slice(&fs::read(file).map_err(error)?).map_err(error)?;
    if snapshot.format != 1 {
        return Err("Formato de recuperação não suportado.".into());
    }
    uuid::Uuid::parse_str(&snapshot.game.id).map_err(error)?;
    if let Some(profile) = &snapshot.game.profile {
        uuid::Uuid::parse_str(profile).map_err(error)?;
    }
    for item in &snapshot.game.mods {
        uuid::Uuid::parse_str(&item.id).map_err(error)?;
        let p = Path::new(&item.archive)
            .strip_prefix(&snapshot.storage)
            .map_err(|_| "Pacote fora do armazenamento de recuperação.")?;
        let p = p.to_str().ok_or("Nome do pacote inválido.")?;
        relative(p)?;
        if !snapshot.files.contains_key(p) {
            return Err("Pacote ausente no backup.".into());
        }
    }
    validate_files(&root.join("data"), &snapshot.files)?;
    Ok(snapshot)
}
fn capture_configs(
    game: &NexusGame,
    storage: &Path,
    data: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let Some(module) = super::nexus_deployment::definition(game) else {
        return Ok(());
    };
    let manifest = data.join("deployment.json");
    if !manifest.exists() {
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest).map_err(error)?).map_err(error)?;
    let Some(sources) = value["sources"].as_object() else {
        return Ok(());
    };
    for (key, source) in sources {
        if !super::nexus_deployment::config(&module, key) {
            continue;
        }
        let source = Path::new(source.as_str().ok_or("Fonte de configuração inválida.")?);
        let rel = source.strip_prefix(storage).map_err(error)?;
        relative(rel.to_str().ok_or("Configuração inválida.")?)?;
        if !rel.starts_with("profiles") {
            return Err("Configuração fora do perfil.".into());
        }
        let current = super::nexus_deployment::destination(game, key)?;
        if current.is_file() {
            let dest = data.join(rel);
            if !dest.is_file() {
                continue;
            }
            fs::copy(&current, &dest).map_err(error)?;
            files.insert(rel.to_string_lossy().into_owned(), hash(&dest)?);
        }
    }
    Ok(())
}
fn write_snapshot(game: &NexusGame, target: &Path) -> Result<(), String> {
    if super::nexus_profile::running(game) {
        return Err(format!(
            "Feche o jogo para atualizar sua recuperação: {}",
            game.game["name"]
        ));
    }
    let storage = super::nexus_profile::directory(game)?;
    write_snapshot_from(game, &storage, target)
}
fn write_snapshot_from(game: &NexusGame, storage: &Path, target: &Path) -> Result<(), String> {
    let stage = target.with_file_name(format!(".recovery-{}", uuid::Uuid::new_v4()));
    directory(&stage)?;
    let result = (|| {
        let mut files = copy_tree(storage, &stage.join("data"))?;
        capture_configs(game, storage, &stage.join("data"), &mut files)?;
        json(
            &stage.join("snapshot.json"),
            &Snapshot {
                format: 1,
                game: game.clone(),
                storage: storage.to_string_lossy().into_owned(),
                files,
            },
        )?;
        // Validate the copy before making it the current recovery point.
        read_snapshot(&stage)?;
        let previous = target.with_file_name(format!(".previous-{}", uuid::Uuid::new_v4()));
        if target.exists() {
            directory(target)?;
            fs::rename(target, &previous).map_err(error)?;
        }
        if let Err(e) = fs::rename(&stage, target) {
            if previous.exists() {
                let _ = fs::rename(&previous, target);
            }
            return Err(error(e));
        }
        if previous.exists() {
            let _ = fs::remove_dir_all(previous);
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}
fn game_copy(game: &NexusGame) -> Result<PathBuf, String> {
    let root = Path::new(
        game.game["directory"]
            .as_str()
            .ok_or("Pasta do jogo ausente.")?,
    );
    if !root.is_dir() {
        return Err(format!("Pasta do jogo indisponível: {}", root.display()));
    }
    let hidden = root.join(".peligames");
    directory(&hidden)?;
    Ok(hidden.join("mod-recovery"))
}
// Delete only the recovery copy; neighboring preferences and game files are preserved.
fn delete_game_copy(root: &Path) -> Result<(), String> {
    let root = fs::canonicalize(root).map_err(error)?;
    existing_directory(&root)?;
    let hidden = root.join(".peligames");
    match fs::symlink_metadata(&hidden) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(error(e)),
        Ok(_) => existing_directory(&hidden)?,
    }
    let copy = hidden.join("mod-recovery");
    match fs::symlink_metadata(&copy) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(error(e)),
        Ok(_) => { existing_directory(&copy)?; fs::remove_dir_all(&copy).map_err(error)?; },
    }
    if fs::read_dir(&hidden).map_err(error)?.next().is_none() {
        fs::remove_dir(&hidden).map_err(error)?;
    }
    Ok(())
}
/// Coalesce writes and perform disk copies away from the webview/UI thread.
pub(super) fn schedule(data: &Workspace) {
    if !settings().is_ok_and(|s| any_enabled(&s)) {
        return;
    }
    if let Ok(mut pending) = PENDING.lock() {
        *pending = Some(data.games.clone());
    }
    use std::sync::atomic::Ordering;
    if WORKER.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| loop {
        let games = {
            let mut pending = PENDING.lock().unwrap();
            match pending.take() {
                Some(g) => g,
                None => {
                    WORKER.store(false, Ordering::SeqCst);
                    break;
                }
            }
        };
        let _operation = OPERATIONS.lock().unwrap();
        let mut messages = Vec::new();
        if let Ok(current) = settings() {
            for game in games.iter().filter(|g| game_enabled(&current, &g.id) && has_installed_mods(g)) {
                if let Err(e) = game_copy(game).and_then(|p| write_snapshot(game, &p)) {
                    super::nexus_reports::event(game, "ERROR", "cópia de recuperação", &e);
                    messages.push(e);
                }
            }
        }
        if let Ok(mut s) = settings() {
            s.last_error = messages.join("\n");
            let _ = json(&settings_path().unwrap(), &s);
        }
    });
}
fn rebase(value: &mut serde_json::Value, old: &Path, new: &Path) {
    match value {
        serde_json::Value::String(s) => {
            if let Ok(p) = Path::new(s.as_str()).strip_prefix(old) {
                *s = if p.as_os_str().is_empty() {
                    new.to_path_buf()
                } else {
                    new.join(p)
                }
                .to_string_lossy()
                .into_owned();
            }
        }
        serde_json::Value::Array(a) => {
            for v in a {
                rebase(v, old, new)
            }
        }
        serde_json::Value::Object(o) => {
            for v in o.values_mut() {
                rebase(v, old, new)
            }
        }
        _ => {}
    }
}
// Relink only a link still pointing to its recorded, unchanged old profile. Regular or
// externally edited game files are never replaced by recovery.
fn repair_links(
    game: &NexusGame,
    data: &Path,
    old_storage: &Path,
    new_storage: &Path,
    old_game: &Path,
    new_game: &Path,
) -> Result<(), String> {
    let pending = data.join("recovery-deployment.json");
    let manifest = if pending.is_file() {
        pending
    } else {
        data.join("deployment.json")
    };
    if !manifest.is_file() {
        return Ok(());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest).map_err(error)?).map_err(error)?;
    if value["method"] != "symlink" {
        return Ok(());
    }
    let sources = value["sources"]
        .as_object()
        .ok_or("Fontes da implantação inválidas.")?;
    let targets = value["targets"]
        .as_object()
        .ok_or("Destinos da implantação inválidos.")?;
    let mut changes = Vec::new();
    for (key, source) in sources {
        let Some(source) = source.as_str() else {
            return Err("Fonte inválida.".into());
        };
        let Some(target) = targets.get(key).and_then(|v| v.as_str()) else {
            continue;
        };
        let source_rel = Path::new(source)
            .strip_prefix(old_storage)
            .map_err(|_| "Fonte fora do perfil.")?;
        relative(source_rel.to_str().ok_or("Fonte inválida.")?)?;
        if !source_rel.starts_with("profiles") {
            return Err("Fonte fora dos perfis de mods.".into());
        }
        // Prefix-based targets must wait for the user to configure this distribution.
        let Ok(target_rel) = Path::new(target).strip_prefix(old_game) else {
            continue;
        };
        relative(target_rel.to_str().ok_or("Destino inválido.")?)?;
        let dest = new_game.join(target_rel);
        let next = new_storage.join(source_rel);
        if super::nexus_deployment::destination(game, key)? != dest {
            return Err("Destino de recuperação divergente do módulo do jogo.".into());
        }
        if !fs::symlink_metadata(&dest).is_ok_and(|m| m.file_type().is_symlink()) {
            continue;
        }
        if fs::read_link(&dest).map_err(error)? != Path::new(source) {
            continue;
        }
        let mut parent = new_game.to_path_buf();
        for part in target_rel.parent().unwrap_or(Path::new("")).components() {
            parent.push(part.as_os_str());
            directory(&parent)?;
        }
        let m = fs::symlink_metadata(&next).map_err(error)?;
        if !m.is_file() || m.file_type().is_symlink() {
            return Err("Perfil recuperado inválido.".into());
        }
        if Path::new(source) == next {
            continue;
        }
        if dest.exists() && hash(&dest)? != hash(&next)? {
            return Err(format!(
                "Link modificado fora do perfil; preservado: {}",
                dest.display()
            ));
        }
        changes.push((dest, next, PathBuf::from(source)));
    }
    let mut applied: Vec<(PathBuf, PathBuf, PathBuf)> = Vec::new();
    for (dest, next, previous) in changes {
        let temporary = dest.with_file_name(format!(".peligames-link-{}", uuid::Uuid::new_v4()));
        let result = std::os::unix::fs::symlink(&next, &temporary)
            .and_then(|_| fs::rename(&temporary, &dest));
        if let Err(e) = result {
            let _ = fs::remove_file(&temporary);
            for (dest, _, old) in applied.iter().rev() {
                let temp = dest.with_file_name(format!(".peligames-link-{}", uuid::Uuid::new_v4()));
                if std::os::unix::fs::symlink(old, &temp).is_ok() {
                    let _ = fs::rename(&temp, dest);
                }
            }
            return Err(error(e));
        }
        applied.push((dest, next, previous));
    }
    Ok(())
}
fn restore_game(
    source: &Path,
    directory_override: Option<&Path>,
    workspace: &mut Workspace,
) -> Result<bool, String> {
    restore_game_at(source, directory_override, workspace, None)
}
fn restore_game_at(
    source: &Path,
    directory_override: Option<&Path>,
    workspace: &mut Workspace,
    storage_override: Option<&Path>,
) -> Result<bool, String> {
    let mut snapshot = read_snapshot(source)?;
    let old_game = PathBuf::from(
        snapshot.game.game["directory"]
            .as_str()
            .ok_or("Pasta original ausente.")?,
    );
    let new_game = directory_override.unwrap_or(&old_game);
    if workspace
        .games
        .iter()
        .any(|g| g.id == snapshot.game.id || g.game["directory"].as_str() == new_game.to_str())
    {
        return Ok(false);
    }
    if !snapshot.game.compat_data.is_empty()
        && super::nexus_prefix::resolve(Path::new(&snapshot.game.compat_data)).is_err()
    {
        snapshot.game.compat_data.clear();
    }
    if !snapshot.game.proton.is_empty()
        && !Path::new(&snapshot.game.proton).join("proton").is_file()
    {
        snapshot.game.proton.clear();
    }
    if let Some(root) = directory_override {
        rebase(&mut snapshot.game.game, &old_game, root);
        // Current distribution owns its own runner/prefix; never import an obsolete home path.
        snapshot.game.compat_data.clear();
        snapshot.game.proton.clear();
    }
    super::nexus_deployment::detect(&mut snapshot.game);
    snapshot.game.game["prefix"] = snapshot.game.compat_data.clone().into();
    snapshot.game.game["proton"] = snapshot.game.proton.clone().into();
    let target = match storage_override {
        Some(p) => p.to_path_buf(),
        None => super::nexus_profile::directory(&snapshot.game)?,
    };
    if target.exists() {
        return Err("Já existe armazenamento local para este jogo; preservado.".into());
    }
    let stage = target.with_file_name(format!(".restore-{}", uuid::Uuid::new_v4()));
    directory(&stage)?;
    let result = (|| {
        // Copy only verified listed files, never arbitrary extras supplied alongside a backup.
        for name in snapshot.files.keys() {
            let rel = relative(name)?;
            let dest = stage.join(rel);
            fs::create_dir_all(dest.parent().unwrap()).map_err(error)?;
            fs::copy(source.join("data").join(rel), dest).map_err(error)?;
        }
        for item in &mut snapshot.game.mods {
            item.archive = target
                .join(
                    Path::new(&item.archive)
                        .strip_prefix(&snapshot.storage)
                        .map_err(error)?,
                )
                .to_string_lossy()
                .into_owned();
        }
        let old_manifest = stage.join("deployment.json");
        let repair_record = stage.join("recovery-deployment.json");
        if old_manifest.exists() {
            fs::copy(&old_manifest, &repair_record).map_err(error)?;
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&old_manifest).map_err(error)?).map_err(error)?;
            rebase(&mut value, Path::new(&snapshot.storage), &target);
            rebase(&mut value, &old_game, new_game);
            json(&old_manifest, &value)?;
        }
        fs::rename(&stage, &target).map_err(error)?;
        // Relinking reads the old ownership record; the live manifest already uses this home.
        if new_game.is_dir() {
            if let Err(e) = repair_links(
                &snapshot.game,
                &target,
                Path::new(&snapshot.storage),
                &target,
                &old_game,
                new_game,
            ) {
                let _ = fs::remove_dir_all(&target);
                return Err(e);
            }
        }
        let _ = fs::remove_file(target.join("recovery-deployment.json"));
        // Packages retain their active state; only verified broken links were repaired.
        workspace.games.push(snapshot.game);
        Ok(true)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}
pub(super) fn recover(directory: &Path, workspace: &mut Workspace) -> Result<bool, String> {
    let path = directory.join(".peligames/mod-recovery");
    if !path.exists() {
        return Ok(false);
    }
    existing_directory(&directory.join(".peligames"))?;
    let marker = read_snapshot(&path)?;
    if let Some(game) = workspace.games.iter_mut().find(|g| g.id == marker.game.id) {
        let old = PathBuf::from(game.game["directory"].as_str().unwrap_or(""));
        if old == directory || old.is_dir() {
            return Ok(false);
        }
        let mut relocated = game.clone();
        rebase(&mut relocated.game, &old, directory);
        relocated.compat_data.clear();
        relocated.proton.clear();
        let storage = super::nexus_profile::directory(&relocated)?;
        repair_links(&relocated, &storage, &storage, &storage, &old, directory)?;
        let manifest = storage.join("deployment.json");
        if manifest.exists() {
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&manifest).map_err(error)?).map_err(error)?;
            rebase(&mut value, &old, directory);
            json(&manifest, &value)?;
        }
        *game = relocated;
        return Ok(true);
    }
    restore_game(&path, Some(directory), workspace)
}
#[tauri::command]
pub async fn load_mod_backup_settings() -> Result<BackupSettings, String> {
    settings()
}
#[tauri::command]
pub async fn set_mod_backup_settings(per_game: bool) -> Result<BackupSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _op = OPERATIONS.lock().map_err(error)?;
        let mut s = settings()?;
        s.per_game = per_game;
        s.last_error.clear();
        s.synchronizing = false;
        json(&settings_path()?, &s)?;
        drop(_op);
        {
            let _lock = nexus_local::STORAGE.lock().map_err(error)?;
            schedule(&nexus_local::load()?);
        }
        Ok(s)
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub async fn set_nexus_game_backup(
    game_id: String,
    enabled: bool,
) -> Result<BackupSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _op = OPERATIONS.lock().map_err(error)?;
        let _lock = nexus_local::STORAGE.lock().map_err(error)?;
        let data = nexus_local::load()?;
        if !data.games.iter().any(|g| g.id == game_id) {
            return Err("Jogo não encontrado.".into());
        }
        let mut s = settings()?;
        s.game_overrides.insert(game_id, enabled);
        s.last_error.clear();
        json(&settings_path()?, &s)?;
        drop(_op);
        schedule(&data);
        Ok(s)
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub async fn delete_nexus_game_backup(game_id: String) -> Result<BackupSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _op = OPERATIONS.lock().map_err(error)?;
        let _lock = nexus_local::STORAGE.lock().map_err(error)?;
        let data = nexus_local::load()?;
        let game = data.games.iter().find(|g| g.id == game_id).ok_or("Jogo não encontrado.")?;
        let root = Path::new(game.game["directory"].as_str().ok_or("Pasta do jogo ausente.")?);
        // Save the explicit override before deletion so queued copies cannot recreate it.
        let mut s = settings()?;
        s.game_overrides.insert(game_id, false);
        s.last_error.clear();
        json(&settings_path()?, &s)?;
        delete_game_copy(root)?;
        Ok(s)
    }).await.map_err(error)?
}
// Temporary staging is removed on success and on every error path.
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}
fn staging(parent: &Path) -> Result<Staging, String> {
    let path = parent.join(format!(".peligames-backup-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&path).map_err(error)?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(error)?;
    }
    Ok(Staging(path))
}
fn compress_backup(root: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::OpenOptions::new().write(true).create_new(true).open(destination).map_err(error)?;
    let result = (|| {
        let mut archive = zip::ZipWriter::new(file);
        let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true).unix_permissions(0o600);
        for entry in walkdir::WalkDir::new(root).follow_links(false) {
            let entry = entry.map_err(error)?;
            if entry.file_type().is_dir() { continue; }
            if !entry.file_type().is_file() { return Err("Arquivo inválido no backup.".into()); }
            let name = entry.path().strip_prefix(root).map_err(error)?.to_str().ok_or("Nome de arquivo inválido.")?.replace('\\', "/");
            relative(&name)?;
            archive.start_file(name, options).map_err(error)?;
            std::io::copy(&mut fs::File::open(entry.path()).map_err(error)?, &mut archive).map_err(error)?;
        }
        archive.finish().map_err(error)?.sync_all().map_err(error)
    })();
    if result.is_err() { let _ = fs::remove_file(destination); }
    result
}
fn extract_backup(source: &Path, target: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(source).map_err(error)?;
    if !meta.is_file() || meta.file_type().is_symlink() { return Err("Selecione um arquivo ZIP de backup válido.".into()); }
    let mut archive = zip::ZipArchive::new(fs::File::open(source).map_err(error)?).map_err(error)?;
    if archive.len() > 200_000 { return Err("Backup contém arquivos demais.".into()); }
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(error)?;
        let name = entry.name().trim_end_matches('/');
        let path = relative(name)?;
        // Reject symlinks/devices and unexpected payloads before writing anything.
        if entry.unix_mode().is_some_and(|mode| !matches!(mode & 0o170000, 0 | 0o100000 | 0o040000)) {
            return Err("Links e arquivos especiais não são permitidos no backup.".into());
        }
        if name != "backup.json" && !path.starts_with("Nexus") && !path.starts_with("DLSSNR") {
            return Err("Conteúdo estranho ao backup Nexus.".into());
        }
        // Old backups may contain DLSSNR; it is intentionally ignored.
        if path.starts_with("DLSSNR") { continue; }
        let dest = target.join(path);
        if entry.is_dir() { fs::create_dir_all(&dest).map_err(error)?; continue; }
        fs::create_dir_all(dest.parent().ok_or("Caminho inválido.")?).map_err(error)?;
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&dest).map_err(error)?;
        let copied = std::io::copy(&mut entry, &mut file).map_err(error)?;
        if copied != entry.size() { return Err("ZIP incompleto.".into()); }
    }
    Ok(())
}
#[tauri::command]
pub async fn create_mod_backup(destination: String) -> Result<Report, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _op = OPERATIONS.lock().map_err(error)?;
        let _lock = nexus_local::STORAGE.lock().map_err(error)?;
        let data = nexus_local::load()?;
        let eligible: Vec<_> = data.games.iter().filter(|g| has_installed_mods(g)).collect();
        if eligible.is_empty() { return Err("Nenhum jogo com mods Nexus instalados para fazer backup.".into()); }
        let parent = fs::canonicalize(&destination).map_err(error)?;
        let mods = crate::core::paths::app_root()?.join("Mod");
        if parent.starts_with(&mods) { return Err("Salve o backup fora do armazenamento de mods.".into()); }
        let stage = staging(&parent)?;
        directory(&stage.0.join("Nexus"))?;
        let mut games = Vec::new();
        for game in eligible {
            let storage = super::nexus_storage::directory(game)?;
            let folder = storage.file_name().and_then(|n| n.to_str()).ok_or("Pasta do jogo inválida.")?.to_owned();
            write_snapshot(game, &stage.0.join("Nexus").join(&folder))?;
            games.push(folder);
        }
        json(&stage.0.join("backup.json"), &Backup { format: 2, games })?;
        let target = parent.join(format!("PeliGames-Nexus-{}.zip", uuid::Uuid::new_v4()));
        compress_backup(&stage.0, &target)?;
        Ok(Report { path: target.to_string_lossy().into_owned(), ..Default::default() })
    }).await.map_err(error)?
}
#[tauri::command]
pub async fn restore_mod_backup(source: String) -> Result<Report, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _op = OPERATIONS.lock().map_err(error)?;
        let _lock = nexus_local::STORAGE.lock().map_err(error)?;
        let source = PathBuf::from(source);
        // Preserve compatibility with previously exported directory backups.
        let stage;
        let root = if source.is_dir() { source } else {
            stage = staging(&crate::core::paths::app_root()?)?;
            extract_backup(&source, &stage.0)?;
            stage.0.clone()
        };
        existing_directory(&root)?; existing_directory(&root.join("Nexus"))?;
        let manifest = root.join("backup.json");
        let meta = fs::symlink_metadata(&manifest).map_err(error)?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 32 * 1024 * 1024 { return Err("Backup inválido.".into()); }
        let backup: Backup = serde_json::from_slice(&fs::read(manifest).map_err(error)?).map_err(error)?;
        if ![1,2].contains(&backup.format) { return Err("Formato de backup não suportado.".into()); }
        // Validate every game and every checksum before restoring the first one.
        for folder in &backup.games {
            let path = relative(folder)?;
            if path.components().count() != 1 { return Err("Pasta de jogo inválida.".into()); }
            let snapshot = read_snapshot(&root.join("Nexus").join(path))?;
            uuid::Uuid::parse_str(&snapshot.game.id).map_err(error)?;
            if folder != &snapshot.game.id && !folder.ends_with(&format!("--{}", snapshot.game.id)) {
                return Err("Identificador divergente no backup.".into());
            }
        }
        let mut data = nexus_local::load()?; let mut report = Report::default();
        for folder in backup.games {
            let before = data.games.len();
            match restore_game(&root.join("Nexus").join(folder), None, &mut data) {
                Ok(true) => {
                    if let Some(game) = data.games.last() {
                        if !Path::new(game.game["directory"].as_str().unwrap_or("")).is_dir() {
                            report.warnings.push(format!("Jogo recuperado; monte o disco no caminho original ou associe o jogo novamente: {}", game.game["name"]));
                        }
                    }
                    if let Err(e) = nexus_local::save(&data) { data.games.truncate(before); report.warnings.push(e); break; }
                    report.restored += 1;
                },
                Ok(false) => report.skipped += 1,
                Err(e) => report.warnings.push(e),
            }
        }
        Ok(report)
    }).await.map_err(error)?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> PathBuf {
        let p =
            std::env::temp_dir().join(format!("peligames-backup-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn eligibility_requires_an_installed_nexus_mod() {
        let root = base(); let (mut game, _) = fixture(&root);
        assert!(has_installed_mods(&game)); // Disabled but installed is still recoverable.
        game.mods[0].installed = false;
        assert!(!has_installed_mods(&game));
        game.mods.clear(); assert!(!has_installed_mods(&game));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn zip_roundtrip_restores_validated_nexus_snapshot() {
        let root = base(); let (game, storage) = fixture(&root);
        let source = root.join("source"); fs::create_dir_all(source.join("Nexus")).unwrap();
        let folder = format!("example-game--{}", game.id);
        write_snapshot_from(&game, &storage, &source.join("Nexus").join(&folder)).unwrap();
        json(&source.join("backup.json"), &Backup { format: 2, games: vec![folder.clone()] }).unwrap();
        let zip = root.join("backup.zip"); compress_backup(&source, &zip).unwrap();
        assert!(compress_backup(&source, &zip).is_err()); // Never overwrite a backup.
        let dest = root.join("extracted"); fs::create_dir(&dest).unwrap();
        extract_backup(&zip, &dest).unwrap();
        let mut workspace = Workspace::default();
        assert!(restore_game_at(&dest.join("Nexus").join(folder), Some(&root.join("relocated-game")), &mut workspace, Some(&root.join("restored"))).unwrap());
        assert_eq!(fs::read(&workspace.games[0].mods[0].archive).unwrap(), b"package");
        assert!(workspace.games[0].mods[0].installed);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn zip_rejects_traversal_duplicates_and_symlinks() {
        use std::io::Write;
        let root = base();
        for (index, names) in [vec!["../escaped"], vec!["/escaped"], vec!["Nexus/../../escaped"], vec!["Nexus/a", "Nexus/a"]].into_iter().enumerate() {
            let path = root.join(format!("bad-{index}.zip"));
            let mut archive = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            for name in names { archive.start_file(name, zip::write::FileOptions::default()).unwrap(); archive.write_all(b"unsafe").unwrap(); }
            archive.finish().unwrap(); let dest = root.join(format!("out-{index}")); fs::create_dir(&dest).unwrap();
            assert!(extract_backup(&path, &dest).is_err());
        }
        let path = root.join("symlink.zip"); let mut archive = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        archive.add_symlink("Nexus/link", "../../escaped", zip::write::FileOptions::default()).unwrap(); archive.finish().unwrap();
        let dest = root.join("out-link"); fs::create_dir(&dest).unwrap(); assert!(extract_backup(&path, &dest).is_err());
        assert!(!root.join("escaped").exists()); fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn deletion_preserves_game_and_neighboring_preferences() {
        let root = base();
        let hidden = root.join(".peligames");
        fs::create_dir_all(hidden.join("mod-recovery/archives")).unwrap();
        fs::write(hidden.join("mod-recovery/archives/mod.zip"), b"mod").unwrap();
        fs::write(hidden.join("dlssnr.json"), b"preferences").unwrap();
        fs::write(root.join("game.exe"), b"game").unwrap();
        delete_game_copy(&root).unwrap();
        assert!(!hidden.join("mod-recovery").exists());
        assert_eq!(fs::read(hidden.join("dlssnr.json")).unwrap(), b"preferences");
        assert!(root.join("game.exe").exists());
        fs::remove_file(hidden.join("dlssnr.json")).unwrap();
        delete_game_copy(&root).unwrap();
        assert!(!hidden.exists());
        delete_game_copy(&root).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn deletion_rejects_linked_backup_directories() {
        use std::os::unix::fs::symlink;
        let root = base(); let outside = base();
        fs::write(outside.join("keep"), b"safe").unwrap();
        symlink(&outside, root.join(".peligames")).unwrap();
        assert!(delete_game_copy(&root).is_err());
        fs::remove_file(root.join(".peligames")).unwrap();
        fs::create_dir(root.join(".peligames")).unwrap();
        symlink(&outside, root.join(".peligames/mod-recovery")).unwrap();
        assert!(delete_game_copy(&root).is_err());
        assert_eq!(fs::read(outside.join("keep")).unwrap(), b"safe");
        fs::remove_dir_all(root).unwrap(); fs::remove_dir_all(outside).unwrap();
    }
    #[test]
    fn individual_choices_override_global_without_affecting_other_games() {
        let mut s = BackupSettings::default();
        assert!(!game_enabled(&s, "a"));
        s.game_overrides.insert("a".into(), true);
        assert!(game_enabled(&s, "a"));
        assert!(!game_enabled(&s, "b"));
        assert!(any_enabled(&s));
        s.per_game = true;
        s.game_overrides.insert("a".into(), false);
        assert!(!game_enabled(&s, "a"));
        assert!(game_enabled(&s, "b"));
        s.per_game = false;
        assert!(!any_enabled(&s));
    }
    #[test]
    fn old_settings_default_to_global_and_overrides_survive_serialization() {
        let mut s: BackupSettings = serde_json::from_str(r#"{"per_game":true}"#).unwrap();
        assert!(game_enabled(&s, "a"));
        s.game_overrides.insert("a".into(), false);
        let s: BackupSettings = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert!(!game_enabled(&s, "a"));
        assert!(game_enabled(&s, "b"));
    }
    #[test]
    fn rejects_escape_and_absolute_paths() {
        for p in ["../x", "/etc/passwd", "a/../b", "a\\b", ""] {
            assert!(relative(p).is_err());
        }
        assert!(relative("archives/file.zip").is_ok());
    }
    #[test]
    fn validates_content_and_detects_corruption() {
        let p = base();
        fs::write(p.join("a"), "mod").unwrap();
        let files = BTreeMap::from([("a".into(), hash(&p.join("a")).unwrap())]);
        validate_files(&p, &files).unwrap();
        fs::write(p.join("a"), "changed").unwrap();
        assert!(validate_files(&p, &files).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn copy_does_not_walk_symlinks() {
        let p = base();
        let source = p.join("source");
        fs::create_dir(&source).unwrap();
        std::os::unix::fs::symlink("/etc", source.join("external")).unwrap();
        assert!(copy_tree(&source, &p.join("target")).is_err());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn rebase_uses_path_boundaries() {
        let mut v = serde_json::json!({"path":"/old/home/profiles/file","other":"/old/home-evil/file","text":"Name /old/home"});
        rebase(&mut v, Path::new("/old/home"), Path::new("/new/home"));
        assert_eq!(v["path"], "/new/home/profiles/file");
        assert_eq!(v["other"], "/old/home-evil/file");
        assert_eq!(v["text"], "Name /old/home");
    }
    fn fixture(base: &Path) -> (NexusGame, PathBuf) {
        let storage = base.join("old-store");
        fs::create_dir(&storage).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let mid = uuid::Uuid::new_v4().to_string();
        let profile = uuid::Uuid::new_v4().to_string();
        fs::create_dir(storage.join("archives")).unwrap();
        fs::write(storage.join("archives/mod.zip"), b"package").unwrap();
        fs::create_dir_all(storage.join("profiles").join(&profile)).unwrap();
        fs::write(
            storage.join("profiles").join(&profile).join("init.lua"),
            b"script",
        )
        .unwrap();
        fs::write(storage.join("deployment.json"),serde_json::to_vec(&serde_json::json!({"method":"copy","sources":{"init.lua":storage.join("profiles").join(&profile).join("init.lua")},"targets":{"init.lua":base.join("old-game/init.lua")},"files":{}})).unwrap()).unwrap();
        let game=serde_json::from_value(serde_json::json!({"id":id,"game":{"name":"Example","directory":base.join("old-game")},"platform":"proton","mods":[{"id":mid,"name":"Readable mod","version":"1.2","archive":storage.join("archives/mod.zip"),"size":7,"installed":true,"enabled":false}],"profile":profile})).unwrap();
        (game, storage)
    }
    #[test]
    fn repairs_owned_links_and_preserves_regular_files_and_external_links() {
        let base = base();
        let (mut game, old_storage) = fixture(&base);
        let root = base.join("new-game");
        fs::create_dir_all(root.join("bin/x64")).unwrap();
        fs::write(root.join("bin/x64/Cyberpunk2077.exe"), b"MZfixture").unwrap();
        game.game["directory"] = root.to_string_lossy().into_owned().into();
        let new_storage = base.join("new-store");
        fs::create_dir(&new_storage).unwrap();
        let revision = game.profile.as_deref().unwrap();
        let profile = PathBuf::from("profiles").join(revision);
        fs::create_dir_all(new_storage.join(&profile)).unwrap();
        let names = [
            "archive/pc/mod/a.archive",
            "archive/pc/mod/b.archive",
            "archive/pc/mod/c.archive",
            "archive/pc/mod/d.archive",
        ];
        let mut sources = serde_json::Map::new();
        let mut targets = serde_json::Map::new();
        fs::create_dir_all(root.join("archive/pc/mod")).unwrap();
        for (i, name) in names.iter().enumerate() {
            let old = old_storage.join(&profile).join(format!("{i}.archive"));
            let new = new_storage.join(&profile).join(format!("{i}.archive"));
            fs::write(&new, b"archive").unwrap();
            sources.insert(name.to_string(), old.to_string_lossy().into_owned().into());
            targets.insert(
                name.to_string(),
                root.join(name).to_string_lossy().into_owned().into(),
            );
            match i {
                0 => std::os::unix::fs::symlink(&old, root.join(name)).unwrap(),
                1 => fs::write(root.join(name), b"user file").unwrap(),
                2 => std::os::unix::fs::symlink(base.join("unrelated"), root.join(name)).unwrap(),
                _ => {
                    fs::write(&old, b"archive").unwrap();
                    std::os::unix::fs::symlink(&old, root.join(name)).unwrap();
                }
            }
        }
        json(
            &new_storage.join("deployment.json"),
            &serde_json::json!({"method":"symlink","sources":sources,"targets":targets}),
        )
        .unwrap();
        repair_links(
            &game,
            &new_storage,
            &old_storage,
            &new_storage,
            &root,
            &root,
        )
        .unwrap();
        assert_eq!(
            fs::read_link(root.join(names[0])).unwrap(),
            new_storage.join(&profile).join("0.archive")
        );
        assert_eq!(fs::read(root.join(names[1])).unwrap(), b"user file");
        assert_eq!(
            fs::read_link(root.join(names[2])).unwrap(),
            base.join("unrelated")
        );
        assert_eq!(
            fs::read_link(root.join(names[3])).unwrap(),
            new_storage.join(&profile).join("3.archive")
        );
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn roundtrip_keeps_packages_states_and_rebases_records() {
        let base = base();
        let (game, storage) = fixture(&base);
        let backup = base.join("snapshot");
        write_snapshot_from(&game, &storage, &backup).unwrap();
        let target = base.join("new-store");
        let new_game = base.join("new-game");
        let mut workspace = Workspace::default();
        assert!(restore_game_at(&backup, Some(&new_game), &mut workspace, Some(&target)).unwrap());
        let restored = &workspace.games[0];
        assert_eq!(restored.mods[0].name, "Readable mod");
        assert_eq!(restored.mods[0].version, "1.2");
        assert!(restored.mods[0].installed);
        assert!(!restored.mods[0].enabled);
        assert_eq!(fs::read(&restored.mods[0].archive).unwrap(), b"package");
        assert_eq!(
            restored.game["directory"],
            new_game.to_string_lossy().as_ref()
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(target.join("deployment.json")).unwrap()).unwrap();
        assert!(manifest["sources"]["init.lua"]
            .as_str()
            .unwrap()
            .starts_with(target.to_str().unwrap()));
        assert_eq!(
            manifest["targets"]["init.lua"],
            new_game.join("init.lua").to_string_lossy().as_ref()
        );
        assert!(!restore_game_at(
            &backup,
            Some(&new_game),
            &mut workspace,
            Some(&base.join("other"))
        )
        .unwrap());
        assert!(!base.join("other").exists());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn corrupt_snapshot_does_not_create_destination() {
        let base = base();
        let (game, storage) = fixture(&base);
        let backup = base.join("snapshot");
        write_snapshot_from(&game, &storage, &backup).unwrap();
        fs::write(backup.join("data/archives/mod.zip"), "bad").unwrap();
        let target = base.join("new-store");
        assert!(restore_game_at(&backup, None, &mut Workspace::default(), Some(&target)).is_err());
        assert!(!target.exists());
        fs::remove_dir_all(base).unwrap();
    }
}

/// Explicitly selected disk/library root, for games not registered in a fresh Steam/home.
#[tauri::command]
pub async fn recover_mods_from_disk(directory: String) -> Result<Report, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _op=OPERATIONS.lock().map_err(error)?;
        let _lock=nexus_local::STORAGE.lock().map_err(error)?;
        let root=fs::canonicalize(directory).map_err(error)?;existing_directory(&root)?;
        let mut data=nexus_local::load()?;let mut report=Report::default();let mut count=0;
        for entry in walkdir::WalkDir::new(&root).max_depth(10).follow_links(false).into_iter().filter_entry(|entry| {
            entry.file_type().is_dir() && (entry.depth()==0 || !matches!(entry.file_name().to_str(),Some(".peligames"|"node_modules"|"pfx"|"compatdata"|".git")))
        }) {
            let entry=match entry{Ok(e)=>e,Err(e)=>{report.warnings.push(e.to_string());continue}};
            count+=1;if count>50_000{report.warnings.push("Limite de varredura atingido; selecione uma pasta de biblioteca mais específica.".into());break;}
            let hidden=entry.path().join(".peligames");if !hidden.exists(){continue;}
            if hidden.join("mod-recovery").exists(){match recover(entry.path(),&mut data){Ok(true)=>{nexus_local::save(&data)?;report.restored+=1;},Ok(false)=>report.skipped+=1,Err(e)=>report.warnings.push(e)}}

        }
        Ok(report)
    }).await.map_err(error)?
}
