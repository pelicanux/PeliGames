//! Durable reset journal lives outside every reset target. Renames stay on the
//! same filesystem; deletion follows successful staging and requested preservation.
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
    #[serde(default)]
    preserve_accounts: bool,
    #[serde(default)]
    preserve_games: bool,
    #[serde(default)]
    preserve_mangohud: bool,
    #[serde(default)]
    finish_on_close: bool,
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
    let mut allowed = reset_paths(
        &base(dirs::config_dir())?,
        &base(dirs::data_dir())?,
        &base(dirs::cache_dir())?,
    );
    allowed.extend(super::super::steam_runtime::profile_directories()?);
    allowed.push(super::super::launch_profiles::profiles_directory()?);
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
        preserve_accounts: false,
        preserve_games: false,
        preserve_mangohud: false,
        finish_on_close: false,
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
// Copy only session stores after every owning process has exited. Backups remain
// untouched for recovery. No keyring secret is exported into the reset journal.
fn session_paths(root: &Path) -> Vec<PathBuf> {
    match root.file_name().and_then(|n| n.to_str()) {
        Some("PeliGames" | "peligames") => [
            "webview/nexus-browser", "nexus-browser",
            "webview/databases", "webview/IndexedDB", "webview/storage",
            "webview/cookies.sqlite", "webview/cookies.sqlite-wal", "webview/cookies.sqlite-shm",
        ].into_iter().map(PathBuf::from).collect(),
        Some("com.pelicano.peligames" | "com.pelicano.dlssnr-installer") => [
            "databases",
            "IndexedDB",
            "storage",
            "cookies.sqlite",
            "cookies.sqlite-wal",
            "cookies.sqlite-shm",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect(),
        _ => Vec::new(),
    }
}
fn copy_session(from: &Path, to: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    validate_ancestors(from)?;
    validate_ancestors(to)?;
    let meta = fs::symlink_metadata(from).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || !(meta.is_file() || meta.is_dir()) {
        return Err("Sessão contém um arquivo não regular; cópia antiga preservada.".into());
    }
    if meta.is_dir() {
        fs::create_dir(to).map_err(|e| e.to_string())?;
        fs::set_permissions(to, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        for child in fs::read_dir(from).map_err(|e| e.to_string())? {
            let child = child.map_err(|e| e.to_string())?;
            copy_session(&child.path(), &to.join(child.file_name()))?;
        }
        fs::File::open(to)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
    } else {
        use std::os::unix::fs::OpenOptionsExt;
        let mut input = fs::File::open(from).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(to)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut output)
            .and_then(|_| output.sync_all())
            .map_err(|e| e.to_string())?;
    }
    sync_parent(to)
}
fn preserve_sessions(journal: &Journal) -> Result<(), String> {
    use std::os::unix::fs::DirBuilderExt;
    for entry in &journal.entries {
        let mut preserved = if journal.preserve_accounts {
            session_paths(&entry.original)
        } else {
            vec![]
        };
        if journal.preserve_games
            && entry
                .original
                .file_name()
                .is_some_and(|n| n == "PeliGames" || n == "peligames")
        {
            // Prefer the organized index, accepting backups made by older versions.
            if let Some(relative) = ["library/installed-destinations.json", "installed-destinations.json"]
                .into_iter().find(|relative| exists(&entry.backup.join(relative)))
            {
                preserved.push(relative.into());
            }
        }
        if journal.preserve_mangohud && entry.original.file_name().is_some_and(|name| name == "PeliGames" || name == "peligames") {
            // Keep only user profiles, never downloaded libraries or generated per-game configs.
            for filename in ["profile.json", "custom-presets.json", "MangoHud.conf"] {
                if let Some(relative) = [format!("components/MangoHUD/{filename}"), format!("MangoHUD/{filename}")]
                    .into_iter().find(|relative| exists(&entry.backup.join(relative))) {
                    let meta = fs::symlink_metadata(entry.backup.join(&relative)).map_err(|e|e.to_string())?;
                    if !meta.is_file() || meta.file_type().is_symlink() {
                        return Err("Perfil MangoHUD inválido; a cópia anterior foi preservada para recuperação.".into());
                    }
                    preserved.push(relative.into());
                }
            }
        }
        for relative in preserved {
            let from = entry.backup.join(&relative);
            if !exists(&from) {
                continue;
            }
            validate_ancestors(&entry.original)?;
            if !exists(&entry.original) {
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&entry.original)
                    .map_err(|e| e.to_string())?;
                sync_parent(&entry.original)?;
            }
            let to = if relative.file_name().is_some_and(|name| name == "installed-destinations.json") {
                crate::core::paths::layout::join(&entry.original, "installed-destinations.json")
            } else if relative.starts_with("components/MangoHUD") || relative.starts_with("MangoHUD") {
                crate::core::paths::layout::join(&entry.original, "MangoHUD").join(relative.file_name().ok_or("Perfil MangoHUD inválido.")?)
            } else { entry.original.join(relative) };
            validate_ancestors(&to)?;
            if let Some(parent) = to.parent() {
                fs::DirBuilder::new().recursive(true).mode(0o700).create(parent).map_err(|e|e.to_string())?;
            }
            copy_session(&from, &to)?;
        }
    }
    Ok(())
}
fn stage(dir: &Path, journal: &mut Journal) -> Result<(), String> {
    for entry in &journal.entries {
        rename(&entry.original, &entry.backup)?;
    }
    if journal.preserve_accounts || journal.preserve_games || journal.preserve_mangohud {
        preserve_sessions(journal)?;
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

// A confirmed close operation is complete once staging and preservation succeeded.
// It does not need a GUI boot, and must not recreate a default profile.
fn finish_closed_reset(dir: &Path, journal: &mut Journal) -> Result<(), String> {
    if journal.phase == Phase::Ready {
        if journal.entries.iter().any(|entry| !exists(&entry.backup)) {
            return Err("A limpeza foi interrompida antes de preservar todos os arquivos.".into());
        }
        journal.phase = Phase::Committed;
        save(dir, journal)?;
    }
    collect(dir, journal)
}
fn validate_installation_paths(paths: &[String]) -> Result<(), String> {
    for target in paths {
        let path = Path::new(target);
        if !path.is_absolute() || path.parent().is_none() || path.components().any(|part| matches!(part, std::path::Component::ParentDir)) {
            return Err("Destino de desinstalação inválido.".into());
        }
        validate_ancestors(path)?;
    }
    Ok(())
}
fn remove_installation_paths(paths: &[String]) -> Result<(), String> {
    validate_installation_paths(paths)?;
    for target in paths { delete(Path::new(target))?; }
    Ok(())
}
fn complete_helper_operation(dir: &Path, journal: &mut Journal, mode: &str, uninstall_paths: &[String]) -> Result<bool, String> {
    let closing = matches!(mode, "reset-close" | "uninstall-close");
    if (!matches!(mode, "reset" | "restore" | "reset-close" | "uninstall-close"))
        || (mode != "uninstall-close" && !uninstall_paths.is_empty())
        || (closing && !journal.finish_on_close) {
        return Err("Operação de manutenção inválida.".into());
    }
    if mode == "uninstall-close" { validate_installation_paths(uninstall_paths)?; }
    let result = match (mode, &journal.phase) {
        ("reset" | "reset-close" | "uninstall-close", Phase::Prepared) => stage(dir, journal),
        ("restore", Phase::Restoring) => restore(dir, journal),
        _ => Err("Operação de recuperação inválida.".into()),
    }.and_then(|_| if closing { finish_closed_reset(dir, journal) } else { Ok(()) });
    if let Err(error) = result {
        // A partially collected transaction cannot be rolled back.
        if journal.phase != Phase::Committed { journal.phase = Phase::Recovery; }
        journal.error = Some(error.clone());
        save(dir, journal)?;
        return if closing { Err(error) } else { Ok(true) };
    }
    if mode == "uninstall-close" { remove_installation_paths(uninstall_paths)?; }
    Ok(!closing)
}
fn mark_startup(dir: &Path, journal: &mut Journal, owns_launcher: bool) -> Result<bool, String> {
    if !owns_launcher { return Ok(false); }
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
pub fn begin_startup(owns_launcher: bool) -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|e| e.to_string())?;
    let dir = directory()?;
    let _disk = disk_lock(&dir)?;
    let Some(mut journal) = load(&dir)? else {
        return Ok(());
    };
    if owns_launcher && journal.finish_on_close && matches!(journal.phase, Phase::Ready | Phase::Committed) {
        finish_closed_reset(&dir, &mut journal)?;
        crate::core::paths::pause_migrations(false);
        return Ok(());
    }
    if mark_startup(&dir, &mut journal, owns_launcher)? {
        *BOOT.lock().map_err(|e| e.to_string())? = Some((journal.id.clone(), Instant::now()));
    }
    crate::core::paths::pause_migrations(journal.phase == Phase::Recovery || (!owns_launcher && journal.phase != Phase::Committed));
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
    if (journal.preserve_accounts || journal.preserve_games || journal.preserve_mangohud) && journal.phase == Phase::Recovery {
        return Err("A preservação de contas, jogos ou perfis pode ter sido interrompida. Restaure os arquivos anteriores e tente redefinir novamente para manter os dados com segurança.".into());
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
    crate::core::paths::organize(true)?;
    Ok(())
}
fn spawn_helper(app: &tauri::AppHandle, mode: &str, uninstall_paths: &[String]) -> Result<(), String> {
    // Run through AppRun/the installed command, not its internal ELF: the
    // GTK/WebKit subprocess paths and bundled libraries need their normal setup.
    let binary = crate::core::pelinstall::launcher_binary()?;
    let mut command = Command::new(&binary);
    crate::core::env_utils::host_environment(&mut command);
    command
        .env_remove("APPIMAGE")
        .env_remove("APPDIR")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD")
        .args([HELPER_FLAG, &std::process::id().to_string(), mode])
        .arg(&binary)
        .args(uninstall_paths)
        .current_dir("/")
        .stdin(Stdio::null());
    let log = std::env::temp_dir().join("peligames-reset.log");
    crate::core::bounded_logs::capture(&mut command,&log)?;
    command.spawn().map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
pub fn close(
    app: &tauri::AppHandle,
    paths: &[String],
    preserve_accounts: bool,
    preserve_games: bool,
    preserve_mangohud: bool,
    uninstall_paths: &[String],
) -> Result<(), String> {
    validate_installation_paths(uninstall_paths)?;
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
    let mut paths = paths.to_vec();
    // The stable executable is retained. Only its mutable profiles enter the
    // same rename/recovery journal as preferences, so reset cannot leave stale
    // launch adjustments or break a Steam line pointing at the helper.
    for profiles in super::super::steam_runtime::profile_directories()?.into_iter().chain(std::iter::once(super::super::launch_profiles::profiles_directory()?)) {
        if exists(&profiles) {
            validate_ancestors(&profiles)?;
            let path = profiles.to_string_lossy().into_owned();
            if !paths.contains(&path) { paths.push(path); }
        }
    }
    let mut journal = new_journal(&paths);
    journal.preserve_accounts = preserve_accounts;
    journal.preserve_games = preserve_games;
    journal.preserve_mangohud = preserve_mangohud;
    journal.finish_on_close = true;
    save(&dir, &journal)?;
    if let Err(error) = spawn_helper(app, if uninstall_paths.is_empty() { "reset-close" } else { "uninstall-close" }, uninstall_paths) {
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
    if let Err(error) = spawn_helper(&app, "restore", &[]) {
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
    let reopen = complete_helper_operation(&dir, &mut journal, mode, &args[3..])?;
    if !reopen { return Ok(()); }
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(binary);
    crate::core::env_utils::host_environment(&mut command);
    Err(command.current_dir("/").exec().to_string())
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
    fn close_reset_cleans_without_recreating_profiles_or_requesting_reopen() {
        let mut f = Fixture::new();
        f.journal.finish_on_close = true;
        assert!(!complete_helper_operation(&f.root, &mut f.journal, "reset-close", &[]).unwrap());
        assert_eq!(f.journal.phase, Phase::Committed);
        assert!(!f.root.join("reset.json").exists());
        for entry in &f.journal.entries {
            assert!(!exists(&entry.original)); assert!(!exists(&entry.backup)); assert!(!exists(&entry.discarded));
        }
    }
    #[test]
    fn close_reset_preserves_only_selected_mangohud_files() {
        let mut f = mangohud_fixture("MangoHUD", true);
        f.journal.finish_on_close = true;
        assert!(!complete_helper_operation(&f.root, &mut f.journal, "reset-close", &[]).unwrap());
        let kept = f.journal.entries[0].original.join("components/MangoHUD");
        assert!(kept.join("custom-presets.json").is_file());
        assert!(!kept.join("libMangoHud.so").exists());
        assert!(!f.journal.entries[1].original.exists());
        assert!(!f.root.join("reset.json").exists());
    }
    #[test]
    fn uninstall_close_removes_installation_and_data_but_keeps_external_games() {
        let mut f = Fixture::new(); f.journal.finish_on_close = true;
        let install=f.root.join("installation"); fs::create_dir(&install).unwrap();
        fs::write(install.join("AppRun"), b"launcher").unwrap();
        let game=f.root.join("external-game"); fs::create_dir(&game).unwrap();
        fs::write(game.join("save"), b"keep").unwrap();
        assert!(!complete_helper_operation(&f.root, &mut f.journal, "uninstall-close", &[install.to_string_lossy().into_owned()]).unwrap());
        assert!(!install.exists()); assert_eq!(fs::read(game.join("save")).unwrap(), b"keep");
        for entry in &f.journal.entries { assert!(!exists(&entry.original)); }
    }
    #[test]
    fn invalid_uninstall_is_rejected_before_staging_or_removing_data() {
        let mut f=Fixture::new(); f.journal.finish_on_close=true;
        assert!(complete_helper_operation(&f.root, &mut f.journal, "uninstall-close", &["/".into()]).is_err());
        assert_eq!(f.journal.phase, Phase::Prepared);
        for entry in &f.journal.entries {assert!(entry.original.is_dir());assert!(!exists(&entry.backup));}
    }
    #[test]
    fn close_preservation_failure_keeps_recoverable_backups_and_does_not_reopen() {
        let mut f=mangohud_fixture("MangoHUD", true); f.journal.finish_on_close=true;
        let profile=f.journal.entries[0].original.join("MangoHUD/profile.json");
        fs::remove_file(&profile).unwrap(); std::os::unix::fs::symlink("missing", &profile).unwrap();
        assert!(complete_helper_operation(&f.root, &mut f.journal,"reset-close",&[]).is_err());
        assert_eq!(f.journal.phase,Phase::Recovery);
        assert!(f.root.join("reset.json").exists());
        restore(&f.root,&mut f.journal).unwrap();
        assert!(profile.is_symlink());
    }
    #[test]
    fn interrupted_closed_cleanup_can_finish_without_starting_gui() {
        let mut f=Fixture::new(); f.journal.finish_on_close=true;
        stage(&f.root,&mut f.journal).unwrap();
        finish_closed_reset(&f.root,&mut f.journal).unwrap();
        for entry in &f.journal.entries {assert!(!exists(&entry.original));assert!(!exists(&entry.backup));}
        assert!(!f.root.join("reset.json").exists());
    }
    #[test]
    fn legacy_reset_still_requests_recovery_startup_and_defaults_to_old_mode() {
        let mut f=Fixture::new();
        let mut value=serde_json::to_value(&f.journal).unwrap(); value.as_object_mut().unwrap().remove("finish_on_close");
        assert!(!serde_json::from_value::<Journal>(value).unwrap().finish_on_close);
        assert!(complete_helper_operation(&f.root,&mut f.journal,"reset",&[]).unwrap());
        assert_eq!(f.journal.phase,Phase::Ready);
        assert!(f.root.join("reset.json").exists());
    }
    fn mangohud_fixture(layout: &str, preserve: bool) -> Fixture {
        let mut f = Fixture::new();
        let entry = &mut f.journal.entries[0];
        let original = entry.original.with_file_name("PeliGames");
        fs::rename(&entry.original, &original).unwrap();
        entry.original = original;
        entry.backup = sibling(&entry.original, &f.journal.id, "reset");
        entry.discarded = sibling(&entry.original, &f.journal.id, "replaced");
        let folder = entry.original.join(layout);
        fs::create_dir_all(folder.join("configs")).unwrap();
        fs::write(folder.join("profile.json"), br#"{"fps":true}"#).unwrap();
        fs::write(folder.join("custom-presets.json"), br#"[{"name":"Meu perfil","settings":{"fps":true}}]"#).unwrap();
        fs::write(folder.join("MangoHud.conf"), b"fps\n").unwrap();
        fs::write(folder.join("libMangoHud.so"), b"downloaded component").unwrap();
        fs::write(folder.join("configs/game.conf"), b"generated").unwrap();
        f.journal.preserve_mangohud = preserve;
        f
    }
    #[test]
    fn mangohud_profiles_survive_reset_without_accounts_or_games_in_both_layouts() {
        for layout in ["MangoHUD", "components/MangoHUD"] {
            let mut f = mangohud_fixture(layout, true);
            stage(&f.root, &mut f.journal).unwrap();
            let entry = &f.journal.entries[0];
            let kept = entry.original.join("components/MangoHUD");
            for file in ["profile.json", "custom-presets.json", "MangoHud.conf"] {
                assert_eq!(fs::read(kept.join(file)).unwrap(), fs::read(entry.backup.join(layout).join(file)).unwrap());
            }
            assert!(!kept.join("libMangoHud.so").exists());
            assert!(!kept.join("configs").exists());
            assert!(!entry.original.join("mods").exists());
            let saved: Journal = serde_json::from_slice(&fs::read(f.root.join("reset.json")).unwrap()).unwrap();
            assert!(saved.preserve_mangohud);
            f.journal.phase = Phase::Committed;
            collect(&f.root, &f.journal).unwrap();
            assert!(kept.join("custom-presets.json").is_file());
        }
    }
    #[test]
    fn mangohud_preservation_is_optional_and_originals_stay_recoverable() {
        for layout in ["MangoHUD", "components/MangoHUD"] {
            let mut f = mangohud_fixture(layout, false);
            stage(&f.root, &mut f.journal).unwrap();
            assert!(!f.journal.entries[0].original.exists());
            assert!(f.journal.entries[0].backup.join(layout).join("custom-presets.json").is_file());
            restore(&f.root, &mut f.journal).unwrap();
            assert!(f.journal.entries[0].original.join(layout).join("custom-presets.json").is_file());
            let mut old = serde_json::to_value(&f.journal).unwrap();
            old.as_object_mut().unwrap().remove("preserve_mangohud");
            assert!(!serde_json::from_value::<Journal>(old).unwrap().preserve_mangohud);
        }
    }
    #[test]
    fn mangohud_preservation_rejects_links_and_leaves_backups_for_recovery() {
        let mut f = mangohud_fixture("MangoHUD", true);
        let profile = f.journal.entries[0].original.join("MangoHUD/profile.json");
        fs::remove_file(&profile).unwrap();
        let external = f.root.join("external-profile.json");
        fs::write(&external, b"do not touch").unwrap();
        std::os::unix::fs::symlink(&external, &profile).unwrap();
        assert!(stage(&f.root, &mut f.journal).unwrap_err().contains("MangoHUD"));
        assert_eq!(fs::read(external).unwrap(), b"do not touch");
        assert!(f.journal.entries[0].backup.join("MangoHUD/profile.json").is_symlink());
        restore(&f.root, &mut f.journal).unwrap();
        assert!(f.journal.entries[0].original.join("MangoHUD/profile.json").is_symlink());
    }
    #[test]
    fn mangohud_organized_profiles_take_priority_over_legacy_files() {
        let mut f = mangohud_fixture("MangoHUD", true);
        let modern = f.journal.entries[0].original.join("components/MangoHUD");
        fs::create_dir_all(&modern).unwrap();
        fs::write(modern.join("profile.json"), b"modern profile").unwrap();
        stage(&f.root, &mut f.journal).unwrap();
        assert_eq!(fs::read(modern.join("profile.json")).unwrap(), b"modern profile");
        assert!(modern.join("custom-presets.json").is_file());
    }
    #[test]
    fn game_preservation_is_independent_of_accounts_and_keeps_only_the_index() {
        let mut f = Fixture::new();
        let entry = &mut f.journal.entries[0];
        let target = entry.original.with_file_name("PeliGames");
        fs::rename(&entry.original, &target).unwrap();
        entry.original = target;
        entry.backup = sibling(&entry.original, &f.journal.id, "reset");
        entry.discarded = sibling(&entry.original, &f.journal.id, "replaced");
        fs::write(
            entry.original.join("installed-destinations.json"),
            b"[\"/offline/Game\"]",
        )
        .unwrap();
        fs::create_dir(entry.original.join("nexus-browser")).unwrap();
        fs::write(entry.original.join("nexus-browser/cookie"), b"fake").unwrap();
        f.journal.preserve_games = true;
        stage(&f.root, &mut f.journal).unwrap();
        let entry = &f.journal.entries[0];
        assert_eq!(
            fs::read(entry.original.join("library/installed-destinations.json")).unwrap(),
            b"[\"/offline/Game\"]"
        );
        assert!(!entry.original.join("nexus-browser").exists());
        assert!(entry.backup.join("nexus-browser/cookie").is_file());
        f.journal.phase = Phase::Committed;
        collect(&f.root, &f.journal).unwrap();
        assert!(f.journal.entries[0]
            .original
            .join("library/installed-destinations.json")
            .is_file());
    }
    #[test]
    fn account_preservation_copies_only_sessions_and_keeps_recovery_backups() {
        let mut f = Fixture::new();
        for entry in &mut f.journal.entries {
            let name = if entry.original.ends_with("config") {
                "PeliGames"
            } else {
                "com.pelicano.peligames"
            };
            let target = entry.original.with_file_name(name);
            fs::rename(&entry.original, &target).unwrap();
            entry.original = target;
            entry.backup = sibling(&entry.original, &f.journal.id, "reset");
            entry.discarded = sibling(&entry.original, &f.journal.id, "replaced");
            let store = if name == "PeliGames" {
                "nexus-browser"
            } else {
                "databases"
            };
            fs::create_dir_all(entry.original.join(store)).unwrap();
            fs::write(
                entry.original.join(store).join("session"),
                b"fake encrypted session",
            )
            .unwrap();
            fs::write(entry.original.join("preferences.json"), b"old preferences").unwrap();
        }
        f.journal.preserve_accounts = true;
        stage(&f.root, &mut f.journal).unwrap();
        for entry in &f.journal.entries {
            let store = if entry.original.ends_with("PeliGames") {
                "nexus-browser"
            } else {
                "databases"
            };
            assert_eq!(
                fs::read(entry.original.join(store).join("session")).unwrap(),
                b"fake encrypted session"
            );
            assert!(!entry.original.join("preferences.json").exists());
            assert!(entry.backup.join("preferences.json").exists());
        }
        restore(&f.root, &mut f.journal).unwrap();
        assert!(f.journal.entries[0]
            .original
            .join("preferences.json")
            .exists());
    }
    #[test]
    fn organized_reset_preserves_nested_sessions_and_the_registered_games() {
        let mut f = Fixture::new();
        let entry = &mut f.journal.entries[0];
        let target = entry.original.with_file_name("PeliGames");
        fs::rename(&entry.original, &target).unwrap();
        entry.original = target;
        entry.backup = sibling(&entry.original, &f.journal.id, "reset");
        entry.discarded = sibling(&entry.original, &f.journal.id, "replaced");
        for relative in ["webview/nexus-browser/cookie", "webview/storage/session", "library/installed-destinations.json"] {
            let path = entry.original.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, relative).unwrap();
        }
        fs::create_dir_all(entry.original.join("settings")).unwrap();
        fs::write(entry.original.join("settings/config.json"), b"old preferences").unwrap();
        f.journal.preserve_accounts = true;
        f.journal.preserve_games = true;
        stage(&f.root, &mut f.journal).unwrap();
        let entry = &f.journal.entries[0];
        for relative in ["webview/nexus-browser/cookie", "webview/storage/session", "library/installed-destinations.json"] {
            assert_eq!(fs::read_to_string(entry.original.join(relative)).unwrap(), relative);
            assert!(entry.backup.join(relative).is_file());
        }
        assert!(!entry.original.join("settings/config.json").exists());
        assert!(mark_startup(&f.root, &mut f.journal, true).unwrap());
        // A helper starting while the UI validates must not turn Checking into Recovery.
        assert!(!mark_startup(&f.root, &mut f.journal, false).unwrap());
        assert_eq!(f.journal.phase, Phase::Checking);
        restore(&f.root, &mut f.journal).unwrap();
        assert_eq!(fs::read(&f.journal.entries[0].original.join("settings/config.json")).unwrap(), b"old preferences");
    }
    #[test]
    fn background_startup_does_not_consume_the_reset_boot_attempt() {
        let mut f = Fixture::new();
        stage(&f.root, &mut f.journal).unwrap();
        assert!(!mark_startup(&f.root, &mut f.journal, false).unwrap());
        assert_eq!(f.journal.phase, Phase::Ready);
        let saved: Journal = serde_json::from_slice(&fs::read(f.root.join("reset.json")).unwrap()).unwrap();
        assert_eq!(saved.phase, Phase::Ready);
        assert!(mark_startup(&f.root, &mut f.journal, true).unwrap());
    }
    #[test]
    fn nested_session_links_fail_without_touching_the_external_directory() {
        let mut f = Fixture::new();
        let entry = &mut f.journal.entries[0];
        let target = entry.original.with_file_name("PeliGames");
        fs::rename(&entry.original, &target).unwrap();
        entry.original = target;
        entry.backup = sibling(&entry.original, &f.journal.id, "reset");
        entry.discarded = sibling(&entry.original, &f.journal.id, "replaced");
        let outside = f.root.join("external-webview");
        fs::create_dir_all(outside.join("nexus-browser")).unwrap();
        fs::write(outside.join("nexus-browser/cookie"), b"untouched").unwrap();
        std::os::unix::fs::symlink(&outside, entry.original.join("webview")).unwrap();
        f.journal.preserve_accounts = true;
        assert!(stage(&f.root, &mut f.journal).is_err());
        assert_eq!(fs::read(outside.join("nexus-browser/cookie")).unwrap(), b"untouched");
        assert!(f.journal.entries[0].backup.join("mods/nested/keep").is_file());
        restore(&f.root, &mut f.journal).unwrap();
    }
    #[test]
    fn preserving_accounts_rejects_links_without_losing_original_data() {
        let f = Fixture::new();
        let source = f.root.join("session-source");
        fs::create_dir(&source).unwrap();
        std::os::unix::fs::symlink(f.root.join("config"), source.join("unexpected-link")).unwrap();
        assert!(copy_session(&source, &f.root.join("copy")).is_err());
        assert!(f.root.join("config/mods/nested/keep").exists());
        // Old journals keep the original disconnect-and-reset behavior.
        let mut value = serde_json::to_value(&f.journal).unwrap();
        value.as_object_mut().unwrap().remove("preserve_accounts");
        assert!(
            !serde_json::from_value::<Journal>(value)
                .unwrap()
                .preserve_accounts
        );
    }
    #[test]
    fn a_crash_before_health_confirmation_offers_recovery_on_the_next_boot() {
        let mut f = Fixture::new();
        stage(&f.root, &mut f.journal).unwrap();
        assert!(mark_startup(&f.root, &mut f.journal, true).unwrap());
        // Re-read only the durable journal, as a fresh process after power loss would.
        let mut persisted: Journal =
            serde_json::from_slice(&fs::read(f.root.join("reset.json")).unwrap()).unwrap();
        assert_eq!(persisted.phase, Phase::Checking);
        assert!(!mark_startup(&f.root, &mut persisted, true).unwrap());
        assert_eq!(persisted.phase, Phase::Recovery);
        assert!(collect(&f.root, &persisted).is_err());
        for entry in &persisted.entries {
            assert!(entry.backup.join("mods/nested/keep").is_file());
        }
        restore(&f.root, &mut persisted).unwrap();
        assert!(mark_startup(&f.root, &mut persisted, true).unwrap());
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
