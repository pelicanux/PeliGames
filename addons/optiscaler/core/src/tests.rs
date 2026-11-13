use super::*;
use std::collections::BTreeMap;
struct Fixture {
    root: PathBuf,
    game: PathBuf,
    package: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "peligames-optiscaler-test-{}",
            uuid::Uuid::new_v4()
        ));
        let game = root.join("game with spaces");
        let package = root.join("package/release");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(package.join("OptiScaler")).unwrap();
        let mut pe = vec![0; 256];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c..0x40].copy_from_slice(&128u32.to_le_bytes());
        pe[128..132].copy_from_slice(b"PE\0\0");
        pe[132..134].copy_from_slice(&[0x64, 0x86]);
        fs::write(package.join("OptiScaler.dll"), &pe).unwrap();
        fs::write(game.join("Fixture.exe"), &pe).unwrap();
        fs::write(game.join("save.dat"), "keep original save").unwrap();
        fs::write(package.join("OptiScaler.ini"),"; keep\r\n[Upscalers]\r\nDx12Upscaler=auto\r\n[Menu]\r\nScale=auto\r\nShortcutKey=auto\r\n[FrameGen]\r\nEnabled=false\r\nFGInput=auto\r\nFGOutput=auto\r\nFGNvngxReplacement=auto\r\n[Plugins]\r\nLoadAsiPlugins=true\r\n").unwrap();
        fs::write(package.join("OptiScaler/libxess.dll"), "fake dependency").unwrap();
        fs::write(package.join("OptiScaler_LICENSE.txt"), "required license").unwrap();
        Self {
            root,
            game,
            package,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn upgrade_preserves_ini_choices_plugins_and_files_outside_the_journal() {
    let f = Fixture::new();
    install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).unwrap();
    update(&f.game, "Menu.Scale", "1.5").unwrap();
    patcher_package(&f);
    patcher::enable(&f.game, &f.package).unwrap();
    let ini = fs::read_to_string(f.game.join("OptiScaler.ini")).unwrap();
    fs::write(
        f.game.join("OptiScaler.ini"),
        format!("{ini}\n[Custom]\nKeep=hello\n"),
    )
    .unwrap();
    fs::write(
        f.game.join("OptiScaler/user-plugin.asi"),
        "keep user plugin",
    )
    .unwrap();
    let mut dll = fs::read(f.package.join("OptiScaler.dll")).unwrap();
    dll.extend_from_slice(b"new release");
    fs::write(f.package.join("OptiScaler.dll"), &dll).unwrap();
    fs::write(
        f.package.join("OptiScaler/new-library.dll"),
        "new dependency",
    )
    .unwrap();
    let defaults = fs::read_to_string(f.package.join("OptiScaler.ini")).unwrap();
    fs::write(
        f.package.join("OptiScaler.ini"),
        format!("{defaults}\n[NewSection]\nNewKey=true\n"),
    )
    .unwrap();
    assert_eq!(
        upgrade(&f.package, &f.game).unwrap(),
        "WINEDLLOVERRIDES=\"winmm=n,b\" %command%"
    );
    assert_eq!(fs::read(f.game.join("winmm.dll")).unwrap(), dll);
    let current = fs::read_to_string(f.game.join("OptiScaler.ini")).unwrap();
    assert!(current.contains("Keep=hello") && current.contains("; keep"));
    assert_eq!(
        state(&f.game, &f.package).unwrap().settings["Menu.Scale"],
        "1.5"
    );
    assert_eq!(
        peligames_mod_config::ini::get(
            &peligames_mod_config::ini::sections(&current),
            "NewSection",
            "NewKey"
        ),
        Some("true")
    );
    assert!(patcher::state(&f.game).unwrap().enabled);
    assert!(f.game.join("OptiScaler/user-plugin.asi").is_file());
    remove(&f.game).unwrap();
    assert!(
        f.game.join("save.dat").is_file() && f.game.join("OptiScaler/user-plugin.asi").is_file()
    );
    assert!(!f.game.join("OptiScaler/new-library.dll").exists());
}

