//! Microphone and output choice for voice calls, through PipeWire/PulseAudio
//! (`pactl`), as pavucontrol does: the call opens the system default devices
//! and its streams are then moved to the chosen ones. Moving works mid-call,
//! without reconnecting. Without `pactl` only the system default is offered.
use serde::Serialize;
use serde_json::Value;
use std::{
    process::Command,
    sync::Mutex,
    time::Duration,
};

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}
#[derive(Serialize)]
pub struct AudioDevices {
    inputs: Vec<AudioDevice>,
    outputs: Vec<AudioDevice>,
}

/// Chosen devices (PipeWire node names); `None` follows the system default.
static ROUTE: Mutex<(Option<String>, Option<String>)> = Mutex::new((None, None));

fn pactl(args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("pactl").args(args).output().ok()?;
    output.status.success().then_some(output.stdout)
}
fn pactl_json(kind: &str) -> Vec<Value> {
    pactl(&["-f", "json", "list", kind])
        .and_then(|bytes| serde_json::from_slice::<Vec<Value>>(&bytes).ok())
        .unwrap_or_default()
}
/// Real devices of a `pactl list sources|sinks` answer; monitors (the
/// "what is playing" of each output) are not microphones.
fn devices(list: &[Value]) -> Vec<AudioDevice> {
    list.iter()
        .filter(|item| item["properties"]["device.class"] != "monitor" && !item["name"].as_str().unwrap_or("").ends_with(".monitor"))
        .filter_map(|item| {
            let id = item["name"].as_str().filter(|id| valid_id(id))?;
            let name = item["description"].as_str().filter(|name| !name.is_empty()).unwrap_or(id);
            Some(AudioDevice { id: id.into(), name: name.chars().take(120).collect() })
        })
        .collect()
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.starts_with('-') && id.bytes().all(|b| b.is_ascii_graphic())
}

#[tauri::command]
pub async fn matrix_audio_devices() -> AudioDevices {
    tauri::async_runtime::spawn_blocking(|| AudioDevices {
        inputs: devices(&pactl_json("sources")),
        outputs: devices(&pactl_json("sinks")),
    })
    .await
    .unwrap_or(AudioDevices { inputs: Vec::new(), outputs: Vec::new() })
}

/// Saves the chosen devices and moves the running call to them.
#[tauri::command]
pub async fn matrix_audio_route(input: Option<String>, output: Option<String>) -> Result<(), String> {
    if input.as_deref().is_some_and(|id| !valid_id(id)) || output.as_deref().is_some_and(|id| !valid_id(id)) {
        return Err("Dispositivo de áudio inválido.".into());
    }
    if let Ok(mut route) = ROUTE.lock() {
        *route = (input, output);
    }
    tauri::async_runtime::spawn_blocking(apply).await.map_err(|_| "Não foi possível trocar o dispositivo de áudio.".to_string())?;
    Ok(())
}

/// This process's streams of one kind (`source-outputs` or `sink-inputs`).
fn own_streams(kind: &str) -> Vec<String> {
    let pid = std::process::id().to_string();
    own_stream_ids(&pactl_json(kind), &pid)
}
fn own_stream_ids(list: &[Value], pid: &str) -> Vec<String> {
    list.iter()
        .filter(|item| item["properties"]["application.process.id"].as_str() == Some(pid))
        .filter_map(|item| item["index"].as_u64().map(|index| index.to_string()))
        .collect()
}
/// Moves our streams to the chosen devices, or back to the defaults. Returns
/// whether any stream was found.
fn apply() -> bool {
    let (input, output) = ROUTE.lock().map(|route| route.clone()).unwrap_or_default();
    let mut found = false;
    for (kind, command, default, chosen) in [
        ("source-outputs", "move-source-output", "@DEFAULT_SOURCE@", input),
        ("sink-inputs", "move-sink-input", "@DEFAULT_SINK@", output),
    ] {
        let target = chosen.unwrap_or_else(|| default.into());
        for stream in own_streams(kind) {
            found = true;
            let _ = pactl(&[command, &stream, &target]);
        }
    }
    found
}
/// A call just opened its devices: PipeWire lists the new streams a moment
/// later, so try for a few seconds.
pub(super) fn route_new_streams() {
    std::thread::spawn(|| {
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(300));
            if apply() {
                return;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn lists_real_devices_without_monitors() {
        let list = json!([
            {"name":"alsa_output.usb.analog-stereo.monitor","description":"Monitor of Headset","properties":{"device.class":"monitor"}},
            {"name":"alsa_input.usb.mono","description":"Headset Mono","properties":{"device.class":"sound"}},
            {"name":"--bad","description":"Option-like","properties":{}},
            {"name":"virtual_sink","description":"","properties":{}},
        ]);
        assert_eq!(devices(list.as_array().unwrap()), vec![
            AudioDevice { id: "alsa_input.usb.mono".into(), name: "Headset Mono".into() },
            AudioDevice { id: "virtual_sink".into(), name: "virtual_sink".into() },
        ]);
    }
    #[test]
    fn only_this_process_streams_are_moved() {
        let list = json!([
            {"index":7,"properties":{"application.process.id":"100"}},
            {"index":8,"properties":{"application.process.id":"200"}},
            {"index":9,"properties":{}},
        ]);
        assert_eq!(own_stream_ids(list.as_array().unwrap(), "100"), vec!["7".to_string()]);
    }
}
