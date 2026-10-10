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

fn run_mode(installer: bool) {
    #[cfg(unix)]
    if let Err(error) = core::launcher_instance::wait_for_restart_parent() { eprintln!("{error}"); return; }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--verify-ui"] {
        let context = app_context();
        let assets: Vec<String> = context.assets().iter().map(|(name, _)| name.to_string()).collect();
        let embedded = !tauri::is_dev()
            && assets.iter().any(|name| name.trim_start_matches('/') == "index.html")
            && assets.iter().any(|name| name.ends_with(".js"))
            && assets.iter().any(|name| name.ends_with(".css"));
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
    let mut context = app_context();
    if startup.module != "launcher" {
        if let Some(window) = context.config_mut().app.windows.first_mut() {
            window.title = if startup.module == "launch-error" { "PeliGames — execução" } else { "Pelinstall" }.into();
            window.width = 560.; window.height = if startup.module == "pelinstall" { 340. } else { 490. };
            window.min_width = Some(480.); window.min_height = Some(if startup.module == "pelinstall" { 260. } else { 420. });
            window.max_width = Some(560.); window.max_height = Some(490.);
            window.maximizable = false;
            window.center = true;
        }
    }
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
            crate::core::logger::log_launcher(
                &app_handle,
                "INFO",
                &format!("PeliGames v{} iniciado", env!("CARGO_PKG_VERSION")),
            );
            commands::app_updates::cleanup_obsolete_updates(&app_handle);
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
            commands::nexus_account::connect_nexus_account,
            commands::nexus_account::load_nexus_account,
            commands::nexus_account::disconnect_nexus_account,
            commands::mod_backup::load_mod_backup_settings,
            commands::mod_backup::set_mod_backup_settings,
            commands::mod_backup::set_nexus_game_backup,
            commands::mod_backup::delete_nexus_game_backup,
            commands::mod_backup::create_mod_backup,
            commands::mod_backup::restore_mod_backup,
            commands::mod_backup::recover_mods_from_disk,
            commands::nexus_local::load_nexus_workspace,
            commands::nexus_local::scan_nexus_library,
            commands::nexus_local::add_nexus_game,
            commands::nexus_local::inspect_nexus_game,
            commands::nexus_local::plan_nexus_installation,
            commands::nexus_modules::list_nexus_modules,
            commands::nexus_local::set_nexus_game_cover,
            commands::nexus_local::import_nexus_archive,
            commands::nexus_local::remove_nexus_archive,
            commands::nexus_local::install_nexus_mod,
            commands::nexus_local::set_nexus_mod_enabled,
            commands::nexus_local::launch_nexus_game,
            commands::nexus_local::open_nexus_game_tool,
            commands::nexus_local::stop_nexus_game,
            commands::nexus_local::read_nexus_logs,
            commands::nexus_local::read_nexus_mod_logs,
            commands::nexus_reports::export_nexus_report,
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
            commands::game_installation::cancel_game_installer,
            commands::game_installation::run_wine_tool,
            commands::game_installation::add_peligames_entry,
            commands::game_installation::uninstall_peligames_entry,
            commands::game_installation::list_peligames_entries,
            commands::game_installation::update_peligames_settings,
            commands::game_installation::start_peligames_game,
            commands::game_installation::get_peligames_execution,
            commands::game_installation::get_peligames_game_logs,
            commands::game_installation::cancel_peligames_game,
            commands::proton::check_proton_release,
            commands::proton::install_proton,
            commands::proton::cancel_proton_install,
            commands::neural_settings::get_neural_startup,
            commands::neural_settings::set_neural_startup,
            commands::app_updates::check_launcher_update,
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
            commands::covers::import_game_cover,
            commands::scanner::scan_installed_games,
            commands::scanner::fetch_steam_release_date,
            commands::scanner::launch_game,
            commands::scanner::request_steam_uninstall,
            commands::scanner::open_folder,
            commands::scanner::collect_and_open_logs,
            commands::scanner::fetch_steamgriddb_cover_command,
            commands::analyzer::analyze_game,
            commands::config::load_app_config,
            commands::config::get_launcher_log,
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
        .run(context)
        .expect("error while running tauri application");
}