#[test]
fn upgrade_rejects_conflicts_and_changed_dlls_before_modifying_the_installation() {
    let f = Fixture::new();
    install(&f.package, &f.game, "dxgi.dll", &BTreeMap::new()).unwrap();
    let ini = fs::read(f.game.join("OptiScaler.ini")).unwrap();
    fs::write(f.package.join("libxess.dll"), "new library").unwrap();
    fs::write(f.game.join("libxess.dll"), "existing other mod").unwrap();
    assert!(upgrade(&f.package, &f.game)
        .unwrap_err()
        .contains("already exists"));
    assert_eq!(fs::read(f.game.join("OptiScaler.ini")).unwrap(), ini);
    assert_eq!(
        fs::read_to_string(f.game.join("libxess.dll")).unwrap(),
        "existing other mod"
    );
    fs::remove_file(f.game.join("libxess.dll")).unwrap();
    fs::write(f.game.join("dxgi.dll"), "changed outside launcher").unwrap();
    assert!(upgrade(&f.package, &f.game).is_err());
    assert_eq!(fs::read(f.game.join("OptiScaler.ini")).unwrap(), ini);
}

#[test]
fn failed_upgrade_restores_files_already_replaced_and_keeps_the_original_journal() {
    let f = Fixture::new();
    install(&f.package, &f.game, "dxgi.dll", &BTreeMap::new()).unwrap();
    let ini = fs::read(f.game.join("OptiScaler.ini")).unwrap();
    let journal = fs::read(f.game.join(installation::JOURNAL)).unwrap();
    fs::create_dir_all(f.package.join("OptiScaler/blocked")).unwrap();
    fs::write(
        f.package.join("OptiScaler/blocked/new.dll"),
        "new dependency",
    )
    .unwrap();
    fs::write(
        f.game.join("OptiScaler/blocked"),
        "a user file occupies the directory",
    )
    .unwrap();
    assert!(upgrade(&f.package, &f.game).is_err());
    assert_eq!(fs::read(f.game.join("OptiScaler.ini")).unwrap(), ini);
    assert_eq!(
        fs::read(f.game.join(installation::JOURNAL)).unwrap(),
        journal
    );
    assert!(state(&f.game, &f.package).unwrap().installed);
    assert_eq!(
        fs::read_to_string(f.game.join("OptiScaler/blocked")).unwrap(),
        "a user file occupies the directory"
    );
    assert!(!fs::read_dir(&f.game).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".peligames-optiscaler-update-")));
}
#[test]
fn lifecycle_installs_without_models_edits_before_launch_and_removes_only_its_files() {
    let f = Fixture::new();
    let mut choices = BTreeMap::new();
    choices.insert("Menu.ShortcutKey".into(), "0x22".into());
    choices.insert("Upscalers.Dx12Upscaler".into(), "xess".into());
    let line = install(&f.package, &f.game, "winmm.dll", &choices).unwrap();
    assert_eq!(line, "WINEDLLOVERRIDES=\"winmm=n,b\" %command%");
    let s = state(&f.game, &f.package).unwrap();
    assert!(s.installed && s.managed && s.nr_route.is_none());
    assert_eq!(s.proxy.as_deref(), Some("winmm.dll"));
    assert_eq!(s.settings["Menu.ShortcutKey"], "0x22");
    assert!(!f.game.join("dlssnr-amd").exists());
    assert!(f
        .game
        .join("OptiScaler/Licenses/OptiScaler_LICENSE.txt")
        .is_file());
    update(&f.game, "Menu.Scale", "1.25").unwrap();
    let text = fs::read_to_string(f.game.join("OptiScaler.ini")).unwrap();
    assert!(text.contains("Enabled=false"));
    assert!(text.contains("LoadAsiPlugins=true"));
    assert_eq!(
        state(&f.game, &f.package).unwrap().settings["Menu.Scale"],
        "1.25"
    );
    remove(&f.game).unwrap();
    assert_eq!(
        fs::read_to_string(f.game.join("save.dat")).unwrap(),
        "keep original save"
    );
    assert!(f.game.join("Fixture.exe").is_file());
    assert!(!f.game.join("winmm.dll").exists());
    assert!(!f.game.join("OptiScaler.ini").exists());
}
#[test]
fn collisions_and_incompatible_input_fail_before_writing_any_game_file() {
    let f = Fixture::new();
    fs::write(f.game.join("dxgi.dll"), "original loader").unwrap();
    assert!(install(&f.package, &f.game, "dxgi.dll", &BTreeMap::new()).is_err());
    assert_eq!(
        fs::read_to_string(f.game.join("dxgi.dll")).unwrap(),
        "original loader"
    );
    assert!(!f.game.join("OptiScaler.ini").exists());
    install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).unwrap();
    assert_eq!(
        state(&f.game, &f.package).unwrap().proxy.as_deref(),
        Some("winmm.dll")
    );
    remove(&f.game).unwrap();
    assert!(f.game.join("dxgi.dll").exists());
    let mut settings = BTreeMap::new();
    settings.insert("FrameGen.Unsupported".into(), "true".into());
    assert!(install(&f.package, &f.game, "winmm.dll", &settings).is_err());
    assert!(!managed(&f.game));
    let mut bytes = fs::read(f.game.join("Fixture.exe")).unwrap();
    bytes[132..134].copy_from_slice(&[0x4c, 0x01]);
    fs::write(f.game.join("Fixture.exe"), bytes).unwrap();
    assert!(install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).is_err());
    assert!(!f.game.join("OptiScaler.ini").exists());
}
#[test]
fn shared_nr_keeps_its_loader_plugins_and_neural_settings() {
    let f = Fixture::new();
    fs::write(f.game.join("dxgi.dll"), "NR loader").unwrap();
    fs::write(
        f.game.join("OptiScaler.ini"),
        "[Menu]\nScale=auto\n[Plugins]\nLoadAsiPlugins=true\n[FrameGen]\nEnabled=false\n",
    )
    .unwrap();
    fs::write(f.game.join("dlssnr-amd-install.txt"), "dxgi.dll").unwrap();
    fs::create_dir_all(f.game.join("OptiScaler/plugins")).unwrap();
    fs::write(
        f.game.join("OptiScaler/plugins/dlssnr-amd.asi"),
        "NR plugin",
    )
    .unwrap();
    fs::write(
        f.game.join("dlssnr-amd.ini"),
        "[Network]\nModel=CC_SMALL_FP8",
    )
    .unwrap();
    update(&f.game, "Menu.Scale", "1.5").unwrap();
    assert_eq!(
        state(&f.game, &f.package).unwrap().nr_route,
        Some(NrRoute::Optiscaler)
    );
    assert!(install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).is_err());
    assert!(remove(&f.game).is_err());
    assert_eq!(
        fs::read_to_string(f.game.join("dlssnr-amd.ini")).unwrap(),
        "[Network]\nModel=CC_SMALL_FP8"
    );
}
#[cfg(unix)]
#[test]
fn redirected_paths_and_changed_binaries_are_rejected() {
    let f = Fixture::new();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("settings.ini"), "[Menu]\nScale=auto").unwrap();
    std::os::unix::fs::symlink(outside.join("settings.ini"), f.game.join("OptiScaler.ini"))
        .unwrap();
    assert!(update(&f.game, "Menu.Scale", "2").is_err());
    assert_eq!(
        fs::read_to_string(outside.join("settings.ini")).unwrap(),
        "[Menu]\nScale=auto"
    );
    fs::remove_file(f.game.join("OptiScaler.ini")).unwrap();
    std::os::unix::fs::symlink(&outside, f.game.join("OptiScaler")).unwrap();
    assert!(install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).is_err());
    assert!(!f.game.join("winmm.dll").exists());
    fs::remove_file(f.game.join("OptiScaler")).unwrap();
    install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).unwrap();
    fs::write(f.game.join("winmm.dll"), "changed by user").unwrap();
    assert!(remove(&f.game).is_err());
    assert!(f.game.join("OptiScaler.ini").exists());
    assert!(managed(&f.game));
}
#[test]
fn invalid_field_and_missing_license_do_not_create_installation() {
    let f = Fixture::new();
    fs::remove_file(f.package.join("OptiScaler_LICENSE.txt")).unwrap();
    assert!(install(&f.package, &f.game, "dxgi.dll", &BTreeMap::new()).is_err());
    assert!(!f.game.join("dxgi.dll").exists());
    assert!(!managed(&f.game));
}

