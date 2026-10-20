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
pub fn installation_active() -> bool { INSTALLATION.lock().map(|slot| slot.is_some()).unwrap_or(true) }

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
        directory: super::installation_layout::destination_of(&prefix).ok_or("Prefixo inválido.")?.to_string_lossy().into_owned(), cover_url: entry.cover_url.clone() };
    run_inner(installation, Some((entry, prefix)), None)
}
pub fn run(request: InstallationRequest) -> Result<InstallationResult, String> { run_inner(request, None, None) }
pub(crate) struct LauncherInstallPlan { pub id: String, pub executable_paths: Vec<String>, pub dependencies: Vec<String>, pub cancel: Arc<AtomicBool> }
pub(crate) fn run_launcher(request: InstallationRequest, plan: LauncherInstallPlan) -> Result<InstallationResult, String> { run_inner(request, None, Some(plan)) }
fn run_inner(request: InstallationRequest, repair: Option<(super::installed_library::InstalledEntry, PathBuf)>, launcher: Option<LauncherInstallPlan>) -> Result<InstallationResult, String> {
    let cancel = launcher.as_ref().map(|plan|plan.cancel.clone()).unwrap_or_else(||Arc::new(AtomicBool::new(false)));
    {
        let mut slot = INSTALLATION.lock().map_err(|e| e.to_string())?;
        if slot.is_some() { return Err("Um instalador já está em execução.".into()); }
        *slot = Some(InstallationSession { cancel: cancel.clone(), accepting_cancel: true });
    }
    let _guard = InstallationGuard;
    if cancel.load(Ordering::Acquire) { return Err("Operação cancelada.".into()); }
    let mut baseline = Vec::new();
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
    } else {
        let umu = find_umu()?;
        baseline = prefix_baseline(&request, &umu, &cancel)?;
        if let Some(plan) = &launcher {
            if !plan.dependencies.is_empty() {
                let mut prepared=prepare_run_mode(request.clone(), &umu, None, None, true)?;
                let mut command=Command::new(&umu);
                super::env_utils::host_environment(&mut command);
                command.arg("winetricks").args(&plan.dependencies);
                for (key,value) in prepared.command.get_envs() { if let Some(value)=value { command.env(key,value); } else { command.env_remove(key); } }
                command.current_dir(&prepared.prefix).stdin(Stdio::null());
                prepared.command=command;
                wait_for_installation(prepared, &cancel)?;
            }
        }
        run_with_umu_cancellable(request.clone(), &umu, &cancel)?
    };
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
    let mut record = super::installed_library::InstallationRecord {
        name: request.name,
        directory: absolute(&request.directory)?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned(),
        prefix: result.prefix.clone(),
        proton: absolute(&super::paths::migrated_runner_path(&request.proton))?
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
        selected_executable: None,
        overrides: vec![],
        baseline,
        known_shortcuts: None,
        launcher_games: vec![],
        install_shortcuts: None,
        profile_kind: Default::default(),
    };
    if let Some(plan)=launcher {
        let prefix=Path::new(&record.prefix).canonicalize().map_err(|e|e.to_string())?;
        let matches: std::collections::BTreeSet<_> = plan.executable_paths.iter().filter_map(|relative| {
            let path=prefix.join(relative).canonicalize().ok()?;
            if !path.starts_with(&prefix) { return None; }
            validate_executable(&path.to_string_lossy()).ok()
        }).collect();
        if matches.len()!=1 { return Err(format!("A instalação encerrou, mas o executável do launcher não foi confirmado. O prefixo e os logs foram preservados em {}.",record.directory)); }
        let target=matches.into_iter().next().unwrap();
        // Hide helper executables even after a rescan; never register the setup EXE.
        record.overrides=super::installed_library::prefix_executables(&prefix)?.into_iter().filter(|path| Path::new(path) != target).map(|source| super::installed_library::EntryOverride {
            source,name:String::new(),proton:record.proton.clone(),executable:String::new(),prefix:None,advanced:None,cover_url:None,
        }).collect();
        record.overrides.push(super::installed_library::EntryOverride { source:target.to_string_lossy().into_owned(), name:record.name.clone(), proton:record.proton.clone(), executable:target.to_string_lossy().into_owned(), prefix:None,advanced:None,cover_url:None });
        fs::write(prefix.join(".peligames-launcher.json"),serde_json::to_vec(&serde_json::json!({"id":plan.id,"executable":target})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    }
    match super::installed_library::register(record) {
        Ok(count) => result.registered_count = count,
        Err(error) => result.library_error = Some(error),
    }
    Ok(result)
}
pub(crate) fn prepare_launch(path: String) -> Result<PreparedRun, String> {
    prepare_launch_program(path, None, Vec::new())
}
pub(crate) fn prepare_launch_program(
    path: String,
    executable: Option<String>,
    extra_arguments: Vec<String>,
) -> Result<PreparedRun, String> {
    if path.starts_with("external:") || path.starts_with("nexus:") {
        let (name, prefix, proton, _) = super::wine_tools::maintenance_entry(&path)?;
        let prefix = PathBuf::from(prefix).canonicalize().map_err(|_| "Prefixo do jogo indisponível.")?;
        if !prefix.join("drive_c").is_dir() { return Err("O prefixo Wine ainda não foi inicializado.".into()); }
        let executable = executable.ok_or("Selecione um programa para executar neste prefixo.")?;
        let mut prepared = prepare_run_in_prefix(InstallationRequest {
            name, directory: prefix.parent().ok_or("Prefixo inválido.")?.to_string_lossy().into_owned(),
            executable, proton, cover_url: None,
        }, &find_umu()?, Some(&prefix), None)?;
        prepared.command.args(extra_arguments);
        return Ok(prepared);
    }
    let entry = super::installed_library::list(false)?
        .into_iter()
        .find(|entry| entry.path == path)
        .ok_or("Executável não registrado na biblioteca PeliGames.")?;
    super::installed_library::ensure_prefix_writable(Path::new(&entry.prefix))?;
    // A game's profile prefix (overlay) is mounted before anything runs in it.
    if let Some(profile) = &entry.profile { super::game_profiles::ensure_ready(profile)?; }
    // This launches the registered executable without replacing the installation manifest.
    let registered_program = executable.is_none();
    let arguments = if registered_program { entry.launch_arguments.clone() } else { extra_arguments };
    let mut prepared = prepare_run_in_prefix(
        InstallationRequest {
            name: entry.name.clone(),
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
    if registered_program {
        if let Some(options) = &entry.advanced.launch_options {
            crate::commands::nexus_launch_options::apply_custom(&mut prepared.command, options)?;
        } else { prepared.command.args(arguments); }
    } else {
        // Another program of the prefix (a shortcut's arguments, e.g. Battle.net `--exec`).
        prepared.command.args(arguments);
    }
    super::mangohud_config::apply_with_preset(&mut prepared.command, entry.advanced.options.get("mangohud") == Some(&true) || entry.advanced.options.get("mangohud_env") == Some(&true), entry.advanced.mangohud_fps_limit, entry.advanced.mangohud_enabled, entry.advanced.mangohud_preset.as_ref())?;
    super::mangohud_config::apply_game_logs(&mut prepared.command, &entry.name, entry.advanced.mangohud_preset.as_ref())?;
    Ok(prepared)
}
pub fn launch(path: String) -> Result<(), String> {
    let mut prepared = prepare_launch(path)?;
    super::bounded_logs::capture(&mut prepared.command,&prepared.log)?;
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
    pub name: String,
    pub command: Command,
    pub prefix: PathBuf,
    pub log: PathBuf,
    pub executable: PathBuf,
}
/// Executables in the prefix before the installer runs. A new prefix is first
/// created silently (`wineboot -u`), so what Proton itself adds (its Steam stub,
/// runtimes) never looks like the installed game.
fn prefix_baseline(request: &InstallationRequest, umu: &Path, cancel: &AtomicBool) -> Result<Vec<String>, String> {
    let ready = |prefix: &Path| prefix.join("system.reg").is_file() && prefix.join("drive_c").is_dir();
    if !ready(&super::installation_layout::prefix_for(&absolute(&request.directory)?)) {
        let prepared = prepare_run_mode(request.clone(), umu, None, None, true)?;
        let log = prepared.log.to_string_lossy().into_owned();
        let status = super::game_execution::wait_for_prefix_update(prepared, cancel)?;
        if status.state == "cancelled" || cancel.load(Ordering::Acquire) { return Err(format!("Instalação cancelada. Log: {log}")); }
        if status.state == "failed" { return Err(status.error.unwrap_or_else(|| format!("Falha ao preparar o prefixo. Log: {log}"))); }
    }
    let prefix = super::installation_layout::prefix_for(&absolute(&request.directory)?.canonicalize().map_err(|e| e.to_string())?);
    // Without a ready prefix the comparison is skipped; the installer still runs.
    if !ready(&prefix) { return Ok(Vec::new()); }
    super::installed_library::prefix_executables(&prefix)
}
fn run_with_umu_cancellable(request: InstallationRequest, umu: &Path, cancel: &AtomicBool) -> Result<InstallationResult, String> {
    wait_for_installation(prepare_run(request, umu)?, cancel)
}
fn wait_for_installation(mut prepared: PreparedRun, cancel: &AtomicBool) -> Result<InstallationResult, String> {
    let prefix = prepared.prefix.to_string_lossy().into_owned();
    let log = prepared.log.to_string_lossy().into_owned();
    if cancel.load(Ordering::Acquire) { return Err(format!("Instalação cancelada. Log: {log}")); }
    super::bounded_logs::capture(&mut prepared.command,&prepared.log)?;
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
    let migrated = super::paths::migrated_runner_path(value);
    let value = migrated.as_str();
    let proton = absolute(value)?
        .canonicalize()
        .map_err(|e| format!("Proton não encontrado: {e}"))?;
    if !executable_file(&proton.join("proton")) {
        return Err("A versão selecionada não contém um Proton executável.".into());
    }
    Ok(proton)
}
/// Proton that last ran a prefix, as Proton itself records it in `config_info`
/// (second line: `<proton>/files/share/fonts/`, or `dist/` on older builds).
/// The file sits beside `pfx`, so both the prefix and its parent are checked.
pub(crate) fn proton_from_prefix(prefix: &Path) -> Option<String> {
    let info = [prefix.join("config_info"), prefix.parent()?.join("config_info")].into_iter().find_map(|file| fs::read_to_string(file).ok())?;
    let fonts = info.lines().nth(1)?.trim();
    let root = ["/files/", "/dist/"].iter().find_map(|marker| fonts.rfind(marker).map(|at| &fonts[..at]))?;
    validate_proton(root).ok().map(|path| path.to_string_lossy().into_owned())
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
    if let Some(prefix)=selected_prefix {super::installed_library::ensure_prefix_writable(prefix)?;}
    let directory = absolute(&request.directory)?;
    let executable = validate_executable(&request.executable)?;
    let proton = validate_proton(&request.proton)?;
    if let Some(settings) = advanced {
        settings.validate_runner(&proton)?;
        if settings.options.get("steam_client") == Some(&true) && !crate::commands::external_library::launcher_open("Steam") {
            return Err("Abra a Steam e entre na sua conta antes de iniciar um jogo com a comunicação Steam ativada.".into());
        }
    }
    let extension = executable
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    // Validate inputs before creating anything; never replace an existing prefix.
    if selected_prefix.is_none() {
        super::installation_layout::check_new_destination(&directory)?;
    }
    fs::create_dir_all(&directory).map_err(|e| format!("Não foi possível criar o destino: {e}"))?;
    let directory = directory.canonicalize().map_err(|e| e.to_string())?;
    if selected_prefix.is_none() {
        super::installation_layout::migrate(&directory)?;
        super::installation_layout::claim(&directory)?;
    }
    let prefix = selected_prefix
        .map(Path::to_path_buf)
        .unwrap_or_else(|| super::installation_layout::prefix_for(&directory));
    let logs = directory.join("logs");
    fs::create_dir_all(&prefix)
        .and_then(|_| fs::create_dir_all(&logs))
        .map_err(|e| format!("Não foi possível preparar o prefixo: {e}"))?;
    let log_path = logs.join("installer.log");
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
    let runtime_proton = if advanced.is_some_and(|settings| settings.options.get("steam_client") == Some(&true)) {
        super::steam_client::runner(&proton)?
    } else { proton.clone() };
    command.env("WINEPREFIX", &prefix)
        .env("PROTONPATH", &runtime_proton)
        .env("GAMEID", "umu-default")
        .env("STORE", "none")
        .env("PROTON_VERB", "waitforexitandrun")
        .env("PROTON_LOG", "0")
        .env("PROTON_LOG_DIR", &logs)
        .env_remove("UMU_NO_PROTON")
        .env_remove("UMU_NO_RUNTIME")
        .stdin(Stdio::null());
    drop(log);drop(stderr);
    Ok(PreparedRun {
        name: request.name,
        command,
        prefix,
        log: log_path,
        executable: if prefix_update { PathBuf::from("wineboot.exe") } else { executable },
    })
}

#[cfg(test)]
mod prefix_proton_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn proton_is_read_from_the_config_info_beside_pfx() {
        let root = std::env::temp_dir().join(format!("peligames-prefix-proton-{}", std::process::id()));
        let proton = root.join("compatibilitytools.d/Proton Test");
        let compat = root.join("compatdata/42");
        fs::create_dir_all(proton.join("files/share/fonts")).unwrap();
        fs::create_dir_all(compat.join("pfx")).unwrap();
        fs::write(proton.join("proton"), "#!/bin/sh\n").unwrap();
        fs::set_permissions(proton.join("proton"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(proton_from_prefix(&compat.join("pfx")), None);
        fs::write(compat.join("config_info"), format!("Test-1.0\n{}/files/share/fonts/\n{}/files/lib/\n", proton.display(), proton.display())).unwrap();
        let expected = Some(proton.canonicalize().unwrap().to_string_lossy().into_owned());
        assert_eq!(proton_from_prefix(&compat.join("pfx")), expected);
        assert_eq!(proton_from_prefix(&compat), expected);
        fs::write(compat.join("config_info"), "Test-1.0\n/missing/Proton/files/share/fonts/\n").unwrap();
        assert_eq!(proton_from_prefix(&compat.join("pfx")), None);
        fs::remove_dir_all(root).unwrap();
    }
}
