//! Resolve existing Wine/Proton prefixes without creating or initializing one.
use std::{fs, path::{Path, PathBuf}};

pub(super) fn resolve(source: &Path) -> Result<(PathBuf, PathBuf), String> {
    let selected = fs::canonicalize(source).map_err(|error| format!("Prefixo indisponível: {error}"))?;
    let wine = if selected.join("pfx/system.reg").is_file() { selected.join("pfx") } else { selected.clone() };
    if !wine.join("system.reg").is_file() || !wine.join("drive_c").is_dir() {
        return Err("Selecione um prefixo existente com drive_c e system.reg (ou sua pasta compatdata).".into());
    }
    let data = if wine.file_name().and_then(|name| name.to_str()) == Some("pfx") {
        wine.parent().ok_or("Prefixo inválido.")?.to_path_buf()
    } else { selected };
    Ok((data, wine))
}

pub(super) fn containing(source: &Path) -> Option<PathBuf> {
    let source = fs::canonicalize(source).ok()?;
    source.ancestors().find_map(|parent| {
        // The game must actually be inside this prefix's drive_c.
        let (data, wine) = resolve(parent).ok()?;
        source.starts_with(wine.join("drive_c")).then_some(data)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_existing_layouts_and_only_detects_containing_prefix() {
        let root = std::env::temp_dir().join(format!("peli-prefix-test-{}", std::process::id()));
        fs::create_dir_all(root.join("steam/pfx/drive_c/Game")).unwrap();
        fs::write(root.join("steam/pfx/system.reg"), b"test").unwrap();
        fs::create_dir_all(root.join("wine/drive_c/Game")).unwrap();
        fs::write(root.join("wine/system.reg"), b"test").unwrap();
        let steam = fs::canonicalize(root.join("steam")).unwrap();
        let wine = fs::canonicalize(root.join("wine")).unwrap();
        assert_eq!(resolve(&steam).unwrap().0, steam);
        assert_eq!(resolve(&steam.join("pfx")).unwrap().0, steam);
        assert_eq!(resolve(&wine).unwrap().0, wine);
        assert_eq!(containing(&wine.join("drive_c/Game")), Some(wine));
        assert_eq!(containing(&steam.join("pfx/drive_c/Game")), Some(steam));
        assert!(containing(&root).is_none());
        assert!(resolve(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
