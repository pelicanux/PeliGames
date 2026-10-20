//! Lifecycle of launcher-owned Proton sessions isolated by prefix. No global Wine/Steam kills.
use super::game_installation::PreparedRun;
use serde::{Serialize, Deserialize};
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExecutionStatus {
    pub id: String,
    pub path: String,
    pub state: String,
    pub log: String,
    pub error: Option<String>,
}
impl ExecutionStatus {
    pub fn active(&self) -> bool {
        matches!(self.state.as_str(), "starting" | "running" | "stopping")
    }
}
struct Session {
    status: Mutex<ExecutionStatus>,
    cancel: AtomicBool,
}
static SESSION: Mutex<Option<Arc<Session>>> = Mutex::new(None);
static OWNED_SESSIONS: Mutex<Vec<Arc<Session>>> = Mutex::new(Vec::new());
pub(crate) fn local_status() -> Result<Option<ExecutionStatus>, String> {
    let slot = SESSION.lock().map_err(|e| e.to_string())?;
    slot.as_ref()
        .map(|session| {
            session
                .status
                .lock()
                .map(|s| s.clone())
                .map_err(|e| e.to_string())
        })
        .transpose()
}
pub fn statuses()->Result<Vec<ExecutionStatus>,String>{
    let mut result=super::execution_registry::statuses_at(&super::execution_registry::directory()?);
    if let Some(local)=local_status()? {result.retain(|s|s.id!=local.id);result.push(local);}
    for session in OWNED_SESSIONS.lock().map_err(|e|e.to_string())?.iter(){
        let local=session.status.lock().map_err(|e|e.to_string())?.clone();
        result.retain(|s|s.id!=local.id);result.push(local);
    }
    Ok(result)
}
pub fn status()->Result<Option<ExecutionStatus>,String>{
    let all=statuses()?;
    Ok(all.iter().rev().find(|s|s.active()).cloned().or_else(||all.last().cloned()))
}

