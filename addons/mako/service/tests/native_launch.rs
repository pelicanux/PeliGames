#[path = "../../tests/fixtures.rs"]
mod fixtures;
use fixtures::Fixture;
use peligames_addon_kit::protocol::LaunchContext;
use std::{fs, process::Command};
fn command(f: &Fixture, enabled: bool, profile: std::path::PathBuf) -> Command {
    let context = LaunchContext {
        component_directory: f.0.clone(),
        settings: serde_json::json!({"profile":profile,"enabled":enabled}),
        gamescope: false,
        monitoring: false,
        options: vec![],
    };
    let mut command = Command::new(env!("CARGO_BIN_EXE_peligames-mako-service"));
    command
        .env_clear()
        .env("PATH", "/bin:/usr/bin")
        .env("HOME", &f.0)
        .arg("--peligames-launch")
        .arg(serde_json::to_string(&context).unwrap())
        .arg("--");
    command
}
#[test]
fn native_service_preserves_arguments_environment_and_exit_status() {
    let f = Fixture::new();
    let profile = f.profile();
    let config = fs::read(profile.with_extension("toml")).unwrap();
    let output = command(&f, true, profile.clone())
        .env("KEEP", "yes")
        .env("VK_INSTANCE_LAYERS", "VK_LAYER_NR")
        .args([
            "/bin/sh",
            "-c",
            "printf '%s\\n' \"$KEEP\" \"$MAKO_PROFILE\" \"$VK_INSTANCE_LAYERS\" \"$@\"; exit 23",
            "fixture",
            "space arg",
            ";literal",
            "ç",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(23),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "yes\nPeliGames\nVK_LAYER_MAKO_render:VK_LAYER_NR\nspace arg\n;literal\nç\n"
    );
    assert_eq!(fs::read(profile.with_extension("toml")).unwrap(), config);
}
#[test]
fn disabled_launch_does_not_require_profile_or_runtime() {
    let f = Fixture::new();
    let output = command(&f, false, f.0.join("missing.sh"))
        .args(["/bin/sh", "-c", "exit 17"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(17),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn conflicting_environment_and_external_profile_are_rejected_before_the_game() {
    let f = Fixture::new();
    let profile = f.profile();
    let output = command(&f, true, profile)
        .env("PROTON_MLFG_UPGRADE", "1")
        .arg("/bin/true")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Conflicting"));
    let output = command(&f, true, f.0.join("foreign.sh"))
        .arg("/bin/true")
        .output()
        .unwrap();
    assert!(!output.status.success());
}