#[test]
fn nr_detection_distinguishes_plugin_reshade_legacy_and_unconfirmed_records() {
    let f = Fixture::new();
    fs::write(f.game.join("dxgi.dll"), "original game dll").unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), None);
    fs::write(f.game.join("OptiScaler.ini"), "[Menu]\nScale=auto").unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), None);
    fs::write(
        f.game.join("dlssnr-amd-install.txt"),
        "dxgi.dll\nOptiScaler.ini\n",
    )
    .unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), Some(NrRoute::Optiscaler));
    fs::write(
        f.game.join("dlssnr-amd-install.txt"),
        "dxgi.dll\nReShade.ini\ndlssnr_amd.addon64\n",
    )
    .unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), Some(NrRoute::Reshade));
    assert_eq!(
        state(&f.game, &f.package).unwrap().nr_route,
        Some(NrRoute::Reshade)
    );
    assert!(install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).is_err());
    assert!(!f.game.join("winmm.dll").exists());
    fs::remove_file(f.game.join("dlssnr-amd-install.txt")).unwrap();
    fs::write(f.game.join("dlssnr_amd.addon64"), "addon").unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), Some(NrRoute::Reshade));
    fs::remove_file(f.game.join("dlssnr_amd.addon64")).unwrap();
    fs::write(f.game.join("dlssnr_core.dll"), "old NR core").unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), Some(NrRoute::Optiscaler));
    fs::remove_file(f.game.join("OptiScaler.ini")).unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), Some(NrRoute::Unknown));
    fs::remove_file(f.game.join("dlssnr_core.dll")).unwrap();
    fs::write(
        f.game.join("dlssnr-amd-install.txt"),
        "unconfirmed installation\n",
    )
    .unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), Some(NrRoute::Unknown));
    assert!(install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).is_err());
    fs::remove_file(f.game.join("dlssnr-amd-install.txt")).unwrap();
    fs::create_dir_all(f.game.join("OptiScaler/plugins")).unwrap();
    fs::write(
        f.game.join("OptiScaler/plugins/dlssnr-amd.asi"),
        "NR plugin",
    )
    .unwrap();
    assert_eq!(nr_route(&f.game).unwrap(), Some(NrRoute::Optiscaler));
    assert!(install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).is_err());
    assert!(!f.game.join("winmm.dll").exists());
}

