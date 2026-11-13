#[cfg(test)]
mod tests {
    use crate::profiles::*;
    use crate::settings::check_settings;
    use crate::support::optiscaler_fg_enabled;
    use crate::test_support::RetryOutput;
    use crate::*;
    use peligames_archive::unpack_tar_xz as unpack;
    use std::io::Write;
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    #[test]
    fn live_edits_preserve_pause_dll_runtime_and_unknown_options() {
        let before = Settings {
            dll: "/games/Lossless.dll".into(),
            ..Default::default()
        };
        let text = config(&before).replace(
            "frame_generation_enabled = true",
            "frame_generation_enabled = false",
        ) + "custom_option = 123\n";
        let next = Settings {
            multiplier: 4,
            adaptive: true,
            target_fps: 144,
            flow_scale: 0.5,
            performance: true,
            ..before
        };
        let updated = live_config(&text, &next).unwrap();
        for line in [
            "multiplier = 4",
            "adaptive_max_multiplier = 4",
            "target_fps = 144",
            "flow_scale = 0.50",
            "performance_mode = true",
            "adaptive = true",
            "frame_generation_enabled = false",
            "custom_option = 123",
            "dll = \"/games/Lossless.dll\"",
        ] {
            assert!(updated.lines().any(|v| v == line), "{line}");
        }
        assert!(live_config(&(text.clone() + "multiplier = 2\n"), &next).is_err());
        assert!(live_config(&text.replace("target_fps = 60\n", ""), &next).is_err());
    }

    #[test]
    fn additional_controls_validate_generate_and_preserve_launch_identity() {
        let base = Settings::default();
        let mut next = Settings {
            gpu: "0000:01:00.0".into(),
            base_fps_cap: 60,
            refresh_threshold: 90,
            allow_fp16: false,
            ultra_performance: true,
            live_enabled: false,
            scaling_enabled: true,
            scaling_method: "mako".into(),
            scaling_factor: 1.5,
            scaling_sharpness: 0.6,
            ..base.clone()
        };
        assert!(check_settings(&next).is_ok());
        let text = config(&next);
        for expected in [
            "allow_fp16 = false",
            "base_fps_cap = 60",
            "frame_generation_refresh_threshold = 90",
            "ultra_performance = true",
            "frame_generation_enabled = false",
            "scaling_method = \"mako\"",
            "scaling_factor = 1.50",
            "scaling_sharpness = 0.60",
            "gpu = \"0000:01:00.0\"",
        ] {
            assert!(text.contains(expected), "{expected}");
        }
        assert!(!same_launch_profile(&base, &next));
        next = base.clone();
        next.base_fps_cap = 60;
        next.refresh_threshold = 144;
        next.live_enabled = false;
        assert!(same_launch_profile(&base, &next));
        next.scaling_factor = f32::NAN;
        assert!(check_settings(&next).is_err());
        next = base.clone();
        next.gpu = "GPU 0\ninvalid".into();
        assert!(check_settings(&next).is_err());
        next = base.clone();
        next.scaling_method = "unknown".into();
        assert!(check_settings(&next).is_err());
        next = base;
        next.base_fps_cap = 1001;
        assert!(check_settings(&next).is_err());
    }
    #[test]
    fn additional_live_controls_upgrade_legacy_profiles_without_unpausing() {
        let settings = Settings {
            base_fps_cap: 72,
            refresh_threshold: 120,
            ..Default::default()
        };
        let old = config(&Settings::default())
            .lines()
            .filter(|line| {
                !line.starts_with("base_fps_cap")
                    && !line.starts_with("frame_generation_refresh_threshold")
            })
            .collect::<Vec<_>>()
            .join("\n")
            .replace(
                "frame_generation_enabled = true",
                "frame_generation_enabled = false",
            );
        let text = live_config(&old, &settings).unwrap();
        assert!(text.contains("base_fps_cap = 72"));
        assert!(text.contains("frame_generation_refresh_threshold = 120"));
        assert!(text.contains("frame_generation_enabled = false"));
        assert_eq!(live_config(&text, &settings).unwrap(), text);
    }

