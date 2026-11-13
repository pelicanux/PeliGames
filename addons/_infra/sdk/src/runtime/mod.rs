//! Lazy services with one bounded actor/queue per integration. This is process
//! isolation and lifecycle management, not an operating-system sandbox.
mod actor;
mod host;
mod selection;
#[cfg(all(test, target_os = "linux"))]
mod tests;
mod transport;

pub use host::{Pending, ServiceHost};
use serde::Serialize;
use std::time::Duration;

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Unavailable,
    InvalidRequest,
    Verification,
    Startup,
    Protocol,
    Remote,
    Timeout,
    Crashed,
    Stopped,
    Busy,
}
#[derive(Debug, Clone, Serialize)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub remote_code: Option<i32>,
}
impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            remote_code: None,
        }
    }
    fn verification(_details: impl Into<String>) -> Self {
        Self::new(
            ErrorKind::Verification,
            "Add-on selection or payload verification failed",
        )
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for Error {}

#[derive(Clone)]
pub struct Limits {
    pub startup: Duration,
    pub request: Duration,
    pub shutdown: Duration,
    pub reap: Duration,
    pub queue: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            startup: Duration::from_secs(10),
            request: Duration::from_secs(15),
            shutdown: Duration::from_secs(1),
            reap: Duration::from_secs(1),
            queue: 8,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub id: String,
    pub version: String,
    pub phase: Phase,
    pub pid: Option<u32>,
    pub last_error: Option<ErrorKind>,
}
