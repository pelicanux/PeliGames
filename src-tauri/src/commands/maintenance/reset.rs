//! Durable reset journal lives outside every reset target. Renames stay on the
//! same filesystem; no recursive deletion is allowed before the UI health check.
use super::*;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const HELPER_FLAG: &str = "--peligames-reset-helper";
static LOCK: Mutex<()> = Mutex::new(());
static BOOT: Mutex<Option<(String, Instant)>> = Mutex::new(None);
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
enum Phase {
    Prepared,
    Ready,
    Checking,
    Recovery,
    Restoring,
    Restored,
    Committed,
}
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    original: PathBuf,
    backup: PathBuf,
    discarded: PathBuf,
}
#[derive(Clone, Serialize, Deserialize)]
struct Journal {
    id: String,
    phase: Phase,
    entries: Vec<Entry>,
    error: Option<String>,
}
#[derive(Serialize)]
pub struct Status {
    pub(super) state: String,
    error: Option<String>,
    backups: Vec<String>,
}

fn directory() -> Result<PathBuf, String> {
    Ok(base(dirs::data_dir())?.join(".peligames-maintenance"))
}
// Serialize helpers and normal startups too, not just commands in one process.
fn disk_lock(dir: &Path) -> Result<Option<fs::File>, String> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    if !exists(dir) {
        return Ok(None);
    }
    validate_ancestors(&dir.join("lock"))?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dir.join("lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(Some(file))
}
fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
fn sync_parent(path: &Path) -> Result<(), String> {
    fs::File::open(path.parent().ok_or("Caminho sem diretório pai.")?)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())
}
fn sibling(path: &Path, id: &str, kind: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".peligames-{kind}-{id}"));
    path.with_file_name(name)
}
fn save(dir: &Path, journal: &Journal) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    validate_ancestors(&dir.join("reset.json"))?;
    let temporary = dir.join(format!("journal-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(journal).map_err(|e| e.to_string())?)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&temporary, dir.join("reset.json")).map_err(|e| e.to_string())?;
    sync_parent(&dir.join("reset.json"))
}
fn load(dir: &Path) -> Result<Option<Journal>, String> {
    let path = dir.join("reset.json");
    validate_ancestors(&path)?;
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 65536 {
        return Err("Registro de redefinição inválido; cópias preservadas.".into());
    }
    let journal: Journal = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Não foi possível ler o registro de redefinição: {e}"))?;
    uuid::Uuid::parse_str(&journal.id).map_err(|e| e.to_string())?;
    let allowed = reset_paths(
        &base(dirs::config_dir())?,
        &base(dirs::data_dir())?,
        &base(dirs::cache_dir())?,
    );
    for entry in &journal.entries {
        if !allowed.contains(&entry.original)
            || entry.backup != sibling(&entry.original, &journal.id, "reset")
            || entry.discarded != sibling(&entry.original, &journal.id, "replaced")
        {
            return Err(
                "Caminhos inválidos no registro de redefinição; cópias preservadas.".into(),
            );
        }
        validate_ancestors(&entry.original)?;
    }
    Ok(Some(journal))
}
fn new_journal(paths: &[String]) -> Journal {
    let id = uuid::Uuid::new_v4().to_string();
    Journal {
        entries: paths
            .iter()
            .map(|p| {
                let original = PathBuf::from(p);
                Entry {
                    backup: sibling(&original, &id, "reset"),
                    discarded: sibling(&original, &id, "replaced"),
                    original,
                }
            })
            .collect(),
        id,
        phase: Phase::Prepared,
        error: None,
    }
}
fn rename(from: &Path, to: &Path) -> Result<(), String> {
    validate_ancestors(from)?;
    if exists(to) {
        return Err(format!("O destino já existe: {}", to.display()));
    }
    fs::rename(from, to)
        .map_err(|e| format!("Não foi possível renomear {}: {e}", from.display()))?;
    sync_parent(to)
}
fn stage(dir: &Path, journal: &mut Journal) -> Result<(), String> {
    for entry in &journal.entries {
        rename(&entry.original, &entry.backup)?;
    }
    journal.phase = Phase::Ready;
    save(dir, journal)
}
fn restore(dir: &Path, journal: &mut Journal) -> Result<(), String> {
    journal.phase = Phase::Restoring;
    save(dir, journal)?;
    for entry in &journal.entries {
        if exists(&entry.backup) {
            if exists(&entry.original) {
                rename(&entry.original, &entry.discarded)?;
            }
            rename(&entry.backup, &entry.original)?;
        } else if !exists(&entry.original) {
            return Err(format!(
                "Cópia indisponível para recuperar {}",
                entry.original.display()
            ));
        }
    }
    journal.phase = Phase::Restored;
    journal.error = None;
    save(dir, journal)
}
fn delete(path: &Path) -> Result<(), String> {
    validate_ancestors(path)?;
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    }
    .map_err(|e| format!("Não foi possível remover {}: {e}", path.display()))?;
    if exists(path) {
        return Err(format!("A cópia ainda existe: {}", path.display()));
    }
    sync_parent(path)
}
fn collect(dir: &Path, journal: &Journal) -> Result<(), String> {
    if journal.phase != Phase::Committed {
        return Err("A inicialização ainda não foi validada.".into());
    }
    for entry in &journal.entries {
        delete(&entry.backup)?;
        delete(&entry.discarded)?;
    }
    fs::remove_file(dir.join("reset.json")).map_err(|e| e.to_string())?;
    sync_parent(&dir.join("reset.json"))
}

