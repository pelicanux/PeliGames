use peligames_addon_kit::{
    jobs::Jobs,
    protocol::{RpcError, ServiceCall},
};
use peligames_dlssnr_mochizuki::{game, runtime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
fn error(message: impl Into<String>) -> RpcError {
    RpcError {
        code: -32000,
        message: bounded(message.into()),
    }
}
fn decode<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, RpcError> {
    serde_json::from_value(value).map_err(|_| error("Invalid DLSSNR request"))
}
fn encode(value: impl Serialize) -> Result<Value, RpcError> {
    serde_json::to_value(value).map_err(|e| error(e.to_string()))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    path: PathBuf,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Game {
    directory: PathBuf,
    bits: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    directory: PathBuf,
    bits: String,
    action: game::Action,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    id: u64,
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
        if !root.is_absolute() {
            return Err(error("Invalid managed component directory"));
        }
        if [
            "runtime.inspect",
            "runtime.check",
            "runtime.download",
            "runtime.remove",
        ]
        .contains(&method)
            && !request.is_null()
        {
            return Err(error("This operation accepts no parameters"));
        }
        match method {
            "runtime.inspect" => encode(runtime::state(&root).map_err(error)?),
            "runtime.check" => {
                let id = self.jobs.start(&self.runtime, |_| async move {
                    runtime::latest()
                        .await
                        .map(|text| Value::String(bounded(text)))
                        .map_err(bounded)
                })?;
                Ok(json!({"job":id}))
            }
            "runtime.download" => {
                let id = self.jobs.start(&self.runtime, move |progress| async move {
                    serde_json::to_value(runtime::download(root, progress).await?)
                        .map_err(|e| e.to_string())
                })?;
                Ok(json!({"job":id}))
            }
            "runtime.import" | "model.import" | "model.extract" => {
                let source: Source = decode(request)?;
                let method = method.to_owned();
                let id = self.jobs.start(&self.runtime, move |_| async move {
                    tokio::task::spawn_blocking(move || {
                        let result = match method.as_str() {
                            "runtime.import" => runtime::import(&root, &source.path),
                            "model.import" => runtime::import_models(&root, &source.path),
                            _ => runtime::extract(&root, &source.path),
                        }
                        .map_err(bounded)?;
                        serde_json::to_value(result).map_err(|e| e.to_string())
                    })
                    .await
                    .map_err(|e| e.to_string())?
                })?;
                Ok(json!({"job":id}))
            }
            "runtime.remove" => {
                self.jobs.idle()?;
                encode(runtime::remove(&root).map_err(error)?)
            }
            "game.inspect" => {
                let game: Game = decode(request)?;
                encode(game::inspect(&root, &game.directory, &game.bits).map_err(error)?)
            }
            "game.mutate" => {
                let game: Mutation = decode(request)?;
                let id = self.jobs.start(&self.runtime, move |_| async move {
                    tokio::task::spawn_blocking(move || {
                        game::mutate(&root, &game.directory, &game.bits, game.action)
                    })
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|text| Value::String(bounded(text)))
                    .map_err(bounded)
                })?;
                Ok(json!({"job":id}))
            }
            "job.inspect" => encode(self.jobs.status(decode::<Job>(request)?.id)?),
            _ => Err(RpcError {
                code: -32601,
                message: "Method not found".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rpc_inspection_is_lazy_and_rejects_invalid_requests() {
        let root =
            std::env::temp_dir().join(format!("peligames-mochi-service-{}", std::process::id()));
        let service = Service::new().unwrap();
        let request = |value| json!({"host":{"component_directory":root},"request":value});
        let result = service
            .handle("runtime.inspect", request(Value::Null))
            .unwrap();
        assert_eq!(result["ready"], false);
        assert!(!root.exists());
        assert!(service
            .handle("runtime.remove", request(json!({"unexpected":true})))
            .is_err());
        assert!(service.handle("game.mutate",request(json!({"directory":"relative","bits":"invalid","action":{"operation":"execute","script":"anything"}}))).is_err());
    }
}

fn bounded(text: String) -> String {
    if text.len() <= 96 * 1024 {
        return text;
    }
    let tail: String = text
        .chars()
        .rev()
        .take(96 * 1024)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("[Earlier log truncated]\n{tail}")
}
