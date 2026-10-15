use super::storage::{automount, udisks::Udisks};
use serde_json::{json, Value};
use std::sync::Mutex;
static SAVE_LOCK: Mutex<()> = Mutex::new(());
fn run(request: Value) -> Result<Value, String> {
    let mut system = Udisks::connect()?;
    match request["action"].as_str().unwrap_or("") {
        "list" => {
            let objects = automount::System::objects(&mut system)?;
            Ok(Value::Array(automount::storage_listing(&objects)))
        }
        "save" => automount::save(&request, &mut system),
        "operation" => automount::storage_operation(&request, &mut system),
        "format_ntfs" => automount::format_ntfs(&request, &mut system),
        "prepare_storage" => automount::prepare_storage(&request, &mut system),
        _ => Err("Operação inválida.".into()),
    }
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
