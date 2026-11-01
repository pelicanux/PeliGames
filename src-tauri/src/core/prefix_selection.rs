//! Resolve a selected prefix without creating files or running Wine.
use std::{
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

fn real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}
fn initialized(path: &Path) -> bool {
    real_directory(path)
        && real_directory(&path.join("drive_c"))
        && fs::symlink_metadata(path.join("system.reg"))
            .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
}
fn new_path(path: &Path) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("Informe o caminho completo da nova pasta, sem '..'.".into());
    }
    let mut parent = path;
    let mut missing = Vec::new();
    loop {
        match fs::symlink_metadata(parent) {
            Ok(_) => {
                if !parent.is_dir() {
                    return Err("A pasta pai do prefixo não é um diretório.".into());
                }
                let mut result = parent.canonicalize().map_err(|e| e.to_string())?;
                for name in missing.into_iter().rev() {
                    result.push(name);
                }
                return Ok(result);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                missing.push(
                    parent
                        .file_name()
                        .ok_or("Caminho de prefixo inválido.")?
                        .to_os_string(),
                );
                parent = parent.parent().ok_or("Caminho de prefixo inválido.")?;
            }
            Err(error) => return Err(format!("Prefixo indisponível: {error}")),
        }
    }
}
pub(crate) fn resolve(value: &str, allow_new: bool) -> Result<PathBuf, String> {
    let path = super::game_installation::absolute(value)?;
    match fs::symlink_metadata(&path) {
        Err(error) if allow_new && error.kind() == ErrorKind::NotFound => return new_path(&path),
        Err(error) => return Err(format!("Prefixo indisponível: {error}")),
        Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
            return Err("Escolha uma pasta real de prefixo Wine/Proton.".into());
        }
        _ => {}
    }
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if canonical.parent().is_none() {
        return Err("Não use a raiz do sistema como prefixo.".into());
    }
    if initialized(&canonical) {
        return Ok(canonical);
    }
    // The old PeliGames destination contains `prefix`; Steam compatdata contains `pfx`.
    let legacy = super::installation_layout::prefix_for(&canonical);
    let mut nested: Vec<_> = [legacy, canonical.join("pfx")]
        .into_iter()
        .filter(|candidate| candidate != &canonical && initialized(candidate))
        .collect();
    nested.sort();
    nested.dedup();
    if nested.len() > 1 {
        return Err(
            "Há mais de um prefixo nesta pasta. Selecione a pasta do prefixo desejado.".into(),
        );
    }
    if let Some(prefix) = nested.pop() {
        return prefix.canonicalize().map_err(|e| e.to_string());
    }
    if allow_new
        && fs::read_dir(&canonical)
            .map_err(|e| e.to_string())?
            .next()
            .transpose()
            .map_err(|e| e.to_string())?
            .is_none()
    {
        return Ok(canonical);
    }
    Err(if allow_new {
        "Selecione um prefixo com drive_c e system.reg ou uma pasta nova ou vazia para criar um ao jogar. A pasta escolhida contém outros arquivos.".into()
    } else {
        "Selecione o prefixo Wine/Proton que contém drive_c e system.reg.".into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "peligames-prefix-selection-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    fn prefix(p: &Path) {
        fs::create_dir_all(p.join("drive_c")).unwrap();
        fs::write(p.join("system.reg"), b"registry").unwrap();
    }
    #[test]
    fn new_prefix_path_is_saved_without_creating_it() {
        let base = fixture();
        let path = base.join("new/child");
        assert_eq!(resolve(path.to_str().unwrap(), true).unwrap(), path);
        assert!(!base.join("new").exists());
        assert!(resolve(path.to_str().unwrap(), false).is_err());
        fs::write(base.join("file"), b"keep").unwrap();
        assert!(resolve(base.join("file/child").to_str().unwrap(), true).is_err());
        assert!(resolve(base.join("new/../other").to_str().unwrap(), true).is_err());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn existing_prefix_keeps_old_launcher_files_and_internal_proton_link() {
        let base = fixture();
        prefix(&base);
        fs::write(base.join("installation.json"), b"old launcher metadata").unwrap();
        fs::create_dir(base.join("logs")).unwrap();
        std::os::unix::fs::symlink(".", base.join("pfx")).unwrap();
        assert_eq!(resolve(base.to_str().unwrap(), true).unwrap(), base);
        assert_eq!(fs::read(base.join("system.reg")).unwrap(), b"registry");
        assert!(base.join("logs").is_dir());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn legacy_destination_and_steam_compatdata_resolve_to_the_real_prefix() {
        let base = fixture();
        let old = base.join("old");
        prefix(&old.join("prefix"));
        fs::write(
            old.join("installation.json"),
            serde_json::json!({"prefix":old.join("prefix")}).to_string(),
        )
        .unwrap();
        assert_eq!(
            resolve(old.to_str().unwrap(), true).unwrap(),
            old.join("prefix")
        );
        let steam = base.join("steam");
        prefix(&steam.join("pfx"));
        assert_eq!(
            resolve(steam.to_str().unwrap(), false).unwrap(),
            steam.join("pfx")
        );
        prefix(&old.join("pfx"));
        assert!(resolve(old.to_str().unwrap(), true).is_err());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn unrelated_files_incomplete_prefixes_and_symlinks_remain_rejected() {
        let base = fixture();
        let invalid = base.join("other");
        fs::create_dir(&invalid).unwrap();
        fs::write(invalid.join("save.dat"), b"keep").unwrap();
        assert!(resolve(invalid.to_str().unwrap(), true).is_err());
        prefix(&base.join("real"));
        std::os::unix::fs::symlink(base.join("real"), base.join("link")).unwrap();
        assert!(resolve(base.join("link").to_str().unwrap(), true).is_err());
        std::os::unix::fs::symlink(base.join("real"), invalid.join("pfx")).unwrap();
        assert!(resolve(invalid.to_str().unwrap(), true).is_err());
        assert!(resolve("/", true).is_err());
        assert_eq!(fs::read(invalid.join("save.dat")).unwrap(), b"keep");
        fs::remove_dir_all(base).unwrap();
    }
}
