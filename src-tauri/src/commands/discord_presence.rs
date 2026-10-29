use crate::core::{
    discord_presence::{Preferences, Presence, Status},
    discord_socket, paths,
};
use std::sync::{Arc, OnceLock};
use tauri::Emitter;
static SERVICE: OnceLock<Result<Presence, String>> = OnceLock::new();
fn service(app: &tauri::AppHandle) -> Result<&'static Presence, String> {
    SERVICE
        .get_or_init(|| {
            let app = app.clone();
            let runtime = discord_socket::runtime_dir();
            let mut endpoints = discord_socket::candidates(&runtime);
            let temporary = std::env::temp_dir();
            if temporary != runtime {
                endpoints.extend(discord_socket::candidates(&temporary));
            }
            Presence::new(
                paths::app_path("discord-presence.json")?,
                endpoints,
                Arc::new(move |status| {
                    let _ = app.emit("discord-presence-changed", status);
                }),
            )
        })
        .as_ref()
        .map_err(Clone::clone)
}
#[tauri::command]
pub fn discord_presence_status(app: tauri::AppHandle) -> Result<Status, String> {
    Ok(service(&app)?.status())
}
#[tauri::command]
pub fn discord_presence_preferences(
    app: tauri::AppHandle,
    preferences: Preferences,
) -> Result<Status, String> {
    let status = service(&app)?.preferences(preferences)?;
    let _ = app.emit("discord-presence-changed", &status);
    Ok(status)
}
#[tauri::command]
pub fn discord_presence_game(app: tauri::AppHandle, name: Option<String>) -> Result<(), String> {
    service(&app)?.game(name);
    Ok(())
}
pub fn capture(status: &crate::core::capture::CaptureStatus) {
    if let Some(Ok(service)) = SERVICE.get() {
        service.capture(crate::core::discord_presence::CaptureActivity {
            active: status.phase == "active",
            recording: status.recording,
            replay: status.replay,
            game_name: status.game_name.clone(),
        });
    }
}
pub fn shutdown() {
    if let Some(Ok(service)) = SERVICE.get() {
        service.shutdown();
    }
}
