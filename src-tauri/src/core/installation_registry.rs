//! Independent installation records survive removal of a Wine prefix.
//! The portable manifest remains useful for importing games on another machine.
use super::InstallationRecord;
use crate::core::paths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Saved {
    format: u32,
    installation: InstallationRecord,
    /// Detect intentional edits by an older launcher without undoing newer settings.
    portable_sha256: Option<String>,
}
fn folder(index: &Path) -> Result<PathBuf, String> {
    let folder =
        paths::layout::installation_records(index).ok_or("Índice de instalações inválido.")?;
    match fs::symlink_metadata(&folder) {
        Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
            return Err("A pasta dos cadastros foi redirecionada.".into())
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.to_string()),
        _ => {}
    }
    Ok(folder)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Registro indisponível em {}: {error}",
                path.display()
            ))
        }
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 32 * 1024 * 1024
    {
        return Err(format!(
            "Registro inválido ou redirecionado: {}",
            path.display()
        ));
    }
    fs::read(path).map(Some).map_err(|error| error.to_string())
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Pasta do registro inválida.")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if !fs::symlink_metadata(parent).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
        return Err("A pasta dos registros foi redirecionada.".into());
    }
    let temporary = parent.join(format!(".installation-record-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())?;
        fs::File::open(parent)
            .and_then(|file| file.sync_all())
            .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn validate(record: &InstallationRecord, directory: &Path) -> Result<(), String> {
    if !directory.is_absolute()
        || directory.parent().is_none()
        || Path::new(&record.directory) != directory
    {
        return Err("O cadastro da instalação não corresponde à pasta registrada.".into());
    }
    Ok(())
}
fn saved(index: &Path, directory: &Path) -> Result<Option<Saved>, String> {
    let root = folder(index)?;
    let Some(path) = paths::names::find(&root, &directory.to_string_lossy()) else {
        return Ok(None);
    };
    let Some(data) = bytes(&path.with_extension("json"))? else {
        return Ok(None);
    };
    let record: Saved = serde_json::from_slice(&data)
        .map_err(|e| format!("Cadastro independente inválido: {e}"))?;
    if record.format != 1 {
        return Err("Versão desconhecida do cadastro de instalações.".into());
    }
    validate(&record.installation, directory)?;
    Ok(Some(record))
}
fn remember(
    index: &Path,
    record: &InstallationRecord,
    portable_sha256: Option<String>,
) -> Result<(), String> {
    let directory = Path::new(&record.directory);
    validate(record, directory)?;
    let path = paths::names::allocate(&folder(index)?, &record.name, &record.directory)?
        .with_extension("json");
    let value = Saved {
        format: 1,
        installation: record.clone(),
        portable_sha256,
    };
    atomic(
        &path,
        &serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
}
fn load(
    index: &Path,
    directory: &Path,
    migrate: bool,
) -> Result<Option<InstallationRecord>, String> {
    let cached = saved(index, directory)?;
    let local = bytes(&directory.join("installation.json"))?;
    if let Some(data) = local {
        let hash = digest(&data);
        if let Some(cached) = &cached {
            if cached.portable_sha256.as_ref() == Some(&hash) {
                if migrate {
                    let current =
                        serde_json::to_value(&cached.installation).map_err(|e| e.to_string())?;
                    let portable: serde_json::Value =
                        serde_json::from_slice(&data).map_err(|e| e.to_string())?;
                    if current != portable {
                        repair(index, &cached.installation);
                    }
                }
                return Ok(Some(cached.installation.clone()));
            }
        }
        let record: InstallationRecord = serde_json::from_slice(&data)
            .map_err(|e| format!("Registro inválido em {}: {e}", directory.display()))?;
        validate(&record, directory)?;
        if migrate {
            remember(index, &record, Some(hash))?;
        }
        return Ok(Some(record));
    }
    if let Some(cached) = &cached {
        if migrate {
            repair(index, &cached.installation);
        }
    }
    Ok(cached.map(|record| record.installation))
}
/// Restore a missing/stale portable copy without making a readable registration
/// depend on a writable disk. store() never recreates an absent installation folder.
fn repair(index: &Path, record: &InstallationRecord) {
    if !Path::new(&record.directory).is_dir() {
        return;
    }
    if let Err(error) = store(index, record) {
        #[cfg(not(test))]
        crate::core::logger::event("WARN", &format!(
            "Cadastro disponível na configuração; não foi possível recuperar a cópia portátil: {error}"
        ));
        #[cfg(test)]
        let _ = error;
    }
}
pub(super) fn read(index: &Path, directory: &Path) -> Result<Option<InstallationRecord>, String> {
    load(index, directory, true)
}
pub(super) fn peek(index: &Path, directory: &Path) -> Result<Option<InstallationRecord>, String> {
    load(index, directory, false)
}
pub(super) fn store(index: &Path, record: &InstallationRecord) -> Result<(), String> {
    let directory = Path::new(&record.directory);
    validate(record, directory)?;
    let portable_available = match fs::symlink_metadata(directory) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Ok(_) => return Err("A pasta da instalação foi redirecionada.".into()),
        Err(error) => return Err(error.to_string()),
    };
    let portable = directory.join("installation.json");
    let previous = bytes(&portable)?
        .map(|data| digest(&data))
        .or(saved(index, directory)?.and_then(|record| record.portable_sha256));
    // Save outside the prefix first. Do not recreate a deleted/offline game folder.
    remember(index, record, previous)?;
    if portable_available {
        let data = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
        match atomic(&portable, &data) {
            Ok(()) => remember(index, record, Some(digest(&data)))?,
            Err(error) => {
                #[cfg(not(test))]
                crate::core::logger::event("WARN", &format!("Cadastro salvo na configuração; não foi possível atualizar a cópia portátil: {error}"));
                #[cfg(test)]
                let _ = error;
            }
        }
    }
    Ok(())
}
pub(super) fn forget(index: &Path, directory: &Path) -> Result<(), String> {
    let Some(path) = paths::names::find(&folder(index)?, &directory.to_string_lossy()) else {
        return Ok(());
    };
    for file in [path.with_extension("json"), path.with_extension("identity")] {
        match fs::remove_file(file) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                return Err(error.to_string())
            }
            _ => {}
        }
    }
    Ok(())
}
