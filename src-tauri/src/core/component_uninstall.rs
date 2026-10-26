//! Remove only explicitly named runtime directories, never game data or profiles.
use std::{
    fs,
    path::{Path, PathBuf},
};
pub fn remove_runtime(storage: &Path, backend: &str) -> Result<(), String> {
    if !matches!(backend, "mako" | "afmf") {
        return Err("Componente inválido.".into());
    }
    let mut targets = Vec::<PathBuf>::new();
    if !storage.exists() {
        return Ok(());
    }
    if fs::symlink_metadata(storage)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Pasta de componentes inválida.".into());
    }
    for entry in fs::read_dir(storage).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let legacy = name
            .strip_prefix(&format!("{backend}-"))
            .is_some_and(|version| {
                let parts: Vec<_> = version.split('.').collect();
                parts.len() == 3
                    && parts
                        .iter()
                        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
            });
        if name == backend || legacy {
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err("Runtime inválido; desinstalação recusada.".into());
            }
            targets.push(entry.path());
        }
    }
    // Loaded modules can outlive the launcher profile. Refuse to unlink their
    // runtime while a same-user process still has a mapped library from it.
    #[cfg(target_os = "linux")]
    for entry in fs::read_dir("/proc").map_err(|e| e.to_string())?.flatten() {
        use std::os::unix::fs::MetadataExt;
        if !entry
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|b| b.is_ascii_digit())
        {
            continue;
        }
        if !entry
            .metadata()
            .is_ok_and(|m| m.uid() == unsafe { libc::geteuid() })
        {
            continue;
        }
        if let Ok(maps) = fs::read_to_string(entry.path().join("maps")) {
            if targets
                .iter()
                .any(|path| maps.contains(&format!("{}/", path.display())))
            {
                return Err(
                    "Encerre os jogos que estão usando este componente antes de desinstalar."
                        .into(),
                );
            }
        }
    }
    for target in targets {
        fs::remove_dir_all(target).map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uninstall_removes_current_and_legacy_but_preserves_other_components_and_profiles() {
        let root = std::env::temp_dir().join(format!("peli-uninstall-{}", uuid::Uuid::new_v4()));
        for name in [
            "mako/versions/4.5.0",
            "mako-4.4.0",
            "afmf/versions/1.3.1",
            "profiles",
            "overlay",
        ] {
            fs::create_dir_all(root.join(name)).unwrap();
            fs::write(root.join(name).join("keep.txt"), "fixture").unwrap();
        }
        remove_runtime(&root, "mako").unwrap();
        assert!(!root.join("mako").exists());
        assert!(!root.join("mako-4.4.0").exists());
        for name in ["afmf/versions/1.3.1", "profiles", "overlay"] {
            assert!(root.join(name).join("keep.txt").is_file());
        }
        assert!(remove_runtime(&root, "../profiles").is_err());
        remove_runtime(&root, "afmf").unwrap();
        assert!(root.join("profiles").is_dir());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn uninstall_preserves_runtime_loaded_by_a_process() {
        use std::os::fd::AsRawFd;
        let root =
            std::env::temp_dir().join(format!("peli-uninstall-loaded-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("afmf")).unwrap();
        let path = root.join("afmf/module");
        fs::write(&path, vec![0u8; 4096]).unwrap();
        let file = fs::File::open(path).unwrap();
        let mapping = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ,
                libc::MAP_PRIVATE,
                file.as_raw_fd(),
                0,
            )
        };
        assert_ne!(mapping, libc::MAP_FAILED);
        let result = remove_runtime(&root, "afmf");
        unsafe {
            libc::munmap(mapping, 4096);
        }
        assert!(result.unwrap_err().contains("Encerre os jogos"));
        assert!(root.join("afmf/module").is_file());
        remove_runtime(&root, "afmf").unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn uninstall_rejects_symlink_without_removing_other_targets() {
        let root =
            std::env::temp_dir().join(format!("peli-uninstall-link-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("mako")).unwrap();
        fs::create_dir(root.join("outside")).unwrap();
        std::os::unix::fs::symlink(root.join("outside"), root.join("mako-4.4.0")).unwrap();
        assert!(remove_runtime(&root, "mako").is_err());
        assert!(root.join("mako").is_dir());
        assert!(root.join("outside").is_dir());
        fs::remove_dir_all(root).unwrap();
    }
}
