pub mod script_runner;
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
pub mod mangohud_root;
pub mod pe_imports;
pub mod game_executable;

pub mod neural_models;