    #[test]
    fn existing_profiles_do_not_enable_the_internal_layer() {
        let old: Settings = serde_json::from_str(r#"{"backend":"mako","enabled":true}"#).unwrap();
        assert!(!old.overlay_enabled);
        assert!(!Settings::default().overlay_enabled);
    }
    #[test]
    fn live_controls_reuse_launch_identity_but_runtime_changes_do_not() {
        let previous = Settings {
            enabled: true,
            runtime_version: "2.0.0".into(),
            dll: "/games/Lossless.dll".into(),
            ..Default::default()
        };
        let mut next = previous.clone();
        next.multiplier = 4;
        next.flow_scale = 0.5;
        next.performance = true;
        next.overlay_enabled = true;
        assert!(same_launch_profile(&previous, &next));
        next.runtime_version = "2.0.1".into();
        assert!(!same_launch_profile(&previous, &next));
        next = previous.clone();
        next.dll = "/other/Lossless.dll".into();
        assert!(!same_launch_profile(&previous, &next));
        next = previous.clone();
        next.backend = "removed".into();
        assert!(!same_launch_profile(&previous, &next));
        next = previous.clone();
        next.enabled = false;
        assert!(!same_launch_profile(&previous, &next));
    }
    #[test]
    fn wrappers_preserve_arguments_and_quote_paths() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "peligames-framegen-wrapper-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        let config = root.join("a ' quoted profile.toml");
        let runtime = root.join("runtime with spaces");
        let launcher = root.join("launcher.conf");
        let script = root.join("run.sh");
        fs::create_dir_all(runtime.join("bin")).unwrap();
        fs::write(runtime.join("bin/mako-launch"), b"#!/bin/sh\nexec \"$@\"\n").unwrap();
        fs::set_permissions(
            runtime.join("bin/mako-launch"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        fs::write(
            &script,
            wrapper_script(&Settings::default(), &config, &runtime, &launcher),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let result=std::process::Command::new(&script).args(["/usr/bin/python3","-c","import os,sys,json; print(json.dumps([sys.argv[1:],os.environ['MAKO_CONFIG'],os.environ['DISABLE_LSFGVK']]))","arg with spaces","$(do-not-execute)"]).output_retrying();
        assert!(result.status.success());
        let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            data[0],
            serde_json::json!(["arg with spaces", "$(do-not-execute)"])
        );
        assert_eq!(data[1], config.to_string_lossy().as_ref());
        assert_eq!(data[2], "1");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "Requires official pinned runtime archives in an isolated /tmp fixture directory"]
    fn official_archives_and_generated_configs_validate_with_upstream_cli() {
        let fixtures = PathBuf::from(
            std::env::var("PELI_FRAMEGEN_FIXTURES").expect("fixture directory required"),
        );
        assert!(
            fixtures.parent() == Some(Path::new("/tmp"))
                && fixtures
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("peligames-framegen-")
        );
        let root = std::env::temp_dir().join(format!(
            "peligames-framegen-validation-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        for backend in ["mako"] {
            let archive = fs::read(fixtures.join(format!("{backend}.tar.xz"))).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&archive)),
                release(backend).unwrap().2
            );
            let target = root.join(backend);
            fs::create_dir(&target).unwrap();
            unpack(&archive, &target).unwrap();
            validate_runtime_at(&target, backend).unwrap();
            let mut settings = Settings::default();
            settings.backend = backend.into();
            settings.adaptive = backend == "mako";
            let path = root.join(format!("{backend}.toml"));
            fs::write(&path, config(&settings)).unwrap();
            let binary = target.join("bin/mako-cli");
            let result = std::process::Command::new(binary)
                .args(["validate", "--config"])
                .arg(path)
                .output_retrying();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn optiscaler_checks_explicit_activation_without_mislabeling_upscaling_or_auto() {
        assert!(optiscaler_fg_enabled("[FrameGen]\nEnabled = true\n"));
        assert!(optiscaler_fg_enabled("[framegen]\nenabled=1 ; user choice"));
        assert!(!optiscaler_fg_enabled("[FrameGen]\nEnabled=auto"));
        assert!(!optiscaler_fg_enabled(
            "[Upscaling]\nEnabled=true\n[FrameGen]\nEnabled=false"
        ));
    }
    #[test]
    fn pins_only_stable_versions() {
        assert!(release("lsfg").is_err());
        assert_eq!(release("mako").unwrap().0, "4.5.0");
        assert!(release("nightly").is_err())
    }
    #[test]
    fn documented_mako_limits_and_sandbox_identity() {
        let mut s = Settings::default();
        for fps in [10, 30, 240, 1000] {
            s.target_fps = fps;
            assert!(check_settings(&s).is_ok());
        }
        for fps in [0, 9, 1001] {
            s.target_fps = fps;
            assert!(check_settings(&s).is_err());
        }
        s.target_fps = 60;
        s.multiplier = 6;
        assert!(check_settings(&s).is_err());
        let original = Settings::default();
        let mut sandbox = original.clone();
        sandbox.flatpak = true;
        assert!(!same_launch_profile(&original, &sandbox));
    }
    #[test]
    #[ignore = "Requires the official MAKO runtime installed locally; validates profiles without launching a game"]
    fn installed_mako_cli_accepts_profiles_and_launcher_environment() {
        let runtime =
            PathBuf::from(std::env::var("PELI_MAKO_RUNTIME").expect("MAKO runtime fixture"));
        validate_runtime_at(&runtime, "mako").unwrap();
        let dir =
            std::env::temp_dir().join(format!("peligames-mako-contract-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let cfg = dir.join("profile.toml");
        for adaptive in [false, true] {
            for target in [10, 60, 1000] {
                let settings = Settings {
                    enabled: true,
                    adaptive,
                    target_fps: target,
                    ..Default::default()
                };
                fs::write(&cfg, config(&settings)).unwrap();
                let result = std::process::Command::new(runtime.join("bin/mako-cli"))
                    .args(["validate", "--config"])
                    .arg(&cfg)
                    .output_retrying();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
            }
        }
        use std::os::unix::fs::PermissionsExt;
        let launch = dir.join("profile.sh");
        let launch_cfg = dir.join("launch.conf");
        fs::write(&launch_cfg, b"version=1\ndisable_steam_overlay=0\n").unwrap();
        fs::write(
            &launch,
            wrapper_script(&Settings::default(), &cfg, &runtime, &launch_cfg),
        )
        .unwrap();
        fs::set_permissions(&launch, fs::Permissions::from_mode(0o700)).unwrap();
        let result=std::process::Command::new(launch).args(["/usr/bin/python3","-c","import os,sys,json; print(json.dumps([os.environ['MAKO_CONFIG'],os.environ['MAKO_PROFILE'],os.environ['VK_IMPLICIT_LAYER_PATH'],sys.argv[1:]]))","arg with spaces"]).output_retrying();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(data[0], cfg.to_string_lossy().as_ref());
        assert_eq!(data[1], "PeliGames");
        assert_eq!(
            data[2],
            runtime
                .join("share/mako-render/vulkan/implicit_layer.d")
                .to_string_lossy()
                .as_ref()
        );
        assert_eq!(data[3], serde_json::json!(["arg with spaces"]));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn archives_reject_traversal_and_links() {
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_size(0);
        h.set_mode(0o777);
        h.set_cksum();
        b.append_link(&mut h, "bad", "/tmp").unwrap();
        let tar = b.into_inner().unwrap();
        let mut z = xz2::write::XzEncoder::new(Vec::new(), 1);
        z.write_all(&tar).unwrap();
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        assert!(unpack(&z.finish().unwrap(), &root).is_err());
        fs::remove_dir(root).unwrap();
    }
}

#[cfg(test)]
mod profile_lifecycle {
    use crate::{game, profiles, Settings};
    use std::fs;
    #[test]
    fn inspecting_missing_or_legacy_selections_never_allocates_or_renames_files() {
        let root =
            std::env::temp_dir().join(format!("peligames-mako-read-only-{}", uuid::Uuid::new_v4()));
        assert!(
            !game::inspect(&root, "fixture", "Fixture", None)
                .unwrap()
                .installed
        );
        assert!(!root.exists());
        fs::create_dir_all(root.join("profiles")).unwrap();
        let old = root
            .join("profiles")
            .join(format!("{}.selection.json", crate::profile_key("fixture")));
        fs::write(
            &old,
            serde_json::to_vec(&Settings {
                multiplier: 4,
                ..Default::default()
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            game::inspect(&root, "fixture", "Fixture", None)
                .unwrap()
                .settings
                .multiplier,
            4
        );
        assert!(old.is_file());
        assert!(!root.join("profiles/selections").exists());
        let selection =
            profiles::selection_file(&root.join("profiles"), "fixture", "Fixture").unwrap();
        assert!(selection.is_file());
        assert!(!old.exists());
        assert_eq!(
            profiles::selected(&root, "fixture")
                .unwrap()
                .unwrap()
                .multiplier,
            4
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn redirected_profile_directories_are_refused_before_writing() {
        let root =
            std::env::temp_dir().join(format!("peligames-mako-boundary-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("outside")).unwrap();
        std::os::unix::fs::symlink(root.join("outside"), root.join("profiles")).unwrap();
        assert!(profiles::prepare(
            &root,
            "fixture",
            "Fixture",
            &root,
            Settings::default(),
            None
        )
        .is_err());
        assert_eq!(fs::read_dir(root.join("outside")).unwrap().count(), 0);
        assert!(game::inspect(&root, "fixture", "Fixture", None).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
