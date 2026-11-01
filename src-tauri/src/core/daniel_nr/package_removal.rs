//! Remove one shared prepared package; game copies, downloads and saved settings are independent.
use super::{runtimes::Runtime, *};

pub fn remove(
    base: &Path,
    runtime: Runtime,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    let folder = base.join("prepared");
    for path in [&folder, &runtime.package(base)] {
        let meta = match fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.to_string()),
        };
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("Pasta do pacote Daniel NR inválida; nenhum arquivo foi removido.".into());
        }
    }
    runtime_switch::ensure_package_removable(base, runtime)?;
    progress(format!(
        "Desinstalando pacote Daniel {}…",
        runtime.version()
    ));
    fs::remove_dir_all(runtime.package(base)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "peligames-package-removal-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn file(&self, rel: &str) -> PathBuf {
            let path = self.0.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, rel).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn each_removal_preserves_other_versions_game_copies_settings_and_downloads() {
        let f = Fixture::new();
        let keep = [
            "game/winmm.dll",
            "runtime-settings/Game-0.6.0/OptiScaler.ini",
            "downloads/runtime.zip",
            "AMDNR-Linux-v0.3.6/linux/manage.py",
        ];
        for rel in keep {
            f.file(rel);
        }
        for version in ["0.6.0", "0.8.0"] {
            f.file(&format!("prepared/daniel-{version}/winmm.dll"));
        }
        remove(&f.0, Runtime::V06, &|_| {}).unwrap();
        assert!(!Runtime::V06.package(&f.0).exists());
        assert!(Runtime::V08.package(&f.0).join("winmm.dll").is_file());
        remove(&f.0, Runtime::V06, &|_| {}).unwrap();
        remove(&f.0, Runtime::V08, &|_| {}).unwrap();
        for rel in keep {
            assert_eq!(fs::read_to_string(f.0.join(rel)).unwrap(), rel);
        }
    }
    #[test]
    fn refuses_symlinked_package_or_parent_and_does_not_follow_nested_links() {
        let f = Fixture::new();
        let outside = Fixture::new();
        let keep = outside.file("save.dat");
        fs::create_dir(f.0.join("prepared")).unwrap();
        std::os::unix::fs::symlink(&outside.0, Runtime::V06.package(&f.0)).unwrap();
        assert!(remove(&f.0, Runtime::V06, &|_| {}).is_err());
        fs::remove_file(Runtime::V06.package(&f.0)).unwrap();
        fs::remove_dir(f.0.join("prepared")).unwrap();
        std::os::unix::fs::symlink(&outside.0, f.0.join("prepared")).unwrap();
        assert!(remove(&f.0, Runtime::V06, &|_| {}).is_err());
        fs::remove_file(f.0.join("prepared")).unwrap();
        fs::create_dir_all(Runtime::V06.package(&f.0)).unwrap();
        std::os::unix::fs::symlink(&outside.0, Runtime::V06.package(&f.0).join("link")).unwrap();
        remove(&f.0, Runtime::V06, &|_| {}).unwrap();
        assert_eq!(fs::read_to_string(keep).unwrap(), "save.dat");
    }
    #[test]
    fn keeps_package_needed_by_interrupted_switch_but_allows_other_version() {
        let f = Fixture::new();
        for version in ["0.6.0", "0.8.0"] {
            f.file(&format!("prepared/daniel-{version}/winmm.dll"));
        }
        let record = f.file("version-changes/Game.json");
        fs::write(&record, r#"{"schema":1,"from":"0.6.0","to":"0.8.0","proxy":"winmm.dll","executable":"/fixture/Game.exe"}"#).unwrap();
        assert!(remove(&f.0, Runtime::V06, &|_| {})
            .unwrap_err()
            .contains("Recupere"));
        assert!(Runtime::V06.package(&f.0).join("winmm.dll").exists());
        remove(&f.0, Runtime::V08, &|_| {}).unwrap();
        fs::write(record, "invalid").unwrap();
        assert!(remove(&f.0, Runtime::V06, &|_| {}).is_err());
    }
}
