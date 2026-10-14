use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;
static SAVE_LOCK: Mutex<()> = Mutex::new(());
const HELPER: &str = include_str!("storage/automount.py");
fn run(request: Value) -> Result<Value, String> {
    // Fixed interpreter/script; all untrusted input travels through JSON stdin, never a shell.
    let mut child = Command::new("/usr/bin/python3").args(["-I", "-c", HELPER])
        .env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD").env_remove("GI_TYPELIB_PATH")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|_| "Python 3 e PyGObject são necessários para acessar o UDisks2.".to_string())?;
    child.stdin.take().ok_or("Entrada indisponível")?.write_all(request.to_string().as_bytes()).map_err(|e| e.to_string())?;
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() { return Err(String::from_utf8_lossy(&output.stderr).trim().to_string()); }
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn list_storage_volumes() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| run(json!({"action":"list"}))).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn save_storage_automount(id: String, uuid: String, name: String, mount_at_startup: bool) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de montagem em andamento.".to_string())?;
        run(json!({"action":"save", "id":id, "uuid":uuid, "name":name, "mount_at_startup":mount_at_startup}))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn format_ntfs_storage(id: String, uuid: String, name: String, filesystem: String, mount_at_startup: bool, confirmation_device: String, confirmation_word: String, backup_confirmed: bool) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de armazenamento em andamento.".to_string())?;
        run(json!({"action":"format_ntfs", "id":id, "uuid":uuid, "name":name,
            "filesystem":filesystem, "mount_at_startup":mount_at_startup,
            "confirmation_device":confirmation_device, "confirmation_word":confirmation_word, "backup_confirmed":backup_confirmed}))
    }).await.map_err(|e| e.to_string())?
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageOperation { Mount, Unmount, RemoveAutomount, Remount }
#[tauri::command]
pub async fn prepare_empty_storage(mut request: Value) -> Result<Value, String> {
    if request.to_string().len() > 65536 { return Err("Pedido de preparação muito grande.".into()); }
    request.as_object_mut().ok_or("Pedido de preparação inválido.")?.insert("action".into(), "prepare_storage".into());
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de armazenamento em andamento.".to_string())?;
        run(request)
    }).await.map_err(|error| error.to_string())?
}
#[tauri::command]
pub async fn manage_storage_volume(id: String, uuid: String, operation: StorageOperation) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de armazenamento em andamento.".to_string())?;
        run(json!({"action":"operation", "id":id, "uuid":uuid, "operation":operation}))
    }).await.map_err(|e| e.to_string())?
}
