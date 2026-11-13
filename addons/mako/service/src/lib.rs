pub mod launcher;
use peligames_addon_kit::{
    jobs::Jobs,
    protocol::{RpcError, ServiceCall},
};
use peligames_mako::{self as mako, Settings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
fn error(message: impl Into<String>) -> RpcError {
    RpcError {
        code: -32000,
        message: message.into(),
    }
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value).map_err(|e| error(format!("Invalid MAKO request: {e}")))
}
fn encode(value: Result<impl Serialize, String>) -> Result<Value, RpcError> {
    serde_json::to_value(value.map_err(error)?).map_err(|e| error(e.to_string()))
}
#[derive(Deserialize)]
struct Runtime {
    version: Option<String>,
}
#[derive(Deserialize)]
struct Selected {
    version: String,
}
#[derive(Deserialize)]
struct Job {
    id: u64,
}
#[derive(Deserialize)]
struct Inspect {
    key: String,
    label: String,
    profiles: Vec<PathBuf>,
}
#[derive(Deserialize)]
struct Prepare {
    key: String,
    label: String,
    directory: PathBuf,
    settings: Settings,
    profile: Option<PathBuf>,
    options: Vec<String>,
}
#[derive(Deserialize)]
struct Live {
    profile: PathBuf,
    settings: Settings,
}
#[derive(Deserialize)]
struct Sandbox {
    key: String,
    dll: String,
    version: String,
}
#[derive(Deserialize)]
struct Libraries {
    libraries: Vec<PathBuf>,
}
pub struct Service {
    runtime: tokio::runtime::Runtime,
    jobs: Jobs,
}
impl Service {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
            jobs: Default::default(),
        })
    }
    pub fn handle(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let ServiceCall { host, request }: ServiceCall<Value> = decode(params)?;
        let root = host.component_directory;
        peligames_addon_kit::files::check_directory(&root).map_err(error)?;
        match method {
            "runtime.inspect" => encode(mako::game::inspect(&root, "runtime", "MAKO", None)),
            "runtime.check" => {
                let id = self.jobs.start(&self.runtime, move |_| async move {
                    serde_json::to_value(
                        mako::versions::check_updates(&root, "mako".into(), true).await?,
                    )
                    .map_err(|e| e.to_string())
                })?;
                Ok(json!({"job":id}))
            }
            "runtime.download" => {
                let selected: Runtime = decode(request)?;
                let id = self.jobs.start(&self.runtime, move |progress| async move {
                    mako::management::install(
                        root,
                        "mako".into(),
                        selected.version,
                        std::sync::Arc::new(move |p| {
                            progress(peligames_addon_kit::protocol::TransferProgress {
                                phase: p.phase,
                                downloaded: p.downloaded,
                                total: p.total,
                            })
                        }),
                    )
                    .await?;
                    Ok(Value::Null)
                })?;
                Ok(json!({"job":id}))
            }
            "runtime.remove" => {
                self.jobs.idle()?;
                encode(mako::versions::uninstall(&root, "mako".into()))
            }
            "runtime.remove-backup" => {
                self.jobs.idle()?;
                encode(mako::versions::remove_backup(
                    &root,
                    "mako".into(),
                    decode::<Selected>(request)?.version,
                ))
            }
            "dll.detect" => {
                let libraries = decode::<Libraries>(request)?.libraries;
                let found = libraries
                    .into_iter()
                    .map(|path| path.join("steamapps/common/Lossless Scaling/Lossless.dll"))
                    .find(|path| mako::dll_valid(path));
                encode(Ok(found))
            }
            "game.inspect" => {
                let own: Inspect = decode(request)?;
                let profile = own
                    .profiles
                    .iter()
                    .find(|path| mako::profiles::checked_profile(&root, path).is_ok());
                encode(mako::game::inspect(
                    &root,
                    &own.key,
                    &own.label,
                    profile.map(PathBuf::as_path),
                ))
            }
            "config.prepare" => {
                self.jobs.idle()?;
                let own: Prepare = decode(request)?;
                if own.settings.enabled {
                    mako::support::constraints(&own.options).map_err(error)?;
                }
                let profile = mako::profiles::prepare(
                    &root,
                    &own.key,
                    &own.label,
                    &own.directory,
                    own.settings.clone(),
                    own.profile.as_deref(),
                )
                .map_err(error)?;
                let settings = mako::profiles::read(&root, &profile).map_err(error)?;
                let mut data = serde_json::to_value(settings).map_err(|e| error(e.to_string()))?;
                data["profile"] = json!(profile);
                data["game_key"] = json!(own.key);
                data["game_name"] = json!(own.label);
                data["directory"] = json!(own.directory);
                Ok(data)
            }
            "config.live" => {
                self.jobs.idle()?;
                let own: Live = decode(request)?;
                encode(mako::profiles::update_live(
                    &root,
                    own.profile.to_string_lossy().into(),
                    own.settings,
                ))
            }
            "config.defaults" => encode(Ok(Settings::default())),
            "config.validate" => {
                let own: Settings = decode(request)?;
                mako::settings::check_settings(&own).map_err(error)?;
                encode(Ok(own))
            }
            "flatpak.inspect" => {
                let own: Sandbox = decode(request)?;
                encode(mako::flatpak::inspect(
                    &root,
                    &own.key,
                    &own.dll,
                    &own.version,
                ))
            }
            "job.inspect" => encode(
                self.jobs
                    .status(decode::<Job>(request)?.id)
                    .map_err(|e| e.message),
            ),
            _ => Err(RpcError {
                code: -32601,
                message: "Method not found".into(),
            }),
        }
    }
}
