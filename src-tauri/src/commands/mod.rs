pub mod installer;
pub mod updater;
pub mod scanner;
mod steam_readiness;
mod steam_installation;
mod steam_processes;
mod steam_restart;
pub mod analyzer;
pub mod config;
pub mod covers;

mod heroic_library;
mod launcher_library;
mod native_games;
pub mod game_activity;
mod store_artwork;

pub mod app_updates;

pub mod gpu;

mod upscalers;
pub mod neural_settings;

pub mod proton;

pub mod installation_paths;
pub mod game_installation;
pub mod pelinstall;

mod nexus_dependency_order;
pub mod nexus_local;
pub mod nexus_reports;

mod nexus_profile;
pub mod nexus_launch_options;
pub mod mod_launch_options;
pub mod steam_launch_options;
pub(crate) mod steam_runtime;
pub mod launch_profiles;
pub mod matrix_gifs;

mod nexus_deployment;
mod nexus_deploy_methods;
mod nexus_archive;
mod nexus_targets;
mod nexus_activation;
mod nexus_bg3;
mod nexus_fomod;
mod nexus_bain;
pub mod nexus_progress;
pub mod nexus_loot;
pub mod nexus_load_order;
pub mod nexus_gamebryo;
pub mod nexus_plugins;
pub mod nexus_user_profiles;
pub mod nexus_saves;
pub mod nexus_archive_order;
mod nexus_scan;
mod nexus_unity;
mod nexus_discovery;

pub mod nexus_account;
pub mod nexus_oauth;
pub mod discord_account;
pub mod matrix_chat;
pub mod official_build;

pub mod nexus_catalog;
pub mod nexus_translation;

pub mod nexus_collections;
mod nexus_download_watch;
pub mod nexus_downloads;

mod nexus_prefix;

pub mod nexus_modules;

pub mod nexus_patches;

pub mod nexus_smapi;

pub mod nexus_storage;
pub mod nexus_durable;

pub mod browser;
pub mod nexus_browser;

pub mod nexus_loader_logs;

pub mod mod_backup;

pub mod log_windows;

mod nexus_crash_logs;

pub mod external_library;

mod storage;
pub mod storage_automount;

pub mod launcher_installation;

pub mod runtime_activity;

pub mod mangohud;

pub mod mangohud_install;


pub mod matrix_voice;

pub mod matrix_audio;
pub mod matrix_call;
pub mod matrix_screen;
pub mod matrix_screen_audio;
pub mod notification_sound;

pub mod matrix_rtc;

mod epic_library;

pub mod maintenance;

pub mod frame_generation;

pub mod frame_overlay_shortcut;

pub mod frame_overlay_runtime;
