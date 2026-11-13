use super::*;
use std::io::Write;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("peligames-daniel-tests-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn exe(&self, name: &str, machine: u16) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut bytes = vec![0; 1024];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&128u32.to_le_bytes());
        bytes[128..132].copy_from_slice(b"PE\0\0");
        bytes[132..134].copy_from_slice(&machine.to_le_bytes());
        bytes[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
        fs::write(&path, bytes).unwrap();
        path
    }
    fn game(&self, exe: &Path) -> Game {
        Game {
            directory: string(&self.0),
            executable: Some(string(exe)),
            name: "Example game".into(),
            flatpak: false,
            selected_executable: false,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn refuses_32bit_outside_executables_and_flatpak() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let exe = fixture.exe("Game.exe", 0x8664);
    assert!(target(&fixture.game(&exe)).is_ok());
    let x86 = fixture.exe("Old.exe", 0x14c);
    assert!(target(&fixture.game(&x86)).is_err());
    let other = outside.exe("Other.exe", 0x8664);
    assert!(target(&fixture.game(&other)).is_err());
    let mut game = fixture.game(&exe);
    game.flatpak = true;
    assert!(target(&game).unwrap_err().contains("Flatpak"));
}
#[test]
fn compiler_discovery_rejects_wine_and_32bit_files() {
    let fixture = Fixture::new();
    let x86 = fixture.exe("x86/d3dcompiler_47.dll", 0x14c);
    assert!(!compiler_valid(&x86));
    let wine = fixture.exe("wine/d3dcompiler_47.dll", 0x8664);
    let mut file = fs::OpenOptions::new().write(true).open(&wine).unwrap();
    file.seek(SeekFrom::Start(64)).unwrap();
    file.write_all(b"Wine builtin DLL").unwrap();
    assert!(!compiler_valid(&wine));
    let native = fixture.exe("native/d3dcompiler_47.dll", 0x8664);
    assert_eq!(find_compiler(&fixture.0), Some(native));
}
#[test]
fn existing_mods_are_never_adopted_or_overwritten() {
    let fixture = Fixture::new();
    let exe = fixture.exe("Game.exe", 0x8664);
    assert!(conflict(&fixture.0, false).is_none());
    fs::write(fixture.0.join("OptiScaler.ini"), "user settings").unwrap();
    assert!(conflict(&fixture.0, false)
        .unwrap()
        .contains("OptiScaler.ini"));
    assert!(!owned(&fixture.0, &exe));
    fs::write(fixture.0.join(RECEIPT), b"broken").unwrap();
    assert!(!owned(&fixture.0, &exe));
}
#[test]
fn diagnostics_redact_paths_secrets_and_bound_input() {
    let fixture = Fixture::new();
    let exe = fixture.exe("Game.exe", 0x8664);
    fs::write(
        fixture.0.join("amd_presr.log"),
        "token=private\n/home/player/private/game\ncompleted jobs=20\n",
    )
    .unwrap();
    let report = diagnostics(&fixture.0, &fixture.game(&exe)).unwrap();
    assert!(report.contains("completed jobs=20"));
    assert!(!report.contains("token=private"));
    assert!(!report.contains("/home/player"));
    assert!(report.contains("Log ainda não disponível"));
}
#[test]
fn preparation_rejects_invalid_compiler_before_running_any_script() {
    let fixture = Fixture::new();
    let invalid = fixture.0.join("bad.dll");
    fs::write(&invalid, b"not PE").unwrap();
    assert!(prepare(&fixture.0, &invalid, &|_| {})
        .unwrap_err()
        .contains("Microsoft"));
    assert!(!fixture.0.join("operation.log").exists());
}

#[test]
fn removal_preserves_edited_ini_and_does_not_accept_other_modified_files() {
    let fixture = Fixture::new();
    let game_dir = fixture.0.join("game");
    fs::create_dir(&game_dir).unwrap();
    let exe = fixture.exe("game/Game.exe", 0x8664);
    let game = fixture.game(&exe);
    fs::write(
        game_dir.join("OptiScaler.ini"),
        "[DlssNr]\nNrBackend=daniel\nMySetting=1\n",
    )
    .unwrap();
    fs::write(game_dir.join(JOURNAL), r#"{"version":1,"files":[{"path":"OptiScaler.ini","sha256":"old"},{"path":"winmm.dll","sha256":"unchanged"}],"installed":["OptiScaler.ini","winmm.dll"]}"#).unwrap();
    operations::preserve_settings(&fixture.0, &game_dir, &game).unwrap();
    let saved = walkdir::WalkDir::new(fixture.0.join("backups"))
        .into_iter()
        .filter_map(Result::ok)
        .find(|e| e.file_name() == "OptiScaler.ini")
        .unwrap();
    assert_eq!(
        fs::read_to_string(saved.path()).unwrap(),
        fs::read_to_string(game_dir.join("OptiScaler.ini")).unwrap()
    );
    let journal: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(game_dir.join(JOURNAL)).unwrap()).unwrap();
    assert_ne!(journal["files"][0]["sha256"], "old");
    assert_eq!(journal["files"][1]["sha256"], "unchanged");
    assert_eq!(journal["installed"].as_array().unwrap().len(), 2);
}
#[test]
fn removal_never_follows_an_ini_symlink() {
    let fixture = Fixture::new();
    let exe = fixture.exe("Game.exe", 0x8664);
    let outside = Fixture::new();
    fs::write(outside.0.join("private.ini"), "private").unwrap();
    std::os::unix::fs::symlink(
        outside.0.join("private.ini"),
        fixture.0.join("OptiScaler.ini"),
    )
    .unwrap();
    assert!(
        operations::preserve_settings(&fixture.0, &fixture.0, &fixture.game(&exe))
            .unwrap_err()
            .contains("link")
    );
    assert_eq!(
        fs::read_to_string(outside.0.join("private.ini")).unwrap(),
        "private"
    );
}

#[test]
fn diagnostics_do_not_follow_a_linked_bundle_outside_the_game() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    let exe = fixture.exe("Game.exe", 0x8664);
    fs::write(outside.0.join("launch.log"), "private external text").unwrap();
    std::os::unix::fs::symlink(&outside.0, fixture.0.join("amdnr-linux")).unwrap();
    assert!(!diagnostics(&fixture.0, &fixture.game(&exe))
        .unwrap()
        .contains("private external text"));
}
#[test]
fn installation_logs_are_separated_by_game_identity() {
    let fixture = Fixture::new();
    let a = Fixture::new();
    let b = Fixture::new();
    let log_a = operations::game_log(&fixture.0, &a.0, "Same name").unwrap();
    let log_b = operations::game_log(&fixture.0, &b.0, "Same name").unwrap();
    assert_ne!(log_a, log_b);
    assert_eq!(operation_log(&fixture.0, &a.0), Some(log_a));
    assert_eq!(operation_log(&fixture.0, &b.0), Some(log_b));
}

#[test]
fn reopening_recovers_the_installed_executable_instead_of_the_game_launcher() {
    let fixture = Fixture::new();
    let launcher = fixture.exe("Launcher.exe", 0x8664);
    let selected = fixture.exe("Binaries/Win64/ActualGame.exe", 0x8664);
    let receipt = Receipt {
        version: 1,
        executable: string(&selected),
        backend: DANIEL_VERSION.into(),
    };
    fs::write(
        selected.parent().unwrap().join(RECEIPT),
        serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    let game = fixture.game(&launcher);
    assert_eq!(target(&game).unwrap().1, selected);
    let mut explicit = game;
    explicit.selected_executable = true;
    assert_eq!(target(&explicit).unwrap().1, launcher);
}

#[test]
fn missing_settings_do_not_prevent_the_upstream_restore() {
    let fixture = Fixture::new();
    let exe = fixture.exe("Game.exe", 0x8664);
    assert!(operations::preserve_settings(&fixture.0, &fixture.0, &fixture.game(&exe)).is_ok());
}

#[test]
fn startup_switch_changes_only_the_neural_flag_and_rejects_unowned_installs() {
    let fixture = Fixture::new();
    let exe = fixture.exe("Game.exe", 0x8664);
    let game = fixture.game(&exe);
    let ini = fixture.0.join("OptiScaler.ini");
    fs::write(
        &ini,
        "; user comment\n[DlssNr]\nEnabled=true\nNrBackend=daniel\nStyle=custom\n[Other]\nKeep=1\n",
    )
    .unwrap();
    assert!(set_startup(&game, false).is_err());
    fs::write(
        fixture.0.join(RECEIPT),
        serde_json::to_vec(&Receipt {
            version: 1,
            executable: string(&exe),
            backend: DANIEL_VERSION.into(),
        })
        .unwrap(),
    )
    .unwrap();
    set_startup(&game, false).unwrap();
    let changed = fs::read_to_string(&ini).unwrap();
    assert!(!startup_enabled(&changed));
    assert!(changed.contains("NrBackend=daniel"));
    assert!(changed.contains("Style=custom"));
    assert!(changed.contains("; user comment"));
    assert!(changed.contains("Keep=1"));
    set_startup(&game, true).unwrap();
    assert!(startup_enabled(&fs::read_to_string(&ini).unwrap()));
}

#[test]
fn loading_dll_state_blocks_foreign_files_links_directories_and_case_collisions() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    fs::create_dir(fixture.0.join("amdnr-linux")).unwrap();
    fs::write(
        fixture.0.join("amdnr-linux/proxy.json"),
        r#"{"proxy":"winmm.dll"}"#,
    )
    .unwrap();
    fs::write(fixture.0.join("winmm.dll"), "managed host").unwrap();
    fs::write(fixture.0.join("VERSION.DLL"), "other mod").unwrap();
    fs::write(outside.0.join("private.dll"), "external").unwrap();
    std::os::unix::fs::symlink(outside.0.join("private.dll"), fixture.0.join("dbghelp.dll"))
        .unwrap();
    fs::create_dir(fixture.0.join("wininet.dll")).unwrap();
    let status = proxy::state(&fixture.0, true);
    assert_eq!(status.current.as_deref(), Some("winmm.dll"));
    assert!(status.error.is_none());
    let available = |name: &str| {
        status
            .options
            .iter()
            .find(|option| option.name == name)
            .unwrap()
            .available
    };
    assert!(available("winmm.dll"));
    assert!(available("winhttp.dll"));
    assert!(!available("version.dll"));
    assert!(!available("dbghelp.dll"));
    assert!(!available("wininet.dll"));
    assert!(!proxy::state(&fixture.0, false).options[0].available);
    fs::write(fixture.0.join("WINMM.DLL"), "second variant").unwrap();
    assert!(!proxy::state(&fixture.0, true).options[0].available);
    assert_eq!(
        fs::read_to_string(fixture.0.join("VERSION.DLL")).unwrap(),
        "other mod"
    );
    assert_eq!(
        fs::read_to_string(outside.0.join("private.dll")).unwrap(),
        "external"
    );
}

#[test]
fn loading_dll_records_are_bounded_strict_and_legacy_compatible() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("amdnr-linux")).unwrap();
    assert_eq!(
        proxy::state(&fixture.0, true).current.as_deref(),
        Some("winmm.dll")
    );
    let record = fixture.0.join("amdnr-linux/proxy.json");
    for invalid in [
        r#"{"proxy":"../other.dll"}"#,
        r#"{"proxy":"auto"}"#,
        r#"{"proxy":"dxgi.dll"}"#,
        r#"{"proxy":"winmm.dll","extra":true}"#,
        r#"{"proxy":"winmm.dll","proxy":"version.dll"}"#,
        "not JSON",
    ] {
        fs::write(&record, invalid).unwrap();
        let status = proxy::state(&fixture.0, true);
        assert!(status.current.is_none(), "{invalid}");
        assert!(status.error.is_some(), "{invalid}");
    }
    fs::write(&record, vec![b' '; 4097]).unwrap();
    assert!(proxy::state(&fixture.0, true).error.is_some());
    fs::remove_file(&record).unwrap();
    let outside = Fixture::new();
    fs::write(outside.0.join("proxy.json"), r#"{"proxy":"version.dll"}"#).unwrap();
    std::os::unix::fs::symlink(outside.0.join("proxy.json"), &record).unwrap();
    assert!(proxy::state(&fixture.0, true).error.is_some());
}

