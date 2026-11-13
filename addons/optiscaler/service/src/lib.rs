//! OptiScaler owns its operations. The launcher supplies only managed storage and
//! checks game writability before dispatching the generic `game.mutate` route.
use peligames_addon_kit::jobs;
use peligames_addon_kit::protocol::{RpcError, ServiceCall};
use peligames_optiscaler::{self as optiscaler, channel::Channel, runtime, settings::Settings};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io, path::PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Game {
    directory: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Patch {
    text: String,
    key: String,
    value: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selected {
    channel: Channel,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    id: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    directory: PathBuf,
    action: Action,
}
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
enum Action {
    Install { proxy: String, settings: Settings },
    Upgrade,
    Remove,
    Update { key: String, value: String },
    Patcher { enabled: bool },
}
pub(crate) fn error(message: impl Into<String>) -> RpcError {
    RpcError {
        code: -32000,
        message: message.into(),
    }
}
fn params<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value).map_err(|_| RpcError {
        code: -32602,
        message: "Invalid request parameters".into(),
    })
}
fn result(value: Result<impl Serialize, String>) -> Result<Value, RpcError> {
    serde_json::to_value(value.map_err(error)?).map_err(|_| error("Cannot encode service result"))
}
pub struct Service {
    runtime: tokio::runtime::Runtime,
    jobs: jobs::Jobs,
}
impl Service {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            runtime: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
            jobs: jobs::Jobs::default(),
        })
    }
    pub fn handle(&mut self, method: &str, value: Value) -> Result<Value, RpcError> {
        let ServiceCall { host, request }: ServiceCall<Value> = params(value)?;
        if !host.component_directory.is_absolute() {
            return Err(error("Invalid managed component directory"));
        }
        let root = host.component_directory;
        match method {
            "game.inspect" => {
                let game: Game = params(request)?;
                result(optiscaler::state(
                    &game.directory,
                    &runtime::component(&root).map_err(error)?,
                ))
            }
            "game.mutate" => {
                self.jobs.idle()?;
                let Mutation { directory, action } = params(request)?;
                if !directory.is_absolute() || !directory.is_dir() {
                    return Err(error("Choose an absolute game directory"));
                }
                match action {
                    Action::Install { proxy, settings } => result(optiscaler::install(
                        &runtime::component(&root).map_err(error)?,
                        &directory,
                        &proxy,
                        &settings,
                    )),
                    Action::Upgrade => result(optiscaler::upgrade(
                        &runtime::component(&root).map_err(error)?,
                        &directory,
                    )),
                    Action::Remove => result(optiscaler::remove(&directory)),
                    Action::Update { key, value } => {
                        result(optiscaler::update(&directory, &key, &value))
                    }
                    Action::Patcher { enabled } => {
                        let id = self.jobs.start(&self.runtime, move |_| async move {
                            runtime::patcher_set(root, directory, enabled)
                                .await
                                .map(|_| Value::Null)
                        })?;
                        Ok(json!({"job":id}))
                    }
                }
            }
            "runtime.inspect" => result(self.runtime.block_on(runtime::state(&root))),
            "runtime.channel" => {
                self.jobs.idle()?;
                let selected: Selected = params(request)?;
                result(
                    self.runtime
                        .block_on(runtime::set_channel(&root, selected.channel)),
                )
            }
            "runtime.remove" => {
                self.jobs.idle()?;
                let selected: Selected = params(request)?;
                result(
                    self.runtime
                        .block_on(runtime::remove(root, selected.channel)),
                )
            }
            "runtime.download" => {
                let selected: Selected = params(request)?;
                let id = self.jobs.start(&self.runtime, move |progress| async move {
                    runtime::download(root, selected.channel, progress)
                        .await
                        .map(|_| Value::Null)
                })?;
                Ok(json!({"job":id}))
            }
            "job.inspect" => {
                let job: Job = params(request)?;
                result(self.jobs.status(job.id).map_err(|e| e.message))
            }
            "config.inspect" => {
                let config: Config = params(request)?;
                result(Ok(optiscaler::settings::read(&config.text)))
            }
            "config.patch" => {
                let patch: Patch = params(request)?;
                result(optiscaler::settings::patch(
                    &patch.text,
                    &patch.key,
                    &patch.value,
                ))
            }
            _ => Err(RpcError {
                code: -32601,
                message: "Method not found".into(),
            }),
        }
    }
}
