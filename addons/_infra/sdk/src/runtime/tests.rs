use super::*;
use crate::{
    catalog::{Artifact, Entry},
    manifest::Manifest,
    package, protocol,
    registry::Store,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const PLATFORM: &str = "linux-x86_64";
const HOST: &str = "0.5.5";
struct Fixture {
    root: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "peligames-service-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let store = Store {
            packages: root.join("addons"),
            state_file: root.join("settings/addons.json"),
        };
        Self { root, store }
    }
    fn prepare(
        &self,
        id: &str,
        version: &str,
        mode: &str,
        dependencies: BTreeMap<String, String>,
    ) -> package::Receipt {
        let payload = self.root.join(format!("payload-{id}-{version}"));
        fs::create_dir_all(payload.join("bin")).unwrap();
        fs::write(payload.join("LICENSE"), "GPL-3.0-only test fixture").unwrap();
        fs::write(
            payload.join("bin/example-service"),
            SCRIPT
                .replace("__ID__", id)
                .replace("__VERSION__", version)
                .replace("__MODE__", mode),
        )
        .unwrap();
        let mut manifest: Manifest =
            serde_json::from_str(include_str!("../../../template/manifest.json")).unwrap();
        manifest.id = id.into();
        manifest.version = version.into();
        manifest.dependencies = dependencies;
        if mode == "background" { manifest.capabilities.push(crate::manifest::Capability::BackgroundService); }
        if let Some(ui) = &manifest.entrypoints.ui {
            let path = payload.join(ui);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "export function createAddon() {}").unwrap();
        }
        let bytes = package::pack(manifest.clone(), &payload).unwrap();
        let entry = Entry { manifest, artifacts: BTreeMap::from([(PLATFORM.into(), Artifact {
            url: "https://github.com/pelicanux/PeliGames/releases/download/addon-tests/service.zip".into(),
            sha256: package::digest(&bytes), bytes: bytes.len() as u64,
        })]) };
        package::prepare(&self.store.packages, &entry, PLATFORM, &bytes).unwrap()
    }
    fn install(&self, id: &str, version: &str, mode: &str) {
        let receipt = self.prepare(id, version, mode, BTreeMap::new());
        self.store.register(&[receipt], HOST, PLATFORM).unwrap();
    }
    fn submit(&self, host: &ServiceHost, id: &str, method: &str) -> Result<Pending> {
        host.submit(
            self.store.clone(),
            &self.store.read().unwrap(),
            id,
            method,
            Value::Null,
            HOST,
            PLATFORM,
        )
    }
    fn directory(&self, id: &str, version: &str) -> PathBuf {
        self.store.packages.join("installed").join(id).join(version)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn limits() -> Limits {
    Limits {
        startup: Duration::from_secs(2),
        request: Duration::from_millis(500),
        shutdown: Duration::from_millis(150),
        reap: Duration::from_millis(500),
        queue: 2,
    }
}
async fn file_ready(path: &Path) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
async fn finished(host: &ServiceHost, id: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if host
                .statuses()
                .iter()
                .any(|s| s.id == id && matches!(s.phase, Phase::Failed | Phase::Stopped))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
fn running(pid: u32) -> bool {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    stat.rsplit_once(')')
        .is_some_and(|(_, rest)| !matches!(rest.trim().chars().next(), Some('Z' | 'X') | None))
}

#[tokio::test]
async fn lazy_start_reuses_the_verified_service_and_shuts_down_gracefully() {
    let f = Fixture::new();
    f.install("example-service", "1.0.0", "normal");
    let host = ServiceHost::new(limits());
    assert!(host.statuses().is_empty());
    let first = f
        .submit(&host, "example-service", "ping")
        .unwrap()
        .wait()
        .await
        .unwrap();
    let pid = first["pid"].as_u64().unwrap() as u32;
    assert_eq!(
        f.submit(&host, "example-service", "ping")
            .unwrap()
            .wait()
            .await
            .unwrap()["pid"],
        first["pid"]
    );
    assert_eq!(host.statuses()[0].phase, Phase::Running);
    host.stop("example-service").await.unwrap();
    assert!(f
        .directory("example-service", "1.0.0")
        .join("graceful")
        .exists());
    assert!(!running(pid));
    assert!(host.statuses().is_empty());
    let second = f
        .submit(&host, "example-service", "ping")
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_ne!(second["pid"], first["pid"]);
    host.shutdown().await.unwrap();
    assert!(f.submit(&host, "example-service", "ping").is_err());
}
#[tokio::test]
async fn valid_remote_errors_and_stderr_do_not_corrupt_the_transport() {
    let f = Fixture::new();
    f.install("example-service", "1.0.0", "normal");
    let host = ServiceHost::new(limits());
    let before = f
        .submit(&host, "example-service", "ping")
        .unwrap()
        .wait()
        .await
        .unwrap();
    let error = f
        .submit(&host, "example-service", "remote-error")
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Remote);
    assert_eq!(error.remote_code, Some(-32601));
    f.submit(&host, "example-service", "stderr")
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(
        f.submit(&host, "example-service", "ping")
            .unwrap()
            .wait()
            .await
            .unwrap()["pid"],
        before["pid"]
    );
    host.shutdown().await.unwrap();
}
#[tokio::test]
async fn requests_fail_before_starting_for_disabled_corrupt_or_reserved_inputs() {
    let f = Fixture::new();
    f.install("example-service", "1.0.0", "normal");
    let host = ServiceHost::new(limits());
    assert_eq!(
        f.submit(&host, "example-service", "initialize")
            .err()
            .unwrap()
            .kind,
        ErrorKind::InvalidRequest
    );
    assert!(host.statuses().is_empty());
    assert_eq!(
        host.submit(
            f.store.clone(),
            &f.store.read().unwrap(),
            "example-service",
            "ping",
            Value::String("x".repeat(protocol::MAX_MESSAGE_BYTES)),
            HOST,
            PLATFORM
        )
        .err()
        .unwrap()
        .kind,
        ErrorKind::InvalidRequest
    );
    assert!(host.statuses().is_empty());
    f.store
        .enable("example-service", false, HOST, PLATFORM)
        .unwrap();
    assert_eq!(
        f.submit(&host, "example-service", "ping")
            .err()
            .unwrap()
            .kind,
        ErrorKind::Unavailable
    );
    f.store
        .enable("example-service", true, HOST, PLATFORM)
        .unwrap();
    fs::write(
        f.directory("example-service", "1.0.0")
            .join("bin/example-service"),
        "changed",
    )
    .unwrap();
    assert_eq!(
        f.submit(&host, "example-service", "ping")
            .unwrap()
            .wait()
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Verification
    );
    assert!(!f
        .directory("example-service", "1.0.0")
        .join("called")
        .exists());
    host.shutdown().await.unwrap();
}
#[tokio::test]
async fn mismatched_identity_and_hanging_initialization_never_enter_running_state() {
    for mode in ["identity", "api", "silent"] {
        let f = Fixture::new();
        f.install("example-service", "1.0.0", mode);
        let mut options = limits();
        options.startup = Duration::from_millis(200);
        let host = ServiceHost::new(options);
        let error = f
            .submit(&host, "example-service", "ping")
            .unwrap()
            .wait()
            .await
            .unwrap_err();
        assert_eq!(
            error.kind,
            if mode == "silent" {
                ErrorKind::Timeout
            } else {
                ErrorKind::Protocol
            }
        );
        assert_eq!(host.statuses()[0].phase, Phase::Failed);
        let pid: u32 = fs::read_to_string(f.directory("example-service", "1.0.0").join("pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(!running(pid));
        assert!(f.submit(&host, "example-service", "ping").is_err()); // no restart loop
        host.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn invalid_overlong_partial_and_crashed_responses_are_bounded_and_isolated() {
    for (method, expected) in [
        ("bad", ErrorKind::Protocol),
        ("oversized", ErrorKind::Protocol),
        ("partial", ErrorKind::Timeout),
        ("crash", ErrorKind::Crashed),
    ] {
        let f = Fixture::new();
        f.install("example-service", "1.0.0", "normal");
        let host = ServiceHost::new(limits());
        let pid = f
            .submit(&host, "example-service", "ping")
            .unwrap()
            .wait()
            .await
            .unwrap()["pid"]
            .as_u64()
            .unwrap() as u32;
        assert_eq!(
            f.submit(&host, "example-service", method)
                .unwrap()
                .wait()
                .await
                .unwrap_err()
                .kind,
            expected
        );
        finished(&host, "example-service").await;
        assert!(!running(pid));
        host.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn busy_worker_has_bounded_queue_and_does_not_block_another_addon() {
    let f = Fixture::new();
    f.install("first", "1.0.0", "normal");
    f.install("second", "1.0.0", "normal");
    let host = ServiceHost::new(limits());
    let pending = f.submit(&host, "first", "hang").unwrap();
    file_ready(&f.directory("first", "1.0.0").join("called")).await;
    let queued = f.submit(&host, "first", "ping").unwrap();
    let queued2 = f.submit(&host, "first", "ping").unwrap();
    assert_eq!(
        f.submit(&host, "first", "ping").err().unwrap().kind,
        ErrorKind::Busy
    );
    let other = tokio::time::timeout(
        Duration::from_millis(300),
        f.submit(&host, "second", "ping").unwrap().wait(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(other["pid"].is_number());
    assert_eq!(pending.wait().await.unwrap_err().kind, ErrorKind::Timeout);
    assert_eq!(queued.wait().await.unwrap_err().kind, ErrorKind::Timeout);
    assert_eq!(queued2.wait().await.unwrap_err().kind, ErrorKind::Timeout);
    host.shutdown().await.unwrap();
}
#[tokio::test]
async fn cancelling_a_caller_does_not_leave_a_half_read_response() {
    let f = Fixture::new();
    f.install("example-service", "1.0.0", "normal");
    let host = ServiceHost::new(limits());
    let first = f
        .submit(&host, "example-service", "ping")
        .unwrap()
        .wait()
        .await
        .unwrap();
    let pending = f.submit(&host, "example-service", "delay").unwrap();
    file_ready(&f.directory("example-service", "1.0.0").join("called")).await;
    drop(pending);
    assert_eq!(
        f.submit(&host, "example-service", "ping")
            .unwrap()
            .wait()
            .await
            .unwrap()["pid"],
        first["pid"]
    );
    host.shutdown().await.unwrap();
}
#[tokio::test]
async fn version_switch_and_dependency_disable_stop_the_affected_workers() {
    let f = Fixture::new();
    f.install("dependency", "1.0.0", "normal");
    let dependent = f.prepare(
        "dependent",
        "1.0.0",
        "normal",
        BTreeMap::from([("dependency".into(), "^1".into())]),
    );
    f.store.register(&[dependent], HOST, PLATFORM).unwrap();
    let host = ServiceHost::new(limits());
    let pid = f
        .submit(&host, "dependent", "ping")
        .unwrap()
        .wait()
        .await
        .unwrap()["pid"]
        .as_u64()
        .unwrap() as u32;
    let newer = f.prepare("dependency", "1.1.0", "normal", BTreeMap::new());
    let state = f.store.register(&[newer], HOST, PLATFORM).unwrap();
    host.reconcile(&state).await.unwrap();
    assert!(!running(pid));
    assert!(host.statuses().is_empty());
    let next = f
        .submit(&host, "dependent", "ping")
        .unwrap()
        .wait()
        .await
        .unwrap()["pid"]
        .as_u64()
        .unwrap() as u32;
    f.store.enable("dependent", false, HOST, PLATFORM).unwrap();
    host.reconcile(&f.store.read().unwrap()).await.unwrap();
    assert!(!running(next));
    host.shutdown().await.unwrap();
}
#[tokio::test]
async fn forced_shutdown_kills_descendants_and_does_not_wait_for_a_stuck_request() {
    let f = Fixture::new();
    f.install("example-service", "1.0.0", "ignore-stop");
    let host = ServiceHost::new(limits());
    let parent = f
        .submit(&host, "example-service", "ping")
        .unwrap()
        .wait()
        .await
        .unwrap()["pid"]
        .as_u64()
        .unwrap() as u32;
    let child = f
        .submit(&host, "example-service", "spawn-child")
        .unwrap()
        .wait()
        .await
        .unwrap()["pid"]
        .as_u64()
        .unwrap() as u32;
    let pending = f.submit(&host, "example-service", "hang").unwrap();
    file_ready(&f.directory("example-service", "1.0.0").join("called")).await;
    tokio::time::timeout(Duration::from_secs(2), host.shutdown())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.wait().await.unwrap_err().kind, ErrorKind::Stopped);
    assert!(!running(parent));
    tokio::time::timeout(Duration::from_secs(1), async {
        while running(child) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn service_environment_does_not_inherit_arbitrary_credentials() {
    let f = Fixture::new();
    f.install("example-service", "1.0.0", "normal");
    let host = ServiceHost::new(limits());
    let key = "PELIGAMES_ADDON_TEST_SECRET";
    let previous = std::env::var_os(key);
    std::env::set_var(key, "fixture-secret");
    let result = f
        .submit(&host, "example-service", "environment")
        .unwrap()
        .wait()
        .await;
    match previous {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
    assert_eq!(result.unwrap()["secret"], "");
    host.shutdown().await.unwrap();
}

#[tokio::test]
async fn stop_interrupts_initialization_and_forces_an_uncooperative_idle_worker() {
    for mode in ["silent", "ignore-stop"] {
        let f = Fixture::new();
        f.install("example-service", "1.0.0", mode);
        let host = ServiceHost::new(limits());
        let pending = f.submit(&host, "example-service", "ping").unwrap();
        file_ready(&f.directory("example-service", "1.0.0").join("pid")).await;
        let pid: u32 = fs::read_to_string(f.directory("example-service", "1.0.0").join("pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let pending = if mode == "ignore-stop" {
            pending.wait().await.unwrap();
            None
        } else {
            Some(pending)
        };
        tokio::time::timeout(Duration::from_secs(1), host.stop("example-service"))
            .await
            .unwrap()
            .unwrap();
        if let Some(pending) = pending {
            assert_eq!(pending.wait().await.unwrap_err().kind, ErrorKind::Stopped);
        }
        assert!(!running(pid));
        host.shutdown().await.unwrap();
    }
}

const SCRIPT: &str = include_str!("fixtures/service.sh");

#[tokio::test]
async fn background_service_gets_time_to_finalize_before_removal() {
    let fixture = Fixture::new();
    fixture.install("capture-worker", "0.1.0", "background");
    let host = ServiceHost::new(limits());
    fixture.submit(&host, "capture-worker", "echo").unwrap().wait().await.unwrap();
    let pending=fixture.submit(&host,"capture-worker","delay").unwrap();
    file_ready(&fixture.directory("capture-worker","0.1.0").join("called")).await;
    let mut state=fixture.store.read().unwrap();
    state.addons.get_mut("capture-worker").unwrap().enabled=false;
    host.reconcile(&state).await.unwrap();
    assert_eq!(pending.wait().await.unwrap_err().kind,ErrorKind::Stopped);
    assert!(fixture.directory("capture-worker","0.1.0").join("finalized").exists());
    host.shutdown().await.unwrap();
}