#[test]
fn loading_dll_switch_rejects_invalid_names_and_unowned_games_before_running_scripts() {
    let fixture = Fixture::new();
    let exe = fixture.exe("ActualGame.exe", 0x8664);
    let game = fixture.game(&exe);
    for invalid in [
        "auto",
        "next",
        "dxgi.dll",
        "../winmm.dll",
        "WINMM.DLL",
        "",
        "winmm.dll;other",
    ] {
        assert!(switch_proxy(&fixture.0, &game, invalid, &|_| {})
            .unwrap_err()
            .contains("inválida"));
    }
    assert!(switch_proxy(&fixture.0, &game, "version.dll", &|_| {})
        .unwrap_err()
        .contains("não pertence"));
    assert!(!fixture.0.join("logs").exists());
    assert!(!fixture.0.join("launcher-state").exists());
}

#[test]
#[ignore = "requires the pinned release in PELIGAMES_DANIEL_ARCHIVE; isolated fixture, no download"]
fn pinned_bridge_switches_aliases_preserves_settings_and_restores_original_files() {
    let fixture = Fixture::new();
    let stage = fixture.0.join("component");
    fs::create_dir(&stage).unwrap();
    // The sibling archive test owns extraction and checksum coverage; unpack
    // this explicitly supplied fixture with the same verified component path.
    let archive =
        PathBuf::from(std::env::var("PELIGAMES_DANIEL_ARCHIVE").expect("fixture archive"));
    let bytes = fs::read(&archive).unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "a0b7c1ed1a055ebdcfbb74d00104d292ea4da3106cf78b6b6fd48865ca536a29"
    );
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(std::io::Cursor::new(bytes)));
    tar.unpack(&stage).unwrap();
    fs::write(
        component(&stage).join("verified.sha256"),
        "a0b7c1ed1a055ebdcfbb74d00104d292ea4da3106cf78b6b6fd48865ca536a29",
    )
    .unwrap();
    let package = fixture.0.join("package");
    let bundle = package.join("amdnr-linux");
    fs::create_dir_all(&bundle).unwrap();
    for name in ["run.sh", "launch.py", "proxies.py"] {
        fs::copy(
            component(&stage).join("linux").join(name),
            bundle.join(name),
        )
        .unwrap();
    }
    fs::write(package.join("winmm.dll"), "test proxy contents").unwrap();
    fs::write(package.join("d3d12.dll"), "test replacement").unwrap();
    fs::write(package.join("OptiScaler.ini"), "[DlssNr]\nEnabled=true\n").unwrap();
    let exe = fixture.exe("game/ActualGame.exe", 0x8664);
    let game = fixture.game(&exe);
    let directory = exe.parent().unwrap();
    fs::write(directory.join("d3d12.dll"), "original game library").unwrap();
    fs::write(directory.join("winhttp.dll"), "foreign mod").unwrap();
    fs::write(directory.join("save.dat"), "game save").unwrap();
    operations::run(
        &stage,
        "manage.py",
        &[
            "install".into(),
            string(&package),
            string(directory),
            "--proxy".into(),
            "version.dll".into(),
        ],
        &stage.join("install.log"),
        &|_| {},
    )
    .unwrap();
    fs::write(
        directory.join(RECEIPT),
        serde_json::to_vec(&Receipt {
            version: 1,
            executable: string(&exe),
            backend: DANIEL_VERSION.into(),
        })
        .unwrap(),
    )
    .unwrap();
    let settings = "[DlssNr]\nEnabled=false\nUserSetting=preserve\n";
    fs::write(directory.join("OptiScaler.ini"), settings).unwrap();
    let receipt = fs::read(directory.join(RECEIPT)).unwrap();
    assert_eq!(
        proxy::state(directory, true).current.as_deref(),
        Some("version.dll")
    );
    switch_proxy(&stage, &game, "dbghelp.dll", &|_| {}).unwrap();
    assert!(!directory.join("version.dll").exists());
    assert_eq!(
        fs::read_to_string(directory.join("dbghelp.dll")).unwrap(),
        "test proxy contents"
    );
    assert_eq!(
        proxy::state(directory, true).current.as_deref(),
        Some("dbghelp.dll")
    );
    assert_eq!(
        fs::read_to_string(directory.join("OptiScaler.ini")).unwrap(),
        settings
    );
    assert_eq!(fs::read(directory.join(RECEIPT)).unwrap(), receipt);
    assert!(stage
        .join("launcher-state/nr-linux-manager/settings.json")
        .is_file());
    assert!(switch_proxy(&stage, &game, "winhttp.dll", &|_| {}).is_err());
    assert_eq!(
        fs::read_to_string(directory.join("winhttp.dll")).unwrap(),
        "foreign mod"
    );
    assert_eq!(
        proxy::state(directory, true).current.as_deref(),
        Some("dbghelp.dll")
    );
    // Repeated switches and switching back update ownership without touching
    // the user's modified settings or the original file backups.
    switch_proxy(&stage, &game, "winmm.dll", &|_| {}).unwrap();
    switch_proxy(&stage, &game, "version.dll", &|_| {}).unwrap();
    remove(&stage, &game, &|_| {}).unwrap();
    for name in ["winmm.dll", "version.dll", "dbghelp.dll", JOURNAL, RECEIPT] {
        assert!(!directory.join(name).exists(), "{name}");
    }
    assert_eq!(
        fs::read_to_string(directory.join("d3d12.dll")).unwrap(),
        "original game library"
    );
    assert_eq!(
        fs::read_to_string(directory.join("winhttp.dll")).unwrap(),
        "foreign mod"
    );
    assert_eq!(
        fs::read_to_string(directory.join("save.dat")).unwrap(),
        "game save"
    );
}

