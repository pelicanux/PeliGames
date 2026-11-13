use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "peligames-afmf-runtime-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let mut elf = [0u8; 64];
        elf[..5].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2]);
        elf[18] = 62;
        fs::write(path.join("libafmf-linux.so"), elf).unwrap();
        fs::write(path.join("afmf-linux.json"),r#"{"layer":{"name":"VK_LAYER_AFMF","library_path":"libafmf-linux.so","enable_environment":{"AFMF_ENABLE":"1"},"disable_environment":{"DISABLE_AFMF":"1"}}}"#).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn private_manifest_resolves_the_library_without_modifying_the_package() {
    let f = Fixture::new();
    let before = fs::read(f.0.join("afmf-linux.json")).unwrap();
    let directory = prepare_manifest(&f.0).unwrap();
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("afmf-linux.json")).unwrap()).unwrap();
    let library = PathBuf::from(value["layer"]["library_path"].as_str().unwrap());
    assert!(library.is_absolute() && library.is_file());
    assert_eq!(value["layer"]["library_arch"], "64");
    assert_eq!(value["file_format_version"], "1.2.1");
    assert!(value["layer"].get("enable_environment").is_none());
    assert!(value["layer"].get("disable_environment").is_none());
    assert_eq!(fs::read(f.0.join("afmf-linux.json")).unwrap(), before);
    validate(&f.0).unwrap();
    assert_eq!(prepare_manifest(&f.0).unwrap(), directory);
}

#[cfg(unix)]
#[test]
fn runtime_redirections_and_untrusted_private_folders_are_refused() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new();
    let private = f.0.join("peligames-explicit-layer");
    symlink(&f.0, &private).unwrap();
    assert!(prepare_manifest(&f.0).is_err());
    fs::remove_file(&private).unwrap();
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&private, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(prepare_manifest(&f.0).is_err());
    fs::remove_dir(&private).unwrap();
    let library = f.0.join("libafmf-linux.so");
    fs::rename(&library, f.0.join("original.so")).unwrap();
    symlink(f.0.join("original.so"), library).unwrap();
    assert!(prepare_manifest(&f.0).is_err());
    assert!(!private.exists());
}

#[test]
fn paths_preserve_legacy_installations_and_reject_noncanonical_versions() {
    let f = Fixture::new();
    assert_eq!(
        location(&f.0, "1.3.1").unwrap(),
        f.0.join("afmf/versions/1.3.1")
    );
    let legacy = f.0.join("afmf-1.3.1");
    fs::create_dir(&legacy).unwrap();
    assert_eq!(location(&f.0, "1.3.1").unwrap(), legacy);
    for invalid in ["../runtime", "01.3.1", "1.3.1+local", "1.3.1-test", "1.3"] {
        assert!(location(&f.0, invalid).is_err(), "{invalid}");
    }
    assert!(location(Path::new("relative"), "1.3.1").is_err());
}

#[test]
fn oversized_or_invalid_runtime_metadata_does_not_create_an_adapter() {
    let f = Fixture::new();
    fs::write(f.0.join("afmf-linux.json"), vec![b' '; 256 * 1024 + 1]).unwrap();
    assert!(prepare_manifest(&f.0).is_err());
    assert!(!f.0.join("peligames-explicit-layer").exists());
}