#[test]
fn frame_generation_edits_preserve_nr_plugins_and_roundtrip_each_provider() {
    let f = Fixture::new();
    install(&f.package, &f.game, "winmm.dll", &BTreeMap::new()).unwrap();
    fs::write(
        f.game.join("dlssnr-amd.ini"),
        "[Network]\nModel=CC_SMALL_FP8\n",
    )
    .unwrap();
    assert!(state(&f.game, &f.package).unwrap().framegen_available);
    for (key, value) in [
        ("FrameGen.Enabled", "true"),
        ("FrameGen.FGInput", "nvngxfg"),
        ("FrameGen.FGOutput", "fsrfg"),
        ("FrameGen.FGNvngxReplacement", "Nukems"),
    ] {
        update(&f.game, key, value).unwrap();
        assert_eq!(state(&f.game, &f.package).unwrap().settings[key], value);
    }
    let text = fs::read_to_string(f.game.join("OptiScaler.ini")).unwrap();
    assert!(text.contains("LoadAsiPlugins=true"));
    assert_eq!(
        fs::read_to_string(f.game.join("dlssnr-amd.ini")).unwrap(),
        "[Network]\nModel=CC_SMALL_FP8\n"
    );
    assert!(update(&f.game, "FrameGen.FGOutput", "nukems").is_err());
    assert_eq!(
        fs::read_to_string(f.game.join("OptiScaler.ini")).unwrap(),
        text
    );
    for value in ["auto", "None", "Nukems", "Arturs", "FFX", "Combo"] {
        update(&f.game, "FrameGen.FGNvngxReplacement", value).unwrap();
        assert_eq!(
            state(&f.game, &f.package).unwrap().settings["FrameGen.FGNvngxReplacement"],
            value
        );
    }
    update(&f.game, "FrameGen.Enabled", "false").unwrap();
    assert_eq!(
        state(&f.game, &f.package).unwrap().settings["FrameGen.FGInput"],
        "nvngxfg"
    );
}
#[test]
fn old_optiscaler_does_not_accept_modern_frame_generation_keys() {
    let f = Fixture::new();
    fs::write(
        f.game.join("OptiScaler.ini"),
        "[FrameGen]\nEnabled=false\nFGType=fsrfg\n",
    )
    .unwrap();
    fs::write(f.game.join("dxgi.dll"), "old loader").unwrap();
    assert!(!state(&f.game, &f.package).unwrap().framegen_available);
    let before = fs::read(f.game.join("OptiScaler.ini")).unwrap();
    assert!(update(&f.game, "FrameGen.FGInput", "upscaler").is_err());
    assert_eq!(fs::read(f.game.join("OptiScaler.ini")).unwrap(), before);
    update(&f.game, "Menu.Scale", "1.25").unwrap();
    assert!(fs::read_to_string(f.game.join("OptiScaler.ini"))
        .unwrap()
        .contains("FGType=fsrfg"));
}

