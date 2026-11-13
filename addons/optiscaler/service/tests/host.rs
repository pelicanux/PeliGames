//! Exercise the real service executable through verified packaging and lazy RPC.
use peligames_addon_kit::{
    catalog::{Artifact, Entry},
    manifest::Manifest,
    package,
    registry::Store,
    runtime::{Limits, Phase, ServiceHost},
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::PathBuf, time::Duration};

struct Fixture {
    root: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "peligames-optiscaler-service-test-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let store = Store {
            packages: root.join("addons"),
            state_file: root.join("settings/addons.json"),
        };
        Self { root, store }
    }
    fn install(&self) {
        let payload = self.root.join("payload");
        fs::create_dir_all(payload.join("bin")).unwrap();
        fs::copy(
            env!("CARGO_BIN_EXE_peligames-optiscaler-service"),
            payload.join("bin/optiscaler-service"),
        )
        .unwrap();
        fs::write(payload.join("LICENSE"), include_str!("../../../../LICENSE")).unwrap();
        fs::create_dir_all(payload.join("ui")).unwrap();
        fs::write(
            payload.join("ui/index.js"),
            "export function createAddon() { return {}; }",
        )
        .unwrap();
        let manifest: Manifest = serde_json::from_str(include_str!("../../manifest.json")).unwrap();
        let bytes = package::pack(manifest.clone(), &payload).unwrap();
        let entry=Entry {manifest,artifacts:BTreeMap::from([("linux-x86_64".into(),Artifact {
            url:"https://github.com/pelicanux/PeliGames/releases/download/addon-tests/optiscaler.zip".into(),
            sha256:package::digest(&bytes),bytes:bytes.len() as u64,
        })])};
        let receipt =
            package::prepare(&self.store.packages, &entry, "linux-x86_64", &bytes).unwrap();
        self.store
            .register(&[receipt], "0.5.5", "linux-x86_64")
            .unwrap();
    }
    async fn call(&self, host: &ServiceHost, method: &str, params: Value) -> Result<Value, String> {
        host.submit(
            self.store.clone(),
            &self.store.read().unwrap(),
            "optiscaler",
            method,
            json!({"host":{"component_directory":self.root.join("component")},"request":params}),
            "0.5.5",
            "linux-x86_64",
        )
        .map_err(|e| e.to_string())?
        .wait()
        .await
        .map_err(|e| e.to_string())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[tokio::test]
async fn verified_service_reads_a_game_and_prepares_edits_without_mutating_it() {
    let f = Fixture::new();
    f.install();
    let game = f.root.join("game with spaces");
    let component = f.root.join("component");
    fs::create_dir(&game).unwrap();
    fs::create_dir(&component).unwrap();
    let original = "; preserve\r\n[Menu]\r\nScale=auto\r\n[Plugins]\r\nLoadAsiPlugins=true\r\n";
    fs::write(game.join("OptiScaler.ini"), original).unwrap();
    fs::write(game.join("dxgi.dll"), b"MZ fixture proxy").unwrap();
    fs::write(game.join("save.dat"), b"keep my save").unwrap();
    let host = ServiceHost::new(Limits {
        startup: Duration::from_secs(5),
        request: Duration::from_secs(2),
        ..Default::default()
    });
    assert!(host.statuses().is_empty());
    let state = f
        .call(&host, "game.inspect", json!({"directory":game}))
        .await
        .unwrap();
    assert_eq!(state["installed"], true);
    assert_eq!(state["proxy"], "dxgi.dll");
    assert_eq!(state["settings"]["Menu.Scale"], "auto");
    let status = &host.statuses()[0];
    assert_eq!(status.phase, Phase::Running);
    let pid = status.pid.unwrap();

    let changed = f
        .call(
            &host,
            "config.patch",
            json!({"text":original,"key":"Menu.Scale","value":"1.5"}),
        )
        .await
        .unwrap();
    let changed = changed.as_str().unwrap();
    assert!(changed.starts_with("; preserve\r\n"));
    let parsed = f
        .call(&host, "config.inspect", json!({"text":changed}))
        .await
        .unwrap();
    assert_eq!(parsed["Menu.Scale"], "1.5");
    assert!(changed.contains("LoadAsiPlugins=true\r\n"));
    assert_eq!(
        fs::read_to_string(game.join("OptiScaler.ini")).unwrap(),
        original
    );
    assert_eq!(fs::read(game.join("save.dat")).unwrap(), b"keep my save");
    assert!(!game.join(".peligames-config.lock").exists());

    for (method, params) in [
        ("game.inspect", json!({"directory":"relative"})),
        ("game.inspect", json!({"directory":game,"extra":true})),
        (
            "config.patch",
            json!({"text":original,"key":"Menu.Scale","value":"1.5\nInjected=1"}),
        ),
        ("config.inspect", json!({"text":original,"extra":true})),
        ("game.remove", json!({"directory":game})),
    ] {
        assert!(f.call(&host, method, params).await.is_err(), "{method}");
    }
    assert_eq!(
        f.call(&host, "config.inspect", json!({"text":original}))
            .await
            .unwrap()["Menu.Scale"],
        "auto"
    );
    assert_eq!(host.statuses()[0].pid, Some(pid));
    // The service owns writes and returns them through the same verified RPC.
    let saved=f.call(&host,"game.mutate",json!({"directory":game,"action":{"operation":"update","key":"Menu.Scale","value":"1.25"}})).await.unwrap();
    assert_eq!(saved["Menu.Scale"], "1.25");
    assert_eq!(
        f.call(&host, "game.inspect", json!({"directory":game}))
            .await
            .unwrap()["settings"]["Menu.Scale"],
        "1.25"
    );
    // Restore the fixture before checking that package removal preserves game files.
    f.call(
        &host,
        "game.mutate",
        json!({"directory":game,"action":{"operation":"update","key":"Menu.Scale","value":"auto"}}),
    )
    .await
    .unwrap();
    assert!(f.call(&host,"game.mutate",json!({"directory":game,"action":{"operation":"update","key":"Menu.Scale","value":"1.5\nInjected=1"}})).await.is_err());
    let restored = fs::read_to_string(game.join("OptiScaler.ini")).unwrap();
    assert!(restored.starts_with("; preserve\r\n"));
    assert_eq!(
        peligames_optiscaler::settings::read(&restored)["Menu.Scale"],
        "auto"
    );
    f.call(&host, "runtime.channel", json!({"channel":"nightly"}))
        .await
        .unwrap();
    let cached = component.join("nightly");
    fs::create_dir(&cached).unwrap();
    fs::write(cached.join("OptiScaler.dll"), "fixture").unwrap();
    f.call(&host, "runtime.remove", json!({"channel":"nightly"}))
        .await
        .unwrap();
    assert!(!cached.exists());
    assert!(game.join("dxgi.dll").is_file());
    assert_eq!(
        f.call(&host, "runtime.inspect", Value::Null).await.unwrap()["channel"],
        "nightly"
    );
    // Disabling the add-on reconciles and closes the real process, without game changes.
    let disabled = f
        .store
        .enable("optiscaler", false, "0.5.5", "linux-x86_64")
        .unwrap();
    host.reconcile(&disabled).await.unwrap();
    assert!(host.statuses().is_empty());
    assert!(f
        .call(&host, "config.inspect", json!({"text":original}))
        .await
        .is_err());
    assert_eq!(
        fs::read_to_string(game.join("OptiScaler.ini")).unwrap(),
        restored
    );
    host.shutdown().await.unwrap();
}
