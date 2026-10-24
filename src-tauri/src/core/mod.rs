#[cfg(test)]
pub(crate) mod test_support;
pub mod script_runner;
#[path = "../../launch-runtime/src/release_features.rs"]
pub mod release_features;
pub mod file_manager;
pub mod env_utils;
pub mod config_manager;
pub mod logger;

pub mod game_info_cache;

pub mod proton_manager;
pub mod game_installation;
pub mod installed_library;
pub mod game_profiles;
pub mod installation_layout;

pub mod game_execution;

pub mod advanced_settings;

pub mod wine_tools;

pub mod umu_manager;
pub(crate) mod steam_client;
pub mod pelinstall;

pub mod executable_icon;

pub mod installation_targets;

pub mod execution_registry;

pub mod game_logs;

pub mod paths;
pub mod playtime;

#[cfg(unix)]
pub mod launcher_instance;

pub mod nxm;

pub mod mangohud_config;
pub mod mangohud_benchmarks;
pub mod mangohud_root;
pub mod pe_imports;
pub mod game_executable;

pub mod neural_models;

pub mod game_copies;
pub mod mangohud_presets;
pub mod bounded_logs;
pub mod game_reset;

pub mod neon_wave_bands;
#[cfg(target_os = "linux")]
pub mod neon_wave_audio;

pub mod openrgb;

#[cfg(target_os = "linux")]
pub mod webkit_graphics;
