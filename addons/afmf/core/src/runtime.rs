//! Explicit runtime paths supplied by the caller; no home-directory discovery.
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub const RELEASE: (&str, &str, &str) = ("1.3.1", "https://github.com/serialexperimentslainnnn/afmf-linux/releases/download/v1.3.1/afmf-linux-1.3.1-x86_64.tar.xz", "7fafa09021ee94dc5ec8da694cb02fe298b54c114b486611bb24b1e1500edb18");

/// AFMF's layer: a relative manifest beside a self-contained x86_64 library,
/// dormant until AFMF_ENABLE=1, so the folder can be offered per game.
pub fn validate(path: &Path) -> Result<(), String> {
    let incomplete = || "Runtime do AFMF incompleto. Instale novamente o componente.".to_string();
    if !path.is_absolute() || !path.is_dir() || path.is_symlink() {
        return Err(incomplete());
    }
    for file in ["libafmf-linux.so", "afmf-linux.json"] {
        let p = path.join(file);
        if !p.is_file() || p.is_symlink() {
            return Err(incomplete());
        }
    }
    let mut header = [0u8; 20];
    fs::File::open(path.join("libafmf-linux.so"))
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|_| incomplete())?;
    if header[..5] != [0x7f, b'E', b'L', b'F', 2] || header[18..20] != [62, 0] {
        return Err("A biblioteca do AFMF não é Linux x86_64.".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path.join("afmf-linux.json"))
        .map_err(|_| incomplete())?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| incomplete())?;
    if bytes.len() > 256 * 1024 {
        return Err(incomplete());
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Manifesto do AFMF inválido.".to_string())?;
    let layer = &manifest["layer"];
    if layer["name"] != "VK_LAYER_AFMF"
        || layer["library_path"] != "libafmf-linux.so"
        || layer["enable_environment"]["AFMF_ENABLE"] != "1"
    {
        return Err(
            "Manifesto do AFMF diferente do esperado. Nenhum runtime foi instalado.".into(),
        );
    }
    Ok(())
}

/// A bare filename is searched in ld.so's global paths, not beside the JSON.
/// Keep the signed package unchanged and offer a private, adapted manifest.
pub fn prepare_manifest(path: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    validate(path)?;
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(path.join("afmf-linux.json")).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    manifest["layer"]["library_path"] =
        serde_json::Value::String(path.join("libafmf-linux.so").to_string_lossy().into_owned());
    manifest["layer"]["library_arch"] = serde_json::Value::String("64".into());
    // Explicit activation keeps GE/Steam above AFMF's native
    // presentation threads. The package and older implicit adapter stay intact.
    manifest["file_format_version"] = serde_json::Value::String("1.2.1".into());
    let layer = manifest["layer"]
        .as_object_mut()
        .ok_or("Manifesto AFMF inválido")?;
    layer.remove("enable_environment");
    layer.remove("disable_environment");
    let directory = path.join("peligames-explicit-layer");
    match fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.to_string()),
    }
    let meta = fs::symlink_metadata(&directory).map_err(|e| e.to_string())?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o077 != 0
    {
        return Err("Pasta do manifesto AFMF sem permissões privadas.".into());
    }
    let destination = directory.join("afmf-linux.json");
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    if fs::read(&destination).ok().as_deref() != Some(bytes.as_slice()) {
        peligames_mod_config::write_atomic_bytes(&destination, &bytes, 0o600)?;
    }
    Ok(directory)
}

/// Preserve legacy runtime locations without moving files used by older profiles.
pub fn location(storage: &Path, version: &str) -> Result<PathBuf, String> {
    peligames_addon_kit::files::check_directory(storage)?;
    if !storage.is_absolute() {
        return Err("An absolute component directory is required".into());
    }
    let parsed = semver::Version::parse(version).map_err(|_| "Versão de runtime inválida.")?;
    if !parsed.pre.is_empty() || !parsed.build.is_empty() || parsed.to_string() != version {
        return Err("Versão de runtime inválida.".into());
    }
    let legacy = storage.join(format!("afmf-{version}"));
    if legacy.is_dir() && !legacy.is_symlink() {
        return Ok(legacy);
    }
    let path = storage.join("afmf/versions").join(version);
    peligames_addon_kit::files::check_directory(&path)?;
    Ok(path)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod tests;
