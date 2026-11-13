//! Compatibility endpoints and application events for an optional capture provider.
//! Recorder implementation and storage live in the selected add-on worker.
use peligames_capture_contract::{shortcuts, CaptureStatus};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};
use tauri::Emitter;
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static WATCHING: AtomicBool = AtomicBool::new(false);
static STATUS: OnceLock<Mutex<CaptureStatus>> = OnceLock::new();
pub fn initialize(app: tauri::AppHandle) {
    let _ = APP.set(app);
}
fn idle() -> CaptureStatus {
    CaptureStatus {
        phase: "idle".into(),
        ..Default::default()
    }
}
fn publish(status: CaptureStatus) {
    *STATUS.get_or_init(|| Mutex::new(idle())).lock().unwrap() = status.clone();
    super::discord_presence::capture(&status);
    if let Some(app) = APP.get() {
        let _ = app.emit("peligames-capture", status);
    }
}
pub fn watch(id: String) {
    if WATCHING.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            match super::addons::addon_service_call(
                id.clone(),
                "session.status".into(),
                Value::Null,
                None,
            )
            .await
            {
                Ok(value) => {
                    if let Ok(status) = serde_json::from_value(value) {
                        publish(status);
                    }
                }
                Err(_) => {
                    publish(idle());
                    break;
                }
            }
            if let Ok(events) = super::addons::addon_service_call(
                id.clone(),
                "events.poll".into(),
                Value::Null,
                None,
            )
            .await
            {
                for event in events.as_array().into_iter().flatten() {
                    if let Some(app) = APP.get() {
                        if event["event"] == "capture-selection-changed" {
                            let _ = app.emit("capture-selection-changed", event["payload"].clone());
                        }
                    }
                }
            }
        }
        WATCHING.store(false, Ordering::Release);
    });
}
async fn call(method: &str, request: Value) -> Result<Value, String> {
    let id = super::addons::capture_provider()
        .await?
        .ok_or("Install a capture add-on first")?;
    super::addons::addon_service_call(id, method.into(), request, None)
        .await
        .map_err(|e| e.to_string())
}
async fn operation(method: &str, request: Value) -> Result<Value, String> {
    let result = call(method, request).await?;
    let Some(id) = result["job"].as_u64() else {
        return Ok(result);
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
        let state = call("job.inspect", json!({"id":id})).await?;
        if state["done"] == true {
            if let Some(error) = state["error"].as_str() {
                return Err(error.into());
            }
            return Ok(state["result"].clone());
        }
        if std::time::Instant::now() >= deadline {
            return Err("Capture operation timed out".into());
        }
        if let Some(app) = APP.get() {
            if !state["progress"].is_null() {
                let _ = app.emit("capture-runtime-progress", state["progress"].clone());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}
#[tauri::command]
pub fn capture_status() -> CaptureStatus {
    STATUS
        .get_or_init(|| Mutex::new(idle()))
        .lock()
        .unwrap()
        .clone()
}
#[tauri::command]
pub async fn capture_start(options: Value, replay: bool) -> Result<Value, String> {
    operation("session.start", json!({"options":options,"replay":replay})).await
}
#[tauri::command]
pub async fn capture_action(action: String) -> Result<Value, String> {
    operation("session.action", json!({"action":action})).await
}
#[tauri::command]
pub async fn capture_settings(game_key: String) -> Result<Value, String> {
    if super::addons::capture_provider().await?.is_none() {
        return Ok(Value::Null);
    }
    call("settings.read", json!({"gameKey":game_key})).await
}
pub(super) fn check_shortcut_conflict(value: &str) -> Result<(), String> {
    if !super::addons::capture_shortcuts_enabled() {
        return Ok(());
    }

    // Shortcut collisions are a shared input contract; no recorder runtime starts.
    let path = crate::core::paths::app_path("capture-shortcuts.json")?;
    let preferences = std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<shortcuts::Preferences>(&b).ok())
        .unwrap_or_default();
    if preferences.conflict(value).is_some() {
        return Err("This shortcut belongs to capture controls".into());
    }
    Ok(())
}

pub(crate) fn diagnostics(key: &str) -> Result<Value, String> {
    tauri::async_runtime::block_on(call("diagnostics.inspect", json!({"gameKey":key})))
}
