use std::{path::{Path, PathBuf}, process::Command};

pub fn prepare(base: &Path, architecture: &str) -> Result<PathBuf, String> {
    if !matches!(architecture, "rdna3" | "rdna4") { return Err("Invalid backend architecture".into()); }
    let directory = base.join(architecture);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed creating mod folder {}: {error}", directory.display()))?;
    Ok(directory)
}

pub fn system_environment(command: &mut Command, appdir: Option<&Path>) {
    command.env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD");
    if let Some(appdir) = appdir {
        // AppImage GTK hooks point these at bundled modules unavailable to Dolphin.
        for key in ["GTK_DATA_PREFIX", "GTK_EXE_PREFIX", "GTK_PATH", "GTK_IM_MODULE_FILE", "GDK_PIXBUF_MODULE_FILE", "GIO_MODULE_DIR", "GI_TYPELIB_PATH", "GSETTINGS_SCHEMA_DIR", "APPDIR", "APPIMAGE"] {
            command.env_remove(key);
        }
        for key in ["PATH", "XDG_DATA_DIRS"] {
            if let Some(value) = std::env::var_os(key) {
                let paths = std::env::split_paths(&value).filter(|path| !path.starts_with(appdir));
                if let Ok(value) = std::env::join_paths(paths) { command.env(key, value); }
            }
        }
    }
}

pub fn open(directory: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let mut command = Command::new("/usr/bin/xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    command.arg(directory);
    let appdir = std::env::var_os("APPDIR").map(PathBuf::from);
    system_environment(&mut command, appdir.as_deref());
    command.stdin(std::process::Stdio::null());
    let output = command.output().map_err(|error| format!("Failed opening {}: {error}", directory.display()))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).chars().take(2048).collect::<String>();
        return Err(format!("Failed opening {} ({}): {}", directory.display(), output.status, detail.trim()));
    }
    Ok(())
}
