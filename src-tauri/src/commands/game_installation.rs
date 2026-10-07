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
