//! Keep the native verifier independent of distro packages and the process PATH.
use std::{env, fs, path::PathBuf, process::Command};

pub fn embed() {
    for path in [
        "build/installer_verifier.rs",
        "../scripts/installer-verifier.sh",
        "../scripts/installer-verifier.json",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=PELIGAMES_INSTALLER_VERIFIER");
    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string("../scripts/installer-verifier.json")
            .expect("installer verifier manifest"),
    )
    .expect("valid installer verifier manifest");
    let version = manifest["version"].as_str().expect("verifier version");
    println!("cargo:rustc-env=PELIGAMES_INSTALLER_VERIFIER_VERSION={version}");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let payload = output.join("installer-verifier.gz");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        fs::write(&payload, []).expect("non-Linux verifier placeholder");
    } else {
        let source = if let Some(path) = env::var_os("PELIGAMES_INSTALLER_VERIFIER") {
            let path = PathBuf::from(path);
            println!("cargo:rerun-if-changed={}", path.display());
            path
        } else {
            assert_eq!(env::var("HOST").ok(), env::var("TARGET").ok(),
                "Cross-compilation requires PELIGAMES_INSTALLER_VERIFIER for the target architecture");
            let folder = output.join("installer-verifier");
            assert!(
                Command::new("bash")
                    .arg("../scripts/installer-verifier.sh")
                    .arg(&folder)
                    .status()
                    .expect("prepare installer verifier")
                    .success(),
                "Failed to prepare the bundled installer verifier"
            );
            folder.join("osslsigncode.gz")
        };
        let decoded = Command::new("gzip")
            .arg("-dc")
            .arg(&source)
            .output()
            .expect("read compressed installer verifier");
        let bytes = &decoded.stdout;
        let machine = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
            Ok("x86_64") => [62, 0],
            Ok("aarch64") => [183, 0],
            _ => panic!("Unsupported installer verifier architecture"),
        };
        assert!(
            decoded.status.success()
                && bytes.len() >= 64
                && &bytes[..7] == b"\x7fELF\x02\x01\x01"
                && bytes[18..20] == machine,
            "Installer verifier is not a Linux ELF for the target architecture"
        );
        fs::copy(source, &payload).expect("embed installer verifier");
    }
    println!(
        "cargo:rustc-env=PELIGAMES_INSTALLER_VERIFIER_EMBEDDED={}",
        payload.display()
    );
}
