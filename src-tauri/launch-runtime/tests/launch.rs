//! These tests execute only the isolated helper and tiny shell fixtures, never
//! Steam, Proton, a game, the GUI, or the user's real configuration.
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("peligames-helper-{}-{id}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::create_dir(path.join("profiles")).unwrap();
        fs::copy(env!("CARGO_BIN_EXE_pelirun"), path.join("pelirun")).unwrap();
        Self(path)
    }
    fn profile(&self, content: &serde_json::Value) {
        let file = self.0.join("profiles/42.json");
        fs::write(&file, serde_json::to_vec(content).unwrap()).unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn command(&self) -> Command {
        let mut command = Command::new(self.0.join("pelirun"));
        command.env_remove("PELI_HELPER_TEST");
        command.args([
            "--app",
            "42",
            "--",
            "/bin/sh",
            "-c",
            "printf '%s\\n' \"${PELI_HELPER_TEST-unset}\" \"$@\"",
            "--",
        ]);
        command
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn edits_are_read_on_each_launch_and_arguments_remain_literal() {
    let fixture = Fixture::new();
    let mut profile = serde_json::json!({"version":1,"app_id":42,"command":["__PELIGAMES_STEAM_COMMAND__"],"environment":{"PELI_HELPER_TEST":"first"},"extra_args":["two words","$(not-a-command)"]});
    fixture.profile(&profile);
    let before = fixture.command().arg("original argument").output().unwrap();
    assert!(before.status.success());
    assert_eq!(
        String::from_utf8(before.stdout).unwrap(),
        "first\noriginal argument\ntwo words\n$(not-a-command)\n"
    );
    profile["environment"]["PELI_HELPER_TEST"] = serde_json::Value::Null;
    fixture.profile(&profile);
    let after = fixture
        .command()
        .env("PELI_HELPER_TEST", "inherited")
        .output()
        .unwrap();
    assert!(after.status.success());
    assert_eq!(
        String::from_utf8(after.stdout).unwrap(),
        "unset\ntwo words\n$(not-a-command)\n"
    );
}

#[test]
fn published_helper_ignores_an_old_experimental_wrapper_without_losing_dlss() {
    let features: serde_json::Value = serde_json::from_str(include_str!("../../../release-features.json")).unwrap();
    if features["experimental_frame_generation"] == true { return; }
    let fixture = Fixture::new();
    let forbidden = fixture.0.join("FrameGeneration/profiles/old.sh");
    fs::create_dir_all(forbidden.parent().unwrap()).unwrap();
    let marker = fixture.0.join("must-not-execute");
    fs::write(&forbidden, format!("#!/bin/sh\ntouch '{}'\nexit 99\n", marker.display())).unwrap();
    fs::set_permissions(&forbidden,fs::Permissions::from_mode(0o700)).unwrap();
    fixture.profile(&serde_json::json!({"version":1,"app_id":42,"command":[forbidden.to_string_lossy(),"__PELIGAMES_STEAM_COMMAND__"],"environment":{"ENABLE_MAKO":"1","MAKO_CONFIG":"/tmp/old.toml","WINEDLLOVERRIDES":"dxgi=n,b","VK_INSTANCE_LAYERS":"VK_LAYER_MAKO_render:VK_LAYER_DLSSNR_int4:VK_LAYER_PELIGAMES_overlay"},"extra_args":[]}));
    let output = Command::new(fixture.0.join("pelirun")).args(["--app","42","--","/bin/sh","-c","printf '%s|%s|%s' \"${ENABLE_MAKO-unset}\" \"$VK_INSTANCE_LAYERS\" \"$WINEDLLOVERRIDES\""]).env_remove("ENABLE_MAKO").output().unwrap();
    assert!(output.status.success());
    assert!(!marker.exists());
    assert_eq!(String::from_utf8(output.stdout).unwrap(),"unset|VK_LAYER_DLSSNR_int4|dxgi=n,b");
}

#[test]
fn reset_without_a_profile_preserves_normal_execution_and_exit_status() {
    let fixture = Fixture::new();
    let output = fixture.command().arg("normal").output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "unset\nnormal\n");
    let status = Command::new(fixture.0.join("pelirun"))
        .args(["--app", "42", "--", "/bin/sh", "-c", "exit 23"])
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(23));
}

