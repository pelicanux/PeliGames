//! Host checks are advisory in the editor and repeated before launching a game.
use super::advanced_settings::AdvancedSettings;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

fn vendors(root: &Path) -> Vec<u32> {
    fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            if !name.starts_with("card") || !name[4..].bytes().all(|n| n.is_ascii_digit()) {
                return None;
            }
            let text = fs::read_to_string(entry.path().join("device/vendor")).ok()?;
            u32::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok()
        })
        .collect()
}
fn layer_present(roots: &[PathBuf], needle: &str) -> bool {
    use std::io::Read;
    roots.iter().any(|root| {
        fs::read_dir(root)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|entry| {
                if entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "json")
                {
                    return false;
                }
                let mut text = String::new();
                fs::File::open(entry.path()).ok().is_some_and(|file| {
                    file.take(128 * 1024).read_to_string(&mut text).is_ok()
                        && serde_json::from_str::<serde_json::Value>(&text)
                            .ok()
                            .is_some_and(|json| {
                                json.get("layer")
                                    .and_then(|layer| layer.get("name"))
                                    .and_then(|name| name.as_str())
                                    .is_some_and(|name| name.to_ascii_lowercase().contains(needle))
                                    || json
                                        .get("layers")
                                        .and_then(|layers| layers.as_array())
                                        .is_some_and(|layers| {
                                            layers.iter().any(|layer| {
                                                layer
                                                    .get("name")
                                                    .and_then(|name| name.as_str())
                                                    .is_some_and(|name| {
                                                        name.to_ascii_lowercase().contains(needle)
                                                    })
                                            })
                                        })
                            })
                })
            })
    })
}
pub fn host() -> BTreeMap<String, String> {
    let mut issues = BTreeMap::new();
    let gpus = vendors(Path::new("/sys/class/drm"));
    if gpus.len() < 2 {
        for id in ["gpu_prime", "nvidia_offload"] {
            issues.insert(
                id.into(),
                "A seleção de GPU alternativa requer pelo menos duas GPUs detectadas.".into(),
            );
        }
    }
    if !gpus.contains(&0x10de) {
        for id in ["nvidia_offload", "nvapi", "dlss_updater", "dlss_indicator"] {
            issues.insert(
                id.into(),
                "Requer uma GPU NVIDIA detectada e driver compatível.".into(),
            );
        }
    }
    if !gpus.iter().any(|vendor| [0x1002, 0x8086].contains(vendor)) {
        for id in ["gpu_prime", "glthread"] {
            issues.insert(
                id.into(),
                "Requer uma GPU AMD/Intel com Mesa compatível.".into(),
            );
        }
    }
    let mut data = vec![
        PathBuf::from("/usr/share"),
        PathBuf::from("/usr/local/share"),
    ];
    if let Some(home) = dirs::data_local_dir() {
        data.push(home);
    }
    if let Some(paths) = std::env::var_os("XDG_DATA_DIRS") {
        data.extend(std::env::split_paths(&paths));
    }
    let mut layers = Vec::new();
    for dir in data {
        for name in ["implicit_layer.d", "explicit_layer.d"] {
            layers.push(dir.join("vulkan").join(name));
        }
    }
    for variable in [
        "VK_LAYER_PATH",
        "VK_ADD_LAYER_PATH",
        "VK_IMPLICIT_LAYER_PATH",
        "VK_ADD_IMPLICIT_LAYER_PATH",
    ] {
        if let Some(paths) = std::env::var_os(variable) {
            layers.extend(std::env::split_paths(&paths));
        }
    }
    if !layer_present(&layers, "vkbasalt") {
        issues.insert("vkbasalt".into(),"A camada vkBasalt não foi encontrada no ambiente do launcher. Instale-a no ambiente que executa o jogo e confira suas bibliotecas de 32/64 bits.".into());
    }
    issues
}
pub fn validate(settings: &AdvancedSettings) -> Result<(), String> {
    for (id, message) in host() {
        if settings.options.get(&id) == Some(&true) {
            return Err(message);
        }
    }
    if settings.gamescope.hdr || settings.gamescope.adaptive_sync {
        let help = gamescope_help()?;
        for (enabled, flag) in [
            (settings.gamescope.hdr, "--hdr-enabled"),
            (settings.gamescope.adaptive_sync, "--adaptive-sync"),
        ] {
            if enabled && !help.contains(flag) {
                return Err(format!("O Gamescope instalado não anuncia suporte a {flag}. Atualize-o ou desative a opção."));
            }
        }
    }
    Ok(())
}
fn gamescope_help() -> Result<String, String> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let executable = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join("gamescope"))
        .find(|path| path.is_file())
        .ok_or("Gamescope não encontrado no PATH")?;
    let mut command = Command::new(executable);
    command
        .arg("--help")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    super::env_utils::host_environment(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Não foi possível verificar Gamescope: {error}"))?;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let read = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stream.take(128 * 1024).read_to_string(&mut text);
            text
        })
    };
    let stdout = read(Box::new(out));
    let stderr = read(Box::new(err));
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.to_string());
            }
        }
        if start.elapsed() > Duration::from_secs(2) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Gamescope demorou para informar os recursos disponíveis".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(format!(
        "{} {}",
        stdout.join().unwrap_or_default(),
        stderr.join().unwrap_or_default()
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_gpu_cards_and_valid_layer_manifests() {
        let root = std::env::temp_dir().join(format!("advanced-hardware-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("card0/device")).unwrap();
        fs::write(root.join("card0/device/vendor"), "0x1002\n").unwrap();
        fs::create_dir_all(root.join("card0-HDMI-A-1/device")).unwrap();
        fs::write(root.join("card0-HDMI-A-1/device/vendor"), "0x10de").unwrap();
        assert_eq!(vendors(&root), vec![0x1002]);
        fs::write(root.join("bad.json"), "vkbasalt").unwrap();
        assert!(!layer_present(&[root.clone()], "vkbasalt"));
        fs::write(
            root.join("layer.json"),
            r#"{"layer":{"name":"VK_LAYER_VKBASALT_post_processing"}}"#,
        )
        .unwrap();
        assert!(layer_present(&[root.clone()], "vkbasalt"));
        fs::remove_dir_all(root).unwrap();
    }
}
