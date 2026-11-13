use super::*;
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
#[test]
fn managed_binary_publication_is_atomic_private_and_refuses_redirects() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new();
    let path = f.0.join("profiles/game.sh");
    write_atomic_bytes(&path, b"#!/bin/sh\nexit 0\n", 0o700).unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    write_atomic_bytes(&path, &[0, 255, 1], 0o600).unwrap();
    assert_eq!(fs::read(&path).unwrap(), [0, 255, 1]);
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(write_atomic_bytes(&path, b"invalid", 0o4700).is_err());
    let redirected = f.0.join("redirected");
    symlink(&path, &redirected).unwrap();
    assert!(write_atomic_bytes(&redirected, b"changed", 0o600).is_err());
    assert_eq!(fs::read(&path).unwrap(), [0, 255, 1]);
    assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "peligames-config-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn child(&self, mode: &str) -> Child {
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests::lock_child"])
            .env("PELIGAMES_CONFIG_TEST_DIRECTORY", &self.0)
            .env("PELIGAMES_CONFIG_TEST_MODE", mode)
            .spawn()
            .unwrap()
    }
    fn ready(&self) {
        let start = Instant::now();
        while !self.0.join("ready").exists() {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "child did not start"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn lock_child() {
    let Some(root) = std::env::var_os("PELIGAMES_CONFIG_TEST_DIRECTORY") else {
        return;
    };
    let root = PathBuf::from(root);
    let mode = std::env::var("PELIGAMES_CONFIG_TEST_MODE").unwrap();
    if mode == "edit" {
        fs::write(root.join("ready"), "").unwrap();
    }
    let _lock = DirectoryLock::acquire(&root).unwrap();
    if mode == "hold" {
        fs::write(root.join("ready"), "").unwrap();
        let start = Instant::now();
        while !root.join("release").exists() {
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(5));
        }
    } else {
        let path = root.join("OptiScaler.ini");
        let before = fs::read_to_string(&path).unwrap();
        write_atomic(&path, &ini::put(&before, "Menu", "Scale", Some("1.5"))).unwrap();
    }
}

#[test]
fn independent_writers_preserve_both_edits() {
    let f = Fixture::new();
    let path = f.0.join("OptiScaler.ini");
    fs::write(
        &path,
        "; preserve\r\n[Menu]\r\nScale=auto\r\n[DlssNr]\r\nEnabled=false\r\n",
    )
    .unwrap();
    let guard = DirectoryLock::acquire(&f.0).unwrap();
    let mut child = f.child("edit");
    f.ready();
    let before = fs::read_to_string(&path).unwrap();
    write_atomic(&path, &ini::put(&before, "DlssNr", "Enabled", Some("true"))).unwrap();
    drop(guard);
    assert!(child.wait().unwrap().success());
    let text = fs::read_to_string(path).unwrap();
    let values = ini::sections(&text);
    assert_eq!(ini::get(&values, "Menu", "Scale"), Some("1.5"));
    assert_eq!(ini::get(&values, "DlssNr", "Enabled"), Some("true"));
    assert!(text.starts_with("; preserve\r\n"));
}

#[test]
fn locks_time_out_and_process_exit_releases_them_without_unlinking() {
    let f = Fixture::new();
    let mut child = f.child("hold");
    f.ready();
    let start = Instant::now();
    assert!(DirectoryLock::with_timeout(&f.0, Duration::from_millis(30)).is_err());
    assert!(start.elapsed() < Duration::from_secs(1));
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(f.0.join(".peligames-config.lock").is_file());
    let guard = DirectoryLock::acquire(&f.0).unwrap();
    drop(guard);
    assert!(f.0.join(".peligames-config.lock").is_file());
}

#[test]
fn sparse_edits_handle_bom_duplicates_and_removal_without_resetting_other_keys() {
    let text="\u{feff}[Menu]\r\n; keep\r\nScale=auto\r\nscale=2\r\n[Menu]\r\nScale=3\r\nCustom=yes\r\n[Network]\r\nModel=anything\r\n";
    let edited = ini::put(text, "Menu", "Scale", Some("1.5"));
    let parsed = ini::sections(&edited);
    assert_eq!(ini::get(&parsed, "Menu", "Scale"), Some("1.5"));
    assert_eq!(ini::get(&parsed, "Menu", "Custom"), Some("yes"));
    assert_eq!(ini::get(&parsed, "Network", "Model"), Some("anything"));
    assert_eq!(edited.matches("Scale = 1.5").count(), 1);
    assert!(edited.contains("; keep\r\n"));
    let removed = ini::put(&edited, "Menu", "Scale", None);
    assert_eq!(ini::get(&ini::sections(&removed), "Menu", "Scale"), None);
}

#[cfg(unix)]
#[test]
fn atomic_writes_preserve_permissions_and_refuse_symlinks() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new();
    let path = f.0.join("OptiScaler.ini");
    fs::write(&path, "old").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    write_atomic(&path, "new").unwrap();
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    let link = f.0.join("redirected.ini");
    symlink(&path, &link).unwrap();
    assert!(write_atomic(&link, "changed").is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
    symlink(&path, f.0.join(".peligames-config.lock")).unwrap();
    assert!(DirectoryLock::acquire(&f.0).is_err());
    assert!(!fs::read_dir(&f.0).unwrap().any(|p| p
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}
