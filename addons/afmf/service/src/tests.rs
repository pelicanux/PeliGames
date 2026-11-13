use super::*;
use peligames_addon_kit::{
    manifest::Manifest,
    protocol::{Request, Response},
    service::serve,
};
use serde_json::json;
use std::{
    fs,
    io::Cursor,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "peligames-afmf-service-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }
    fn rpc(&self, method: &str, request: Value) -> Result<Value, RpcError> {
        call(
            method,
            json!({"host":{"component_directory":self.0},"request":request}),
        )
    }
    fn runtime(&self) -> PathBuf {
        let path = runtime::location(&self.0, "1.3.1").unwrap();
        fs::create_dir_all(&path).unwrap();
        let mut elf = [0u8; 64];
        elf[..5].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2]);
        elf[18] = 62;
        fs::write(path.join("libafmf-linux.so"), elf).unwrap();
        fs::write(path.join("afmf-linux.json"),r#"{"layer":{"name":"VK_LAYER_AFMF","library_path":"libafmf-linux.so","enable_environment":{"AFMF_ENABLE":"1"}}}"#).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn launch(enabled: bool) -> Value {
    json!({"settings":Settings {enabled,..Default::default()},"runtime_version":"1.3.1","gamescope":false,"environment":{},"mangohud_directory":null})
}

#[test]
fn disabled_launch_removes_activation_without_loading_a_runtime() {
    let f = Fixture::new();
    let result = f.rpc("launch.prepare", launch(false)).unwrap();
    for key in ["AFMF_ENABLE", "AFMF_ASYNC", "AFMF_GAMESCOPE"] {
        assert!(result.as_object().unwrap().contains_key(key));
        assert!(result[key].is_null());
    }
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}

#[test]
fn compatibility_uses_structured_probe_results_without_reading_a_game_directory() {
    let f = Fixture::new();
    let result = f
        .rpc(
            "game.support",
            json!({"bits":32,"windows":true,"flatpak":false}),
        )
        .unwrap();
    assert_eq!(result["code"], "requires-wow64");
    assert_eq!(result["available"], true);
    assert!(f
        .rpc(
            "game.support",
            json!({"bits":64,"windows":true,"flatpak":false,"extra":true})
        )
        .is_err());
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
}

#[test]
fn prepared_environment_preserves_other_mods_limits_and_overlay_order() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let monitoring = f.0.join("monitoring");
    fs::create_dir(&monitoring).unwrap();
    let original = fs::read(runtime.join("afmf-linux.json")).unwrap();
    let mut request = launch(true);
    request["gamescope"] = json!(true);
    request["environment"] = json!({
        "VK_INSTANCE_LAYERS":"VK_LAYER_VALVE_steam_overlay_64:VK_LAYER_NR:VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64",
        "VK_ADD_LAYER_PATH":"/fixture/other-mod", "VK_LAYER_PATH":"/fixture/override",
        "MANGOHUD_CONFIGFILE":"/fixture/MangoHud.conf","MANGOHUD_CONFIG":"read_cfg,fps_limit=144,vsync=1,no_display=1",
        "PELI_OVERLAY_BACKGROUND_FPS":"30", "PROTON_ENABLE_WAYLAND":"1",
    });
    request["mangohud_directory"] = json!(monitoring);
    let result = f.rpc("launch.prepare", request).unwrap();
    assert_eq!(result["VK_INSTANCE_LAYERS"],"VK_LAYER_VALVE_steam_overlay_64:VK_LAYER_NR:VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64");
    assert_eq!(result["AFMF_ASYNC"], "1");
    assert_eq!(result["AFMF_GAMESCOPE"], "1");
    assert_eq!(result["DISABLE_MANGOHUD"], "1");
    assert_eq!(
        result["MANGOHUD_CONFIG"],
        "read_cfg,fps_limit=144,no_display=1,vsync=-1"
    );
    assert_eq!(result["PELI_OVERLAY_BACKGROUND_FPS"], "30");
    assert_eq!(result["PROTON_ENABLE_WAYLAND"], "1");
    let paths = result["VK_LAYER_PATH"].as_str().unwrap();
    assert!(paths.ends_with("/fixture/other-mod:/fixture/override"));
    assert_eq!(fs::read(runtime.join("afmf-linux.json")).unwrap(), original);
}

