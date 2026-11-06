//! Model identities pinned by the upstream Linux installer. Models stay local.
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}};
use sha2::{Digest, Sha256};

pub const ORIGINAL: &str = "dlssnr.bin";
pub const SMALL: &str = "dlssnr-small.bin";
pub const ORIGINAL_NETWORK: &str = "CC_SILVER_AARDWOLD.bin";
pub const SMALL_NETWORK: &str = "CC_SMALL_FP8.bin";
const ORIGINAL_SHA: &str = "2b41c888cf4155b8958c665ba64018ab0bd25c85fc71a2b6db86d0d04d1f7fbd";
const SMALL_SHA: &str = "7e966e2639dbec6596aa0bac3b8761137d2c40ca8cb4aca30df7ca10043c6e33";

/// Keep the cache names stable while accepting upstream's network-based filenames.
pub fn find(directory: &Path, identity: &str) -> Option<PathBuf> {
    let aliases = match identity {
        ORIGINAL => [ORIGINAL, ORIGINAL_NETWORK],
        SMALL => [SMALL, SMALL_NETWORK],
        _ => return None,
    };
    aliases.into_iter().map(|name| directory.join(name)).find(|path| path.is_file())
}

/// Adapt only the temporary package to the filenames its installer expects.
pub fn prepare_for_backend(directory: &Path, script: &str) -> Result<(), String> {
    let named = script.contains("/CC_SILVER_AARDWOLD.bin");
    let mut models = Vec::new();
    for (identity, network) in [(ORIGINAL, ORIGINAL_NETWORK), (SMALL, SMALL_NETWORK)] {
        if let Some(source) = find(directory, identity) {
            if identify(&source)? != identity {
                return Err(format!("O nome do modelo não corresponde ao seu conteúdo: {identity}"));
            }
            models.push((source, directory.join(if named { network } else { identity })));
        }
    }
    for (source, target) in models {
        if source != target { copy_atomic(&source, &target)?; }
    }
    // install.sh copies this entire directory into the game. Keep one filename
    // per identity there, without changing the persistent cache's stable names.
    for obsolete in if named { [ORIGINAL, SMALL] } else { [ORIGINAL_NETWORK, SMALL_NETWORK] } {
        let path = directory.join(obsolete);
        if path.is_file() { fs::remove_file(path).map_err(|error| error.to_string())?; }
    }
    Ok(())
}

pub fn identify(path: &Path) -> Result<&'static str, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("Não foi possível ler o modelo {}: {e}", path.display()))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        hash.update(&buffer[..n]);
    }
    match format!("{:x}", hash.finalize()).as_str() {
        ORIGINAL_SHA => Ok(ORIGINAL),
        SMALL_SHA => Ok(SMALL),
        _ => Err("Modelo inválido: o SHA-256 não corresponde aos modelos 310.8.0/310.9.16. Extraia novamente a DLL no assistente.".into()),
    }
}

pub fn copy_atomic(source: &Path, destination: &Path) -> Result<(), String> {
    let parent = destination.parent().ok_or("Destino do modelo inválido")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(".model-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut input = fs::File::open(source)?;
        let mut output = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        std::io::copy(&mut input, &mut output)?;
        output.flush()?;
        output.sync_all()?;
        fs::rename(&temporary, destination)
    })();
    if result.is_err() { let _ = fs::remove_file(&temporary); }
    result.map_err(|e| format!("Falha ao salvar modelo {}: {e}", destination.display()))
}

/// Stage the selected model by its contents, including a validated sibling model.
/// A renamed small BIN must never overwrite the original model's filename.
pub fn stage(selected: &Path, destination: &Path) -> Result<(), String> {
    let selected_name = identify(selected)?;
    let mut sources = vec![(selected.to_path_buf(), selected_name)];
    if let Some(parent) = selected.parent() {
        let other = if selected_name == ORIGINAL { SMALL } else { ORIGINAL };
        if let Some(companion) = find(parent, other).filter(|path| path != selected) {
            if identify(&companion)? != other { return Err("O nome do modelo complementar não corresponde ao seu conteúdo.".into()); }
            sources.push((companion, other));
        }
    }
    if selected_name == SMALL && !sources.iter().any(|(_, name)| *name == ORIGINAL) {
        return Err("O modelo novo precisa estar ao lado de dlssnr.bin. Extraia a DLL 310.9.16 no assistente para obter os dois arquivos.".into());
    }
    for (source, name) in sources { copy_atomic(&source, &destination.join(name))?; }
    Ok(())
}

/// Publish both models only after all generated files have passed validation.
pub fn cache(source: &Path, destination: &Path) -> Result<(), String> {
    let mut models = Vec::new();
    for name in [ORIGINAL, SMALL] {
        if let Some(path) = find(source, name) {
            if identify(&path)? != name { return Err(format!("Modelo extraído inválido: {name}")); }
            models.push((path, name));
        }
    }
    if !models.iter().any(|(_, name)| *name == ORIGINAL) { return Err("O backend não gerou o modelo original necessário (dlssnr.bin).".into()); }
    for (path, name) in models { copy_atomic(&path, &destination.join(name))?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_discovery_accepts_both_layouts_and_ignores_directories() {
        let root = std::env::temp_dir().join(format!("model-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join(ORIGINAL)).unwrap();
        fs::write(root.join(ORIGINAL_NETWORK), b"placeholder").unwrap();
        fs::write(root.join(SMALL_NETWORK), b"placeholder").unwrap();
        assert_eq!(find(&root, ORIGINAL), Some(root.join(ORIGINAL_NETWORK)));
        assert_eq!(find(&root, SMALL), Some(root.join(SMALL_NETWORK)));
        assert_eq!(find(&root, "unknown"), None);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_alias_does_not_publish_compatibility_files() {
        let root = std::env::temp_dir().join(format!("model-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(ORIGINAL), b"invalid").unwrap();
        assert!(prepare_for_backend(&root, "pkg_model=$here/dlssnr-amd/CC_SILVER_AARDWOLD.bin").is_err());
        assert!(!root.join(ORIGINAL_NETWORK).exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_bin_is_rejected_without_overwriting_destination() {
        let root = std::env::temp_dir().join(format!("model-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("cache")).unwrap();
        fs::write(root.join("selected.bin"), b"not a model").unwrap();
        fs::write(root.join("cache").join(ORIGINAL), b"keep").unwrap();
        assert!(stage(&root.join("selected.bin"), &root.join("cache")).is_err());
        assert_eq!(fs::read(root.join("cache").join(ORIGINAL)).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn atomic_copy_replaces_and_leaves_no_partial_file() {
        let root = std::env::temp_dir().join(format!("model-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("source"), b"complete").unwrap();
        copy_atomic(&root.join("source"), &root.join("target")).unwrap();
        assert_eq!(fs::read(root.join("target")).unwrap(), b"complete");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
