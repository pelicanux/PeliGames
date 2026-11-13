use super::{selection::Verified, Error, ErrorKind, Limits, Result};
use crate::protocol::{Request, Response, MAX_MESSAGE_BYTES};
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

pub(super) struct Session {
    pub child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    stderr: tokio::task::JoinHandle<()>,
    next_id: u64,
    #[cfg(unix)]
    group: Option<i32>,
}
fn command(verified: &Verified) -> Command {
    let mut command = Command::new(&verified.executable);
    command
        .current_dir(&verified.directory)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Inherit desktop connectivity, not API credentials or AppImage loader hooks.
    for key in [
        "HOME",
        "LANG",
        "LANGUAGE",
        "LC_ALL",
        "LC_CTYPE",
        "TZ",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_STATE_HOME",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        let bundle = std::env::var_os("APPDIR").map(std::path::PathBuf::from);
        let paths = std::env::split_paths(&path).filter(|p| {
            !bundle
                .as_ref()
                .is_some_and(|b| !b.as_os_str().is_empty() && p.starts_with(b))
        });
        if let Ok(path) = std::env::join_paths(paths) {
            command.env("PATH", path);
        }
    }
    #[cfg(unix)]
    command.process_group(0);
    command
}
impl Session {
    pub async fn spawn(verified: Verified) -> Result<Self> {
        let mut attempts = 0;
        let mut child = loop {
            match command(&verified).spawn() {
                Ok(child) => break child,
                Err(error) if cfg!(unix) && error.raw_os_error() == Some(26) && attempts < 5 => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(_) => {
                    return Err(Error::new(
                        ErrorKind::Startup,
                        "Could not start the add-on service",
                    ))
                }
            }
        };
        let input = child.stdin.take().expect("piped stdin");
        let output = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut stderr = child.stderr.take().expect("piped stderr");
        // Continuously drain diagnostics with bounded buffers; never copy raw
        // service output or request parameters into launcher reports.
        let stderr = tokio::spawn(async move {
            let mut buffer = [0_u8; 4096];
            while let Ok(n) = stderr.read(&mut buffer).await {
                if n == 0 {
                    break;
                }
            }
        });
        #[cfg(unix)]
        let group = child.id().and_then(|pid| i32::try_from(pid).ok());
        Ok(Self {
            child,
            input: Some(input),
            output,
            stderr,
            next_id: 0,
            #[cfg(unix)]
            group,
        })
    }
    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }
    pub async fn exchange(
        &mut self,
        method: &str,
        params: Value,
        duration: Duration,
    ) -> Result<Value> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Protocol, "Service request ID exhausted"))?;
        let id = self.next_id;
        let mut bytes = serde_json::to_vec(&Request::new(id, method, params))
            .map_err(|_| Error::new(ErrorKind::InvalidRequest, "Invalid add-on request"))?;
        bytes.push(b'\n');
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(Error::new(
                ErrorKind::InvalidRequest,
                "Add-on request is too large",
            ));
        }
        let result = tokio::time::timeout(duration, async {
            let input = self
                .input
                .as_mut()
                .ok_or_else(|| Error::new(ErrorKind::Stopped, "Service input is closed"))?;
            input
                .write_all(&bytes)
                .await
                .map_err(|_| Error::new(ErrorKind::Crashed, "Service closed its input"))?;
            input
                .flush()
                .await
                .map_err(|_| Error::new(ErrorKind::Crashed, "Service closed its input"))?;
            let mut line = Vec::new();
            loop {
                let available = self.output.fill_buf().await.map_err(|_| {
                    Error::new(ErrorKind::Protocol, "Could not read service response")
                })?;
                if available.is_empty() {
                    return Err(Error::new(ErrorKind::Crashed, "Service closed its output"));
                }
                let n = available
                    .iter()
                    .position(|b| *b == b'\n')
                    .map(|at| at + 1)
                    .unwrap_or(available.len());
                if line.len() + n > MAX_MESSAGE_BYTES {
                    return Err(Error::new(
                        ErrorKind::Protocol,
                        "Service response is too large",
                    ));
                }
                let complete = available[n - 1] == b'\n';
                line.extend_from_slice(&available[..n]);
                self.output.consume(n);
                if complete {
                    break;
                }
            }
            let line = std::str::from_utf8(&line)
                .map_err(|_| Error::new(ErrorKind::Protocol, "Service returned invalid UTF-8"))?;
            match Response::decode(line, id)
                .map_err(|_| Error::new(ErrorKind::Protocol, "Invalid service response envelope"))?
            {
                Ok(value) => Ok(value),
                Err(error) => Err(Error {
                    kind: ErrorKind::Remote,
                    message: error.message.chars().take(2048).collect(),
                    remote_code: Some(error.code),
                }),
            }
        })
        .await;
        result.unwrap_or_else(|_| {
            Err(Error::new(
                ErrorKind::Timeout,
                "Add-on service response timed out",
            ))
        })
    }
    fn kill(&mut self) {
        #[cfg(unix)]
        if let Some(group) = self.group.take() {
            // Every child is started in its own process group; descendants must
            // not keep running or hold a pipe open after the add-on stops.
            unsafe {
                libc::kill(-group, libc::SIGKILL);
            }
        }
        let _ = self.child.start_kill();
    }
    pub async fn stop(&mut self, graceful: bool, limits: &Limits) {
        if graceful {
            let _ = tokio::time::timeout(limits.shutdown, async {
                let _ = self
                    .exchange(crate::protocol::SHUTDOWN, Value::Null, limits.shutdown)
                    .await;
                self.input.take();
                let _ = self.child.wait().await;
            })
            .await;
        }
        self.input.take();
        self.kill();
        let _ = tokio::time::timeout(limits.reap, self.child.wait()).await;
        self.stderr.abort();
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.kill();
        self.stderr.abort();
    }
}
