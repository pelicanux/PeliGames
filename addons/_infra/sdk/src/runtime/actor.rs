use super::{
    host::Job, selection::Selection, transport::Session, Error, ErrorKind, Limits, Phase, Status,
};
use crate::{
    protocol::{self, Initialize},
    registry::Store,
};
use serde_json::json;
use tokio::sync::{mpsc, watch};

async fn cancelled(stop: &mut watch::Receiver<bool>) {
    while !*stop.borrow() {
        if stop.changed().await.is_err() {
            return;
        }
    }
}
struct Completion(watch::Sender<bool>);
impl Drop for Completion {
    fn drop(&mut self) {
        let _ = self.0.send(true);
    }
}
fn report(
    status: &watch::Sender<Status>,
    phase: Phase,
    pid: Option<u32>,
    error: Option<ErrorKind>,
) {
    status.send_modify(|status| {
        status.phase = phase;
        status.pid = pid;
        status.last_error = error;
    });
}
pub(super) async fn serve(
    store: Store,
    selection: Selection,
    host: String,
    platform: String,
    mut queue: mpsc::Receiver<Job>,
    mut stop: watch::Receiver<bool>,
    status: watch::Sender<Status>,
    done: watch::Sender<bool>,
    mut limits: Limits,
) {
    let background = selection
        .manifest
        .capabilities
        .contains(&crate::manifest::Capability::BackgroundService);
    if background {
        limits.shutdown = std::time::Duration::from_secs(8);
        limits.request = std::time::Duration::from_secs(30);
    }
    let _completion = Completion(done);
    let deadline = tokio::time::Instant::now() + limits.startup;
    let stopped = || Error::new(ErrorKind::Stopped, "Add-on service stopped");
    let verification = tokio::select! { biased;
        _ = cancelled(&mut stop) => Err(stopped()),
        result = tokio::time::timeout_at(deadline, selection.verify(store, host, platform)) => result.unwrap_or_else(|_| Err(Error::new(ErrorKind::Timeout, "Add-on verification timed out"))),
    };
    let session = match verification {
        Ok(verified) => {
            tokio::select! { biased;
                _ = cancelled(&mut stop) => Err(stopped()),
                result = tokio::time::timeout_at(deadline, Session::spawn(verified)) => result.unwrap_or_else(|_| Err(Error::new(ErrorKind::Timeout, "Add-on startup timed out"))),
            }
        }
        Err(error) => Err(error),
    };
    let mut session = match session {
        Ok(session) => session,
        Err(error) => {
            finish(&mut queue, &status, &error);
            return;
        }
    };
    let hello = tokio::select! { biased;
        _ = cancelled(&mut stop) => Err(stopped()),
        result = session.exchange(protocol::INITIALIZE, json!({"host_api":crate::manifest::HOST_API}), deadline.saturating_duration_since(tokio::time::Instant::now())) => result,
    }.and_then(|value| {
        let identity: Initialize = serde_json::from_value(value).map_err(|_| Error::new(ErrorKind::Protocol, "Invalid service initialization"))?;
        identity.validate(&selection.manifest.id, &selection.manifest.version).map_err(|_| Error::new(ErrorKind::Protocol, "Service identity or API mismatch"))
    });
    if let Err(error) = hello {
        session.stop(false, &limits).await;
        finish(&mut queue, &status, &error);
        return;
    }
    report(&status, Phase::Running, session.pid(), None);
    let mut graceful = true;
    let failure = loop {
        tokio::select! { biased;
            _ = cancelled(&mut stop) => break stopped(),
            _ = session.child.wait() => { graceful = false; break Error::new(ErrorKind::Crashed, "Add-on service exited"); }
            job = queue.recv() => {
                let Some(job) = job else { break stopped(); };
                if job.reply.is_closed() { continue; }
                let result = tokio::select! { biased;
                    _ = cancelled(&mut stop) => { graceful = background; Err(stopped()) },
                    result = session.exchange(&job.method, job.params, limits.request) => result,
                };
                let fatal = result.as_ref().err().filter(|error| error.kind != ErrorKind::Remote && error.kind != ErrorKind::InvalidRequest).cloned();
                let _ = job.reply.send(result);
                if let Some(error) = fatal { graceful = background && error.kind == ErrorKind::Stopped; break error; }
            }
        }
    };
    report(&status, Phase::Stopping, session.pid(), Some(failure.kind));
    session.stop(graceful, &limits).await;
    finish(&mut queue, &status, &failure);
}
fn finish(queue: &mut mpsc::Receiver<Job>, status: &watch::Sender<Status>, error: &Error) {
    queue.close();
    while let Ok(job) = queue.try_recv() {
        let _ = job.reply.send(Err(error.clone()));
    }
    report(
        status,
        if error.kind == ErrorKind::Stopped {
            Phase::Stopped
        } else {
            Phase::Failed
        },
        None,
        if error.kind == ErrorKind::Stopped {
            None
        } else {
            Some(error.kind)
        },
    );
}