pub fn active() -> bool {
    status().ok().flatten().is_some_and(|s| s.active())
}
pub fn start(path: String) -> Result<ExecutionStatus, String> {
    start_program(path, None)
}
pub fn start_program(path: String, executable: Option<String>) -> Result<ExecutionStatus, String> {
    start_program_with(path, executable, Vec::new())
}
/// Prefix of the library entry whose session is running, if any.
pub fn running_prefix() -> Option<String> {
    let state = status().ok().flatten().filter(|state| state.active())?;
    super::wine_tools::maintenance_entry(&state.path).ok().map(|(_, prefix, _, _)| prefix)
}
/// Opens another program inside a prefix that is already running (Cheat Engine
/// beside the game): Proton joins the running Wine instead of waiting for it to
/// end, and the game's session is left as it is.
pub fn run_in_prefix(path: String, executable: String, arguments: Vec<String>) -> Result<(), String> {
    let mut prepared = super::game_installation::prepare_launch_program(path, Some(executable), arguments)?;
    super::bounded_logs::capture(&mut prepared.command,&prepared.log)?;
    prepared.command.env("PROTON_VERB", "runinprefix");
    let mut child = prepared.command.spawn().map_err(|e| format!("Não foi possível abrir o programa: {e}"))?;
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(())
}
pub fn start_program_with(path: String, executable: Option<String>, arguments: Vec<String>) -> Result<ExecutionStatus, String> {
    if super::wine_tools::busy() { return Err("Uma ferramenta Wine está em execução.".into()); }
    let mut sessions=OWNED_SESSIONS.lock().map_err(|e|e.to_string())?;
    let registry=super::execution_registry::directory()?;
    let current=sessions.iter().filter_map(|session|session.status.lock().ok().map(|s|s.clone()))
        .chain(super::execution_registry::statuses_at(&registry)).find(|s|s.active()&&s.path==path);
    if let Some(current)=current {
        if executable.is_none(){return Ok(current);}
        return Err("Esta entrada já está em execução. Abra ferramentas pelo próprio prefixo.".into());
    }
    let may_reuse = executable.is_none();
    let prepared = super::game_installation::prepare_launch_program(path.clone(), executable, arguments)?;
    let session = match start_prepared_registered(path.clone(), prepared, true, Some(registry.clone())) {
        Ok(session) => session,
        Err(error) => {
            // Two desktop invocations can race before the first monitor publishes its state.
            if may_reuse && error.contains("já usa este prefixo") {
                for _ in 0..10 {
                    if let Some(state) = super::execution_registry::statuses_at(&registry).into_iter().find(|s|s.path==path) { return Ok(state); }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
            return Err(error);
        }
    };
    let initial = session.status.lock().map_err(|e| e.to_string())?.clone();
    sessions.retain(|s|s.status.lock().map(|s|s.active()).unwrap_or(true));
    sessions.push(session);
    Ok(initial)
}
pub fn cancel(id: &str) -> Result<ExecutionStatus, String> {
    let primary=SESSION.lock().map_err(|e|e.to_string())?.clone();
    let sessions=OWNED_SESSIONS.lock().map_err(|e|e.to_string())?;
    let session=sessions.iter().chain(primary.iter()).find(|s|s.status.lock().is_ok_and(|s|s.id==id));
    let Some(session)=session else{return super::execution_registry::cancel_at(&super::execution_registry::directory()?,id);};
    let mut status = session.status.lock().map_err(|e| e.to_string())?;
    if status.id != id {
        return Err("Esta execução já foi substituída.".into());
    }
    if status.active() {
        status.state = "stopping".into();
        session.cancel.store(true, Ordering::Release);
    }
    Ok(status.clone())
}

// An installer has its own cancellation token, independent of the library's game session.
pub(crate) fn wait_for_installer(prepared: PreparedRun, cancel: &AtomicBool) -> Result<ExecutionStatus, String> {
    let session = start_prepared("installer".into(), prepared)?;
    loop {
        if cancel.load(Ordering::Acquire) { session.cancel.store(true, Ordering::Release); }
        let state = session.status.lock().map_err(|e| e.to_string())?.clone();
        if !state.active() { return Ok(state); }
        std::thread::sleep(Duration::from_millis(100));
    }
}
// wineboot returning successfully signals that prefix initialization/update completed.
// Unlike an installer, it does not need to wait for every background Wine service.
pub(crate) fn wait_for_prefix_update(prepared: PreparedRun, cancel: &AtomicBool) -> Result<ExecutionStatus, String> {
    let session = start_prepared_mode("prefix-update".into(), prepared, false)?;
    loop {
        if cancel.load(Ordering::Acquire) { session.cancel.store(true, Ordering::Release); }
        let state = session.status.lock().map_err(|e| e.to_string())?.clone();
        if !state.active() { return Ok(state); }
        std::thread::sleep(Duration::from_millis(100));
    }
}
pub(crate) fn prefix_in_use(prefix: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        processes(prefix, Path::new(""))
            .iter()
            .any(|process| process.prefix)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = prefix;
        active()
    }
}
#[cfg(target_os = "linux")]
#[derive(Clone)]
struct Process {
    pid: i32,
    parent: i32,
    group: i32,
    birth: u64,
    prefix: bool,
    executable: bool,
    application: bool,
}
#[cfg(target_os = "linux")]
fn processes(prefix: &Path, executable: &Path) -> Vec<Process> {
    let prefix_env = format!("WINEPREFIX={}", prefix.display());
    let name = if executable
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("msi"))
    {
        std::borrow::Cow::Borrowed("msiexec.exe")
    } else {
        executable.file_name().unwrap_or_default().to_string_lossy()
    };
    let mut result = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return result;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|s| s.parse::<i32>().ok())
        else {
            continue;
        };
        let directory = entry.path();
        let Ok(stat) = fs::read_to_string(directory.join("stat")) else {
            continue;
        };
        let Some((_, fields)) = stat.rsplit_once(") ") else {
            continue;
        };
        let fields: Vec<_> = fields.split_whitespace().collect();
        if fields.len() < 20 || fields[0] == "Z" {
            continue;
        }
        let (Ok(parent), Ok(group), Ok(birth)) =
            (fields[1].parse(), fields[2].parse(), fields[19].parse())
        else {
            continue;
        };
        let env = fs::read(directory.join("environ")).unwrap_or_default();
        let owns_prefix = env.split(|b| *b == 0).any(|s| s == prefix_env.as_bytes());
        let cmdline = fs::read(directory.join("cmdline")).unwrap_or_default();
        let first = String::from_utf8_lossy(cmdline.split(|b| *b == 0).next().unwrap_or_default());
        let comm = fs::read_to_string(directory.join("comm")).unwrap_or_default();
        let is_executable = first
            .rsplit(['/', '\\'])
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case(&name))
            || comm.trim().eq_ignore_ascii_case(&name);
        let app_name = first.rsplit(['/', '\\']).next().unwrap_or_default().to_ascii_lowercase();
        let application = app_name.ends_with(".exe") && !["explorer.exe", "services.exe", "winedevice.exe", "rpcss.exe", "wineboot.exe", "conhost.exe", "plugplay.exe"].contains(&app_name.as_str());
        result.push(Process {
            pid,
            parent,
            group,
            birth,
            prefix: owns_prefix,
            executable: is_executable,
            application,
        });
    }
    result
}
#[cfg(target_os = "linux")]
fn start_prepared(path: String, prepared: PreparedRun) -> Result<Arc<Session>, String> { start_prepared_mode(path, prepared, true) }
#[cfg(target_os = "linux")]
fn start_prepared_mode(path: String, prepared: PreparedRun, wait_for_children: bool) -> Result<Arc<Session>, String> { start_prepared_registered(path,prepared,wait_for_children,None) }
#[cfg(target_os = "linux")]
fn start_prepared_registered(path: String, mut prepared: PreparedRun, wait_for_children: bool, registry: Option<std::path::PathBuf>) -> Result<Arc<Session>, String> {
    use std::os::unix::process::CommandExt;
    let lease = super::game_installation::lock_prefix(&prepared.prefix)?;
    if processes(&prepared.prefix, &prepared.executable)
        .iter()
        .any(|p| p.prefix)
    {
        return Err(
            "Já existe um processo usando este prefixo. Feche-o antes de iniciar pelo launcher."
                .into(),
        );
    }
    if registry.is_some() { super::game_logs::prepare(&path, &mut prepared)?; }
    super::logger::event("INFO",&format!("Iniciando {}: Proton={}, log={}",prepared.name,prepared.command.get_envs().find(|(key,_)|*key=="PROTONPATH").and_then(|(_,value)|value).unwrap_or(std::ffi::OsStr::new("padrão")).to_string_lossy(),prepared.log.display()));
    prepared.command.process_group(0);
    let mut child = prepared
        .command
        .spawn()
        .map_err(|e| {super::logger::event("ERROR",&format!("Falha ao iniciar {}: {e}",prepared.name));format!("Falha ao iniciar: {e}. Log: {}", prepared.log.display())})?;
    let root = child.id() as i32;
    let session = Arc::new(Session {
        status: Mutex::new(ExecutionStatus {
            id: uuid::Uuid::new_v4().to_string(),
            path,
            state: "starting".into(),
            log: prepared.log.display().to_string(),
            error: None,
        }),
        cancel: AtomicBool::new(false),
    });
    if let Some(dir) = &registry {
        if let Err(error) = super::execution_registry::publish(dir,&*session.status.lock().map_err(|e|e.to_string())?) {
            unsafe {libc::kill(-root,libc::SIGKILL);} let _=child.wait();return Err(error);
        }
    }
    let worker = session.clone();
    std::thread::spawn(move || {
        let _lease = lease;
        let mut published = worker.status.lock().ok().map(|s|s.clone());
        let mut tracked = BTreeSet::<(i32, u64)>::new();
        let mut responded = None;
        let mut playing: Option<(std::time::SystemTime, Instant)> = None;
        let mut stopping = None;
        let mut exit = None;
        let mut failure = None;
        loop {
            if let Some(dir) = &registry {
                if let Ok(mut status) = worker.status.lock() {
                    if super::execution_registry::cancellation_requested(dir,&status.id) {worker.cancel.store(true,Ordering::Release);status.state="stopping".into();}
                    if published.as_ref()!=Some(&*status) {
                        if super::execution_registry::publish(dir,&status).is_ok() {published=Some(status.clone());}
                    }
                }
            }
            let all = processes(&prepared.prefix, &prepared.executable);
            // Discover descendants repeatedly, including processes reparented after detection.
            loop {
                let before = tracked.len();
                for p in &all {
                    if (p.group == root && (exit.is_none() || tracked.contains(&(p.pid, p.birth))))
                        || p.prefix
                        || all.iter().any(|parent| {
                            parent.pid == p.parent && tracked.contains(&(parent.pid, parent.birth))
                        })
                    {
                        tracked.insert((p.pid, p.birth));
                    }
                }
                if tracked.len() == before {
                    break;
                }
            }
            let owned: Vec<_> = all
                .iter()
                .filter(|p| tracked.contains(&(p.pid, p.birth)))
                .collect();
            if worker.cancel.load(Ordering::Acquire) {
                stopping.get_or_insert_with(Instant::now);
                // SIGKILL targets this new process group and its known/prefix-owned descendants.
                if owned.iter().any(|p| p.group == root) {
                    unsafe {
                        libc::kill(-root, libc::SIGKILL);
                    }
                }
                for p in &owned {
                    // Re-read birth time before signalling, avoiding a reused PID.
                    if processes(&prepared.prefix, &prepared.executable)
                        .iter()
                        .any(|now| now.pid == p.pid && now.birth == p.birth)
                    {
                        if unsafe { libc::kill(p.pid, libc::SIGKILL) } != 0 {
                            let error = std::io::Error::last_os_error();
                            if error.raw_os_error() != Some(libc::ESRCH) {
                                failure =
                                    Some(format!("Falha ao encerrar processo {}: {error}", p.pid));
                            }
                        }
                    }
                }
            } else if owned.iter().any(|p| p.executable || (p.pid != root && p.application)) {
                let since = responded.get_or_insert_with(Instant::now);
                if since.elapsed() >= Duration::from_millis(500) {
                    if let Ok(mut status) = worker.status.lock() {
                        if status.state == "starting" {
                            status.state = "running".into();
                            playing.get_or_insert_with(|| (std::time::SystemTime::now(), Instant::now()));
                        }
                    }
                }
            } else {
                responded = None;
            }
            if exit.is_none() {
                match child.try_wait() {
                    Ok(value) => exit = value,
                    Err(error) => {
                        failure = Some(error.to_string());
                        break;
                    }
                }
            }
            if exit.is_some() && (owned.is_empty() || (!wait_for_children && !worker.cancel.load(Ordering::Acquire))) {
                break;
            }
            if stopping.is_some_and(|since| since.elapsed() > Duration::from_secs(10)) {
                if let Ok(mut status) = worker.status.lock() {
                    status.error =
                        Some("Aguardando o sistema encerrar os processos restantes.".into());
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let cancelled = worker.cancel.load(Ordering::Acquire);
        if !cancelled && exit.is_some_and(|s| !s.success()) {
            failure = Some(format!(
                "Proton encerrou com código {:?}. Log: {}",
                exit.and_then(|s| s.code()),
                prepared.log.display()
            ));
        }
        if failure.is_some() {
            if let Some(hint)=crate::commands::runtime_activity::failure_hint(&prepared.log) {
                failure=Some(format!("{hint}\n{}",failure.unwrap_or_default()));
            }
        }
        super::logger::event(if failure.is_some(){"ERROR"}else{"INFO"},&format!("Execução de {}: {}; código de saída={:?}; log={}",prepared.name,if failure.is_some(){"falhou"}else if cancelled{"cancelada"}else{"encerrada"},exit.and_then(|s|s.code()),prepared.log.display()));
        if let Ok(mut status) = worker.status.lock() {
            status.state = if failure.is_some() {
                "failed"
            } else if cancelled {
                "cancelled"
            } else {
                "exited"
            }
            .into();
            status.error = failure;
            if let Some(dir) = &registry { super::execution_registry::finish(dir,&status.id); }
            // Only game sessions (registered) count as playtime, not tools.
            if let (Some(_), Some((started, since))) = (&registry, playing) {
                super::playtime::record(&status.path, started, since.elapsed());
            }
        }
    });
    Ok(session)
}
#[cfg(not(target_os = "linux"))]
fn start_prepared_mode(_: String, _: PreparedRun, _: bool) -> Result<Arc<Session>, String> { Err("Monitoramento de Proton disponível no Linux.".into()) }
#[cfg(not(target_os = "linux"))]
fn start_prepared(_: String, _: PreparedRun) -> Result<Arc<Session>, String> {
    Err("Monitoramento de Proton disponível no Linux.".into())
}

#[cfg(target_os = "linux")]
pub mod steam;
