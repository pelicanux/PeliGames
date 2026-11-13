//! Read-only collectors for feature-specific support evidence.
use super::{
    files::{read_at, section, technical},
    projection::neural_settings_snapshot,
};
use crate::{
    commands::capture as commands_capture,
    core::{paths, report_privacy::select},
};
use serde_json::{json, Value};
use std::{fs, path::Path};
pub(super) fn system(out: &mut String) {
    let os = fs::read_to_string("/etc/os-release")
        .unwrap_or_default()
        .lines()
        .filter(|l| l.starts_with("PRETTY_NAME=") || l.starts_with("VERSION_ID="))
        .collect::<Vec<_>>()
        .join("\n");
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    section(
        out,
        "SISTEMA",
        &format!(
            "{os}\nKernel: {}\nArquitetura: {}\nSessão: {}\nDesktop: {}",
            kernel.trim(),
            std::env::consts::ARCH,
            std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
            std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
        ),
    );
    let mut gpus = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten().take(64) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name
                .strip_prefix("card")
                .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            {
                continue;
            }
            let read = |key| {
                fs::read_to_string(entry.path().join(key))
                    .unwrap_or_default()
                    .chars()
                    .take(128)
                    .collect::<String>()
                    .trim()
                    .to_string()
            };
            gpus.push(json!({"vendor":read("device/vendor"),"device":read("device/device"),"driver":fs::read_link(entry.path().join("device/driver")).ok().and_then(|p|p.file_name().map(|n|n.to_string_lossy().into_owned()))}));
        }
    }
    technical(
        out,
        "GPU (identificação PCI, sem número de série)",
        &json!(gpus),
    );
}
pub(super) fn neural(out: &mut String, base: &Path) {
    for arch in ["rdna4"] {
        if let Ok(backend) = paths::app_path(format!("Mod/DLSSNR/backend-files/{arch}")) {
            if let Some(version) = read_at(&backend, "version.txt") {
                section(out, &format!("BACKEND NR {arch}"), &version);
            }
        }
    }
    let root = base.to_path_buf();
    let mut presence = serde_json::Map::new();
    for name in [
        "OptiScaler.dll",
        "OptiScaler/plugins/dlssnr-amd.asi",
        "dlssnr_amd.addon64",
        "dlssnr_amd.addon32",
        "dlssnr-amd/dlssnr.bin",
        "dlssnr-amd/dlssnr-small.bin",
        "dlssnr-amd/CC_SILVER_AARDWOLD.bin",
        "dlssnr-amd/CC_SMALL_FP8.bin",
        "dlssnr-amd/CC_SMALL_FP8_EXPERIMENTAL.bin",
        "dlssnr-amd/CC_SILVER_AARDWOLD_OPTIMIZED.bin",
        "dlssnr-amd/CC_SILVER_AARDWOLD_OPTIMIZED_INT4.bin",
        "dlssnr-amd/dlssnr-small-experimental.bin",
        "dlssnr-amd/dlssnr-int4.bin",
    ] {
        presence.insert(name.into(), root.join(name).is_file().into());
    }
    technical(
        out,
        "ARQUIVOS DO NR (presença não comprova carregamento)",
        &presence.into(),
    );
    for name in ["dlssnr-amd.ini", "OptiScaler.ini"] {
        if let Some(text) = read_at(&root, name) {
            technical(out, name, &neural_settings_snapshot(&text));
        }
    }
    for name in [
        "dlssnr-amd/nr-report.txt",
        "dlssnr-amd.log",
        "OptiScaler.log",
        "ReShade.log",
    ] {
        if let Some(text) = read_at(&root, name) {
            section(out, name, &text);
        }
    }
}
pub(super) fn frame_profiles(out: &mut String, value: &Value) {
    let Ok(root) = paths::app_path("FrameGeneration/profiles")
        .and_then(|p| p.canonicalize().map_err(|e| e.to_string()))
    else {
        return;
    };
    let wrappers=value["wrappers"].as_array().into_iter().flatten().filter_map(|v|v["program"].as_str());
    let integrations=value["addon_launch"].as_object().into_iter().flatten().filter_map(|(_,v)|v["data"]["profile"].as_str().or_else(||v["data"]["legacy_profile"].as_str()));
    for program in wrappers.chain(integrations).take(16) {
        let program = Path::new(program);
        if program
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .as_ref()
            != Some(&root)
            || program.extension().is_none_or(|e| e != "sh")
        {
            continue;
        }
        let Some(file) = program
            .with_extension("json")
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
        else {
            continue;
        };
        if let Some(text) = read_at(&root, &file) {
            if let Ok(value) = serde_json::from_str::<Value>(&text) {
                technical(
                    out,
                    "PERFIL MAKO",
                    &select(
                        &value,
                        &[
                            "backend",
                            "runtime_version",
                            "enabled",
                            "overlay_enabled",
                            "multiplier",
                            "adaptive",
                            "target_fps",
                            "flow_scale",
                            "performance",
                            "base_fps_cap",
                            "refresh_threshold",
                            "ultra_performance",
                            "allow_fp16",
                            "live_enabled",
                            "scaling_enabled",
                            "scaling_method",
                            "scaling_factor",
                            "scaling_sharpness",
                        ],
                    ),
                );
            }
        }
    }
}
pub(super) fn captures(out: &mut String, key: &str) {
    if let Ok(snapshot)=commands_capture::diagnostics(key){technical(out,"CONFIGURAÇÃO DA CAPTURA", &snapshot);}

    let status = commands_capture::capture_status();
    if status.game_key.as_deref() == Some(key) {
        technical(
            out,
            "ESTADO DA CAPTURA",
            &select(
                &serde_json::to_value(status).unwrap_or_default(),
                &["phase", "recording", "replay", "error"],
            ),
        );
    } else {
        section(
            out,
            "ESTADO DA CAPTURA",
            "Nenhuma sessão de captura vinculada a este jogo.",
        );
    }

}