#[test]
fn runtime_versions_reject_unknown_values_and_recognize_legacy_receipts() {
    assert!(runtimes::Runtime::parse("0.8").is_err());
    assert!(runtimes::Runtime::parse("../0.8.0").is_err());
    let f = Fixture::new();
    let exe = f.exe("Game.exe", 0x8664);
    for v in ["0.6.0", "0.8.0"] {
        fs::write(
            f.0.join(RECEIPT),
            serde_json::to_vec(&Receipt {
                version: 1,
                executable: string(&exe),
                backend: v.into(),
            })
            .unwrap(),
        )
        .unwrap();
        assert!(owned(&f.0, &exe));
        assert_eq!(state(&f.0, &f.game(&exe)).unwrap().version, v);
    }
}
#[test]
fn version08_startup_sets_all_required_selectors_and_preserves_other_settings() {
    let f = Fixture::new();
    let exe = f.exe("Game.exe", 0x8664);
    let game = f.game(&exe);
    fs::write(
        f.0.join(RECEIPT),
        serde_json::to_vec(&Receipt {
            version: 1,
            executable: string(&exe),
            backend: "0.8.0".into(),
        })
        .unwrap(),
    )
    .unwrap();
    fs::write(
        f.0.join("OptiScaler.ini"),
        "[DlssNr]\nEnabled=true\n[Other]\nKeep=1\n",
    )
    .unwrap();
    set_startup(&game, false).unwrap();
    let s = peligames_mod_config::ini::sections(&text(&f.0.join("OptiScaler.ini")));
    assert_eq!(
        peligames_mod_config::ini::get(&s, "DLSS", "NeuralHubNrEnabled"),
        Some("false")
    );
    assert_eq!(
        peligames_mod_config::ini::get(&s, "DLSS", "NeuralHubEnabled"),
        Some("true")
    );
    assert_eq!(
        peligames_mod_config::ini::get(&s, "Other", "Keep"),
        Some("1")
    );
    assert!(
        peligames_mod_config::ini::get(&s, "Libraries", "NvngxPath")
            .unwrap()
            .contains("neuralhub_host.dll")
    );
    set_startup(&game, true).unwrap();
    assert!(startup_enabled(&text(&f.0.join("OptiScaler.ini"))));
}
#[test]
fn incomplete_version_change_is_visible_without_a_receipt() {
    let f = Fixture::new();
    let exe = f.exe("Game.exe", 0x8664);
    let base = Fixture::new();
    let key = super::super::paths::names::allocate(
        &base.0.join("version-changes"),
        "Test",
        &string(&f.0),
    )
    .unwrap();
    fs::write(key.with_extension("json"), "{}").unwrap();
    assert!(
        state(&base.0, &f.game(&exe))
            .unwrap()
            .pending_version_change
    );
}
#[test]
#[ignore = "explicit isolated 0.8 fixture required in PELIGAMES_DANIEL08_FIXTURE"]
fn runtime08_import_verifies_hashes_and_refuses_links_and_extra_files() {
    let src = PathBuf::from(std::env::var("PELIGAMES_DANIEL08_FIXTURE").unwrap());
    let base = Fixture::new();
    runtimes::import(&base.0, &src, &|_| {}).unwrap();
    let p = runtimes::Runtime::V08.package(&base.0);
    assert!(runtimes::ready(&base.0, runtimes::Runtime::V08));
    runtimes::verify(&p).unwrap();
    fs::write(p.join("unexpected.dll"), "extra").unwrap();
    assert!(runtimes::verify(&p).is_err());
    fs::remove_file(p.join("unexpected.dll")).unwrap();
    fs::write(p.join("OptiScaler.ini"), "changed").unwrap();
    assert!(runtimes::verify(&p).unwrap_err().contains("Integridade"));
    fs::remove_file(p.join("OptiScaler.ini")).unwrap();
    std::os::unix::fs::symlink(src.join("OptiScaler.ini"), p.join("OptiScaler.ini")).unwrap();
    assert!(runtimes::verify(&p).is_err());
}
#[test]
#[ignore = "pinned bridge archive and synthetic 0.8 package required; never touches real games"]
fn versions_switch_forward_back_preserve_proxy_settings_and_recover_failed_install() {
    let f = Fixture::new();
    let base = f.0.join("component");
    fs::create_dir(&base).unwrap();
    let bytes = fs::read(std::env::var("PELIGAMES_DANIEL_ARCHIVE").unwrap()).unwrap();
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(std::io::Cursor::new(bytes)));
    tar.unpack(&base).unwrap();
    fs::write(
        component(&base).join("verified.sha256"),
        "a0b7c1ed1a055ebdcfbb74d00104d292ea4da3106cf78b6b6fd48865ca536a29",
    )
    .unwrap();
    let old = package(&base);
    fs::create_dir_all(old.join("amdnr-linux/rocm/lib")).unwrap();
    for file in ["run.sh", "launch.py", "proxies.py"] {
        fs::copy(
            component(&base).join("linux").join(file),
            old.join("amdnr-linux").join(file),
        )
        .unwrap();
    }
    fs::write(
        old.join("amdnr-linux/components.json"),
        r#"{"neural_runtime":"daniel","daniel":{"version":"0.6.0"}}"#,
    )
    .unwrap();
    for file in [
        "winmm.dll",
        "dlssnr_amd_pass1.dll",
        "dlssnr_amd_pass2.dll",
        "dlssnr_amd_pass3.dll",
        "dlssnr_on_amd_weights.bin",
        "amdnr-linux/rocm/lib/libamdhip64.so.7",
    ] {
        fs::write(old.join(file), b"fixture").unwrap();
    }
    fs::write(
        old.join("OptiScaler.ini"),
        "[DlssNr]\nEnabled=true\nSetting06=initial\n",
    )
    .unwrap();
    runtimes::import(
        &base,
        &PathBuf::from(std::env::var("PELIGAMES_DANIEL08_FIXTURE").unwrap()),
        &|_| {},
    )
    .unwrap();
    let exe = f.exe("game/ActualGame.exe", 0x8664);
    let game = f.game(&exe);
    let dir = exe.parent().unwrap();
    fs::write(dir.join("save.dat"), b"save").unwrap();
    fs::write(dir.join("foreign.dll"), b"foreign").unwrap();
    operations::install_version(
        &base,
        &game,
        true,
        "version.dll",
        runtimes::Runtime::V06,
        &|_| {},
    )
    .unwrap();
    fs::write(
        dir.join("OptiScaler.ini"),
        "[DlssNr]\nEnabled=false\nSetting06=custom\n",
    )
    .unwrap();
    runtime_switch::switch(&base, &game, runtimes::Runtime::V08, &|_| {}).unwrap();
    assert_eq!(state(&base, &game).unwrap().version, "0.8.0");
    assert_eq!(
        proxy::state(dir, true).current.as_deref(),
        Some("version.dll")
    );
    let settings = dir.join(runtimes::HUB).join("settings.ini");
    fs::write(
        &settings,
        "[Compute]\nBackend=HIP\n[Debug]\nHipPredicatedWait=0\n[Custom]\nKeep=1\n",
    )
    .unwrap();
    runtime_switch::switch(&base, &game, runtimes::Runtime::V06, &|_| {}).unwrap();
    assert_eq!(state(&base, &game).unwrap().version, "0.6.0");
    assert!(text(&dir.join("OptiScaler.ini")).contains("Setting06=custom"));
    assert!(!startup_enabled(&text(&dir.join("OptiScaler.ini"))));
    runtime_switch::switch(&base, &game, runtimes::Runtime::V08, &|_| {}).unwrap();
    assert!(text(&settings).contains("Keep=1"));
    runtime_switch::switch(&base, &game, runtimes::Runtime::V06, &|_| {}).unwrap();
    // Create a collision only after the original version was removed, to exercise rollback.
    let inject = std::sync::atomic::AtomicBool::new(false);
    let result = runtime_switch::switch(&base, &game, runtimes::Runtime::V08, &|message| {
        if message.contains("Instalando o teste")
            && !inject.swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            fs::create_dir(dir.join(runtimes::HUB)).unwrap();
            fs::create_dir(dir.join(runtimes::HUB).join("neuralhub_host.dll")).unwrap();
        }
    });
    assert!(result.unwrap_err().contains("restaurado"));
    assert_eq!(state(&base, &game).unwrap().version, "0.6.0");
    assert!(!runtime_switch::pending(&base, dir));
    fs::remove_dir(dir.join(runtimes::HUB).join("neuralhub_host.dll")).unwrap();
    // Simulate a restart after uninstall completed but before the next version installed.
    let key = super::super::paths::names::allocate(
        &base.join("version-changes"),
        &game.name,
        &string(dir),
    )
    .unwrap();
    fs::write(key.with_extension("json"), serde_json::to_vec(&serde_json::json!({"schema":1,"from":"0.6.0","to":"0.8.0","proxy":"version.dll","executable":string(&exe)})).unwrap()).unwrap();
    operations::remove(&base, &game, &|_| {}).unwrap();
    assert!(state(&base, &game).unwrap().pending_version_change);
    runtime_switch::recover(&base, &game, &|_| {}).unwrap();
    assert_eq!(state(&base, &game).unwrap().version, "0.6.0");
    assert!(!state(&base, &game).unwrap().pending_version_change);
    assert!(text(&dir.join("OptiScaler.ini")).contains("Setting06=custom"));
    operations::remove(&base, &game, &|_| {}).unwrap();
    assert!(!dir.join(RECEIPT).exists());
    assert_eq!(fs::read(dir.join("save.dat")).unwrap(), b"save");
    assert_eq!(fs::read(dir.join("foreign.dll")).unwrap(), b"foreign");
}

