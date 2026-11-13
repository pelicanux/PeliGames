use peligames_addon_kit::{
    catalog::{Artifact, Entry},
    manifest::{Capability, Manifest},
    package,
    protocol::LaunchContext,
    registry::{self, Store},
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
#[allow(dead_code)]
#[path = "../../../tools/storage_layout.rs"]
mod layout;
const HOST: &str = "0.5.5";
const PLATFORM: &str = "linux-x86_64";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "peligames-native-addon-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> Store {
        Store {
            packages: self.0.join(layout::ADDON_PACKAGES),
            state_file: self.0.join(layout::ADDON_STATE),
        }
    }
    fn install(&self, version: &str, native: bool) -> PathBuf {
        let mut manifest: Manifest = serde_json::from_str(include_str!(
            "../../../addons/_infra/template/manifest.json"
        ))
        .unwrap();
        manifest.id = "fixture-native".into();
        manifest.version = version.into();
        manifest.contributions.clear();
        manifest.entrypoints.ui = None;
        manifest.component_directory = Some("components/Fixture".into());
        if native {
            manifest.capabilities.push(Capability::NativeLaunch);
        }
        let payload = self.0.join(format!("payload-{version}"));
        fs::create_dir_all(payload.join("bin")).unwrap();
        fs::write(payload.join("LICENSE"), "GPL-3.0-only fixture").unwrap();
        // The fixture implements the native entrypoint contract and passes literal argv on.
        fs::write(payload.join("bin/example-service"),format!("#!/bin/sh\n[ \"$1\" = --peligames-launch ] || exit 90\nprintf '%s\\n' \"$2\"\nshift 3\nexport FIXTURE_VERSION={version}\nexec \"$@\"\n")).unwrap();
        let bytes = package::pack(manifest.clone(), &payload).unwrap();
        let entry=Entry {manifest, artifacts:BTreeMap::from([(PLATFORM.into(),Artifact {url:"https://github.com/pelicanux/PeliGames/releases/download/fixture/native.zip".into(),bytes:bytes.len() as u64,sha256:package::digest(&bytes)})])};
        let receipt = package::prepare(&self.store().packages, &entry, PLATFORM, &bytes).unwrap();
        self.store()
            .register(&[receipt.clone()], HOST, PLATFORM)
            .unwrap();
        registry::entrypoint(&self.store(), "fixture-native", true, HOST, PLATFORM).unwrap()
    }
    fn command(&self) -> Command {
        let context = LaunchContext {
            component_directory: "/untrusted/input".into(),
            settings: serde_json::json!({"enabled":true}),
            gamescope: false,
            monitoring: false,
            options: vec![],
        };
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pelirun"));
        cmd.env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.0)
            .current_dir(&self.0)
            .arg("--addon")
            .arg("fixture-native")
            .arg(&self.0)
            .arg(HOST)
            .arg(serde_json::to_string(&context).unwrap())
            .arg("--");
        cmd
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn verified_native_package_receives_host_path_and_preserves_argv_exit_and_environment() {
    let f = Fixture::new();
    f.install("0.1.0", true);
    f.install("0.2.0", true);
    f.store()
        .select("fixture-native", "0.2.0", HOST, PLATFORM)
        .unwrap();
    let output = f
        .command()
        .env("KEEP_ME", "value")
        .args([
            "/bin/sh",
            "-c",
            "printf '%s\\n' \"$FIXTURE_VERSION\" \"$KEEP_ME\" \"$PWD\" \"$@\"; exit 23",
            "fixture",
            ";literal",
            "space arg",
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
    let text = String::from_utf8(output.stdout).unwrap();
    let mut lines = text.lines();
    let context: LaunchContext = serde_json::from_str(lines.next().unwrap()).unwrap();
    assert_eq!(context.component_directory, f.0.join("components/Fixture"));
    assert_eq!(
        lines.collect::<Vec<_>>(),
        vec![
            "0.2.0",
            "value",
            f.0.to_str().unwrap(),
            ";literal",
            "space arg",
            "ç"
        ]
    );
    f.store()
        .select("fixture-native", "0.1.0", HOST, PLATFORM)
        .unwrap();
    let output = f.command().arg("/usr/bin/env").output().unwrap();
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("FIXTURE_VERSION=0.1.0"));
}

#[test]
fn disabled_and_removed_packages_bypass_stale_launch_commands() {
    let f = Fixture::new();
    f.install("0.1.0", true);
    f.store()
        .enable("fixture-native", false, HOST, PLATFORM)
        .unwrap();
    for removed in [false, true] {
        if removed {
            f.store().remove("fixture-native", "0.1.0").unwrap();
        }
        let output = f
            .command()
            .args([
                "/bin/sh",
                "-c",
                "printf '%s' \"$1\"; exit 19",
                "fixture",
                "literal ; command",
            ])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(output.stdout, b"literal ; command");
    }
}

#[test]
fn tampered_payload_or_missing_capability_never_starts_a_game() {
    for native in [true, false] {
        let f = Fixture::new();
        let service = f.install("0.1.0", native);
        if native {
            fs::write(service, b"#!/bin/sh\nexit 0\n").unwrap();
        }
        let marker = f.0.join("executed");
        let output = f
            .command()
            .arg("/usr/bin/touch")
            .arg(&marker)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!marker.exists());
    }
}

#[test]
fn exported_legacy_profile_uses_registry_and_bypasses_a_disabled_package() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    f.install("0.1.0", true);
    let runner = f.0.join("runner");
    fs::create_dir(&runner).unwrap();
    let helper = runner.join("pelirun");
    fs::copy(env!("CARGO_BIN_EXE_pelirun"), &helper).unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    let legacy =
        f.0.join("components/FrameGeneration/profiles/fixture-game.sh");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::write(&legacy, b"#!/bin/sh\nexit 88\n").unwrap();
    fs::set_permissions(&legacy, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(
        legacy.with_extension("json"),
        br#"{"backend":"fixture-native","enabled":true}"#,
    )
    .unwrap();
    fs::create_dir(runner.join("profiles")).unwrap();
    let profile = runner.join("profiles/fixture-game.json");
    fs::write(&profile,serde_json::to_vec(&serde_json::json!({"version":2,"id":"fixture-game","installation":f.0,"command":[legacy,"__PELIGAMES_COMMAND__"],"environment":{},"extra_args":[]})).unwrap()).unwrap();
    fs::set_permissions(profile, fs::Permissions::from_mode(0o600)).unwrap();
    let run = || {
        let mut command = Command::new(&helper);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &f.0)
            .args([
                "--profile",
                "fixture-game",
                "--",
                "/bin/sh",
                "-c",
                "printf '%s\\n' \"${FIXTURE_VERSION:-bypass}\" \"$@\"; exit 17",
                "fixture",
                ";literal",
                "space arg",
            ]);
        command.output().unwrap()
    };
    let output = run();
    assert_eq!(
        output.status.code(),
        Some(17),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("0.1.0\n;literal\nspace arg\n"));
    f.store()
        .enable("fixture-native", false, HOST, PLATFORM)
        .unwrap();
    let output = run();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "bypass\n;literal\nspace arg\n"
    );
    assert!(legacy.is_file());
    fs::remove_file(legacy.with_extension("json")).unwrap();
    let output = run();
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "bypass\n;literal\nspace arg\n"
    );
}
