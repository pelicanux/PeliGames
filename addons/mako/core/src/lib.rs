//! MAKO integration, independent of the launcher and its UI engine.
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};
pub mod settings;
pub use settings::Settings;
pub mod flatpak;
pub mod game;
pub mod management;
pub mod mangohud;
pub mod profiles;
pub mod support;
pub mod versions;
pub use peligames_mod_config::write_atomic_bytes as write;
pub fn profile_key(key: &str) -> String {
    format!("{:x}", Sha256::digest(key.as_bytes()))
}
pub fn release(backend: &str) -> Result<(&'static str, &'static str, &'static str), String> {
    match backend{
 "mako"=>Ok(("4.5.0","https://github.com/eugeniosegala/MAKO/releases/download/render-v4.5.0/MAKO-Renderer-v4.5.0-linux.tar.xz","4dd3e70743a08a0f2f0e6006d35d1e08dfcf751932678a6217cbdcf87c784305")),
 _=>Err("Tecnologia de geração de quadros inválida.".into())}
}
pub fn dll_valid(path: &Path) -> bool {
    let mut b = [0; 2];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok()
        && b == *b"MZ"
}
pub fn validate_runtime_at(path: &Path, backend: &str) -> Result<(), String> {
    release(backend)?;
    peligames_addon_kit::files::check_directory(path)?;
    let files = &[
        "bin/mako-launch",
        "bin/mako-cli",
        "lib/libmako-render.so",
        "lib32/libmako-render.so",
        "share/mako-render/vulkan/implicit_layer.d/VkLayer_MAKO_render.json",
    ];
    for f in files {
        let p = path.join(f);
        if !p.is_file() || p.is_symlink() {
            return Err("Runtime incompleto. Instale novamente o componente.".into());
        }
    }
    Ok(())
}
pub mod removal;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
