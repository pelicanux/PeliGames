//! Execute the real helper with isolated Epic fixtures; never use the user's library.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs,
    os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    original: PathBuf,
    selected: PathBuf,
    routes: PathBuf,
    executable: PathBuf,
}
fn script(path: &Path, label: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!("#!/bin/sh\nprintf '%s\\n' '{label}' \"${{PG_EPIC_TEST:-missing}}\" \"$@\"\n"),
    )
    .unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "peligames-native-epic-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("compat/game/pfx")).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let f = Self {
            original: root.join("original/proton"),
            selected: root.join("Proton with ' quote/proton"),
            routes: root.join("routes.json"),
            executable: root.join("Game with spaces.bin"),
            root,
        };
        script(&f.original, "EPIC");
        script(&f.selected, "SELECTED");
        fs::write(&f.executable, b"fixture, never executed").unwrap();
        f.save(f.route());
        f
    }
    fn route(&self) -> Value {
        json!({"game":{"id":"ns:item:game","compat":self.root.join("compat/game"),"prefix":self.root.join("compat/game/pfx"),"executable":self.executable,"proton":self.selected,"command":["__PELIGAMES_EPIC_PROTON__"],"environment":{"PG_EPIC_TEST":"with spaces"},"arguments":["--extra"],"legacy_adapter":self.root.join("legacy.sh")}})
    }
    fn save(&self, value: Value) {
        fs::write(&self.routes, serde_json::to_vec(&value).unwrap()).unwrap();
        fs::set_permissions(&self.routes, fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn run(&self, artifact: &str, extra: &[OsString]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_pelirun"))
            .args(["--epic-runtime"])
            .arg(&self.routes)
            .arg("--")
            .arg(&self.original)
            .arg("waitforexitandrun")
            .arg(&self.executable)
            .args(extra)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.root)
            .env(
                "STEAM_COMPAT_DATA_PATH",
                self.root.join("compat").join(artifact),
            )
            .env(
                "WINEPREFIX",
                self.root.join("compat").join(artifact).join("pfx"),
            )
            .output()
            .unwrap()
    }
    fn text(&self, artifact: &str) -> String {
        let out = self.run(artifact, &[]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn native_epic_reads_changes_each_launch_and_keeps_other_games_original() {
    let f = Fixture::new();
    assert!(f
        .text("game")
        .starts_with("SELECTED\nwith spaces\nwaitforexitandrun\n"));
    assert!(f.text("game").ends_with("--extra\n"));
    assert!(f.text("other").starts_with("EPIC\nmissing\n"));
    let mut route = f.route();
    route["game"]["environment"]["PG_EPIC_TEST"] = json!("updated");
    f.save(route);
    assert!(f.text("game").starts_with("SELECTED\nupdated\n"));
    let log = fs::read_to_string(f.root.join("launch.jsonl")).unwrap();
    assert!(log.contains("selected"));
    assert!(!log.contains("with spaces"));
    assert!(!log.contains("--extra"));
}
#[test]
fn native_epic_defaults_on_missing_corrupt_ambiguous_or_foreign_profiles() {
    let f = Fixture::new();
    fs::remove_file(&f.routes).unwrap();
    assert!(f.text("game").starts_with("EPIC\nmissing\n"));
    fs::write(&f.routes, b"invalid JSON").unwrap();
    assert!(f.text("game").starts_with("EPIC\nmissing\n"));
    let mut route = f.route();
    route["other"] = route["game"].clone();
    f.save(route);
    assert!(f.text("game").starts_with("EPIC\nmissing\n"));
    let mut route = f.route();
    route["game"]["executable"] = json!(f.root.join("Other.bin"));
    f.save(route);
    assert!(f.text("game").starts_with("EPIC\nmissing\n"));
    let mut route = f.route();
    route["game"]["command"] = json!([]);
    f.save(route);
    assert!(f.text("game").starts_with("EPIC\nmissing\n"));
}
#[test]
fn native_epic_rejects_symlinks_and_world_writable_routes() {
    let f = Fixture::new();
    let target = f.root.join("saved.json");
    fs::rename(&f.routes, &target).unwrap();
    std::os::unix::fs::symlink(&target, &f.routes).unwrap();
    assert!(f.text("game").starts_with("EPIC\nmissing\n"));
    fs::remove_file(&f.routes).unwrap();
    f.save(f.route());
    fs::set_permissions(&f.routes, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(f.text("game").starts_with("EPIC\nmissing\n"));
}
#[test]
fn native_epic_missing_selected_proton_keeps_options_and_logs_only_status() {
    let f = Fixture::new();
    fs::remove_file(&f.selected).unwrap();
    assert!(f.text("game").starts_with("EPIC\nwith spaces\n"));
    let log = fs::read_to_string(f.root.join("launch.jsonl")).unwrap();
    assert!(log.contains("selected-unavailable-using-epic"));
}
#[test]
fn native_epic_unwraps_adapted_proton_and_refuses_corrupted_backup() {
    let f = Fixture::new();
    let bytes = fs::read(&f.selected).unwrap();
    let backup = f.selected.with_file_name("proton.peligames-original-test");
    fs::write(&backup, &bytes).unwrap();
    fs::set_permissions(&backup, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        &f.selected,
        "#!/bin/sh\n# PeliGames Epic runtime adapter v1\nexit 99\n",
    )
    .unwrap();
    fs::write(f.selected.with_file_name(".peligames-runtime.json"),serde_json::to_vec(&json!({"backup":"proton.peligames-original-test","original_sha256":format!("{:x}",Sha256::digest(&bytes)),"shim_sha256":"unused"})).unwrap()).unwrap();
    assert!(f.text("game").starts_with("SELECTED\nwith spaces\n"));
    fs::write(&backup, b"corrupted").unwrap();
    assert!(f.text("game").starts_with("EPIC\nwith spaces\n"));
}
#[test]
fn native_epic_preserves_literal_arguments_and_removes_only_legacy_adapter() {
    let f = Fixture::new();
    let legacy = f.root.join("legacy.sh");
    let args = [
        legacy.into_os_string(),
        "%command%".into(),
        "$(touch should-not-exist)".into(),
        "--keep".into(),
        OsString::from_vec(vec![0xff, b'x']),
    ];
    let out = f.run("game", &args);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("legacy.sh"));
    assert!(!text.contains("%command%"));
    assert!(text.contains("$(touch should-not-exist)"));
    assert!(text.contains("--keep"));
    assert!(out.stdout.windows(2).any(|b| b == [0xff, b'x']));
}
#[test]
fn native_epic_keeps_prefix_and_preserves_the_original_exit_status() {
    let f = Fixture::new();
    let mut route = f.route();
    route["game"]["environment"]["WINEPREFIX"] = json!("/invalid/prefix");
    f.save(route);
    fs::write(
        &f.selected,
        "#!/bin/sh\nprintf '%s\\n' \"$WINEPREFIX\"\nexit 37\n",
    )
    .unwrap();
    let out = f.run("game", &[]);
    assert_eq!(out.status.code(), Some(37));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        f.root.join("compat/game/pfx").to_str().unwrap()
    );
}
