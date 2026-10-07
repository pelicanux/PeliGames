#[path = "../../../src-tauri/src/core/game_logs.rs"]
mod game_logs;
#[path = "../../../src-tauri/src/core/execution_registry.rs"]
mod execution_registry;
#[path = "../../../src-tauri/src/core/installation_targets.rs"]
mod installation_targets;
#[path = "../../../src-tauri/src/core/executable_icon.rs"]
mod executable_icon;
#[path = "../../../src-tauri/src/core/umu_manager.rs"]
mod umu_manager;
#[path = "../../../src-tauri/src/core/pelinstall.rs"]
mod pelinstall;
#[path = "../../../src-tauri/src/core/wine_tools.rs"]
mod wine_tools;
#[path = "../../../src-tauri/src/core/advanced_settings.rs"]
mod advanced_settings;
#[path = "../../../src-tauri/src/core/game_execution.rs"]
mod game_execution;
// Development browser bridge and standalone validation of the same Rust backend.
#[path = "../../../src-tauri/src/core/game_installation.rs"]
mod game_installation;
#[path = "../../../src-tauri/src/core/installation_layout.rs"]
mod installation_layout;
#[path = "../../../src-tauri/src/core/installed_library.rs"]
mod installed_library;
#[path = "../../../src-tauri/src/core/proton_manager.rs"]
mod manager;
use std::{
    io::{BufRead, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
fn output(value: serde_json::Value) {
    println!("{value}");
    let _ = std::io::stdout().flush();
}
#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = async {
        if args.get(1).map(String::as_str) == Some("game-logs") { return serde_json::to_value(game_logs::read(args.get(2).ok_or("Jogo ausente.")?)?).map_err(|e|e.to_string()); }
        if args.get(1).map(String::as_str) == Some("execution-status") { return serde_json::to_value(game_execution::status()?).map_err(|e| e.to_string()); }
        if args.get(1).map(String::as_str) == Some("cancel-execution") { return serde_json::to_value(game_execution::cancel(args.get(2).ok_or("Sessão ausente.")?)?).map_err(|e| e.to_string()); }
        if args.get(1).map(String::as_str) == Some("installation-targets") {
            let hints = installation_targets::discover(std::path::Path::new(args.get(2).ok_or("Prefixo ausente.")?));
            return Ok(serde_json::json!(hints.into_iter().map(|(path,h)| serde_json::json!({"executable":path,"source":h.source,"arguments":h.arguments})).collect::<Vec<_>>()));
        }
        if args.get(1).map(String::as_str) == Some("executable-icon") {
            return serde_json::to_value(executable_icon::extract_png(std::path::Path::new(args.get(2).ok_or("Executável ausente.")?)).ok()).map_err(|e|e.to_string());
        }
        if args.get(1).map(String::as_str) == Some("extract-icon") {
            let exe = std::path::Path::new(args.get(2).ok_or("Executável ausente.")?);
            let directory = std::path::Path::new(args.get(3).ok_or("Destino ausente.")?);
            let path = executable_icon::shortcut_icon_at(exe, directory).map_err(|e| e.to_string())?;
            return Ok(serde_json::json!({"icon": path}));
        }
        if args.get(1).map(String::as_str) == Some("pelinstall-matches") {
            return serde_json::to_value(installed_library::find_executable(args.get(2).ok_or("Executável ausente.")?)?).map_err(|e| e.to_string());
        }
        if args.get(1).map(String::as_str) == Some("repair-game") {
            let request = serde_json::from_str(args.get(2).ok_or("Configuração ausente.")?).map_err(|e| e.to_string())?;
            return serde_json::to_value(game_installation::repair(request)?).map_err(|e| e.to_string());
        }
        if args.get(1).map(String::as_str) == Some("pelinstall-info") {
            return serde_json::to_value(pelinstall::parse_args(&args[2..], true)?).map_err(|e|e.to_string());
        }
        if args.get(1).map(String::as_str) == Some("launch-shortcut") {
            return match pelinstall::monitor_background(args.get(2).ok_or("Entrada ausente.")?) {
                Ok(()) => Ok(serde_json::json!({"state":"exited"})),
                Err(report) => { pelinstall::save_report(&report)?; Err(report.error) }
            };
        }
        if args.get(1).map(String::as_str) == Some("install-umu") {
            return Ok(serde_json::json!({"path":umu_manager::ensure()?.to_string_lossy()}));
        }
        if args.get(1).map(String::as_str) == Some("wine-tools") {
            let request = serde_json::from_str(args.get(2).ok_or("Configuração ausente.")?).map_err(|e|e.to_string())?;
            return serde_json::to_value(wine_tools::execute(request, Arc::new(|line| output(serde_json::json!({"progress":line})))).await?).map_err(|e|e.to_string());
        }
        if args.get(1).map(String::as_str) == Some("add-game") {
            let request = serde_json::from_str(args.get(2).ok_or("Configuração ausente.")?)
                .map_err(|e| e.to_string())?;
            return serde_json::to_value(installed_library::add(request)?)
                .map_err(|e| e.to_string());
        }
        if args.get(1).map(String::as_str) == Some("uninstall-game") {
            installed_library::uninstall(args.get(2).ok_or("Entrada ausente.")?.clone())?;
            return Ok(serde_json::json!(null));
        }
        if args.get(1).map(String::as_str) == Some("update-game") {
            let settings = serde_json::from_str(args.get(2).ok_or("Configuração ausente.")?)
                .map_err(|e| e.to_string())?;
            return serde_json::to_value(installed_library::update_settings(settings)?)
                .map_err(|e| e.to_string());
        }
        if args.get(1).map(String::as_str) == Some("list-games") {
            return serde_json::to_value(installed_library::list(
                args.get(2).map(String::as_str) == Some("rescan"),
            )?)
            .map_err(|e| e.to_string());
        }
        if args.get(1).map(String::as_str) == Some("monitor-game") {
            let session = game_execution::start_program(
                args.get(2).ok_or("Executável ausente.")?.clone(),
                args.get(3).cloned(),
            )?;
            let id = session.id.clone();
            std::thread::spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    if matches!(line.as_deref(), Ok("cancel")) {
                        let _ = game_execution::cancel(&id);
                        break;
                    }
                }
            });
            let mut previous = String::new();
            loop {
                let current = game_execution::local_status()?.ok_or("Execução indisponível.")?;
                let encoded = serde_json::to_string(&current).map_err(|e| e.to_string())?;
                if encoded != previous {
                    output(serde_json::json!({"execution": current}));
                    previous = encoded;
                }
                if !current.active() {
                    return Ok(serde_json::json!({}));
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        if args.get(1).map(String::as_str) == Some("launch-game") {
            game_installation::launch(args.get(2).ok_or("Executável ausente.")?.clone())?;
            return Ok(serde_json::json!({}));
        }
        if args.get(1).map(String::as_str) == Some("run-game") {
            let request = serde_json::from_str(args.get(2).ok_or("Configuração ausente.")?)
                .map_err(|e| e.to_string())?;
            return serde_json::to_value(game_installation::run(request)?)
                .map_err(|e| e.to_string());
        }
        let family = serde_json::from_value(args.get(2).ok_or("Família ausente.")?.clone().into())
            .map_err(|e| e.to_string())?;
        let root = match args.get(3) {
            Some(path) => path.into(),
            None => manager::runners_dir()?,
        };
        match args.get(1).map(String::as_str) {
            Some("check") => Ok(
                serde_json::to_value(manager::check_at(family, &root).await?)
                    .map_err(|e| e.to_string())?,
            ),
            Some("install") => {
                let flag = Arc::new(AtomicBool::new(false));
                let input_flag = flag.clone();
                std::thread::spawn(move || {
                    for line in std::io::stdin().lock().lines() {
                        if line.is_ok() {
                            input_flag.store(true, Ordering::Relaxed);
                            break;
                        }
                    }
                });
                let path = manager::install_at(
                    family,
                    root,
                    flag,
                    Arc::new(|event| output(serde_json::json!({"progress": event}))),
                )
                .await?;
                Ok(serde_json::json!({"path": path}))
            }
            _ => Err("Comando inválido.".to_string()),
        }
    }
    .await;
    match result {
        Ok(result) => output(serde_json::json!({"result": result})),
        Err(error) => {
            output(serde_json::json!({"error": error}));
            std::process::exit(1);
        }
    }
}