fn patcher_package(f: &Fixture) {
    fs::copy(
        f.package.join("OptiScaler.dll"),
        f.package.join("OptiPatcher.asi"),
    )
    .unwrap();
    fs::write(f.package.join("OptiPatcher_LICENSE.txt"), "MIT license").unwrap();
}
#[test]
fn patcher_lifecycle_preserves_nr_plugins_and_shared_loader() {
    let f = Fixture::new();
    patcher_package(&f);
    install(&f.package, &f.game, "dxgi.dll", &BTreeMap::new()).unwrap();
    fs::create_dir_all(f.game.join("OptiScaler/plugins")).unwrap();
    let nr = f.game.join("OptiScaler/plugins/dlssnr-amd.asi");
    fs::write(&nr, "preserve NR").unwrap();
    patcher::enable(&f.game, &f.package).unwrap();
    let state = patcher::state(&f.game).unwrap();
    assert!(state.enabled && state.managed);
    patcher::disable(&f.game).unwrap();
    assert!(!patcher::state(&f.game).unwrap().installed);
    assert_eq!(fs::read_to_string(&nr).unwrap(), "preserve NR");
    assert_eq!(
        peligames_mod_config::ini::get(
            &peligames_mod_config::ini::sections(
                &fs::read_to_string(f.game.join("OptiScaler.ini")).unwrap()
            ),
            "Plugins",
            "LoadAsiPlugins"
        ),
        Some("true")
    );
    fs::remove_file(nr).unwrap();
    patcher::enable(&f.game, &f.package).unwrap();
    remove(&f.game).unwrap();
    assert!(!f.game.join("OptiScaler/plugins/OptiPatcher.asi").exists());
    assert!(f.game.join("save.dat").exists());
}
#[test]
fn patcher_rejects_external_changed_redirected_and_custom_paths() {
    let f = Fixture::new();
    patcher_package(&f);
    install(&f.package, &f.game, "dxgi.dll", &BTreeMap::new()).unwrap();
    fs::create_dir_all(f.game.join("OptiScaler/plugins")).unwrap();
    let plugin = f.game.join("OptiScaler/plugins/OptiPatcher.asi");
    fs::write(&plugin, "external").unwrap();
    assert!(patcher::enable(&f.game, &f.package).is_err());
    assert!(patcher::disable(&f.game).is_err());
    fs::remove_file(&plugin).unwrap();
    let ini = f.game.join("OptiScaler.ini");
    let before = fs::read_to_string(&ini).unwrap();
    fs::write(&ini, format!("{before}\nPath=somewhere-else\n")).unwrap();
    assert!(patcher::enable(&f.game, &f.package).is_err());
    assert!(!plugin.exists());
    fs::write(&ini, &before).unwrap();
    patcher::enable(&f.game, &f.package).unwrap();
    fs::write(&plugin, "modified").unwrap();
    assert!(patcher::disable(&f.game).is_err());
    assert!(plugin.exists());
    fs::remove_file(&plugin).unwrap();
    std::os::unix::fs::symlink(f.game.join("save.dat"), &plugin).unwrap();
    assert!(patcher::state(&f.game).is_err());
    assert!(patcher::disable(&f.game).is_err());
}
#[test]
fn stable_official_framegen_uses_nukem_without_modern_provider() {
    let f = Fixture::new();
    fs::write(
        f.package.join("OptiScaler.ini"),
        "[FrameGen]\nEnabled=auto\nFGInput=auto\nFGOutput=auto\n",
    )
    .unwrap();
    let s = state(&f.game, &f.package).unwrap();
    assert!(s.framegen_available && !s.framegen_modern);
    let choices = BTreeMap::from([
        ("FrameGen.FGInput".into(), "nukems".into()),
        ("FrameGen.FGOutput".into(), "nukems".into()),
        ("FrameGen.Enabled".into(), "true".into()),
    ]);
    install(&f.package, &f.game, "dxgi.dll", &choices).unwrap();
    assert!(update(&f.game, "FrameGen.FGNvngxReplacement", "Nukems").is_err());
    assert_eq!(
        state(&f.game, &f.package).unwrap().settings["FrameGen.FGInput"],
        "nukems"
    );
}
#[test]
fn official_fakenvapi_settings_use_separate_native_configuration() {
    let f = Fixture::new();
    fs::write(
        f.package.join("fakenvapi.ini"),
        "; preserve\n[fakenvapi]\nforce_reflex=0\nforce_latencyflex=0\nlatencyflex_mode=0\n",
    )
    .unwrap();
    let choices = BTreeMap::from([
        ("fakenvapi.ForceReflex".into(), "2".into()),
        ("fakenvapi.ForceLatencyFlex".into(), "true".into()),
    ]);
    install(&f.package, &f.game, "dxgi.dll", &choices).unwrap();
    let s = state(&f.game, &f.package).unwrap();
    assert_eq!(s.settings["fakenvapi.ForceReflex"], "2");
    assert_eq!(s.settings["fakenvapi.ForceLatencyFlex"], "true");
    update(&f.game, "fakenvapi.ForceReflex", "1").unwrap();
    let native = fs::read_to_string(f.game.join("fakenvapi.ini")).unwrap();
    assert!(native.contains("; preserve"));
    assert_eq!(
        peligames_mod_config::ini::get(
            &peligames_mod_config::ini::sections(&native),
            "fakenvapi",
            "force_reflex"
        ),
        Some("1")
    );
    assert!(!fs::read_to_string(f.game.join("OptiScaler.ini"))
        .unwrap()
        .contains("ForceReflex"));
    remove(&f.game).unwrap();
    assert!(!f.game.join("fakenvapi.ini").exists());
}

