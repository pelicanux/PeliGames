//! Open web links using the launcher's graphical activation context.
use tauri::Manager;
pub(super) fn focus_launcher(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = handle.get_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    });
}
fn web_url(value: &str) -> Result<String, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Link inválido.".to_string())?;
    if !["https", "http"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Somente links web sem credenciais são permitidos.".into());
    }
    Ok(url.into())
}
pub(super) async fn open(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let url = web_url(&url)?;
    #[cfg(target_os = "linux")]
    {
        let (send, receive) = tokio::sync::oneshot::channel();
        let handle = app.clone();
        app.run_on_main_thread(move || {
            use gtk::prelude::*;
            let context = handle
                .get_window("main")
                .and_then(|window| window.gtk_window().ok())
                .and_then(|window| window.display().app_launch_context());
            let Some(context) = context else {
                let _ = send.send(Err("Contexto gráfico do launcher indisponível.".into()));
                return;
            };
            // Use the current input event time, or GTK's CURRENT_TIME fallback.
            context.set_timestamp(gtk::current_event_time());
            context.unsetenv("LD_LIBRARY_PATH");
            context.unsetenv("LD_PRELOAD");
            gtk::gio::AppInfo::launch_default_for_uri_async(
                &url,
                Some(&context),
                None::<&gtk::gio::Cancellable>,
                move |result| {
                    let _ =
                        send.send(result.map_err(|error| {
                            format!("Não foi possível abrir o navegador: {error}")
                        }));
                },
            );
        })
        .map_err(|e| e.to_string())?;
        receive
            .await
            .map_err(|_| "A abertura do navegador foi interrompida.".to_string())?
    }
    #[cfg(not(target_os = "linux"))]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string())
    }
}
#[tauri::command]
pub async fn open_browser_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    open(app, url).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_web_links_and_rejects_other_handlers_and_credentials() {
        assert!(web_url("https://www.nexusmods.com/repo/mods/1?tab=files").is_ok());
        for link in [
            "file:///tmp/file",
            "javascript:alert(1)",
            "nxm://repo/mods/1",
            "https://user:secret@example.org",
            "not a url",
        ] {
            assert!(web_url(link).is_err());
        }
    }
}
