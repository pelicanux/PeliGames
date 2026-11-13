//! Newline-delimited JSON-RPC 2.0. stdout carries protocol only; logs use stderr.
use crate::{manifest::HOST_API, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
pub const INITIALIZE: &str = "initialize";
pub const SHUTDOWN: &str = "shutdown";

/// Assigned by the launcher from the selected reviewed recipe, never by the UI.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceContext {
    pub component_directory: std::path::PathBuf,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceCall<T> {
    pub host: ServiceContext,
    pub request: T,
}

/// Native launch entrypoint. The verified service replaces itself with the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchContext {
    pub component_directory: std::path::PathBuf,
    pub settings: Value,
    pub gamescope: bool,
    pub monitoring: bool,
    pub options: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Value,
}
impl Request {
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            method: method.into(),
            params,
        }
    }
    pub fn parse(line: &str) -> Result<Self> {
        if line.len() > MAX_MESSAGE_BYTES {
            return Err("Add-on request is too large".into());
        }
        let request: Self = serde_json::from_str(line).map_err(|e| e.to_string())?;
        if request.jsonrpc != "2.0" || request.method.is_empty() || request.method.len() > 120 {
            return Err("Invalid add-on request".into());
        }
        Ok(request)
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Initialize {
    pub host_api: u32,
    pub addon_id: String,
    pub addon_version: String,
}
impl Initialize {
    pub fn validate(&self, id: &str, version: &str) -> Result<()> {
        if self.host_api != HOST_API || self.addon_id != id || self.addon_version != version {
            return Err("Add-on service identity or API mismatch".into());
        }
        Ok(())
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}
impl Response {
    pub fn success(id: u64, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }
    pub fn failure(id: u64, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
    /// Validate envelope first: null is a valid JSON-RPC result, not a missing result.
    pub fn parse(line: &str, id: u64) -> Result<Value> {
        match Self::decode(line, id)? {
            Ok(value) => Ok(value),
            Err(error) => Err(format!("Add-on error {}: {}", error.code, error.message)),
        }
    }
    /// Keep a valid remote error distinct from a broken transport/session.
    pub fn decode(line: &str, id: u64) -> Result<std::result::Result<Value, RpcError>> {
        if line.len() > MAX_MESSAGE_BYTES {
            return Err("Add-on response is too large".into());
        }
        let value: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let object = value.as_object().ok_or("Invalid add-on response")?;
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || object.get("id").and_then(Value::as_u64) != Some(id)
            || object.contains_key("result") == object.contains_key("error")
        {
            return Err("Invalid add-on response envelope".into());
        }
        let response: Self = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if object.contains_key("error") {
            let error = response.error.ok_or("Invalid add-on error response")?;
            return Ok(Err(error));
        }
        Ok(Ok(object.get("result").cloned().unwrap_or(Value::Null)))
    }
}

#[derive(Clone, Default, Serialize)]
pub struct TransferProgress {
    pub phase: &'static str,
    pub downloaded: u64,
    pub total: Option<u64>,
}
