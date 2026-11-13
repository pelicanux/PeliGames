use crate::protocol::{RpcError, TransferProgress as DownloadProgress};
use serde::Serialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub id: u64,
    pub done: bool,
    pub progress: Option<DownloadProgress>,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn terminal(jobs: &Jobs, id: u64) -> Status {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let status = jobs.status(id).unwrap();
            if status.done {
                return status;
            }
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
    }
    #[test]
    fn operations_are_bounded_and_terminal_failures_do_not_block_the_next_job() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .build()
            .unwrap();
        let jobs = Jobs::default();
        let (send, receive) = tokio::sync::oneshot::channel::<()>();
        let first = jobs
            .start(&runtime, move |progress| async move {
                progress(DownloadProgress {
                    phase: "downloading",
                    downloaded: 3,
                    total: Some(10),
                });
                let _ = receive.await;
                Err("Fixture download failed".into())
            })
            .unwrap();
        assert!(jobs.start(&runtime, |_| async { Ok(Value::Null) }).is_err());
        assert!(jobs.idle().is_err());
        send.send(()).unwrap();
        let done = terminal(&jobs, first);
        assert_eq!(done.error.as_deref(), Some("Fixture download failed"));
        assert_eq!(done.progress.unwrap().downloaded, 3);
        jobs.idle().unwrap();
        let second = jobs
            .start(&runtime, |_| async { Ok(Value::String("complete".into())) })
            .unwrap();
        assert!(jobs.status(first).is_err());
        assert_eq!(
            terminal(&jobs, second).result,
            Some(Value::String("complete".into()))
        );
        let panic = jobs
            .start(&runtime, |_| async { panic!("fixture task failure") })
            .unwrap();
        assert!(terminal(&jobs, panic).error.is_some());
        jobs.idle().unwrap();
    }
}
/// One bounded operation per service; terminal results survive until the next start.
#[derive(Default)]
pub struct Jobs(Arc<Mutex<Status>>);
impl Jobs {
    pub fn start<F, Fut>(&self, runtime: &tokio::runtime::Runtime, work: F) -> Result<u64, RpcError>
    where
        F: FnOnce(Arc<dyn Fn(DownloadProgress) + Send + Sync>) -> Fut,
        Fut: std::future::Future<Output = Result<Value, String>> + Send + 'static,
    {
        let mut current = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if current.id != 0 && !current.done {
            return Err(error("An Add-on operation is already running"));
        }
        let id = current
            .id
            .checked_add(1)
            .ok_or_else(|| error("Restart the Add-on service"))?;
        *current = Status {
            id,
            ..Default::default()
        };
        drop(current);
        let state = self.0.clone();
        let progress = self.0.clone();
        let future = work(Arc::new(move |value| {
            let mut status = progress.lock().unwrap_or_else(|e| e.into_inner());
            if status.id == id && !status.done {
                status.progress = Some(value);
            }
        }));
        runtime.spawn(async move {
            let result = tokio::spawn(future)
                .await
                .unwrap_or_else(|_| Err("Add-on operation failed".into()));
            let mut status = state.lock().unwrap_or_else(|e| e.into_inner());
            if status.id != id {
                return;
            }
            status.done = true;
            match result {
                Ok(value) => status.result = Some(value),
                Err(message) => status.error = Some(message),
            }
        });
        Ok(id)
    }
    pub fn status(&self, id: u64) -> Result<Status, RpcError> {
        let status = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if id == 0 || status.id != id {
            return Err(error("Add-on operation is no longer available"));
        }
        Ok(status.clone())
    }
    pub fn idle(&self) -> Result<(), RpcError> {
        let status = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if status.id != 0 && !status.done {
            return Err(error("Wait for the current Add-on operation"));
        }
        Ok(())
    }
}

fn error(message: impl Into<String>) -> RpcError {
    RpcError {
        code: -32000,
        message: message.into(),
    }
}
