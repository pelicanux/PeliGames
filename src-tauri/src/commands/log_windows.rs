//! Independent local log monitors; they never own or block the launcher window.
use sha2::{Digest, Sha256};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
pub async fn open_game_log_window(app: tauri::AppHandle, kind: String, target: String, title: String) -> Result<(), String> {
    if !matches!(kind.as_str(), "nexus" | "game") || target.is_empty() || target.len() > 8192 {
        return Err("Destino de logs inválido.".into());
    }
    if kind == "nexus" { uuid::Uuid::parse_str(&target).map_err(|_| "Identificador de jogo inválido.")?; }
    let label = format!("game-logs-{:x}", Sha256::digest(format!("{kind}\0{target}").as_bytes()));
    let mut url = reqwest::Url::parse("http://localhost/logs.html").unwrap();
    url.query_pairs_mut().append_pair("logWindow", &kind).append_pair("target", &target).append_pair("name", &title);
    let route = format!("logs.html?{}", url.query().unwrap());
    let (send, receive) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| {
            if let Some(window) = handle.get_webview_window(&label) {
                window.unminimize().map_err(|e| e.to_string())?;
                return window.set_focus().map_err(|e| e.to_string());
            }
            #[cfg(target_os = "linux")]
            let opaque = crate::core::webkit_graphics::opaque_window();
            #[cfg(not(target_os = "linux"))]
            let opaque = false;
            let (r, g, b, a) = if opaque { let (r, g, b) = crate::core::webkit_graphics::OPAQUE_BACKGROUND; (r, g, b, 255) } else { (0, 0, 0, 0) };
            WebviewWindowBuilder::new(&handle, &label, WebviewUrl::App(route.into()))
                .title(format!("Logs · {}", title.chars().take(160).collect::<String>()))
                .inner_size(900., 620.).min_inner_size(480., 320.)
                .decorations(false).resizable(true).transparent(!opaque).shadow(false)
                .background_color(tauri::webview::Color(r, g, b, a))
                // Shown by the page once it has drawn, so it never opens empty or see-through;
                // shown here anyway if the page does not get that far.
                .visible(false)
                .build().map_err(|e| e.to_string())?;
            let window = handle.get_webview_window(&label).ok_or("Janela de logs indisponível.")?;
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                let _ = window.show();
            });
            Ok(())
        })();
        let _ = send.send(result);
    }).map_err(|e| e.to_string())?;
    receive.await.map_err(|e| e.to_string())?
}

/// Keeps a log window above the others. GTK's keep-above does nothing on
/// Wayland, so under KDE the request goes to KWin through a one-off script.
#[tauri::command]
pub async fn set_log_window_on_top(window: tauri::WebviewWindow, on_top: bool) -> Result<(), String> {
    if !window.label().starts_with("game-logs-") { return Err("Janela inválida.".into()); }
    window.set_always_on_top(on_top).map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    if kde_wayland() {
        let caption = window.title().map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || kwin_keep_above(&caption, std::process::id(), on_top))
            .await.map_err(|e| e.to_string())??;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn kde_wayland() -> bool {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var("GDK_BACKEND").map_or(true, |backend| !backend.starts_with("x11"));
    wayland && std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|desktop| desktop.split(':').any(|name| name.eq_ignore_ascii_case("KDE")))
}

/// The KWin script for `kwin_keep_above`: the window of this process with this title.
#[cfg(target_os = "linux")]
fn keep_above_script(caption: &str, pid: u32, on_top: bool) -> String {
    let caption = serde_json::to_string(caption).unwrap_or_else(|_| "\"\"".into());
    format!("const caption = {caption};\n\
        const windows = workspace.windowList ? workspace.windowList() : workspace.clientList();\n\
        for (const window of windows) {{\n\
        \x20   if (window.pid === {pid} && (window.caption === caption || window.caption.startsWith(caption + \" <\"))) window.keepAbove = {on_top};\n\
        }}\n")
}

#[cfg(target_os = "linux")]
fn kwin_keep_above(caption: &str, pid: u32, on_top: bool) -> Result<(), String> {
    use zbus::blocking::{Connection, Proxy};
    let failure = |error: zbus::Error| format!("O KWin não aceitou fixar a janela: {error}");
    let file = std::env::temp_dir().join(format!("peligames-keep-above-{}.js", uuid::Uuid::new_v4()));
    std::fs::write(&file, keep_above_script(caption, pid, on_top)).map_err(|e| e.to_string())?;
    let name = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let result = (|| {
        let connection = Connection::session().map_err(failure)?;
        let scripting = Proxy::new(&connection, "org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting").map_err(failure)?;
        let id: i32 = scripting.call("loadScript", &(file.to_string_lossy().as_ref(), name.as_str())).map_err(failure)?;
        if id < 0 { return Err("O KWin não carregou o script para fixar a janela.".to_string()); }
        let script = Proxy::new(&connection, "org.kde.KWin", format!("/Scripting/Script{id}"), "org.kde.kwin.Script").map_err(failure)?;
        let run: Result<(), String> = script.call("run", &()).map_err(failure);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _: Result<bool, _> = scripting.call("unloadScript", &(name.as_str(),));
        run
    })();
    let _ = std::fs::remove_file(&file);
    result
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn keep_above_script_matches_only_this_window_and_escapes_the_title() {
        let script = super::keep_above_script("Logs · \"Game\" </script>", 42, true);
        assert!(script.contains(r#"const caption = "Logs · \"Game\" </script>";"#));
        assert!(script.contains("window.pid === 42"));
        assert!(script.contains("window.keepAbove = true"));
        assert!(super::keep_above_script("Logs", 1, false).contains("window.keepAbove = false"));
    }
}

/// Resolve only registered log targets; the webview never supplies a folder to open.
#[tauri::command]
pub async fn open_log_folder(kind: String, target: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let folder = match kind.as_str() {
            "game" => crate::core::game_logs::folder(target.as_deref().ok_or("Jogo não informado.")?)?,
            "nexus" => {
                let game = super::nexus_local::log_game(target.as_deref().ok_or("Jogo não informado.")?)?;
                super::nexus_profile::directory(&game)?.join("logs")
            }
            "launcher" => crate::core::paths::app_root()?,
            _ => return Err("Destino de logs inválido.".into()),
        };
        if !folder.is_dir() { return Err("Ainda não há pasta de logs para esta execução.".into()); }
        super::updater::backend_folder::open(&folder)
    }).await.map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn delete_game_logs(kind: String, target: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        match kind.as_str() {
            "game" => {
                let entry=crate::core::installed_library::list(false)?.into_iter().find(|entry|entry.path==target).ok_or("Jogo não registrado na biblioteca.")?;
                if crate::core::game_execution::prefix_in_use(std::path::Path::new(&entry.prefix)){return Err("Encerre o jogo antes de excluir seus logs.".into());}
                crate::core::game_logs::remove(&entry.path)
            },
            "nexus" => {
                let _lock=super::nexus_local::STORAGE.lock().map_err(|e|e.to_string())?;
                let game=super::nexus_local::log_game(&target)?;
                super::nexus_profile::idle(&game)?;
                crate::core::game_logs::remove(game.game["path"].as_str().unwrap_or(&target))?;
                let logs=super::nexus_profile::directory(&game)?.join("logs");
                crate::core::bounded_logs::remove_directory(&logs).map_err(|e|format!("Encerre o jogo antes de excluir seus logs: {e}"))
            },
            _ => Err("Destino de logs inválido.".into()),
        }
    }).await.map_err(|e|e.to_string())?
}