#[test]
fn interrupted_version_change_recovers_unreal_executable_without_receipt() {
    let f = Fixture::new();
    let launcher = f.exe("Launcher.exe", 0x8664);
    let selected = f.exe("Game/Binaries/Win64/ActualGame.exe", 0x8664);
    let base = Fixture::new();
    let directory = selected.parent().unwrap();
    let key = super::super::paths::names::allocate(
        &base.0.join("version-changes"),
        "Game",
        &string(directory),
    )
    .unwrap();
    fs::write(key.with_extension("json"),serde_json::to_vec(&serde_json::json!({"schema":1,"from":"0.6.0","to":"0.8.0","proxy":"version.dll","executable":string(&selected)})).unwrap()).unwrap();
    let mut game = f.game(&launcher);
    game.selected_executable = false;
    let status = state(&base.0, &game).unwrap();
    assert_eq!(status.executable, string(&selected));
    assert!(status.pending_version_change);
    game.selected_executable = true;
    assert_eq!(state(&base.0, &game).unwrap().executable, string(&launcher));
}

#[test]
fn shared_status_does_not_need_a_game_or_create_a_package() {
    let root = std::env::temp_dir().join(format!(
        "peligames-daniel-shared-status-{}",
        uuid::Uuid::new_v4()
    ));
    let state = super::component_state(&root);
    assert_eq!(state.runtimes.len(), 2);
    assert!(state.runtimes.iter().all(|r| !r.ready));
    assert!(
        !root.exists(),
        "a status query must not prepare or download components"
    );
}
