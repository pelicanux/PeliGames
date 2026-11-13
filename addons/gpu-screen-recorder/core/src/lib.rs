//! GPU Screen Recorder integration; no dependency on the launcher or Tauri.
pub mod capture;
pub mod capture_overlay;
pub mod paths;
pub mod runtime;
pub use peligames_audio_processing as audio_denoise;
pub use peligames_mod_config::process as env_utils;
pub mod capture_runtime {
    pub use crate::runtime::{Progress, RuntimeStatus, OPERATION, URL, VERSION};
    use std::path::PathBuf;
    fn root() -> Result<PathBuf, String> {
        crate::paths::app_path("CaptureRuntime")
    }
    pub fn executable() -> Option<PathBuf> {
        crate::runtime::executable(&root().ok()?)
    }
    pub fn status() -> Result<RuntimeStatus, String> {
        crate::runtime::status(&root()?)
    }
    pub fn remove(active: impl Fn() -> bool) -> Result<RuntimeStatus, String> {
        crate::runtime::remove(&root()?, active)
    }
    pub fn install_bytes(
        bytes: &[u8],
        progress: impl Fn(Progress),
        active: impl Fn() -> bool,
    ) -> Result<RuntimeStatus, String> {
        crate::runtime::install_bytes(
            &root()?,
            bytes,
            crate::capture::check_runtime,
            crate::env_utils::host_environment,
            progress,
            active,
        )
    }
}
