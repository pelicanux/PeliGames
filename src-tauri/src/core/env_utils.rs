use std::env;
use std::path::PathBuf;

/// Gets the base directory of the executable.
/// 
/// If running as an AppImage, `std::env::current_exe()` returns a path inside
/// the read-only `/tmp/.mount_xxx` filesystem. To find the actual directory
/// where the user placed the `.AppImage`, we must check the `APPIMAGE` environment variable.
pub fn get_base_dir() -> PathBuf {
    if let Ok(appimage_path) = env::var("APPIMAGE") {
        let p = PathBuf::from(appimage_path);
        if let Some(parent) = p.parent() {
            return parent.to_path_buf();
        }
    }
    
    if let Ok(exe_path) = env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.to_path_buf();
        }
    }
    
    PathBuf::from(".")
}

/// Variables an AppImage's AppRun and its GTK/GStreamer/Python hooks point at
/// the bundle. Programs of the host (umu-run is a Python script, Proton, the
/// file manager) break with them: "No module named 'encodings'", wrong GTK
/// modules, host tools loading the bundle's libraries.
const BUNDLE_VARIABLES: [&str; 22] = [
    "LD_LIBRARY_PATH", "LD_PRELOAD", "PYTHONHOME", "PYTHONPATH", "PERLLIB",
    "GTK_DATA_PREFIX", "GTK_EXE_PREFIX", "GTK_PATH", "GTK_IM_MODULE_FILE", "GDK_PIXBUF_MODULE_FILE",
    "GIO_MODULE_DIR", "GI_TYPELIB_PATH", "GSETTINGS_SCHEMA_DIR", "QT_PLUGIN_PATH",
    "GST_PLUGIN_PATH", "GST_PLUGIN_PATH_1_0", "GST_PLUGIN_SYSTEM_PATH", "GST_PLUGIN_SYSTEM_PATH_1_0", "GST_PLUGIN_SCANNER", "GST_PLUGIN_SCANNER_1_0",
    "APPDIR", "APPIMAGE",
];

/// The bundle's mount or extraction folder when running from an AppImage.
pub fn bundle_dir() -> Option<PathBuf> {
    env::var_os("APPDIR").map(PathBuf::from).filter(|path| !path.as_os_str().is_empty())
}

/// Gives a host program the environment it would have outside the AppImage.
pub fn host_environment(command: &mut std::process::Command) {
    let Some(bundle) = bundle_dir() else {
        command.env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD");
        return;
    };
    for key in BUNDLE_VARIABLES { command.env_remove(key); }
    for key in ["PATH", "XDG_DATA_DIRS"] {
        if let Some(value) = env::var_os(key) {
            let paths = env::split_paths(&value).filter(|path| !path.starts_with(&bundle));
            if let Ok(value) = env::join_paths(paths) { command.env(key, value); }
        }
    }
}

/// Python variables are dropped from the launcher's own environment at start:
/// nothing in it runs Python, and every child would inherit them otherwise.
pub fn drop_bundle_python() {
    if bundle_dir().is_some() {
        env::remove_var("PYTHONHOME");
        env::remove_var("PYTHONPATH");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn host_programs_lose_the_bundle_variables() {
        let mut command = std::process::Command::new("true");
        command.env("PYTHONHOME", "/tmp/.mount_x/usr").env("LD_LIBRARY_PATH", "/tmp/.mount_x/usr/lib");
        super::host_environment(&mut command);
        let removed = |key: &str| command.get_envs().any(|(name, value)| name == key && value.is_none());
        assert!(removed("LD_LIBRARY_PATH"));
        assert!(super::BUNDLE_VARIABLES.contains(&"PYTHONHOME") && super::BUNDLE_VARIABLES.contains(&"PYTHONPATH"));
    }
}
