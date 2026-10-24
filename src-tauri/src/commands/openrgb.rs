use crate::core::openrgb::Client;
use serde::Serialize;
use std::sync::Mutex;
static CLIENT: Mutex<Option<(u16, Client)>> = Mutex::new(None);
#[derive(Serialize)]
pub struct Zone {
    name: String,
    color: Option<[u8; 3]>,
}
#[derive(Serialize)]
pub struct Device {
    key: String,
    name: String,
    zones: Vec<Zone>,
    color: Option<[u8; 3]>,
}
#[tauri::command]
pub async fn read_openrgb(port: u16) -> Result<Vec<Device>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut slot = CLIENT.lock().map_err(|e| e.to_string())?;
        let result = (|| {
            if slot.as_ref().is_none_or(|(p, _)| *p != port) {
                *slot = None;
                *slot = Some((port, Client::connect(port)?));
            }
            slot.as_mut().unwrap().1.devices()
        })();
        match result {
            Ok(devices) => Ok(devices
                .into_iter()
                .map(|d| Device {
                    key: d.key,
                    name: d.name,
                    color: d.color,
                    zones: d
                        .zones
                        .into_iter()
                        .map(|z| Zone {
                            name: z.name,
                            color: z.color,
                        })
                        .collect(),
                })
                .collect()),
            Err(e) => {
                *slot = None;
                Err(e)
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn disconnect_openrgb() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        *CLIENT.lock().map_err(|e| e.to_string())? = None;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}
