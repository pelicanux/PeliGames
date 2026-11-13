use std::{fs, io::Read, path::Path};
pub(crate) fn optiscaler_fg_enabled(text: &str) -> bool {
    let mut frame_gen = false;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.starts_with('[') {
            frame_gen = line.eq_ignore_ascii_case("[FrameGen]");
            continue;
        }
        if frame_gen {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("Enabled")
                    && ["true", "1", "yes", "on"]
                        .contains(&value.trim().to_ascii_lowercase().as_str())
                {
                    return true;
                }
            }
        }
    }
    false
}
pub fn check_local_fg(root: &Path) -> Result<(), String> {
    let folders = vec![root.to_path_buf()];
    for folder in folders {
        let path = folder.join("OptiScaler.ini");
        if let Ok(file) = fs::File::open(path) {
            let mut text = String::new();
            file.take(256 * 1024)
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            if optiscaler_fg_enabled(&text) {
                return Err("A configuração do OptiScaler indica FrameGen ativado. Desative-o no OptiScaler antes de ativar MAKO.".into());
            }
        }
    }
    Ok(())
}

pub fn constraints(options: &[String]) -> Result<(), String> {
    if options.iter().any(|value| {
        [
            "hdr",
            "mlfg",
            "vkd3d_latency",
            "vulkan_latency",
            "obs_capture",
        ]
        .contains(&value.as_str())
    }) {
        return Err(
            "Disable HDR and conflicting generation/capture/latency layers before enabling MAKO"
                .into(),
        );
    }
    Ok(())
}
