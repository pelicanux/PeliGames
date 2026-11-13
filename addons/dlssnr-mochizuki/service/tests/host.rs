//! Local experimental package → verified worker → synthetic game, without a catalog.
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
        let root =
            std::env::temp_dir().join(format!("peligames-mochi-host-{}", std::process::id()));
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
            env!("CARGO_BIN_EXE_peligames-dlssnr-service"),
            payload.join("bin/dlssnr-service"),
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
            "dlssnr-mochizuki",
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
    async fn mutate(&self, host: &ServiceHost, game: &std::path::Path, action: Value) {
        let response = self
            .call(
                host,
                "game.mutate",
                json!({"directory":game,"bits":"64","action":action}),
            )
            .await
            .unwrap();
        let job = response["job"].as_u64().unwrap();
        for _ in 0..100 {
            let status = self
                .call(host, "job.inspect", json!({"id":job}))
                .await
                .unwrap();
            if status["done"] == true {
                assert!(status["error"].is_null(), "{status}");
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("synthetic operation timed out");
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[tokio::test]
async fn local_package_runs_the_real_worker_and_disabled_packages_cannot_run() {
    let f = Fixture::new();
    f.install();
    let host = ServiceHost::new(Limits {
        startup: Duration::from_secs(5),
        request: Duration::from_secs(3),
        ..Default::default()
    });
    assert!(host.statuses().is_empty());
    assert!(f.store.read().unwrap().addons["dlssnr-mochizuki"].versions["0.1.0"].local);
    assert_eq!(
        f.call(&host, "runtime.inspect", Value::Null).await.unwrap()["ready"],
        false
    );
    let game = f.root.join("game with spaces");
    let component = f.root.join("component/rdna4/64");
    fs::create_dir_all(component.join("model-tools")).unwrap();
    fs::create_dir(component.join("dlssnr-amd")).unwrap();
    fs::write(component.join("model-tools/extract_model.sh"), "fixture").unwrap();
    fs::write(component.join("install.sh"),"#!/bin/bash\n[ \"$2\" = remove ] || exit 19\nrm -f \"$1/dlssnr-amd.ini\" \"$1/OptiScaler/plugins/dlssnr-amd.asi\"\nprintf removed\n").unwrap();
    fs::create_dir_all(game.join("OptiScaler/plugins")).unwrap();
    fs::write(game.join("OptiScaler/plugins/dlssnr-amd.asi"), "plugin").unwrap();
    fs::write(game.join("dxgi.dll"), "proxy").unwrap();
    fs::write(game.join("save.dat"), "save").unwrap();
    fs::write(game.join("OptiScaler.ini"), "[Menu]\nKeyMenu=0x2D\n").unwrap();
    fs::write(game.join("dlssnr-amd.ini"),"[Inspect]\nCompare=0\n[HudFix]\nMode=0\n[Network]\nModel=CC_SILVER_AARDWOLD_OPTIMIZED\nOptimizedLayers=3\n[RenoDX]\nEnabled=0\nNeuralUplift=1\n").unwrap();
    f.mutate(
        &host,
        &game,
        json!({"operation":"startup","enabled":false,"renodx":false}),
    )
    .await;
    let state = f
        .call(&host, "game.inspect", json!({"directory":game,"bits":"64"}))
        .await
        .unwrap();
    assert_eq!(state["settings"]["enabled"], false);
    f.mutate(&host, &game, json!({"operation":"optimized","level":1}))
        .await;
    assert!(fs::read_to_string(game.join("dlssnr-amd.ini"))
        .unwrap()
        .to_ascii_lowercase()
        .contains("optimizedlayers = 1"));
    f.mutate(
        &host,
        &game,
        json!({"operation":"shortcut","key":"Page Down","route":"optiscaler"}),
    )
    .await;
    assert_eq!(
        f.call(&host, "game.inspect", json!({"directory":game,"bits":"64"}))
            .await
            .unwrap()["shortcut"],
        "Page Down"
    );
    f.mutate(&host, &game, json!({"operation":"remove"})).await;
    assert_eq!(fs::read_to_string(game.join("save.dat")).unwrap(), "save");
    assert!(!game.join("dlssnr-amd.ini").exists());
    let disabled = f
        .store
        .enable("dlssnr-mochizuki", false, "0.5.5", "linux-x86_64")
        .unwrap();
    host.reconcile(&disabled).await.unwrap();
    assert!(f.call(&host, "runtime.inspect", Value::Null).await.is_err());
    host.shutdown().await.unwrap();
}