#[test]
fn corrupt_profiles_and_symlinks_do_not_launch_an_unvalidated_command() {
    let fixture = Fixture::new();
    fixture.profile(&serde_json::json!({"version":1,"app_id":43,"command":["__PELIGAMES_STEAM_COMMAND__"],"environment":{},"extra_args":[]}));
    let output = fixture.command().output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    fs::remove_file(fixture.0.join("profiles/42.json")).unwrap();
    std::os::unix::fs::symlink("../unrelated", fixture.0.join("profiles/42.json")).unwrap();
    assert!(!fixture.command().status().unwrap().success());
}

fn universal(fixture: &Fixture, value: &str) {
    let path = fixture.0.join("profiles/pg-a73c92e4.json");
    fs::write(&path, serde_json::to_vec(&serde_json::json!({
        "version":2,"id":"pg-a73c92e4","installation":"/games/tomb-raider",
        "command":["__PELIGAMES_STEAM_COMMAND__"],
        "environment":{"PELI_HELPER_TEST":value},"extra_args":["literal argument"]
    })).unwrap()).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn one_universal_profile_serves_other_launchers_and_legacy_steam_links() {
    let fixture = Fixture::new();
    universal(&fixture, "first");
    fixture.profile(&serde_json::json!({"version":1,"app_id":42,
        "pelirun_reference":"pg-a73c92e4","directory":fixture.0.join("profiles")}));
    let run = || Command::new(fixture.0.join("pelirun"))
        .args(["--profile", "pg-a73c92e4", "--", "/bin/sh", "-c", "printf '%s\\n' \"$PELI_HELPER_TEST\" \"$@\"", "--"])
        .arg("other launcher").output().unwrap();
    let output = run();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "first\nother launcher\nliteral argument\n");
    assert_eq!(String::from_utf8(fixture.command().output().unwrap().stdout).unwrap(), "first\nliteral argument\n");
    // The fixed commands stay unchanged. Both callers read the same edited file.
    universal(&fixture, "updated");
    assert!(String::from_utf8(run().stdout).unwrap().starts_with("updated\n"));
    assert!(String::from_utf8(fixture.command().output().unwrap().stdout).unwrap().starts_with("updated\n"));
    fs::remove_file(fixture.0.join("profiles/pg-a73c92e4.json")).unwrap();
    assert_eq!(String::from_utf8(fixture.command().output().unwrap().stdout).unwrap(), "unset\n");
}

#[test]
fn universal_profiles_reject_wrong_identity_symlinks_and_traversal() {
    let fixture = Fixture::new();
    universal(&fixture, "private");
    let run = |id: &str| Command::new(fixture.0.join("pelirun")).args(["--profile",id,"--","/bin/echo","launched"]).output().unwrap();
    for id in ["../42", "pg-../../42", "pg-A73C92E4", "42", "pg-a73c92e4/extra"] {
        let output = run(id);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
    let path = fixture.0.join("profiles/pg-a73c92e4.json");
    let mut profile: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    profile["id"] = "pg-00000000".into();
    fs::write(&path, serde_json::to_vec(&profile).unwrap()).unwrap();
    assert!(!run("pg-a73c92e4").status.success());
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("../unrelated", &path).unwrap();
    assert!(!run("pg-a73c92e4").status.success());
    fixture.profile(&serde_json::json!({"version":1,"app_id":43,
        "pelirun_reference":"pg-a73c92e4","directory":fixture.0.join("profiles")}));
    assert!(!fixture.command().output().unwrap().status.success());
}