#[test]
fn official_proxy_choices_install_detect_and_remove_without_touching_game_files() {
    for proxy in PROXY_DLLS.iter() {
        let f = Fixture::new();
        let choices = BTreeMap::from([("Menu.ShortcutKey".into(), "0x79".into())]);
        let line = install(&f.package, &f.game, proxy, &choices).unwrap();
        assert_eq!(
            line,
            format!(
                "WINEDLLOVERRIDES=\"{}=n,b\" %command%",
                proxy.trim_end_matches(".dll")
            )
        );
        assert_eq!(
            state(&f.game, &f.package).unwrap().proxy.as_deref(),
            Some(proxy.as_str())
        );
        assert_eq!(
            settings::read(&fs::read_to_string(f.game.join("OptiScaler.ini")).unwrap())
                ["Menu.ShortcutKey"],
            "0x79"
        );
        remove(&f.game).unwrap();
        assert!(!f.game.join(proxy).exists());
        assert_eq!(
            fs::read_to_string(f.game.join("save.dat")).unwrap(),
            "keep original save"
        );
    }
}

#[test]
fn recorded_menu_keys_accept_virtual_keys_but_reject_invalid_ini_input() {
    let ini = "[Menu]\nShortcutKey=auto\n[Other]\nKeep=original\n";
    for key in ["0x79", "0x41", "0x22", "0x08", "0x87", "121", "auto", "-1"] {
        let patched = settings::patch(ini, "Menu.ShortcutKey", key).unwrap();
        assert_eq!(settings::read(&patched)["Menu.ShortcutKey"], key);
        assert!(patched.contains("Keep=original"));
    }
    for key in [
        "Ctrl+F1",
        "0x100",
        "255",
        "0x00",
        "-2",
        "0x",
        "0x79\nOther=bad",
        " F1",
    ] {
        assert!(
            settings::patch(ini, "Menu.ShortcutKey", key).is_err(),
            "{key}"
        );
    }
}

