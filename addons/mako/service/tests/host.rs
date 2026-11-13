//! Local experimental package → verified worker → synthetic game, without a catalog.
#[path = "../../tests/fixtures.rs"]
mod fixtures;
use peligames_addon_kit::{
    manifest::Manifest,
    package,
    registry::Store,
    runtime::{Limits, ServiceHost},
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Duration};
struct Fixture {
    root: PathBuf,
    store: Store,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("peligames-mako-host-{}", std::process::id()));
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
        fs::create_dir(payload.join("ui")).unwrap();
        fs::copy(
            env!("CARGO_BIN_EXE_peligames-mako-service"),
            payload.join("bin/mako-service"),
        )
        .unwrap();
        fs::write(
            payload.join("ui/index.js"),
            "export function createAddon() {return {}};",
        )
        .unwrap();
        fs::write(payload.join("LICENSE"), include_str!("../../LICENSE")).unwrap();
        let manifest: Manifest = serde_json::from_str(include_str!("../../manifest.json")).unwrap();
        let bytes = package::pack(manifest, &payload).unwrap();
        let receipt = package::prepare_local(
            &self.store.packages,
            &bytes,
            &package::digest(&bytes),
            "0.5.5",
            "linux-x86_64",
        )
        .unwrap();
        self.store
            .register(&[receipt], "0.5.5", "linux-x86_64")
            .unwrap();
    }
    async fn call(
        &self,
        host: &ServiceHost,
        method: &str,
        request: Value,
    ) -> Result<Value, String> {
        host.submit(
            self.store.clone(),
            &self.store.read().unwrap(),
            "mako",
            method,
            json!({"host":{"component_directory":self.root.join("component")},"request":request}),
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
async fn local_package_prepares_live_profiles_and_stops_after_disable() {
    let f = Fixture::new();
    f.install();
    let host = ServiceHost::new(Limits {
        startup: Duration::from_secs(5),
        request: Duration::from_secs(3),
        ..Default::default()
    });
    assert!(host.statuses().is_empty());
    assert_eq!(
        f.call(&host, "runtime.inspect", Value::Null).await.unwrap()["installed"],
        false
    );
    let settings = peligames_mako::Settings::default();
    let data=f.call(&host,"config.prepare",json!({"key":"fixture-game","label":"Fixture Game","directory":f.root,"settings":settings,"profile":null,"options":["hdr"]})).await.unwrap();
    assert_eq!(data["enabled"], false);
    let profile = PathBuf::from(data["profile"].as_str().unwrap());
    assert!(profile
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("fixture-game"));
    assert!(profile.with_extension("toml").is_file());
    let inspected = f
        .call(
            &host,
            "game.inspect",
            json!({"key":"fixture-game","label":"Fixture Game","profiles":[profile]}),
        )
        .await
        .unwrap();
    assert_eq!(inspected["profile"], data["profile"]);
    let error=f.call(&host,"config.prepare",json!({"key":"fixture-game","label":"Fixture Game","directory":f.root,"settings":peligames_mako::Settings{enabled:true,..settings},"profile":null,"options":["hdr"]})).await.unwrap_err();
    assert!(error.contains("HDR"));
    let component = fixtures::Fixture(f.root.join("component"));
    component.runtime();
    let enabled = mako_settings(&component);
    let data=f.call(&host,"config.prepare",json!({"key":"fixture-game","label":"Fixture Game","directory":f.root,"settings":enabled,"profile":null,"options":[]})).await.unwrap();
    let profile = PathBuf::from(data["profile"].as_str().unwrap());
    let mut live = enabled;
    live.target_fps = 144;
    live.multiplier = 4;
    live.adaptive = true;
    f.call(
        &host,
        "config.live",
        json!({"profile":profile,"settings":live}),
    )
    .await
    .unwrap();
    let config = fs::read_to_string(profile.with_extension("toml")).unwrap();
    assert!(config.contains("target_fps = 144"));
    assert!(config.contains("multiplier = 4"));
    assert!(config.contains("adaptive = true"));
    let disabled = f
        .store
        .enable("mako", false, "0.5.5", "linux-x86_64")
        .unwrap();
    host.reconcile(&disabled).await.unwrap();
    assert!(f.call(&host, "runtime.inspect", Value::Null).await.is_err());
    assert!(profile.with_extension("json").exists());
    host.shutdown().await.unwrap();
}

fn mako_settings(component: &fixtures::Fixture) -> peligames_mako::Settings {
    peligames_mako::Settings {
        enabled: true,
        dll: component.dll().to_string_lossy().into(),
        ..Default::default()
    }
}
