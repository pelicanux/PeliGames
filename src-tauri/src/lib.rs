pub mod core;
pub mod commands;

#[cfg(all(not(debug_assertions), not(feature = "custom-protocol")))]
compile_error!("Binários de produção precisam de --features custom-protocol para embutir a interface, sem depender de localhost.");

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    run_mode(false);
}

pub fn run_installer() { run_mode(true); }

fn app_context() -> tauri::Context<tauri::Wry> {
    tauri::generate_context!()
}

/// reqwest and LiveKit pull rustls with two crypto backends (aws-lc-rs and
/// ring); rustls then cannot pick one by itself and aborts the process on the
/// first TLS connection (joining a voice call). aws-lc-rs, reqwest's own, is
/// chosen once for the whole process.
fn choose_tls_backend() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
}
#[cfg(test)]
mod tls_tests {
    #[test]
    fn a_tls_client_config_builds_with_both_backends_compiled() {
        super::choose_tls_backend();
        // Without a chosen backend this panics: "Could not automatically determine the process-level CryptoProvider".
        let _ = rustls::ClientConfig::builder();
    }
}
/// WebKitGTK 2.42+ draws pages through DMABUF buffers; with the NVIDIA
/// driver this often leaves heavy pages (Nexus sign-in, Cloudflare check)
/// black or frozen. Fall back to its shared-memory renderer on NVIDIA. Set
/// before GTK starts; a value the user already set is kept.
fn webkit_graphics_workaround() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() && nvidia_driver() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}
/// The NVIDIA kernel driver (proprietary or open modules); not nouveau.
#[cfg(target_os = "linux")]
fn nvidia_driver() -> bool {
    std::path::Path::new("/proc/driver/nvidia/version").exists() || std::path::Path::new("/sys/module/nvidia").exists()
}
fn run_mode(installer: bool) {
    // Root helper started through pkexec: nothing else of the launcher runs.
    let first = std::env::args().nth(1);
    if first.as_deref() == Some(core::mangohud_root::FLAG) {
        let args: Vec<String> = std::env::args().skip(2).collect();
        std::process::exit(core::mangohud_root::run(&args));
    }
    if first.as_deref() == Some(commands::storage_automount::ROOT_FLAG) {
        std::process::exit(commands::storage_automount::run_root());
    }
    choose_tls_backend();
    crate::core::env_utils::drop_bundle_python();
    webkit_graphics_workaround();
    #[cfg(unix)]
    if let Err(error) = core::launcher_instance::wait_for_restart_parent() { eprintln!("{error}"); return; }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--matrix-rtc-test"] { commands::matrix_rtc_probe::run_standalone(); return; }
    if args == ["--verify-ui"] {
        let context = app_context();
        let assets: Vec<String> = context.assets().iter().map(|(name, _)| name.to_string()).collect();
        let embedded = !tauri::is_dev()
            && assets.iter().any(|name| name.trim_start_matches('/') == "index.html")
            && assets.iter().any(|name| name.ends_with(".js"))
            && assets.iter().any(|name| name.ends_with(".css"))
            && assets.iter().any(|name| name.ends_with(".wasm"));
        println!("{}", serde_json::json!({"production": !tauri::is_dev(), "embedded_ui": embedded, "asset_count": assets.len()}));
        if !embedded { std::process::exit(1); }
        return;
    }
    if let Err(error) = core::paths::initialize() { eprintln!("Migração PeliGames: {error}"); }
    let nxm_start = if args.len() == 1 && args[0].get(..4).is_some_and(|s| s.eq_ignore_ascii_case("nxm:")) { Some(args[0].clone()) } else { None };
    let launcher_args = if nxm_start.is_some() { Vec::new() } else { args };
    let mut startup = core::pelinstall::parse_args(&launcher_args, installer && nxm_start.is_none()).unwrap_or_else(|error| core::pelinstall::StartupInfo {
        module: "pelinstall".into(), error: Some(error), ..Default::default()
    });
    if startup.module == "background" {
        let entry = startup.entry.clone().unwrap_or_default();
        match core::pelinstall::monitor_background(&entry) {
            Ok(()) => return,
            Err(report) => {
                let _ = core::pelinstall::save_report(&report);
                startup.module = "launch-error".into(); startup.error = Some(report.error); startup.log = report.log;
            }
        }
    } else if startup.module == "launcher" {
        if let Some(report) = startup.entry.as_deref().and_then(core::pelinstall::report_for) {
            startup.error = Some(report.error); startup.log = report.log;
        }
    }
    #[cfg(unix)]
    let launcher_instance = if startup.module == "launcher" {
        match core::paths::app_root().and_then(|root| core::launcher_instance::LauncherInstance::claim_at(&root, nxm_start.as_deref().or(startup.entry.as_deref()))) {
            Ok(Some(instance)) => Some(instance),
            Ok(None) => return,
            Err(error) => { eprintln!("Não foi possível abrir o launcher: {error}"); return; }
        }
    } else { None };
    if let Err(error) = core::pelinstall::register_appimage() { eprintln!("Integração AppImage: {error}"); }
    // Before any webview opens: its storage moves with the rest of the launcher data.
    for error in core::paths::migrate_scattered_data() { eprintln!("Migração de dados: {error}"); }
    let mut context = app_context();
    if startup.module != "launcher" {
        if let Some(window) = context.config_mut().app.windows.first_mut() {
            window.title = if startup.module == "launch-error" { "PeliGames — execução" } else { "Pelinstall" }.into();
            window.width = 560.; window.height = if startup.module == "pelinstall" { 340. } else { 490. };
            window.min_width = Some(480.); window.min_height = Some(if startup.module == "pelinstall" { 260. } else { 420. });
            // Pelinstall grows with its step (Proton list open, executable choice): see PELINSTALL_MAX_HEIGHT.
            window.max_width = Some(560.); window.max_height = Some(if startup.module == "pelinstall" { 720. } else { 490. });
            window.maximizable = false;
            window.center = true;
        }
    }
    // Only the launcher window checks the build; game shortcuts and Pelinstall stay offline.
    let check_build = startup.module == "launcher";
    // Launcher housekeeping (update cache, Nexus session logs) is not the
    // installer's business: Pelinstall and game shortcuts skip it.
    let launcher_window = startup.module == "launcher";
    tauri::Builder::default()
        .manage(startup)
        .manage(commands::nexus_downloads::Downloads::default())
        .manage(commands::nexus_browser::NexusBrowser::default())
        .manage(commands::pelinstall::LauncherOpenRequest::default())
        .manage(commands::app_updates::AppUpdateState::default())
        .manage(commands::proton::ProtonDownloadState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(move |app| {
            #[cfg(unix)]
            if let Some(instance) = launcher_instance {
                use tauri::{Emitter, Manager};
                let handle = app.handle().clone();
                std::thread::spawn(move || instance.serve(move |entry| {
                    let window = handle.get_window("main").ok_or("Janela do launcher indisponível.")?;
                    window.show().map_err(|e| e.to_string())?;
                    window.unminimize().map_err(|e| e.to_string())?;
                    window.set_focus().map_err(|e| e.to_string())?;
                    if let Some(entry) = entry {
                        if entry.get(..4).is_some_and(|s| s.eq_ignore_ascii_case("nxm:")) {
                            return commands::nexus_downloads::receive(&handle, &entry);
                        }
                        *handle.state::<commands::pelinstall::LauncherOpenRequest>().0.lock().unwrap_or_else(|e| e.into_inner()) = Some(entry);
                        handle.emit("launcher-open-request", ()).map_err(|e| e.to_string())?;
                    }
                    Ok(())
                }));
            }
            let app_handle = app.handle().clone();
            if let Some(link) = nxm_start {
                if let Err(error) = commands::nexus_downloads::receive(&app_handle, &link) { eprintln!("Nexus: {error}"); }
            }
            if launcher_window {
                crate::core::logger::log_launcher(
                    &app_handle,
                    "INFO",
                    &format!("PeliGames v{} iniciado", env!("CARGO_PKG_VERSION")),
                );
                commands::app_updates::cleanup_obsolete_updates(&app_handle);
            }
            if check_build { commands::official_build::verify_in_background(app_handle.clone()); }
            commands::nexus_progress::init(&app_handle);
            commands::mod_backup::init(&app_handle);
            // Launch logs start empty on every launcher start (disk work off the UI thread).
            if launcher_window { std::thread::spawn(commands::nexus_local::reset_session_logs_at_startup); }
            Ok(())
        })
        .invoke_handler({
            let handler: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> = Box::new(tauri::generate_handler![
            commands::nexus_downloads::list_nexus_downloads,
            commands::nexus_local::refresh_nexus_mod_labels,
            commands::nexus_browser::open_nexus_browser,
            commands::nexus_browser::nexus_browser_session,
            commands::nexus_browser::resize_nexus_browser,
            commands::nexus_browser::close_nexus_browser,
            commands::nexus_browser::nexus_browser_action,
            commands::nexus_downloads::retry_nexus_download,
            commands::nexus_downloads::start_nexus_download,
            commands::nexus_downloads::request_nexus_dependency,
            commands::nexus_downloads::queue_nexus_downloads,
            commands::nexus_downloads::cancel_nexus_download,
            commands::nexus_downloads::register_nexus_handler,
            commands::nexus_catalog::list_nexus_catalog_games,
            commands::nexus_catalog::list_nexus_catalog_mods,
            commands::nexus_catalog::list_nexus_catalog_page,
            commands::nexus_catalog::get_nexus_catalog_mod,
            commands::nexus_translation::translate_nexus_description,
            commands::nexus_local::configure_nexus_domain,
            commands::nexus_oauth::connect_nexus_oauth,
            commands::nexus_oauth::cancel_nexus_oauth,
            commands::nexus_oauth::nexus_oauth_status,
            commands::nexus_account::connect_nexus_account,
            commands::nexus_account::load_nexus_account,
            commands::nexus_account::disconnect_nexus_account,
            commands::discord_account::connect_discord_account,
            commands::discord_account::load_discord_account,
            commands::discord_account::cached_discord_account,
            commands::discord_account::disconnect_discord_account,
            commands::matrix_chat::connect_matrix_chat,
            commands::matrix_chat::disconnect_matrix_chat,
            commands::matrix_chat::matrix_chat_status,
            commands::matrix_rtc_probe::open_matrix_rtc_probe,
            commands::matrix_rtc_probe::matrix_rtc_probe_environment,
            commands::matrix_rtc_probe::matrix_rtc_probe_report,
            commands::matrix_voice::matrix_voice_start,
            commands::matrix_voice::matrix_voice_stop,
            commands::matrix_rtc::matrix_rtc_identity,
            commands::matrix_rtc::matrix_rtc_request,
            commands::matrix_rtc::matrix_room_call_prepare,
            commands::matrix_audio::matrix_audio_devices,
            commands::matrix_audio::matrix_audio_route,
            commands::matrix_call::matrix_room_call_start,
            commands::matrix_call::matrix_room_call_key,
            commands::matrix_call::matrix_call_start,
            commands::matrix_call::matrix_call_stop,
            commands::matrix_call::matrix_call_controls,
            commands::matrix_chat::matrix_chat_rooms,
            commands::matrix_chat::matrix_chat_join,
            commands::matrix_chat::matrix_chat_create_channel,
            commands::matrix_chat::matrix_chat_delete_channel,
            commands::matrix_chat::matrix_chat_premium_access,
            commands::official_build::official_build_status,
            commands::matrix_chat::matrix_chat_moderate,
            commands::matrix_chat::matrix_chat_sync,
            commands::matrix_chat::matrix_chat_game_activity,
            commands::matrix_chat::matrix_chat_history,
            commands::matrix_chat::matrix_chat_ignore,
            commands::matrix_chat::matrix_chat_report,
            commands::matrix_chat::matrix_chat_send,
            commands::matrix_chat::matrix_chat_read,
            commands::matrix_chat::matrix_chat_typing,
            commands::matrix_chat::matrix_chat_edit,
            commands::matrix_chat::matrix_chat_react,
            commands::matrix_chat::matrix_profile_set_name,
            commands::matrix_chat::matrix_profile_set_avatar,
            commands::matrix_chat::matrix_chat_avatar,
            commands::matrix_chat::matrix_chat_upload,
            commands::matrix_chat::matrix_chat_image,
            commands::mod_backup::load_mod_backup_settings,
            commands::mod_backup::nexus_backup_status,
            commands::mod_backup::set_mod_backup_settings,
            commands::mod_backup::set_nexus_game_backup,
            commands::mod_backup::delete_nexus_game_backup,
            commands::mod_backup::create_mod_backup,
            commands::mod_backup::restore_mod_backup,
            commands::mod_backup::recover_mods_from_disk,
            commands::nexus_local::load_nexus_workspace,
            commands::nexus_local::scan_nexus_library,
            commands::nexus_local::add_nexus_game,
            commands::nexus_local::remove_nexus_game,
            commands::nexus_local::inspect_nexus_game,
            commands::nexus_local::plan_nexus_installation,
            commands::nexus_local::nexus_package_layout,
            commands::nexus_local::set_nexus_manual_root,
            commands::nexus_loot::nexus_loot_status,
            commands::nexus_loot::update_nexus_loot_masterlist,
            commands::nexus_load_order::nexus_load_order,
            commands::nexus_gamebryo::nexus_game_checks,
            commands::nexus_plugins::nexus_plugins,
            commands::nexus_plugins::set_nexus_plugin_enabled,
            commands::nexus_plugins::set_nexus_plugin_order,
            commands::nexus_plugins::set_nexus_plugin_autosort,
            commands::nexus_plugins::sort_nexus_plugins,
            commands::nexus_plugins::set_nexus_plugin_rule,
            commands::nexus_plugins::set_nexus_plugin_group,
            commands::nexus_plugins::add_nexus_plugin_group,
            commands::nexus_plugins::set_nexus_plugin_ghost,
            commands::nexus_plugins::set_nexus_plugin_light,
            commands::nexus_gamebryo::set_nexus_game_file_aside,
            commands::nexus_gamebryo::set_nexus_bsa_redirection,
            commands::nexus_gamebryo::ignore_nexus_extender_errors,
            commands::nexus_gamebryo::download_nexus_loader,
            commands::nexus_archive_order::nexus_archive_order,
            commands::nexus_archive_order::set_nexus_archive_order,
            commands::nexus_load_order::set_nexus_load_order,
            commands::nexus_load_order::set_nexus_load_order_auto_export,
            commands::nexus_load_order::import_nexus_load_order,
            commands::nexus_load_order::export_nexus_load_order,
            commands::nexus_local::set_nexus_mod_replace_originals,
            commands::nexus_user_profiles::nexus_user_profiles,
            commands::nexus_user_profiles::create_nexus_user_profile,
            commands::nexus_user_profiles::rename_nexus_user_profile,
            commands::nexus_user_profiles::delete_nexus_user_profile,
            commands::nexus_user_profiles::switch_nexus_user_profile,
            commands::nexus_user_profiles::set_nexus_profile_options,
            commands::nexus_saves::nexus_saves,
            commands::nexus_saves::nexus_save_screenshot,
            commands::nexus_saves::delete_nexus_saves,
            commands::nexus_saves::transfer_nexus_saves,
            commands::nexus_saves::nexus_other_saves,
            commands::nexus_saves::restore_nexus_save_plugins,
            commands::nexus_local::move_nexus_mod,
            commands::nexus_local::nexus_mod_overrides,
            commands::nexus_collections::load_nexus_collection,
            commands::nexus_collections::install_nexus_collection,
            commands::nexus_modules::list_nexus_modules,
            commands::nexus_local::set_nexus_game_cover,
            commands::nexus_local::import_nexus_archive,
            commands::nexus_local::remove_nexus_archive,
            commands::nexus_local::install_nexus_mod,
            commands::nexus_local::set_nexus_mod_enabled,
            commands::nexus_launch_options::get_nexus_launch_options,
            commands::mod_launch_options::apply_mod_launch_options,
            commands::nexus_launch_options::configure_nexus_advanced,
            commands::nexus_launch_options::configure_nexus_launch_options,
            commands::nexus_local::launch_nexus_game,
            commands::nexus_local::open_nexus_game_tool,
            commands::nexus_local::stop_nexus_game,
            commands::nexus_local::read_nexus_logs,
            commands::nexus_local::read_nexus_mod_logs,
            commands::nexus_reports::export_nexus_report,
            commands::nexus_reports::send_nexus_report,
            commands::nexus_reports::nexus_report_relay_configured,
            commands::browser::open_browser_url,
            commands::nexus_local::configure_nexus_proton,
            commands::nexus_local::configure_nexus_settings,
            commands::nexus_local::configure_nexus_deploy,
            commands::nexus_local::nexus_deploy_options,
            commands::pelinstall::get_startup_context,
            commands::pelinstall::take_launcher_open_request,
            commands::pelinstall::find_pelinstall_matches,
            commands::game_installation::repair_peligames_entry,
            commands::pelinstall::validate_pelinstall_file,
            commands::pelinstall::create_peligames_shortcut,
            commands::pelinstall::find_peligames_shortcuts,
            commands::pelinstall::appimage_install_status,
            commands::pelinstall::install_peligames_appimage,
            commands::pelinstall::get_pelinstall_icon,
            commands::pelinstall::set_peligames_cover,
            commands::pelinstall::remove_peligames_shortcuts,
            commands::pelinstall::update_peligames_shortcuts,
            commands::pelinstall::open_peligames_launcher,
            commands::pelinstall::launch_pelinstall_entry,
            commands::gpu::detect_linux_gpus,
            commands::proton::scan_installed_protons,
            commands::installation_paths::prepare_default_installation_directory,
            commands::game_installation::run_game_installer,
            commands::launcher_installation::install_launcher,
            commands::runtime_activity::get_runtime_activity,
            commands::launcher_installation::cancel_launcher_installation,
            commands::game_installation::cancel_game_installer,
            commands::game_installation::run_wine_tool,
            commands::game_installation::add_peligames_entry,
            commands::game_installation::uninstall_peligames_entry,
            commands::game_installation::list_peligames_entries,
            commands::game_installation::update_peligames_settings,
            commands::game_installation::choose_peligames_entry,
            commands::game_installation::run_peligames_program,
            commands::game_installation::list_peligames_launcher_shortcuts,
            commands::game_installation::start_peligames_shortcut,
            commands::game_installation::peligames_profile_options,
            commands::game_installation::set_peligames_profile_kind,
            commands::game_installation::add_peligames_shortcut_to_library,
            commands::game_installation::remove_peligames_shortcut_from_library,
            commands::game_installation::new_peligames_launcher_games,
            commands::game_installation::add_peligames_launcher_games,
            commands::game_installation::list_peligames_executable_choices,
            commands::game_installation::start_peligames_game,
            commands::game_installation::get_peligames_execution,
            commands::game_installation::get_peligames_game_logs,
            commands::game_installation::cancel_peligames_game,
            commands::proton::check_proton_release,
            commands::proton::install_proton,
            commands::proton::cancel_proton_install,
            commands::proton::uninstall_proton,
            commands::neural_settings::get_neural_startup,
            commands::neural_settings::set_neural_startup,
            commands::app_updates::check_launcher_update,
            commands::app_updates::launcher_experimental_updates,
            commands::app_updates::set_launcher_experimental_updates,
            commands::app_updates::download_launcher_update,
            commands::app_updates::restart_launcher_update,
            commands::installer::install_mod,
            commands::installer::extract_dll,
            commands::installer::extract_dll_wizard,
            commands::installer::check_cached_bin,
            commands::installer::check_game_installation,
            commands::installer::get_installed_dll,
            commands::installer::get_game_installation_details,
            commands::installer::update_shortcut_key_in_game,
            commands::updater::update_backend,
            commands::updater::copy_local_backend,
            commands::updater::delete_backend,
            commands::updater::cancel_update,
            commands::updater::check_backend_version,
            commands::updater::get_backend_path,
            commands::updater::open_backend_folder,
            commands::covers::cover_directories,
            commands::covers::import_game_cover,
            commands::covers::cache_automatic_game_cover,
            commands::mangohud::load_mangohud_profile,
            commands::mangohud::save_mangohud_profile,
            commands::mangohud::update_mangohud_preview,
            commands::mangohud::mangohud_preview,
            commands::mangohud_install::mangohud_downloaded,
            commands::mangohud_install::download_mangohud,
            commands::mangohud_install::cancel_mangohud_download,
            commands::mangohud_install::install_mangohud,
            commands::mangohud::mangohud_status,
            commands::mangohud::mangohud_latest_release,
            commands::storage_automount::list_storage_volumes,
            commands::storage_automount::save_storage_automount,
            commands::storage_automount::format_ntfs_storage,
            commands::storage_automount::prepare_empty_storage,
            commands::storage_automount::manage_storage_volume,
            commands::external_library::scan_external_library,
            commands::game_activity::game_activity,
            commands::external_library::configure_external_game,
            commands::external_library::launch_external_game,
            commands::scanner::scan_installed_games,
            commands::scanner::fetch_steam_release_date,
            commands::scanner::launch_game,
            commands::scanner::start_steam_game,
            commands::scanner::request_steam_uninstall,
            commands::scanner::open_folder,
            commands::scanner::collect_and_open_logs,
            commands::scanner::fetch_steamgriddb_cover_command,
            commands::scanner::fetch_game_banners,
            commands::analyzer::analyze_game,
            commands::config::load_app_config,
            commands::config::get_launcher_log,
            commands::log_windows::open_game_log_window,
            commands::log_windows::set_log_window_on_top,
            commands::config::save_app_config,
            commands::config::migrate_neural_preferences,
            commands::config::load_neural_preferences,
            commands::config::delete_app_config,
            commands::config::save_custom_game_path,
            commands::config::clear_game_caches,
            commands::config::log_cached_library,
            commands::config::emergency_reset_launcher,
            commands::config::restart_after_emergency_reset
        ]);
            move |invoke: tauri::ipc::Invoke<tauri::Wry>| {
                if invoke.message.webview().label().starts_with("nexus-browser-") {
                    invoke.resolver.reject("A página Nexus não tem acesso aos comandos do launcher.");
                    true
                } else { handler(invoke) }
            }
        })
        .build(context)
        .expect("error while building tauri application")
        .run(|_, event| {
            if matches!(event, tauri::RunEvent::Exit) { commands::mangohud::stop_preview(); commands::matrix_call::stop(); commands::matrix_voice::stop(); }
        });
}
