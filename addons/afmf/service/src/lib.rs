//! AFMF service and its explicit native game-launch entrypoint.
pub mod launcher;
use peligames_addon_kit::protocol::{RpcError, ServiceCall};
use peligames_afmf::{compatibility, runtime, Settings};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf, process::Command};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Launch {
    settings: Settings,
    runtime_version: String,
    gamescope: bool,
    #[serde(default)]
    environment: BTreeMap<String, Option<String>>,
    mangohud_directory: Option<PathBuf>,
}

pub(crate) const PRESENTATION_INPUTS: &[&str] = &[
    "VK_ADD_LAYER_PATH",
    "VK_INSTANCE_LAYERS",
    "VK_LAYER_PATH",
    "VK_ADD_IMPLICIT_LAYER_PATH",
    "MANGOHUD_CONFIGFILE",
    "MANGOHUD_CONFIG",
    "MANGOHUD",
    "DISABLE_MANGOHUD",
    "AFMF_ENABLE",
    "AFMF_ASYNC",
    "AFMF_GAMESCOPE",
    "PROTON_ENABLE_WAYLAND",
    "PELI_OVERLAY_BACKGROUND_FPS",
    "DISABLE_AFMF",
];

/// Only presentation inputs are needed; never accept the full desktop environment.
fn presentation_environment(values: &BTreeMap<String, Option<String>>) -> Result<Command, String> {
    let mut command = Command::new("afmf-launch-plan");
    for (key, value) in values {
        if !PRESENTATION_INPUTS.contains(&key.as_str())
            || value
                .as_ref()
                .is_some_and(|value| value.len() > 8192 || value.contains('\0'))
        {
            return Err("Invalid presentation environment".into());
        }
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    Ok(command)
}

fn prepare(component: PathBuf, request: Launch) -> Result<Value, String> {
    request.settings.validate()?;
    if request
        .mangohud_directory
        .as_ref()
        .is_some_and(|directory| !directory.is_absolute() || !directory.is_dir())
    {
        return Err("Choose an existing absolute monitoring directory".into());
    }
    let mut command = presentation_environment(&request.environment)?;
    command
        .env_remove("AFMF_ENABLE")
        .env_remove("AFMF_ASYNC")
        .env_remove("AFMF_GAMESCOPE");
    if request.settings.enabled {
        let path = runtime::location(&component, &request.runtime_version)?;
        let directory = runtime::prepare_manifest(&path)?;
        compatibility::layer(&mut command, &directory, None, None, None)?;
        for (key, value) in request.settings.environment(request.gamescope) {
            command.env(key, value);
        }
        if let Some(directory) = request.mangohud_directory {
            compatibility::monitoring(&mut command, &directory)?;
        }
        compatibility::apply(&mut command);
    } else if let Some(layers) = request
        .environment
        .get("VK_INSTANCE_LAYERS")
        .and_then(|v| v.as_deref())
    {
        // A cached launch environment must not retain this explicit layer after
        // its runtime has been removed. Other overlays retain their order.
        let layers = layers
            .split(':')
            .filter(|layer| *layer != "VK_LAYER_AFMF")
            .collect::<Vec<_>>()
            .join(":");
        command.env("VK_INSTANCE_LAYERS", layers);
    }
    let environment: BTreeMap<_, _> = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    serde_json::to_value(environment).map_err(|error| error.to_string())
}

pub fn call(method: &str, params: Value) -> Result<Value, RpcError> {
    let call: ServiceCall<Value> = serde_json::from_value(params).map_err(|_| RpcError {
        code: -32602,
        message: "Invalid service call".into(),
    })?;
    if !call.host.component_directory.is_absolute() {
        return Err(RpcError {
            code: -32602,
            message: "An absolute component directory is required".into(),
        });
    }
    match method {
        "game.support" => serde_json::from_value(call.request)
            .map_err(|_| "Invalid executable information".to_string())
            .and_then(peligames_afmf::support::inspect)
            .and_then(|support| serde_json::to_value(support).map_err(|error| error.to_string()))
            .map_err(|message| RpcError {
                code: -32602,
                message,
            }),
        "config.defaults" if call.request.is_null() => serde_json::to_value(Settings::default())
            .map_err(|error| RpcError {
                code: -32000,
                message: error.to_string(),
            }),
        "config.validate" => serde_json::from_value::<Settings>(call.request)
            .map_err(|_| "Invalid AFMF settings".to_string())
            .and_then(|settings| {
                settings.validate()?;
                serde_json::to_value(settings).map_err(|error| error.to_string())
            })
            .map_err(|message| RpcError {
                code: -32602,
                message,
            }),
        "launch.prepare" => serde_json::from_value::<Launch>(call.request)
            .map_err(|_| "Invalid launch request".to_string())
            .and_then(|request| prepare(call.host.component_directory, request))
            .map_err(|message| RpcError {
                code: -32000,
                message,
            }),
        _ => Err(RpcError {
            code: -32601,
            message: "Method not found".into(),
        }),
    }
}

#[cfg(test)]
mod tests;

pub struct Service {
    runtime: tokio::runtime::Runtime,
    jobs: peligames_addon_kit::jobs::Jobs,
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
        if !method.starts_with("runtime.") && method != "job.inspect" {
            return call(method, params);
        }
        let call: ServiceCall<Value> =
            serde_json::from_value(params).map_err(|_| error("Invalid service call"))?;
        let root = call.host.component_directory;
        if !root.is_absolute() {
            return Err(error("Invalid component directory"));
        }
        let encode = |result: Result<Value, String>| result.map_err(error);
        match method {
            "runtime.inspect" if call.request.is_null() => {
                serde_json::to_value(peligames_afmf::management::state(&root).map_err(error)?)
                    .map_err(|e| error(e.to_string()))
            }
            "runtime.check" => {
                let force = decode::<Check>(call.request)?.force;
                let job = self.jobs.start(&self.runtime, move |_| async move {
                    peligames_afmf::releases::check(&root, force).await?;
                    serde_json::to_value(peligames_afmf::management::state(&root)?)
                        .map_err(|e| e.to_string())
                })?;
                Ok(serde_json::json!({"job":job}))
            }
            "runtime.download" => {
                let version = decode::<Download>(call.request)?.version;
                if version
                    .as_ref()
                    .is_some_and(|value| !peligames_afmf::releases::compatible(value))
                {
                    return Err(error("Unsupported AFMF version"));
                }
                let job = self.jobs.start(&self.runtime, move |progress| async move {
                    serde_json::to_value(
                        peligames_afmf::download::install(root, version, progress).await?,
                    )
                    .map_err(|e| e.to_string())
                })?;
                Ok(serde_json::json!({"job":job}))
            }
            "runtime.select" | "runtime.remove" => {
                self.jobs.idle()?;
                let version = decode::<Selected>(call.request)?.version;
                let result = if method == "runtime.select" {
                    peligames_afmf::management::select(&root, &version)
                } else {
                    peligames_afmf::management::remove(&root, &version)
                };
                encode(
                    result.and_then(|value| serde_json::to_value(value).map_err(|e| e.to_string())),
                )
            }
            "job.inspect" => {
                serde_json::to_value(self.jobs.status(decode::<Job>(call.request)?.id)?)
                    .map_err(|error| self::error(error.to_string()))
            }
            _ => Err(error("Unknown AFMF operation")),
        }
    }
}
fn error(message: impl Into<String>) -> RpcError {
    RpcError {
        code: -32000,
        message: message.into(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Check {
    #[serde(default)]
    force: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Download {
    version: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selected {
    version: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    id: u64,
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value).map_err(|_| RpcError {
        code: -32602,
        message: "Invalid request parameters".into(),
    })
}
