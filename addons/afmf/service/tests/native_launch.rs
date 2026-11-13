//! Executes the real service entrypoint against a synthetic game, never Vulkan.
#[path = "../../tests/fixtures.rs"]
mod fixtures;
use fixtures::Fixture;
use peligames_addon_kit::protocol::LaunchContext;
use peligames_afmf::{management, Settings};
use std::{fs, process::Command};

fn command(f: &Fixture, monitoring: bool) -> Command {
    let context = LaunchContext {
        component_directory: f.0.clone(),
        settings: serde_json::to_value(Settings {
            enabled: true,
            min_fps: 45,
            ..Default::default()
        })
        .unwrap(),
        gamescope: true,
        monitoring,
        options: vec![],
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_peligames-afmf-service"));
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", &f.0)
        .env("XDG_CONFIG_HOME", f.0.join("config"))
        .env("XDG_DATA_HOME", f.0.join("data"))
        .env("XDG_DATA_DIRS", f.0.join("system"))
        .arg("--peligames-launch")
        .arg(serde_json::to_string(&context).unwrap())
        .arg("--");
    command
}

#[test]
fn native_entrypoint_preserves_layers_settings_arguments_and_exit_status() {
    let f = Fixture::new();
    let runtime = f.runtime("1.4.0");
    management::select(&f.0, "1.4.0").unwrap();
    let original = fs::read(runtime.join("afmf-linux.json")).unwrap();
    let mango = f.0.join("config/vulkan/implicit_layer.d");
    fs::create_dir_all(&mango).unwrap();
    fs::write(mango.join("MangoHud.x86_64.json"), r#"{"layer":{"name":"VK_LAYER_MANGOHUD_overlay_x86_64","type":"GLOBAL","library_path":"fixture.so","enable_environment":{"MANGOHUD":"1"},"disable_environment":{"DISABLE_MANGOHUD":"1"}}}"#).unwrap();
    let output = command(&f, true)
        .env(
            "VK_INSTANCE_LAYERS",
            "VK_LAYER_VALVE_steam_overlay_64:VK_LAYER_NR:VK_LAYER_MANGOHUD_overlay_x86_64",
        )
        .env("VK_ADD_LAYER_PATH", "/fixture/nr")
        .env("PROTON_ENABLE_WAYLAND", "1")
        .env("MANGOHUD_CONFIGFILE", "/fixture/hud.conf")
        .env(
            "MANGOHUD_CONFIG",
            "read_cfg,fps_limit=144,vsync=1,no_display=1",
        )
        .env("PELI_OVERLAY_BACKGROUND_FPS", "30")
        .args([
            "/bin/sh",
            "-c",
            "printf '%s\\n' \"$@\"; /usr/bin/env; exit 17",
            "fixture",
            ";literal",
            "path with spaces",
            "ç",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(17),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with(";literal\npath with spaces\nç\n"));
    for expected in [
        "AFMF_ENABLE=1", "AFMF_ASYNC=1", "AFMF_MIN_FPS=45", "AFMF_GAMESCOPE=1",
        "VK_INSTANCE_LAYERS=VK_LAYER_VALVE_steam_overlay_64:VK_LAYER_NR:VK_LAYER_AFMF:VK_LAYER_MANGOHUD_overlay_x86_64",
        "MANGOHUD_CONFIG=read_cfg,fps_limit=144,no_display=1,vsync=-1",
        "DISABLE_MANGOHUD=1", "PELI_OVERLAY_BACKGROUND_FPS=30", "PROTON_ENABLE_WAYLAND=1",
    ] { assert!(text.lines().any(|line| line == expected), "Missing {expected}: {text}"); }
    assert_eq!(fs::read(runtime.join("afmf-linux.json")).unwrap(), original);
    let private: serde_json::Value = serde_json::from_slice(
        &fs::read(runtime.join("peligames-explicit-layer/afmf-linux.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        private["layer"]["library_path"],
        runtime.join("libafmf-linux.so").to_str().unwrap()
    );
}

#[test]
fn removing_the_runtime_keeps_the_game_launchable_and_other_layers_intact() {
    let f = Fixture::new();
    f.runtime("1.3.1");
    management::remove(&f.0, "1.3.1").unwrap();
    let output = command(&f, false)
        .env("AFMF_ENABLE", "1")
        .env("VK_INSTANCE_LAYERS", "VK_LAYER_NR:VK_LAYER_AFMF")
        .args(["/usr/bin/env"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.lines().any(|line| line.starts_with("AFMF_ENABLE=")));
    assert!(text
        .lines()
        .any(|line| line == "VK_INSTANCE_LAYERS=VK_LAYER_NR"));
    assert!(!f.0.join("afmf/versions/1.3.1").exists());
}

#[test]
fn malformed_native_context_never_executes_the_game() {
    let f = Fixture::new();
    let marker = f.0.join("executed");
    let output = Command::new(env!("CARGO_BIN_EXE_peligames-afmf-service"))
        .args(["--peligames-launch", "{}", "--", "/usr/bin/touch"])
        .arg(&marker)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!marker.exists());
}
