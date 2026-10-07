//! Windows installer execution, shared by Tauri and the development service.
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}},
};

struct InstallationSession { cancel: Arc<AtomicBool>, accepting_cancel: bool }
static INSTALLATION: Mutex<Option<InstallationSession>> = Mutex::new(None);
struct InstallationGuard;
impl Drop for InstallationGuard {
    fn drop(&mut self) { if let Ok(mut slot) = INSTALLATION.lock() { *slot = None; } }
}
pub fn cancel_installation() -> Result<(), String> {
    let slot = INSTALLATION.lock().map_err(|e| e.to_string())?;
    let session = slot.as_ref().ok_or("Nenhuma instalação ativa para cancelar.")?;
    if !session.accepting_cancel { return Err("O instalador já encerrou e o registro está sendo finalizado. Aguarde.".into()); }
    session.cancel.store(true, Ordering::Release);
    Ok(())
}

#[derive(Clone, Deserialize)]
pub struct InstallationRequest {
    pub name: String,
    pub directory: String,
    pub executable: String,
    pub proton: String,
    #[serde(default)]
    pub cover_url: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct InstallationResult {
    pub prefix: String,
    pub log: String,
    pub registered_count: usize,
    pub library_error: Option<String>,
}

pub(crate) fn absolute(value: &str) -> Result<PathBuf, String> {
    let value = value.trim();
    let path = if let Some(rest) = value.strip_prefix("~/") {
        dirs::home_dir()
            .ok_or("Pasta pessoal indisponível.")?
            .join(rest)
    } else {
        PathBuf::from(value)
    };
    if !path.is_absolute() {
        return Err("Use caminhos absolutos para o destino, o executável e o Proton.".into());
    }
    Ok(path)
}
fn executable_file(path: &Path) -> bool {
    #[cfg(unix)]
    {
        path.is_file()
            && fs::metadata(path)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}
pub(crate) fn find_umu() -> Result<PathBuf, String> {
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            let candidate = directory.join("umu-run");
            if executable_file(&candidate) {
                return candidate.canonicalize().map_err(|e| e.to_string());
            }
        }
    }
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".local/bin/umu-run");
        if executable_file(&candidate) {
            return Ok(candidate);
        }
    }
    super::umu_manager::ensure().map_err(|error| format!("Não foi possível preparar o UMU automaticamente: {error}"))
}
#[derive(Deserialize)]
pub struct RepairRequest { pub path: String, pub name: String, pub proton: String, pub prefix: String, pub executable: String }
pub fn repair(request: RepairRequest) -> Result<InstallationResult, String> {
    let entry = super::installed_library::list(false)?.into_iter().find(|entry| entry.path == request.path)
        .ok_or("Jogo não registrado na biblioteca.")?;
    let selected = absolute(&request.prefix)?;
    let prefix = if selected == Path::new(&entry.prefix) {
        if fs::symlink_metadata(&selected).is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()) { return Err("Prefixo inválido.".into()); }
        selected
    } else { super::installed_library::validate_selected_prefix(&request.prefix)? };
    let installation = InstallationRequest { name: request.name, proton: request.proton, executable: request.executable,
        directory: prefix.parent().ok_or("Prefixo inválido.")?.to_string_lossy().into_owned(), cover_url: entry.cover_url.clone() };
    run_inner(installation, Some((entry, prefix)))
}
pub fn run(request: InstallationRequest) -> Result<InstallationResult, String> { run_inner(request, None) }
fn run_inner(request: InstallationRequest, repair: Option<(super::installed_library::InstalledEntry, PathBuf)>) -> Result<InstallationResult, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = INSTALLATION.lock().map_err(|e| e.to_string())?;
        if slot.is_some() { return Err("Um instalador já está em execução.".into()); }
        *slot = Some(InstallationSession { cancel: cancel.clone(), accepting_cancel: true });
    }
    let _guard = InstallationGuard;
    if cancel.load(Ordering::Acquire) { return Err("Operação cancelada.".into()); }
    let mut result = if let Some((entry,prefix)) = &repair {
        let proton = validate_proton(&request.proton)?;
        validate_executable(&request.executable)?;
        if Path::new(&entry.proton).canonicalize().ok().as_ref() == Some(&proton) && prefix == Path::new(&entry.prefix) {
            InstallationResult { prefix: prefix.to_string_lossy().into_owned(), log: String::new(), registered_count: 0, library_error: None }
        } else {
            let prepared = prepare_run_mode(request.clone(), &find_umu()?, Some(prefix), None, true)?;
            let prefix = prepared.prefix.to_string_lossy().into_owned(); let log = prepared.log.to_string_lossy().into_owned();
            let status = super::game_execution::wait_for_prefix_update(prepared, &cancel)?;
            if status.state == "cancelled" || cancel.load(Ordering::Acquire) { return Err(format!("Atualização do prefixo cancelada. Log: {log}")); }
            if status.state == "failed" { return Err(status.error.unwrap_or_else(|| format!("Falha ao atualizar o prefixo. Log: {log}"))); }
            if !Path::new(&prefix).join("drive_c").is_dir() || !Path::new(&prefix).join("system.reg").is_file() { return Err(format!("O Proton não concluiu a preparação do prefixo. Log: {log}")); }
            InstallationResult { prefix, log, registered_count: 0, library_error: None }
        }
    } else { run_with_umu_cancellable(request.clone(), &find_umu()?, &cancel)? };
    {
        let mut slot = INSTALLATION.lock().map_err(|e| e.to_string())?;
        if let Some(session) = slot.as_mut() { session.accepting_cancel = false; }
        if cancel.load(Ordering::Acquire) { return Err(format!("Instalação cancelada. Nenhum jogo foi registrado. Log: {}", result.log)); }
    }
    if let Some((entry, prefix)) = repair {
        let executable = Path::new(&request.executable).strip_prefix(&entry.prefix).ok()
            .map(|relative| prefix.join(relative)).filter(|path| path.is_file())
            .unwrap_or_else(|| PathBuf::from(&request.executable));
        match super::installed_library::update_settings(super::installed_library::EntrySettings {
            path: entry.path, name: request.name, proton: request.proton,
            executable: executable.to_string_lossy().into_owned(), prefix: Some(result.prefix.clone()), advanced: None,
        }) {
            Ok(_) => result.registered_count = 1,
            Err(error) => result.library_error = Some(error),
        }
        return Ok(result);
    }
    let record = super::installed_library::InstallationRecord {
        name: request.name,
        directory: absolute(&request.directory)?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned(),
        prefix: result.prefix.clone(),
        proton: absolute(&request.proton)?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned(),
        installer: absolute(&request.executable)?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned(),
        cover_url: request.cover_url,
        entries: vec![],
        overrides: vec![],
    };
    match super::installed_library::register(record) {
        Ok(count) => result.registered_count = count,
        Err(error) => result.library_error = Some(error),
    }
    Ok(result)
}
pub(crate) fn prepare_launch(path: String) -> Result<PreparedRun, String> {
    prepare_launch_program(path, None)
}
pub(crate) fn prepare_launch_program(
    path: String,
    executable: Option<String>,
) -> Result<PreparedRun, String> {
    let entry = super::installed_library::list(false)?
        .into_iter()
        .find(|entry| entry.path == path)
        .ok_or("Executável não registrado na biblioteca PeliGames.")?;
    // This launches the registered executable without replacing the installation manifest.
    let arguments = if executable.is_none() { entry.launch_arguments.clone() } else { Vec::new() };
    let mut prepared = prepare_run_in_prefix(
        InstallationRequest {
            name: entry.name,
            directory: Path::new(&entry.prefix)
                .parent()
                .ok_or("Prefixo inválido.")?
                .to_string_lossy()
                .into_owned(),
            executable: executable.unwrap_or(entry.executable),
            proton: entry.proton,
            cover_url: entry.cover_url,
        },
        &find_umu()?,
        Some(Path::new(&entry.prefix)),
        Some(&entry.advanced),
    )?;
    prepared.command.args(arguments);
    Ok(prepared)
}
pub fn launch(path: String) -> Result<(), String> {
    let mut prepared = prepare_launch(path)?;
    let status = prepared.command.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Proton encerrou com código {:?}. Log: {}",
            status.code(),
            prepared.log.display()
        ))
    }
}
pub(crate) struct PreparedRun {
    pub command: Command,
    pub prefix: PathBuf,
    pub log: PathBuf,
    pub executable: PathBuf,
}
fn run_with_umu_cancellable(request: InstallationRequest, umu: &Path, cancel: &AtomicBool) -> Result<InstallationResult, String> {
    wait_for_installation(prepare_run(request, umu)?, cancel)
}
fn wait_for_installation(prepared: PreparedRun, cancel: &AtomicBool) -> Result<InstallationResult, String> {
    let prefix = prepared.prefix.to_string_lossy().into_owned();
    let log = prepared.log.to_string_lossy().into_owned();
    if cancel.load(Ordering::Acquire) { return Err(format!("Instalação cancelada. Log: {log}")); }
    let status = super::game_execution::wait_for_installer(prepared, cancel)?;
    if status.state == "cancelled" || cancel.load(Ordering::Acquire) { return Err(format!("Instalação cancelada. Nenhum jogo foi registrado. O prefixo e o log foram preservados: {log}")); }
    if status.state == "failed" { return Err(status.error.unwrap_or_else(|| format!("Falha ao executar o instalador. Log: {log}"))); }
    Ok(InstallationResult {
        prefix,
        log,
        registered_count: 0,
        library_error: None,
    })
}
pub(crate) fn lock_prefix(prefix: &Path) -> Result<File,String> {
    let file=fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(prefix.join(".peligames-session.lock")).map_err(|e|e.to_string())?;
    #[cfg(unix)] { use std::os::fd::AsRawFd; if unsafe {libc::flock(file.as_raw_fd(),libc::LOCK_EX|libc::LOCK_NB)} != 0 { return Err("Outra sessão do PeliGames ou Pelinstall já usa este prefixo.".into()); } }
    Ok(file)
}
pub(crate) fn validate_proton(value: &str) -> Result<PathBuf, String> {
    let proton = absolute(value)?
        .canonicalize()
        .map_err(|e| format!("Proton não encontrado: {e}"))?;
    if !executable_file(&proton.join("proton")) {
        return Err("A versão selecionada não contém um Proton executável.".into());
    }
    Ok(proton)
}
pub(crate) fn validate_executable(value: &str) -> Result<PathBuf, String> {
    let executable = absolute(value)?
        .canonicalize()
        .map_err(|e| format!("Executável não encontrado: {e}"))?;
    let extension = executable
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut header = [0u8; 8];
    let read = File::open(&executable)
        .and_then(|mut f| f.read(&mut header))
        .map_err(|e| format!("Não foi possível ler o instalador: {e}"))?;
    if !((extension == "exe" && read >= 2 && &header[..2] == b"MZ")
        || (extension == "msi"
            && read == 8
            && header == [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]))
    {
        return Err("Selecione um instalador Windows válido (.exe ou .msi).".into());
    }
    Ok(executable)
}
// Inno Setup accepts /DIR; detect its data header before supplying that option.
fn inno_destination(executable: &Path, name: &str) -> Result<Option<String>, String> {
    let mut file = File::open(executable).map_err(|e| e.to_string())?;
    let marker = b"Inno Setup Setup Data (";
    let mut carry = Vec::new(); let mut buffer = [0u8; 65536];
    let mut remaining = 16 * 1024 * 1024;
    while remaining > 0 {
        let count = file.read(&mut buffer[..remaining.min(65536)]).map_err(|e| e.to_string())?;
        if count == 0 { break; }
        carry.extend_from_slice(&buffer[..count]);
        if carry.windows(marker.len()).any(|bytes| bytes == marker) {
            let clean: String = name.chars().map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '_' } else { c }).take(120).collect();
            let clean = clean.trim().trim_matches(['.', ' ']);
            return Ok(Some(format!("/DIR=C:\\Games\\{}", if clean.is_empty() { "Programa" } else { clean })));
        }
        let tail = carry.len().saturating_sub(marker.len() - 1); carry.drain(..tail);
        remaining -= count;
    }
    Ok(None)
}
fn prepare_run(request: InstallationRequest, umu: &Path) -> Result<PreparedRun, String> {
    prepare_run_in_prefix(request, umu, None, None)
}
fn prepare_run_in_prefix(
    request: InstallationRequest,
    umu: &Path,
    selected_prefix: Option<&Path>,
    advanced: Option<&super::advanced_settings::AdvancedSettings>,
) -> Result<PreparedRun, String> { prepare_run_mode(request, umu, selected_prefix, advanced, false) }
fn prepare_run_mode(
    request: InstallationRequest, umu: &Path, selected_prefix: Option<&Path>,
    advanced: Option<&super::advanced_settings::AdvancedSettings>, prefix_update: bool,
) -> Result<PreparedRun, String> {
    if request.name.trim().is_empty() {
        return Err("Confirme o nome do jogo ou programa.".into());
    }
    let directory = absolute(&request.directory)?;
    let executable = validate_executable(&request.executable)?;
    let proton = validate_proton(&request.proton)?;
    if let Some(settings) = advanced { settings.validate_runner(&proton)?; }
    let extension = executable
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    // Validate inputs before creating anything; never replace an existing prefix.
    fs::create_dir_all(&directory).map_err(|e| format!("Não foi possível criar o destino: {e}"))?;
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    if selected_prefix.is_none() {
        super::installation_layout::migrate(&directory)?;
    }
    let prefix = selected_prefix
        .map(Path::to_path_buf)
        .unwrap_or_else(|| directory.join("prefix"));
    let logs = directory.join("logs");
    fs::create_dir_all(&prefix)
        .and_then(|_| fs::create_dir_all(&logs))
        .map_err(|e| format!("Não foi possível preparar o prefixo: {e}"))?;
    let log_path = logs.join(format!("installer-{}.log", uuid::Uuid::new_v4()));
    let mut log = File::create(&log_path).map_err(|e| e.to_string())?;
    writeln!(
        log,
        "PeliGames: {}\nProton: {}\nPrefix: {}\nInstaller: {}",
        request.name,
        proton.display(),
        prefix.display(),
        executable.display()
    )
    .map_err(|e| e.to_string())?;
    let stderr = log.try_clone().map_err(|e| e.to_string())?;
    let mut command = super::advanced_settings::launch_command(umu, advanced)?;
    if prefix_update {
        command.arg("wineboot").arg("-u").current_dir(&prefix);
    } else {
        if extension == "msi" { command.arg("msiexec").arg("/i"); }
        command.arg(&executable);
        if selected_prefix.is_none() && extension == "exe" {
            if let Some(destination) = inno_destination(&executable, &request.name)? {
                writeln!(stderr.try_clone().map_err(|e| e.to_string())?, "Destino inicial Inno Setup: {destination}").map_err(|e| e.to_string())?;
                command.arg(destination);
            }
        }
        command.current_dir(executable.parent().ok_or("Diretório do instalador indisponível.")?);
    }
    command.env("WINEPREFIX", &prefix)
        .env("PROTONPATH", &proton)
        .env("GAMEID", "umu-default")
        .env("STORE", "none")
        .env("PROTON_VERB", "waitforexitandrun")
        .env("PROTON_LOG", "1")
        .env("PROTON_LOG_DIR", &logs)
        .env_remove("UMU_NO_PROTON")
        .env_remove("UMU_NO_RUNTIME")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    Ok(PreparedRun {
        command,
        prefix,
        log: log_path,
        executable: if prefix_update { PathBuf::from("wineboot.exe") } else { executable },
    })
}
