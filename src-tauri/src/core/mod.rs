pub mod discord_presence;
pub mod audio_denoise;
#[cfg(test)]
pub(crate) mod test_support;
#[path = "../../launch-runtime/src/release_features.rs"]
pub mod release_features;
#[cfg(target_os = "linux")]
#[path = "../../launch-runtime/src/discord_socket.rs"]
pub mod discord_socket;
pub mod file_manager;
pub mod env_utils;
pub mod config_manager;
pub mod logger;
pub mod report_privacy;

pub mod game_info_cache;

pub mod proton_manager;
pub mod game_installation;
pub mod installed_library;
pub mod game_profiles;
pub mod installation_layout;

pub mod game_execution;

pub mod advanced_settings;
pub(crate) mod addon_launch;
pub(crate) mod launch_environment;

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
#[cfg(target_os = "linux")]
pub mod webkit_vblank;

pub mod advanced_capabilities;




pub mod daniel_nr;

pub(crate) mod prefix_selection;

pub mod system_locale;