#[test]
fn invalid_settings_unknown_inputs_and_paths_fail_before_manifest_publication() {
    let f = Fixture::new();
    let runtime = f.runtime();
    let mut request = launch(true);
    request["environment"] = json!({"API_TOKEN":"unneeded"});
    assert!(f.rpc("launch.prepare", request).is_err());
    let mut request = launch(true);
    request["settings"]["min_fps"] = json!(241);
    assert!(f.rpc("launch.prepare", request).is_err());
    let mut request = launch(true);
    request["runtime_version"] = json!("../other");
    assert!(f.rpc("launch.prepare", request).is_err());
    let mut request = launch(true);
    request["mangohud_directory"] = json!("relative");
    assert!(f.rpc("launch.prepare", request).is_err());
    assert!(!runtime.join("peligames-explicit-layer").exists());
    assert!(call(
        "config.defaults",
        json!({"host":{"component_directory":"relative"},"request":null})
    )
    .is_err());
    assert_eq!(f.rpc("missing", Value::Null).unwrap_err().code, -32601);
    assert_eq!(
        f.rpc("config.defaults", Value::Null).unwrap(),
        serde_json::to_value(Settings::default()).unwrap()
    );
    assert!(f
        .rpc("config.validate", json!({"performance_mode":"turbo"}))
        .is_err());
}

#[test]
fn real_protocol_dispatch_requires_handshake_and_returns_structured_launch_changes() {
    let f = Fixture::new();
    let manifest: Manifest = serde_json::from_str(include_str!("../../manifest.json")).unwrap();
    manifest.validate().unwrap();
    let params = json!({"host":{"component_directory":f.0},"request":launch(false)});
    let requests = [
        Request::new(1, "launch.prepare", params.clone()),
        Request::new(2, "initialize", json!({"host_api":1})),
        Request::new(3, "launch.prepare", params),
        Request::new(4, "shutdown", Value::Null),
    ];
    let input = requests
        .into_iter()
        .map(|request| serde_json::to_string(&request).unwrap() + "\n")
        .collect::<String>();
    let mut output = Vec::new();
    serve(Cursor::new(input), &mut output, &manifest, call).unwrap();
    let output = String::from_utf8(output).unwrap();
    let lines = output.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 4);
    assert!(Response::parse(lines[0], 1).is_err());
    let identity = Response::parse(lines[1], 2).unwrap();
    assert_eq!(identity["addon_id"], "afmf");
    assert_eq!(
        Response::parse(lines[2], 3).unwrap()["AFMF_ENABLE"],
        Value::Null
    );
    assert_eq!(Response::parse(lines[3], 4).unwrap(), Value::Null);
}

#[test]
fn runtime_jobs_reject_malformed_parameters_and_use_the_cached_catalog_offline() {
    let f = Fixture::new();
    let service = Service::new().unwrap();
    let invoke = |method: &str, request: Value| {
        service.handle(
            method,
            json!({"host":{"component_directory":f.0},"request":request}),
        )
    };
    for (method, request) in [
        ("runtime.check", json!({"force":"yes"})),
        ("runtime.download", json!({"version":123})),
        ("runtime.remove", json!({"version":"1.3.1","extra":true})),
        ("job.inspect", json!({"id":-1})),
    ] {
        assert_eq!(invoke(method, request).unwrap_err().code, -32602);
    }
    assert!(invoke("runtime.inspect", json!({})).is_err());
    assert!(invoke("runtime.select", json!({"version":"../outside"})).is_err());
    assert!(invoke("runtime.download", json!({"version":"2.0.0"})).is_err());
    fs::create_dir_all(f.0.join("afmf")).unwrap();
    let checked = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    fs::write(
        f.0.join("afmf/release.json"),
        serde_json::to_vec(
            &json!({"checked":checked,"release":peligames_afmf::releases::builtin()}),
        )
        .unwrap(),
    )
    .unwrap();
    let job = invoke("runtime.check", json!({"force":false})).unwrap()["job"]
        .as_u64()
        .unwrap();
    let mut completed = false;
    for _ in 0..100 {
        let status = invoke("job.inspect", json!({"id":job})).unwrap();
        if status["done"] == true {
            assert!(status["error"].is_null(), "{status}");
            assert_eq!(status["result"]["current"], "1.3.1");
            completed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(completed, "Cached lookup did not complete");
    assert!(invoke("job.inspect", json!({"id":job+1})).is_err());
}
