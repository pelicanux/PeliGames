//! Model identities pinned by the upstream Linux installer. Models stay local.
use std::{fs, io::{Read, Write}, path::Path};
use sha2::{Digest, Sha256};

pub const ORIGINAL: &str = "dlssnr.bin";
pub const SMALL: &str = "dlssnr-small.bin";
const ORIGINAL_SHA: &str = "2b41c888cf4155b8958c665ba64018ab0bd25c85fc71a2b6db86d0d04d1f7fbd";
const SMALL_SHA: &str = "7e966e2639dbec6596aa0bac3b8761137d2c40ca8cb4aca30df7ca10043c6e33";

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
        let companion = parent.join(other);
        if companion.is_file() && companion != selected {
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
        let path = source.join(name);
        if path.is_file() {
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
