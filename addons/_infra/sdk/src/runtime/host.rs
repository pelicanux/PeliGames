use super::{actor::serve, selection::Selection, Error, ErrorKind, Limits, Phase, Result, Status};
use crate::{
    protocol::{self, Request},
    registry::{State, Store},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot, watch};

pub(super) struct Job {
    pub(super) method: String,
    pub(super) params: Value,
    pub(super) reply: oneshot::Sender<Result<Value>>,
}
#[derive(Clone)]
struct Slot {
    selection: Selection,
    jobs: mpsc::Sender<Job>,
    stop: watch::Sender<bool>,
    status: watch::Receiver<Status>,
    done: watch::Receiver<bool>,
    generation: Arc<()>,
}
pub struct Pending {
    reply: oneshot::Receiver<Result<Value>>,
}
impl Pending {
    pub async fn wait(self) -> Result<Value> {
        self.reply.await.unwrap_or_else(|_| {
            Err(Error::new(
                ErrorKind::Crashed,
                "Add-on service stopped before replying",
            ))
        })
    }
}
pub struct ServiceHost {
    slots: Mutex<BTreeMap<String, Slot>>,
    closing: AtomicBool,
    limits: Limits,
}
impl Default for ServiceHost {
    fn default() -> Self {
        Self::new(Limits::default())
    }
}
impl ServiceHost {
    pub fn new(mut limits: Limits) -> Self {
        limits.queue = limits.queue.clamp(1, 32);
        Self {
            slots: Mutex::new(BTreeMap::new()),
            closing: AtomicBool::new(false),
            limits,
        }
    }
    /// Caller holds its registry-operation lock until submission finishes. Waiting
    /// for a response does not hold that lock, and cancelling the caller cannot
    /// leave the transport halfway through a request.
    pub fn submit(
        &self,
        store: Store,
        state: &State,
        id: &str,
        method: &str,
        params: Value,
        host: &str,
        platform: &str,
    ) -> Result<Pending> {
        validate_request(method, &params)?;
        if self.closing.load(Ordering::Acquire) {
            return Err(Error::new(
                ErrorKind::Stopped,
                "Add-on host is shutting down",
            ));
        }
        let selection = Selection::read(state, id, host, platform)?;
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| Error::new(ErrorKind::Startup, "No async runtime for add-on services"))?;
        let mut slots = self
            .slots
            .lock()
            .map_err(|_| Error::new(ErrorKind::Startup, "Add-on host lock failed"))?;
        if self.closing.load(Ordering::Acquire) {
            return Err(Error::new(
                ErrorKind::Stopped,
                "Add-on host is shutting down",
            ));
        }
        if let Some(slot) = slots.get(id) {
            if slot.selection != selection || *slot.stop.borrow() {
                return Err(Error::new(
                    ErrorKind::Stopped,
                    "Stop the previous service before changing its selection",
                ));
            }
            if *slot.done.borrow()
                || slot.jobs.is_closed()
                || !matches!(slot.status.borrow().phase, Phase::Starting | Phase::Running)
            {
                return Err(Error::new(
                    ErrorKind::Unavailable,
                    "Service stopped; explicitly stop it before retrying",
                ));
            }
        } else {
            let (jobs, queue) = mpsc::channel(self.limits.queue);
            let (stop, stopping) = watch::channel(false);
            let initial = Status {
                id: id.into(),
                version: selection.manifest.version.clone(),
                phase: Phase::Starting,
                pid: None,
                last_error: None,
            };
            let (status, statuses) = watch::channel(initial);
            let (done, completion) = watch::channel(false);
            runtime.spawn(serve(
                store,
                selection.clone(),
                host.into(),
                platform.into(),
                queue,
                stopping,
                status,
                done,
                self.limits.clone(),
            ));
            slots.insert(
                id.into(),
                Slot {
                    selection,
                    jobs,
                    stop,
                    status: statuses,
                    done: completion,
                    generation: Arc::new(()),
                },
            );
        }
        let slot = &slots[id];
        let (reply, response) = oneshot::channel();
        slot.jobs
            .try_send(Job {
                method: method.into(),
                params,
                reply,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => {
                    Error::new(ErrorKind::Busy, "Add-on service queue is full")
                }
                mpsc::error::TrySendError::Closed(_) => {
                    Error::new(ErrorKind::Unavailable, "Add-on service is not running")
                }
            })?;
        Ok(Pending { reply: response })
    }
    pub fn statuses(&self) -> Vec<Status> {
        self.slots
            .lock()
            .map(|slots| {
                slots
                    .values()
                    .map(|slot| {
                        let mut status = slot.status.borrow().clone();
                        if *slot.done.borrow()
                            && matches!(
                                status.phase,
                                Phase::Starting | Phase::Running | Phase::Stopping
                            )
                        {
                            status.phase = Phase::Failed;
                            status.pid = None;
                            status.last_error = Some(ErrorKind::Crashed);
                        }
                        status
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn uses_selection(&self, id: &str, expected: &crate::registry::SelectionSnapshot) -> bool {
        self.slots.lock().is_ok_and(|slots| {
            slots
                .get(id)
                .is_some_and(|slot| &slot.selection.versions == expected)
        })
    }
    pub async fn stop(&self, id: &str) -> Result<()> {
        let slot = self
            .slots
            .lock()
            .map_err(|_| Error::new(ErrorKind::Startup, "Add-on host lock failed"))?
            .get(id)
            .cloned();
        let Some(slot) = slot else {
            return Ok(());
        };
        let _ = slot.stop.send(true);
        let mut limits = self.limits.clone();
        if slot
            .selection
            .manifest
            .capabilities
            .contains(&crate::manifest::Capability::BackgroundService)
        {
            limits.shutdown = Duration::from_secs(8);
        }
        wait_done(slot.done.clone(), &limits).await?;
        let mut slots = self
            .slots
            .lock()
            .map_err(|_| Error::new(ErrorKind::Startup, "Add-on host lock failed"))?;
        if slots
            .get(id)
            .is_some_and(|current| Arc::ptr_eq(&current.generation, &slot.generation))
        {
            slots.remove(id);
        }
        Ok(())
    }
    pub async fn reconcile(&self, state: &State) -> Result<()> {
        let ids: Vec<_> = self
            .slots
            .lock()
            .map_err(|_| Error::new(ErrorKind::Startup, "Add-on host lock failed"))?
            .iter()
            .filter(|(_, slot)| !slot.selection.current(state))
            .map(|(id, _)| id.clone())
            .collect();
        // Signal all affected workers first; their cleanup proceeds concurrently.
        self.signal(&ids);
        for id in ids {
            self.stop(&id).await?;
        }
        Ok(())
    }
    fn signal(&self, ids: &[String]) {
        if let Ok(slots) = self.slots.lock() {
            for id in ids {
                if let Some(slot) = slots.get(id) {
                    let _ = slot.stop.send(true);
                }
            }
        }
    }
    pub async fn shutdown(&self) -> Result<()> {
        self.closing.store(true, Ordering::Release);
        let ids = self
            .slots
            .lock()
            .map_err(|_| Error::new(ErrorKind::Startup, "Add-on host lock failed"))?
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        self.signal(&ids);
        tokio::time::timeout(
            Duration::from_secs(8).max(self.limits.shutdown)
                + self.limits.reap
                + Duration::from_millis(500),
            async {
                for id in ids {
                    self.stop(&id).await?;
                }
                Ok(())
            },
        )
        .await
        .unwrap_or_else(|_| Err(Error::new(ErrorKind::Timeout, "Add-on shutdown timed out")))
    }
}
impl Drop for ServiceHost {
    fn drop(&mut self) {
        if let Ok(slots) = self.slots.get_mut() {
            for slot in slots.values() {
                let _ = slot.stop.send(true);
            }
        }
    }
}
fn validate_request(method: &str, params: &Value) -> Result<()> {
    if method.is_empty()
        || method.len() > 120
        || [protocol::INITIALIZE, protocol::SHUTDOWN].contains(&method)
        || !method
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
    {
        return Err(Error::new(
            ErrorKind::InvalidRequest,
            "Invalid or reserved service method",
        ));
    }
    let bytes = serde_json::to_vec(&Request::new(u64::MAX, method, params.clone()))
        .map_err(|_| Error::new(ErrorKind::InvalidRequest, "Invalid service request"))?;
    if bytes.len() + 1 > protocol::MAX_MESSAGE_BYTES {
        return Err(Error::new(
            ErrorKind::InvalidRequest,
            "Add-on request is too large",
        ));
    }
    Ok(())
}
async fn wait_done(mut done: watch::Receiver<bool>, limits: &Limits) -> Result<()> {
    tokio::time::timeout(
        limits.shutdown + limits.reap + Duration::from_millis(250),
        async {
            while !*done.borrow() {
                if done.changed().await.is_err() {
                    break;
                }
            }
        },
    )
    .await
    .map_err(|_| Error::new(ErrorKind::Timeout, "Service cleanup timed out"))
}
