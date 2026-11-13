use std::{env,path::PathBuf};
pub const BUNDLE_VARIABLES: [&str; 22] = [
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