#[test]
fn fsr_controls_follow_the_package_after_upgrading_and_downgrading_channels() {
    let f = Fixture::new();
    let base = fs::read_to_string(f.package.join("OptiScaler.ini")).unwrap();
    let stable = format!("{base}\n[FSR]\nFsr4Update=auto\nFsr4ForceEnableInt8=auto\n");
    let modern = format!("{base}\n[FSR]\nFsr4GpuSupport=auto\nFsr4Model=auto\n");
    fs::write(f.package.join("OptiScaler.ini"), &stable).unwrap();
    install(&f.package, &f.game, "dxgi.dll", &BTreeMap::new()).unwrap();
    update(&f.game, "FSR.Fsr4ForceEnableInt8", "true").unwrap();
    fs::write(f.package.join("OptiScaler.ini"), &modern).unwrap();
    upgrade(&f.package, &f.game).unwrap();
    let s = state(&f.game, &f.package).unwrap();
    assert!(s.settings.contains_key("FSR.Fsr4GpuSupport"));
    assert!(!s.settings.contains_key("FSR.Fsr4ForceEnableInt8"));
    assert!(update(&f.game, "FSR.Fsr4ForceEnableInt8", "true").is_err());
    update(&f.game, "FSR.Fsr4GpuSupport", "2").unwrap();
    fs::write(f.package.join("OptiScaler.ini"), &stable).unwrap();
    upgrade(&f.package, &f.game).unwrap();
    let s = state(&f.game, &f.package).unwrap();
    assert!(!s.settings.contains_key("FSR.Fsr4GpuSupport"));
    assert!(s.settings.contains_key("FSR.Fsr4ForceEnableInt8"));
    assert!(update(&f.game, "FSR.Fsr4GpuSupport", "2").is_err());
    assert!(f.game.join("save.dat").is_file());
}

#[test]
fn inventory_keeps_game_package_version_until_an_explicit_upgrade() {
    let f = Fixture::new();
    fs::write(f.package.join("version.txt"), "v0.9.4\n").unwrap();
    install(&f.package, &f.game, "dxgi.dll", &BTreeMap::new()).unwrap();
    fs::write(f.package.join("version.txt"), "10.0.0-pre1").unwrap();
    let inventory = state(&f.game, &f.package).unwrap().software;
    assert_eq!(inventory[0].version.as_deref(), Some("v0.9.4"));
    upgrade(&f.package, &f.game).unwrap();
    assert_eq!(
        state(&f.game, &f.package).unwrap().software[0]
            .version
            .as_deref(),
        Some("10.0.0-pre1")
    );
    remove(&f.game).unwrap();
    assert!(!state(&f.game, &f.package).unwrap().software[0].present);
    assert!(state(&f.game, &f.package).unwrap().software[0]
        .version
        .is_none());
}

#[test]
fn installation_receipt_follows_the_shared_ownership_contract() {
    assert_eq!(installation::JOURNAL,peligames_mod_config::ownership::receipt_name("optiscaler").unwrap());
    assert!(peligames_mod_config::ownership::receipt_name("../game").is_none());
}
