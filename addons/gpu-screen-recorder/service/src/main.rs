use peligames_addon_kit::{
    jobs::Jobs,
    manifest::Manifest,
    protocol::{RpcError, ServiceCall},
};
use peligames_gpu_screen_recorder::{
    capture::{self, CaptureManager, CaptureOptions, CaptureStatus},
    capture_overlay::Bridge,
    capture_runtime, paths,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
fn error(message: impl ToString) -> RpcError {
    RpcError {
        code: -32000,
        message: message.to_string(),
    }
}
fn encode(value: Result<impl serde::Serialize, String>) -> Result<Value, RpcError> {
    serde_json::to_value(value.map_err(error)?).map_err(error)
}
struct Service {
    manager: Arc<CaptureManager>,
    bridge: Mutex<Option<Bridge>>,
    events: Arc<Mutex<Vec<Value>>>,
    runtime: tokio::runtime::Runtime,
    jobs: Jobs,
}
impl Service {
    fn new() -> Self {
        Self {
            manager: Arc::new(Default::default()),
            bridge: Mutex::new(None),
            events: Default::default(),
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap(),
            jobs: Default::default(),
        }
    }
    fn notify(&self) -> Arc<dyn Fn(CaptureStatus) + Send + Sync> {
        let events = self.events.clone();
        Arc::new(move |status| {
            let mut e = events.lock().unwrap();
            if e.len() > 32 {
                e.remove(0);
            }
            e.push(json!({"event":"peligames-capture","payload":status}));
        })
    }
    fn ensure_bridge(&self) -> Result<(), String> {
        let mut bridge = self.bridge.lock().unwrap();
        if bridge.is_none() {
            let root = paths::app_root()?;
            let bindings = capture::context::Bindings::new(root.clone());
            let manager = self.manager.clone();
            let notify = self.notify();
            let events = self.events.clone();
            *bridge = Some(Bridge::start(
                &root,
                Arc::new(move |command, peer| {
                    bindings.reply(&manager,command,peer,notify.clone(),|selected|{let mut queue=events.lock().unwrap();if queue.len()>=32{queue.remove(0);}queue.push(json!({"event":"capture-selection-changed","payload":{"game_key":selected.game_key,"selection":selected.selection}}));},capture::encoder_support)
                }),
            )?);
        }
        Ok(())
    }
    fn prepare(&self, options: &CaptureOptions) -> Result<(), String> {
        capture::validate(options)?;
        self.ensure_bridge()?;
        capture::save_settings(options)
    }
    fn handle(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let call: ServiceCall<Value> = serde_json::from_value(params).map_err(error)?;
        let component = call.host.component_directory;
        if component
            .file_name()
            .is_none_or(|s| s != "GPU-Screen-Recorder")
            || component
                .parent()
                .and_then(|p| p.file_name())
                .is_none_or(|s| s != "components")
        {
            return Err(error("Invalid recorder storage context"));
        }
        paths::configure(
            component
                .parent()
                .unwrap()
                .parent()
                .ok_or_else(|| error("Invalid profile root"))?
                .to_owned(),
        )
        .map_err(error)?;
        let p = call.request;
        let string = |name: &str| p[name].as_str().unwrap_or_default().to_owned();
        match method {
            "session.status" => encode(Ok(self.manager.status())),
            "diagnostics.inspect" => {
                let options = capture::saved_settings(&string("gameKey")).map_err(error)?;
                let runtime = capture_runtime::status().map_err(error)?;
                let settings=options.map(|o|json!({"method":o.method,"replay_seconds":o.replay_seconds,"replay_on_launch":o.replay_on_launch,"format":o.selection.format,"codec":o.selection.codec,"resolution":o.selection.resolution,"fps":o.selection.fps,"rate_control":o.selection.rate_control,"bitrate_kbps":o.selection.bitrate_kbps,"quality_preset":o.selection.quality_preset,"desktop_audio":o.selection.desktop_audio,"microphone":o.selection.microphone,"microphone_noise_reduction":o.selection.microphone_noise_reduction}));
                Ok(
                    json!({"settings":settings,"runtime":{"installed":runtime.installed,"version":runtime.version},"shortcuts":capture::shortcut_preferences().map_err(error)?}),
                )
            }
            "events.poll" => encode(Ok(std::mem::take(&mut *self.events.lock().unwrap()))),
            "settings.read" => {
                self.ensure_bridge().map_err(error)?;
                encode(capture::saved_settings(&string("gameKey")))
            }
            "selection.read" => {
                encode(capture::saved_settings(&string("gameKey")).map(|o| o.map(|o| o.selection)))
            }
            "settings.save" => {
                let options: CaptureOptions =
                    serde_json::from_value(p["options"].clone()).map_err(error)?;
                encode(self.prepare(&options))
            }
            "session.start" => {
                let options: CaptureOptions =
                    serde_json::from_value(p["options"].clone()).map_err(error)?;
                self.prepare(&options).map_err(error)?;
                let replay = p["replay"] == true;
                let manager = self.manager.clone();
                let notify = self.notify();
                let id = self.jobs.start(&self.runtime, move |_| async move {
                    tokio::task::spawn_blocking(move || {
                        serde_json::to_value(manager.start(options, replay, notify)?)
                            .map_err(|e| e.to_string())
                    })
                    .await
                    .map_err(|e| e.to_string())?
                })?;
                Ok(json!({"job":id}))
            }
            "session.action" => {
                let action = string("action");
                if action == "stop" && self.manager.status().phase == "starting" {
                    return encode(self.manager.action(&action, self.notify()));
                }
                let manager = self.manager.clone();
                let notify = self.notify();
                let id = self.jobs.start(&self.runtime, move |_| async move {
                    tokio::task::spawn_blocking(move || {
                        serde_json::to_value(manager.action(&action, notify)?)
                            .map_err(|e| e.to_string())
                    })
                    .await
                    .map_err(|e| e.to_string())?
                })?;
                Ok(json!({"job":id}))
            }
            "support.inspect" => encode(capture::support()),
            "encoders.inspect" => encode(capture::encoder_support()),
            "audio.inspect" => encode(capture::audio_sources()),
            "runtime.inspect" => encode(capture_runtime::status()),
            "runtime.remove" => {
                self.jobs.idle()?;
                encode(capture_runtime::remove(|| {
                    self.manager.status().phase != "idle"
                }))
            }
            "runtime.download" => {
                if self.manager.status().phase != "idle" {
                    return Err(error("Stop the active capture first"));
                }
                let manager = self.manager.clone();
                let id = self.jobs.start(&self.runtime, move |progress| async move {
                    let mut response = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(180))
                        .build()
                        .map_err(|e| e.to_string())?
                        .get(capture_runtime::URL)
                        .send()
                        .await
                        .map_err(|e| e.to_string())?
                        .error_for_status()
                        .map_err(|e| e.to_string())?;
                    let total = response.content_length();
                    let limit = 80 * 1024 * 1024;
                    if total.is_some_and(|n| n > limit) {
                        return Err("Recorder package too large".into());
                    }
                    let mut bytes = vec![];
                    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                        if bytes.len() + chunk.len() > limit as usize {
                            return Err("Recorder package too large".into());
                        }
                        bytes.extend_from_slice(&chunk);
                        progress(peligames_addon_kit::protocol::TransferProgress {
                            phase: "downloading",
                            downloaded: bytes.len() as u64,
                            total,
                        });
                    }
                    tokio::task::spawn_blocking(move || {
                        let state = capture_runtime::install_bytes(
                            &bytes,
                            |p| {
                                progress(peligames_addon_kit::protocol::TransferProgress {
                                    phase: if p.phase == "installed" {
                                        "complete"
                                    } else {
                                        p.phase
                                    },
                                    downloaded: p.downloaded,
                                    total: p.total,
                                })
                            },
                            || manager.status().phase != "idle",
                        )?;
                        serde_json::to_value(state).map_err(|e| e.to_string())
                    })
                    .await
                    .map_err(|e| e.to_string())?
                })?;
                Ok(json!({"job":id}))
            }
            "job.inspect" => encode(
                self.jobs
                    .status(p["id"].as_u64().unwrap_or_default())
                    .map_err(|e| e.message),
            ),
            "shortcuts.read" => encode(capture::shortcut_preferences()),
            "shortcuts.save" => {
                let preferences: capture::shortcuts::Preferences =
                    serde_json::from_value(p["preferences"].clone()).map_err(error)?;
                validate_shortcuts(&preferences).map_err(error)?;
                encode(peligames_mod_config::write_atomic_bytes(
                    &paths::app_path("capture-shortcuts.json").map_err(error)?,
                    &serde_json::to_vec(&preferences).map_err(error)?,
                    0o600,
                ))
            }
            "storage.inspect" => encode(capture::storage::info()),
            "storage.configure" => encode(
                self.manager
                    .configure_storage(p["directory"].as_str().map(String::from)),
            ),
            "clips.list" => encode(capture::clips(&string("gameKey"), &string("gameName"))),
            "clips.folder" => encode(capture::output_folder(
                &string("gameKey"),
                &string("gameName"),
            )),
            "clips.details" => {
                let folder = capture::output_folder(&string("gameKey"), &string("gameName"))
                    .map_err(error)?;
                {
                    let details = capture::library::details(
                        &folder,
                        std::path::Path::new(&string("path")),
                        self.manager.status().phase == "idle",
                    )
                    .map_err(error)?;
                    let preview = details.thumbnail.as_ref().and_then(|file| {
                        use base64::Engine;
                        let cache = paths::app_path("CaptureLibrary")
                            .ok()?
                            .canonicalize()
                            .ok()?;
                        let file = std::path::Path::new(file).canonicalize().ok()?;
                        if !file.starts_with(cache)
                            || std::fs::metadata(&file).ok()?.len() > 256 * 1024
                        {
                            return None;
                        }
                        let bytes = std::fs::read(file).ok()?;
                        Some(format!(
                            "data:image/jpeg;base64,{}",
                            base64::engine::general_purpose::STANDARD.encode(bytes)
                        ))
                    });
                    let mut result = serde_json::to_value(details).map_err(error)?;
                    result["preview"] = json!(preview);
                    result["thumbnail"] = Value::Null;
                    Ok(result)
                }
            }
            "clips.rename" => encode(self.manager.rename_clip(
                &string("gameKey"),
                &string("gameName"),
                std::path::Path::new(&string("path")),
                &string("title"),
            )),
            "clips.validate" => {
                let folder = capture::output_folder(&string("gameKey"), &string("gameName"))
                    .map_err(error)?;
                encode(capture::library::validate_file(
                    &folder,
                    std::path::Path::new(&string("path")),
                ))
            }
            _ => Err(RpcError {
                code: -32601,
                message: "Method not found".into(),
            }),
        }
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Some(bridge) = self.bridge.lock().unwrap().take() {
            bridge.shutdown();
        }
        self.manager.shutdown();
    }
}
fn validate_shortcuts(preferences: &capture::shortcuts::Preferences) -> Result<(), String> {
    preferences.validate()?;
    let read = |name: &str| -> Result<Value, String> {
        let path = paths::app_path(name)?;
        match std::fs::read(path) {
            Ok(bytes) if bytes.len() <= 1024 * 1024 => {
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Value::Null),
            _ => Err("Cannot read shortcut preferences".into()),
        }
    };
    let overlay = read("frame-overlay.json")?;
    if preferences
        .conflict(overlay["shortcut"].as_str().unwrap_or("F8"))
        .is_some()
    {
        return Err("This shortcut opens the PeliGames overlay".into());
    }
    let monitoring = read("MangoHUD/profile.json")?;
    if let Some(settings) = monitoring["settings"].as_object() {
        for (key, value) in settings {
            if (key.starts_with("toggle_") || key == "reset_fps_metrics")
                && value
                    .as_str()
                    .is_some_and(|v| preferences.conflict(v).is_some())
            {
                return Err("This shortcut belongs to MangoHUD".into());
            }
        }
    }
    Ok(())
}
fn main() -> std::io::Result<()> {
    let service = Service::new();
    let manifest: Manifest = serde_json::from_str(include_str!("../../manifest.json")).unwrap();
    peligames_addon_kit::service::serve(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &manifest,
        |method, params| service.handle(method, params),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn isolated_service_restores_settings_validates_context_and_releases_bridge() {
        let root =
            std::env::temp_dir().join(format!("peli-recorder-service-{}", std::process::id()));
        std::fs::create_dir_all(root.join("components/GPU-Screen-Recorder")).unwrap();
        let service = Service::new();
        let invoke = |method: &str, request: Value| {
            service.handle(method,json!({"host":{"component_directory":root.join("components/GPU-Screen-Recorder")},"request":request}))
        };
        assert_eq!(
            invoke("session.status", Value::Null).unwrap()["phase"],
            "idle"
        );
        let options = json!({"game_key":"fixture/game","game_name":"Fixture","method":"portal","monitor":"","replay_seconds":137,"replay_on_launch":true});
        invoke("settings.save", json!({"options":options})).unwrap();
        let restored = invoke("settings.read", json!({"gameKey":"fixture/game"})).unwrap();
        assert_eq!(restored["replay_seconds"], 137);
        assert_eq!(restored["replay_on_launch"], true);
        assert_eq!(restored["rate_control"], "qp");
        let diagnostic = invoke("diagnostics.inspect", json!({"gameKey":"fixture/game"})).unwrap();
        assert_eq!(diagnostic["settings"]["replay_seconds"], 137);
        assert!(!diagnostic.to_string().contains(root.to_str().unwrap()));
        assert!(diagnostic["settings"]["game_executable"].is_null());
        assert!(invoke("settings.read", json!({"gameKey":"other"}))
            .unwrap()
            .is_null());
        assert!(service
            .handle(
                "session.status",
                json!({"host":{"component_directory":"/tmp/invalid"},"request":null})
            )
            .is_err());
        assert!(invoke(
            "shortcuts.save",
            json!({"preferences":{"recording":"F8","replay":"Alt_L+F7","save_replay":"Alt_L+F6"}})
        )
        .is_err());
        let preferences =
            json!({"recording":"Control_L+F9","replay":"","save_replay":"Shift_L+F10"});
        invoke("shortcuts.save", json!({"preferences":preferences})).unwrap();
        assert_eq!(invoke("shortcuts.read", Value::Null).unwrap(), preferences);
        assert!(invoke(
            "clips.validate",
            json!({"gameKey":"fixture/game","gameName":"Fixture","path":"/etc/passwd"})
        )
        .is_err());
        assert!(
            root.join("runtime/capture-overlay.json").exists()
                || paths::app_path("capture-overlay.json").unwrap().exists()
        );
        drop(service);
        assert!(!paths::app_path("capture-overlay.json").unwrap().exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
