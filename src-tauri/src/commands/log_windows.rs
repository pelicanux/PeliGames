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
            WebviewWindowBuilder::new(&handle, &label, WebviewUrl::App(route.into()))
                .title(format!("Logs · {}", title.chars().take(160).collect::<String>()))
                .inner_size(900., 620.).min_inner_size(480., 320.)
                .decorations(false).resizable(true).transparent(true).shadow(false)
                .background_color(tauri::webview::Color(0, 0, 0, 0))
                .build().map_err(|e| e.to_string())?;
            Ok(())
        })();
        let _ = send.send(result);
    }).map_err(|e| e.to_string())?;
    receive.await.map_err(|e| e.to_string())?
}
