//! Host Vulkan manifest discovery and bounded private MangoHUD copies.
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
pub const LAYERS: [(&str, &str, &str); 2] = [
    (
        "MangoHud.x86_64.json",
        "VK_LAYER_MANGOHUD_overlay_x86_64",
        "64",
    ),
    ("MangoHud.x86.json", "VK_LAYER_MANGOHUD_overlay_x86", "32"),
];

pub fn host_directories() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for base in [dirs::config_dir(), dirs::data_local_dir()]
        .into_iter()
        .flatten()
    {
        paths.push(base.join("vulkan/implicit_layer.d"));
    }
    if let Some(bases) = std::env::var_os("XDG_DATA_DIRS") {
        paths.extend(
            std::env::split_paths(&bases)
                .filter(|p| p.is_absolute())
                .take(32)
                .map(|p| p.join("vulkan/implicit_layer.d")),
        );
    }
    paths.extend(
        [
            "/etc/vulkan/implicit_layer.d",
            "/usr/local/share/vulkan/implicit_layer.d",
            "/usr/share/vulkan/implicit_layer.d",
        ]
        .map(PathBuf::from),
    );
    paths
}

pub fn manifest(source: &Path, name: &str, arch: &str) -> Option<Value> {
    let mut bytes = Vec::new();
    fs::File::open(source)
        .ok()?
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 128 * 1024 {
        return None;
    }
    let mut value: Value = serde_json::from_slice(&bytes).ok()?;
    let layer = value.get_mut("layer")?.as_object_mut()?;
    if layer.get("name")?.as_str()? != name
        || layer.get("type")?.as_str()? != "GLOBAL"
        || layer.get("enable_environment")? != &json!({"MANGOHUD":"1"})
        || layer.get("disable_environment")? != &json!({"DISABLE_MANGOHUD":"1"})
        || layer
            .get("library_arch")
            .is_some_and(|v| v.as_str() != Some(arch))
    {
        return None;
    }
    let library = layer.get("library_path")?.as_str()?;
    if library.is_empty() || library.chars().any(char::is_control) {
        return None;
    }
    // Relative paths with directories are relative to the original manifest,
    // not to our managed copy. Bare library names retain loader resolution.
    if library.contains('/') {
        let path = source.parent()?.join(library).canonicalize().ok()?;
        let mut elf = [0; 5];
        fs::File::open(&path).ok()?.read_exact(&mut elf).ok()?;
        if &elf[..4] != b"\x7fELF" || elf[4] != if arch == "64" { 2 } else { 1 } {
            return None;
        }
        layer.insert("library_path".into(), json!(path.to_str()?));
    }
    layer.insert("library_arch".into(), json!(arch));
    Some(value)
}

pub fn stage(sources: &[PathBuf], destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    let metadata = fs::symlink_metadata(destination).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Diretório de integração do MangoHUD inválido.".into());
    }
    let mut found = false;
    for (file, name, arch) in LAYERS {
        let path = destination.join(file);
        if let Some(value) = sources
            .iter()
            .find_map(|dir| manifest(&dir.join(file), name, arch))
        {
            let bytes = serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?;
            if fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
                peligames_mod_config::write_atomic_bytes(&path, &bytes, 0o600)?;
            }
            found = true;
        } else {
            match fs::remove_file(&path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    if !found {
        return Err("A camada Vulkan do MangoHUD não foi encontrada. Instale ou repare o MangoHUD em Configurações → Monitoramento.".into());
    }
    Ok(())
}
