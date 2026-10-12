use crate::core::game_installation::{self, InstallationRequest, InstallationResult};
use std::sync::atomic::{AtomicBool, Ordering};
static BUSY: AtomicBool = AtomicBool::new(false);
#[tauri::command]
pub fn cancel_game_installer() -> Result<(), String> { game_installation::cancel_installation() }
#[tauri::command]
pub async fn list_peligames_entries(
    rescan: Option<bool>,
) -> Result<Vec<crate::core::installed_library::InstalledEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::core::installed_library::list(rescan.unwrap_or(false))
    })
    .await
    .map_err(|e| e.to_string())?
}
pub async fn launch_peligames_entry(path: String) -> Result<(), String> {
    start_peligames_game(path, None).await.map(|_| ())
}

#[tauri::command]
pub async fn repair_peligames_entry(request: crate::core::game_installation::RepairRequest) -> Result<InstallationResult, String> {
    if BUSY.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err() { return Err("Aguarde a operação em andamento.".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BusyGuard;
        if crate::core::game_execution::active() { return Err("Encerre os programas antes de reparar.".into()); }
        game_installation::repair(request)
    }).await.map_err(|e| e.to_string())?
}

struct BusyGuard;
impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
#[tauri::command]
pub async fn run_game_installer(
    request: InstallationRequest,
) -> Result<InstallationResult, String> {
    if BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Um instalador já está em execução.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BusyGuard;
        if crate::core::game_execution::active() {
            return Err("Encerre o jogo ou programa antes de instalar.".into());
        }
        game_installation::run(request)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn start_peligames_game(
    path: String,
    executable: Option<String>,
) -> Result<crate::core::game_execution::ExecutionStatus, String> {
    if BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Um jogo ou instalador está sendo iniciado.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BusyGuard;
        crate::core::game_execution::start_program(path, executable)
    })
    .await
    .map_err(|e| e.to_string())?
}
/// Runs a program in the entry's prefix: inside the running prefix when its game
/// is open (returns no session), otherwise as the library's session.
fn run_program(path: String, executable: String, arguments: Vec<String>) -> Result<Option<crate::core::game_execution::ExecutionStatus>, String> {
    let prefix = crate::core::installed_library::list(false)?.into_iter().find(|entry| entry.path == path).map(|entry| entry.prefix)
        .ok_or("Executável não registrado na biblioteca PeliGames.")?;
    if crate::core::game_execution::running_prefix().is_some_and(|running| running == prefix) {
        crate::core::game_execution::run_in_prefix(path, executable, arguments).map(|_| None)
    } else {
        crate::core::game_execution::start_program_with(path, Some(executable), arguments).map(Some)
    }
}
#[tauri::command]
pub async fn run_peligames_program(path: String, executable: String) -> Result<Option<crate::core::game_execution::ExecutionStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || run_program(path, executable, Vec::new())).await.map_err(|e| e.to_string())?
}
/// Games launchers installed in the entry's prefix (Programs panel).
#[tauri::command]
pub async fn list_peligames_launcher_shortcuts(path: String) -> Result<Vec<crate::core::installed_library::LauncherShortcut>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::launcher_shortcuts(&path)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn add_peligames_shortcut_to_library(path: String, id: String) -> Result<crate::core::installed_library::InstalledEntry, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::add_shortcut_to_library(&path, &id)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn remove_peligames_shortcut_from_library(path: String, id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::remove_shortcut_from_library(&path, &id)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn peligames_profile_options(path: String) -> Result<crate::core::installed_library::ProfileOptions, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::profile_options(&path)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_peligames_profile_kind(path: String, kind: crate::core::game_profiles::ProfileKind) -> Result<crate::core::installed_library::ProfileOptions, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::set_profile_kind(&path, kind)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn start_peligames_shortcut(path: String, id: String) -> Result<Option<crate::core::game_execution::ExecutionStatus>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (executable, arguments) = crate::core::installed_library::shortcut_launch(&path, &id)?;
        run_program(path, executable, arguments)
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn get_peligames_execution(
) -> Result<Option<crate::core::game_execution::ExecutionStatus>, String> {
    crate::core::game_execution::status()
}
#[tauri::command]
pub fn cancel_peligames_game(
    id: String,
) -> Result<crate::core::game_execution::ExecutionStatus, String> {
    crate::core::game_execution::cancel(&id)
}

#[tauri::command]
pub async fn update_peligames_settings(
    settings: crate::core::installed_library::EntrySettings,
) -> Result<crate::core::installed_library::InstalledEntry, String> {
    if BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Aguarde a operação em andamento.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BusyGuard;
        if crate::core::game_execution::active() {
            return Err("Encerre a execução antes de alterar suas configurações.".into());
        }
        crate::core::installed_library::update_settings(settings)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The executable chosen after an installation that found several.
#[tauri::command]
pub async fn list_peligames_executable_choices(path: String) -> Result<Vec<crate::core::installed_library::InstalledEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::executable_choices(&path)).await.map_err(|e| e.to_string())?
}

/// Games a launcher of the library installed since it last closed.
#[tauri::command]
pub async fn new_peligames_launcher_games(path: String) -> Result<Vec<crate::core::installed_library::LauncherGame>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::new_launcher_games(&path)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn add_peligames_launcher_games(path: String, added: Vec<crate::core::installed_library::LauncherGame>, declined: Vec<String>) -> Result<Vec<crate::core::installed_library::InstalledEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::installed_library::add_launcher_games(&path, added, declined)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn choose_peligames_entry(path: String) -> Result<crate::core::installed_library::InstalledEntry, String> {
    if BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() { return Err("Aguarde a operação em andamento.".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BusyGuard;
        if crate::core::game_execution::active() { return Err("Encerre a execução antes de alterar o executável.".into()); }
        crate::core::installed_library::choose(&path)
    })
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn add_peligames_entry(
    request: InstallationRequest,
) -> Result<crate::core::installed_library::InstalledEntry, String> {
    if BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Aguarde a operação em andamento.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BusyGuard;
        if crate::core::game_execution::active() {
            return Err("Encerre a execução antes de adicionar.".into());
        }
        crate::core::installed_library::add(request)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn uninstall_peligames_entry(path: String) -> Result<(), String> {
    if BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Aguarde a operação em andamento.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = BusyGuard;
        if crate::core::game_execution::active() {
            return Err("Encerre a execução antes de desinstalar.".into());
        }
        crate::core::installed_library::uninstall(path)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn run_wine_tool(app: tauri::AppHandle, request: crate::core::wine_tools::ToolRequest, request_id: String) -> Result<crate::core::wine_tools::ToolResult, String> {
    use tauri::Emitter;
    if BUSY.compare_exchange(false,true,Ordering::AcqRel,Ordering::Acquire).is_err() { return Err("Aguarde a operação em andamento.".into()); }
    let _guard = BusyGuard;
    crate::core::wine_tools::execute(request, std::sync::Arc::new(move |line| { let _=app.emit("peligames-wine-tool-progress",serde_json::json!({"id":request_id,"line":line})); })).await
}

#[tauri::command]
pub async fn get_peligames_game_logs(path: String) -> Result<crate::core::game_logs::GameLogs, String> {
    tauri::async_runtime::spawn_blocking(move || crate::core::game_logs::read(&path)).await.map_err(|e|e.to_string())?
}
