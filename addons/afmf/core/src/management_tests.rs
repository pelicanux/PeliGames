use crate::{download, management, releases, runtime};
use std::fs;
#[path = "../../tests/fixtures.rs"]
mod fixtures;
use fixtures::Fixture;
fn archive(path: &str, kind: tar::EntryType) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(kind);
    header.set_mode(0o600);
    header.set_size(0);
    header.set_path("placeholder").unwrap();
    if kind == tar::EntryType::Symlink {
        header.set_link_name("/outside").unwrap();
    }
    header.as_mut_bytes()[..100].fill(0);
    header.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
    header.set_cksum();
    builder.append(&header, &[][..]).unwrap();
    builder.finish().unwrap();
    let bytes = builder.into_inner().unwrap();
    use std::io::Write;
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
    encoder.write_all(&bytes).unwrap();
    encoder.finish().unwrap()
}
#[test]
fn version_selection_removal_and_legacy_paths_preserve_other_versions() {
    let f = Fixture::new();
    let first = f.runtime("1.3.1");
    let next = f.runtime("1.4.0");
    management::select(&f.0, "1.4.0").unwrap();
    assert_eq!(management::state(&f.0).unwrap().current, "1.4.0");
    assert!(management::select(&f.0, "2.0.0").is_err());
    assert_eq!(management::current(&f.0), "1.4.0");
    let state = management::remove(&f.0, "1.4.0").unwrap();
    assert!(state.ready);
    assert_eq!(state.current, "1.3.1");
    assert!(first.is_dir());
    assert!(!next.exists());
    let state = management::remove(&f.0, "1.3.1").unwrap();
    assert!(!state.ready);
    assert!(!f.0.join("afmf/current.json").exists());
    fs::rename(first.parent().unwrap(), f.0.join("empty-versions")).unwrap();
    let legacy = f.0.join("afmf-1.3.1");
    fs::create_dir(&legacy).unwrap();
    assert_eq!(runtime::location(&f.0, "1.3.1").unwrap(), legacy);
}
#[test]
fn release_catalog_requires_official_stable_urls_and_checksums() {
    let release = releases::builtin();
    let asset = serde_json::json!({"name":"afmf-linux-1.3.1-x86_64.tar.xz","browser_download_url":release.url,"digest":format!("sha256:{}",release.hash)});
    let mut catalog = serde_json::json!([{"tag_name":"v1.3.1","draft":false,"prerelease":false,"assets":[asset]}]);
    assert_eq!(releases::parse(&catalog).unwrap().version, "1.3.1");
    catalog[0]["prerelease"] = serde_json::json!(true);
    assert!(releases::parse(&catalog).is_none());
    catalog[0]["prerelease"] = serde_json::json!(false);
    catalog[0]["assets"][0]["browser_download_url"] =
        serde_json::json!("https://elsewhere/package.tar.xz");
    assert!(releases::parse(&catalog).is_none());
    catalog[0]["assets"][0]["browser_download_url"] = serde_json::json!(release.url);
    catalog[0]["assets"][0]["digest"] = serde_json::Value::Null;
    assert!(releases::parse(&catalog).is_none());
    assert!(releases::compatible("1.4.2"));
    for invalid in ["1.2.0", "2.0.0", "1.3.1+extra", "../other"] {
        assert!(!releases::compatible(invalid));
    }
}
#[test]
fn extraction_rejects_escape_links_and_invalid_hashes_without_touching_games() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let save = f.0.join("game-save");
    fs::write(&save, b"preserve").unwrap();
    for bytes in [
        archive("../outside", tar::EntryType::Regular),
        archive("link", tar::EntryType::Symlink),
    ] {
        let mut release = releases::builtin();
        release.hash = format!("{:x}", Sha256::digest(&bytes));
        assert!(download::install_bytes(&f.0, &release, &bytes).is_err());
        assert!(!f.0.join("outside").exists());
        assert!(!f.0.join("afmf/versions/1.3.1").exists());
    }
    assert!(download::install_bytes(&f.0, &releases::builtin(), b"wrong download").is_err());
    assert_eq!(fs::read(save).unwrap(), b"preserve");
    assert!(!fs::read_dir(f.0.join("afmf"))
        .unwrap()
        .flatten()
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with("install-stage-")));
}
#[test]
fn verified_archive_is_flattened_and_installed_without_overwriting_another_version() {
    use sha2::{Digest, Sha256};
    use std::io::Write;
    let f = Fixture::new();
    let existing = f.runtime("1.3.1");
    let original = fs::read(existing.join("libafmf-linux.so")).unwrap();
    let mut tar = tar::Builder::new(Vec::new());
    tar.append_dir_all("afmf-linux-1.4.0", &existing).unwrap();
    let raw = tar.into_inner().unwrap();
    let mut encoder = xz2::write::XzEncoder::new(Vec::new(), 1);
    encoder.write_all(&raw).unwrap();
    let bytes = encoder.finish().unwrap();
    let mut release = releases::builtin();
    release.version = "1.4.0".into();
    release.url="https://github.com/serialexperimentslainnnn/afmf-linux/releases/download/v1.4.0/afmf-linux-1.4.0-x86_64.tar.xz".into();
    release.hash = format!("{:x}", Sha256::digest(&bytes));
    download::install_bytes(&f.0, &release, &bytes).unwrap();
    let state = management::state(&f.0).unwrap();
    assert_eq!(state.current, "1.4.0");
    assert_eq!(state.versions.len(), 2);
    assert_eq!(
        fs::read(existing.join("libafmf-linux.so")).unwrap(),
        original
    );
    assert!(runtime::location(&f.0, "1.4.0")
        .unwrap()
        .join("afmf-linux.json")
        .is_file());
}

#[test]
fn redirected_storage_ancestors_cannot_remove_another_directory() {
    let f = Fixture::new();
    let outside = Fixture::new();
    let runtime = outside.runtime("1.3.1");
    std::os::unix::fs::symlink(outside.0.join("afmf"), f.0.join("afmf")).unwrap();
    assert!(management::state(&f.0).is_err());
    assert!(management::remove(&f.0, "1.3.1").is_err());
    assert!(management::select(&f.0, "1.3.1").is_err());
    assert!(runtime.join("libafmf-linux.so").is_file());
}

#[test]
fn runtime_cannot_be_removed_while_a_native_process_maps_its_library() {
    use std::os::fd::AsRawFd;
    let f = Fixture::new();
    let runtime = f.runtime("1.3.1");
    let path = runtime.join("libafmf-linux.so");
    let file = fs::File::open(&path).unwrap();
    let mapping = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            64,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            file.as_raw_fd(),
            0,
        )
    };
    assert_ne!(mapping, libc::MAP_FAILED);
    let result = management::remove(&f.0, "1.3.1");
    unsafe {
        libc::munmap(mapping, 64);
    }
    assert!(result.err().unwrap().contains("Close games"));
    assert!(path.is_file());
    management::remove(&f.0, "1.3.1").unwrap();
    assert!(!path.exists());
}