fn mark_startup(dir: &Path, journal: &mut Journal) -> Result<bool, String> {
    match journal.phase {
        Phase::Ready | Phase::Restored => {
            journal.phase = Phase::Checking;
            save(dir, journal)?;
            return Ok(true);
        }
        Phase::Prepared | Phase::Checking | Phase::Restoring => {
            journal.phase = Phase::Recovery;
            journal.error = Some("A redefinição ou a inicialização anterior foi interrompida. Os arquivos antigos foram preservados.".into());
            save(dir, journal)?;
        }
        _ => {}
    }
    Ok(false)
}
/// Called before storage initialization and webviews. An interrupted validation
/// never counts as success just because another process was started.
pub fn begin_startup() -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    let dir = directory()?;
    let _disk = disk_lock(&dir)?;
    let Some(mut journal) = load(&dir)? else {
        return Ok(());
    };
    if mark_startup(&dir, &mut journal)? {
        *BOOT.lock().map_err(|e| e.to_string())? = Some((journal.id.clone(), Instant::now()));
    }
    crate::core::paths::pause_migrations(journal.phase == Phase::Recovery);
    Ok(())
}
#[tauri::command]
pub fn launcher_reset_status() -> Result<Status, String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    let dir = directory()?;
    let _disk = disk_lock(&dir)?;
    let journal = load(&dir)?;
    Ok(match journal {
        None => Status {
            state: "none".into(),
            error: None,
            backups: Vec::new(),
        },
        Some(j) => Status {
            state: match j.phase {
                Phase::Checking => "checking",
                Phase::Committed => "cleanup",
                _ => "recovery",
            }
            .into(),
            error: j.error,
            backups: j
                .entries
                .iter()
                .filter(|e| exists(&e.backup))
                .map(|e| e.backup.to_string_lossy().into_owned())
                .collect(),
        },
    })
}
#[tauri::command]
pub async fn launcher_reset_confirm_health() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let _lock = LOCK.lock().map_err(|e| e.to_string())?;
        let dir = directory()?;
        let _disk = disk_lock(&dir)?;
        let Some(mut journal) = load(&dir)? else {
            return Ok(());
        };
        if journal.phase != Phase::Committed {
            let boot = BOOT.lock().map_err(|e| e.to_string())?;
            if journal.phase != Phase::Checking
                || !boot.as_ref().is_some_and(|(id, started)| {
                    id == &journal.id && started.elapsed() >= Duration::from_secs(10)
                })
            {
                return Err("Aguarde a validação da nova inicialização.".into());
            }
            // Prove the recreated profile is a real, writable directory on disk.
            let root = crate::core::paths::app_root()?;
            if !fs::symlink_metadata(&root).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
            {
                return Err(
                    "A nova pasta de configuração não foi criada; cópia antiga preservada.".into(),
                );
            }
            let probe = root.join(format!(".reset-health-{}", journal.id));
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&probe)
                .map_err(|e| e.to_string())?;
            file.write_all(b"ready")
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            fs::remove_file(&probe).map_err(|e| e.to_string())?;
            sync_parent(&probe)?;
            journal.phase = Phase::Committed;
            save(&dir, &journal)?;
        }
        collect(&dir, &journal)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn launcher_reset_continue() -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    let dir = directory()?;
    let _disk = disk_lock(&dir)?;
    let mut journal = load(&dir)?.ok_or("Nenhuma recuperação pendente.")?;
    if !matches!(journal.phase, Phase::Recovery | Phase::Checking) {
        return Err("Recuperação indisponível nesta etapa.".into());
    }
    // A partially completed rename must be recovered, not treated as a clean profile.
    if journal.entries.iter().any(|e| !exists(&e.backup)) {
        return Err("A renomeação foi interrompida. Restaure os arquivos anteriores antes de tentar novamente.".into());
    }
    journal.phase = Phase::Checking;
    journal.error = None;
    save(&dir, &journal)?;
    *BOOT.lock().map_err(|e| e.to_string())? = Some((journal.id, Instant::now()));
    crate::core::paths::pause_migrations(false);
    crate::core::paths::initialize()?;
    Ok(())
}
fn spawn_helper(app: &tauri::AppHandle, mode: &str) -> Result<(), String> {
    let binary = tauri::process::current_binary(&app.env()).map_err(|e| e.to_string())?;
    let mut command = Command::new(&binary);
    crate::core::env_utils::host_environment(&mut command);
    command
        .env_remove("APPIMAGE")
        .env_remove("APPDIR")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD")
        .args([HELPER_FLAG, &std::process::id().to_string(), mode])
        .arg(&binary)
        .current_dir("/")
        .stdin(Stdio::null());
    let log = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(std::env::temp_dir().join(format!("peligames-reset-{}.log", uuid::Uuid::new_v4())))
        .map_err(|e| e.to_string())?;
    command
        .stdout(Stdio::from(log.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(log));
    command.spawn().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
pub fn restart(app: &tauri::AppHandle, paths: &[String]) -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    let dir = directory()?;
    let _disk = disk_lock(&dir)?;
    if load(&dir)?.is_some() {
        return Err("Conclua a redefinição anterior antes de iniciar outra.".into());
    }
    use std::os::unix::fs::DirBuilderExt;
    if !exists(&dir) {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|e| e.to_string())?;
        sync_parent(&dir)?;
    }
    let _created_disk = if _disk.is_none() {
        disk_lock(&dir)?
    } else {
        None
    };
    if load(&dir)?.is_some() {
        return Err("Conclua a redefinição anterior antes de iniciar outra.".into());
    }
    let journal = new_journal(paths);
    save(&dir, &journal)?;
    if let Err(error) = spawn_helper(app, "reset") {
        // No helper was started, so no rename can have happened yet. Permit retry.
        fs::remove_file(dir.join("reset.json")).map_err(|e| e.to_string())?;
        sync_parent(&dir.join("reset.json"))?;
        return Err(error);
    }
    Ok(())
}
#[tauri::command]
pub fn launcher_reset_restore(app: tauri::AppHandle) -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    idle(&app)?;
    let dir = directory()?;
    let _disk = disk_lock(&dir)?;
    let mut journal = load(&dir)?.ok_or("Nenhuma recuperação pendente.")?;
    if !matches!(journal.phase, Phase::Recovery | Phase::Checking) {
        return Err("Recuperação indisponível nesta etapa.".into());
    }
    journal.phase = Phase::Restoring;
    save(&dir, &journal)?;
    if let Err(error) = spawn_helper(&app, "restore") {
        journal.phase = Phase::Recovery;
        journal.error = Some(error.clone());
        save(&dir, &journal)?;
        return Err(error);
    }
    Ok(())
}
/// Runs before GTK/Tauri initialization, including when launched from an AppImage.
pub fn run_helper(args: &[String]) -> Result<(), String> {
    let parent: i32 = args
        .first()
        .ok_or("PID ausente.")?
        .parse()
        .map_err(|_| "PID inválido.")?;
    if parent <= 1 {
        return Err("PID inválido.".into());
    }
    let mode = args.get(1).ok_or("Operação ausente.")?;
    let binary = args.get(2).ok_or("Executável ausente.")?;
    if !Path::new(binary).is_absolute() {
        return Err("Executável inválido.".into());
    }
    for _ in 0..120 {
        if unsafe { libc::kill(parent, 0) } != 0 {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    if unsafe { libc::kill(parent, 0) } == 0 {
        return Err("O processo anterior ainda está aberto; arquivos preservados.".into());
    }
    let dir = directory()?;
    let _disk = disk_lock(&dir)?;
    let mut journal = load(&dir)?.ok_or("Registro ausente.")?;
    let result = match (mode.as_str(), &journal.phase) {
        ("reset", Phase::Prepared) => stage(&dir, &mut journal),
        ("restore", Phase::Restoring) => restore(&dir, &mut journal),
        _ => Err("Operação de recuperação inválida.".into()),
    };
    if let Err(error) = result {
        journal.phase = Phase::Recovery;
        journal.error = Some(error);
        save(&dir, &journal)?;
    }
    use std::os::unix::process::CommandExt;
    Err(Command::new(binary).current_dir("/").exec().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        root: PathBuf,
        journal: Journal,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("peligames-safe-reset-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let paths: Vec<String> = ["config", "cache"]
                .iter()
                .map(|name| {
                    let path = root.join(name);
                    fs::create_dir_all(path.join("mods/nested")).unwrap();
                    fs::write(path.join("mods/nested/keep"), name).unwrap();
                    path.to_string_lossy().into_owned()
                })
                .collect();
            let journal = new_journal(&paths);
            save(&root, &journal).unwrap();
            Self { root, journal }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn a_crash_before_health_confirmation_offers_recovery_on_the_next_boot() {
        let mut f = Fixture::new();
        stage(&f.root, &mut f.journal).unwrap();
        assert!(mark_startup(&f.root, &mut f.journal).unwrap());
        // Re-read only the durable journal, as a fresh process after power loss would.
        let mut persisted: Journal =
            serde_json::from_slice(&fs::read(f.root.join("reset.json")).unwrap()).unwrap();
        assert_eq!(persisted.phase, Phase::Checking);
        assert!(!mark_startup(&f.root, &mut persisted).unwrap());
        assert_eq!(persisted.phase, Phase::Recovery);
        assert!(collect(&f.root, &persisted).is_err());
        for entry in &persisted.entries {
            assert!(entry.backup.join("mods/nested/keep").is_file());
        }
        restore(&f.root, &mut persisted).unwrap();
        assert!(mark_startup(&f.root, &mut persisted).unwrap());
        assert_eq!(persisted.phase, Phase::Checking);
    }
    #[test]
    fn reset_renames_and_keeps_every_file_until_health_is_committed() {
        let mut f = Fixture::new();
        stage(&f.root, &mut f.journal).unwrap();
        for entry in &f.journal.entries {
            assert!(!exists(&entry.original));
            assert!(entry.backup.join("mods/nested/keep").is_file());
        }
        for phase in [
            Phase::Prepared,
            Phase::Ready,
            Phase::Checking,
            Phase::Recovery,
            Phase::Restoring,
            Phase::Restored,
        ] {
            f.journal.phase = phase;
            assert!(collect(&f.root, &f.journal).is_err());
            assert!(f.journal.entries[0]
                .backup
                .join("mods/nested/keep")
                .exists());
        }
        for entry in &f.journal.entries {
            fs::create_dir(&entry.original).unwrap();
            fs::write(entry.original.join("new-settings"), "ready").unwrap();
        }
        f.journal.phase = Phase::Committed;
        save(&f.root, &f.journal).unwrap();
        collect(&f.root, &f.journal).unwrap();
        for entry in &f.journal.entries {
            assert!(!exists(&entry.backup));
            assert!(entry.original.join("new-settings").is_file());
        }
        assert!(!f.root.join("reset.json").exists());
    }
    #[test]
    fn partial_rename_can_restore_without_overwriting_new_or_old_files() {
        let mut f = Fixture::new();
        // Power loss between two renames: journal is already durable.
        let first = &f.journal.entries[0];
        rename(&first.original, &first.backup).unwrap();
        let persisted: Journal =
            serde_json::from_slice(&fs::read(f.root.join("reset.json")).unwrap()).unwrap();
        assert_eq!(persisted.phase, Phase::Prepared);
        fs::create_dir(&first.original).unwrap();
        fs::write(first.original.join("new-settings"), "new").unwrap();
        restore(&f.root, &mut f.journal).unwrap();
        for entry in &f.journal.entries {
            assert!(entry.original.join("mods/nested/keep").is_file());
        }
        assert!(f.journal.entries[0]
            .discarded
            .join("new-settings")
            .is_file());
        assert!(collect(&f.root, &f.journal).is_err());
        f.journal.phase = Phase::Committed;
        collect(&f.root, &f.journal).unwrap();
        assert!(!exists(&f.journal.entries[0].discarded));
        assert!(f.journal.entries[0]
            .original
            .join("mods/nested/keep")
            .is_file());
    }
    #[test]
    fn interrupted_restore_resumes_after_the_new_directory_was_moved() {
        let mut f = Fixture::new();
        stage(&f.root, &mut f.journal).unwrap();
        let first = f.journal.entries[0].clone();
        fs::create_dir(&first.original).unwrap();
        fs::write(first.original.join("new-settings"), "new").unwrap();
        rename(&first.original, &first.discarded).unwrap();
        restore(&f.root, &mut f.journal).unwrap();
        // Resuming again must be harmless for entries that were already restored.
        restore(&f.root, &mut f.journal).unwrap();
        for entry in &f.journal.entries {
            assert!(entry.original.join("mods/nested/keep").is_file());
        }
        assert!(first.discarded.join("new-settings").is_file());
    }
    #[test]
    fn cleanup_never_follows_a_backed_up_symlink_into_external_games() {
        let mut f = Fixture::new();
        let external = f.root.join("external-game");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("keep"), "game").unwrap();
        std::os::unix::fs::symlink(&external, f.journal.entries[0].original.join("linked-game"))
            .unwrap();
        stage(&f.root, &mut f.journal).unwrap();
        f.journal.phase = Phase::Committed;
        collect(&f.root, &f.journal).unwrap();
        assert_eq!(fs::read_to_string(external.join("keep")).unwrap(), "game");
    }
}
