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
        let launch_url = url.clone();
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
                &launch_url,
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
        let result = receive.await.map_err(|_| "A abertura do navegador foi interrompida.".to_string())?;
        if result.is_ok() { return result; }
        // Bundled GLib may point at a gio-launch-desktop helper absent on the
        // host. Let the host's own launcher resolve its default browser instead.
        tokio::task::spawn_blocking(move || open_with_host(&url)).await
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
#[cfg(target_os = "linux")]
fn open_with_host(url: &str) -> Result<(), String> {
    let mut command = std::process::Command::new("/usr/bin/xdg-open");
    configure_host_browser(&mut command, url);
    let output = command.output().map_err(|_| "Não foi possível abrir o navegador padrão pelo sistema. Verifique se xdg-utils está instalado.".to_string())?;
    if output.status.success() { Ok(()) }
    else { Err("O sistema não conseguiu abrir o navegador padrão. Verifique a associação de links HTTP/HTTPS nas configurações do sistema.".into()) }
}
#[cfg(target_os = "linux")]
fn configure_host_browser(command: &mut std::process::Command, url: &str) {
    crate::core::env_utils::host_environment(command);
    // A packaged GLib may export this private helper path into child processes.
    command.env_remove("GIO_LAUNCH_DESKTOP");
    command.arg(url).stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
}
/// Discord's remote iframe cannot call Tauri IPC. Route its user-clicked invites
/// through the host browser instead of creating an unmanaged WebKit popup.
#[cfg(target_os = "linux")]
pub(crate) fn configure_discord_widget(app: &tauri::AppHandle, view: &webkit2gtk::WebView) {
    use gtk::prelude::*;
    use webkit2gtk::{
        NavigationPolicyDecision, NavigationPolicyDecisionExt, PolicyDecisionExt,
        PolicyDecisionType, URIRequestExt, WebViewExt,
    };
    let app = app.clone();
    view.connect_decide_policy(move |_, decision, kind| {
        if kind != PolicyDecisionType::NewWindowAction {
            return false;
        }
        let Some(action) = decision
            .dynamic_cast_ref::<NavigationPolicyDecision>()
            .and_then(|policy| policy.navigation_action())
        else {
            return false;
        };
        if !action.is_user_gesture() {
            return false;
        }
        let Some(uri) = action.request().and_then(|request| request.uri()) else {
            return false;
        };
        let Ok(url) = reqwest::Url::parse(&uri) else {
            return false;
        };
        let invite = match url.host_str() {
            Some("discord.gg" | "www.discord.gg") => {
                url.path().trim_matches('/').split('/').count() == 1
                    && !url.path().trim_matches('/').is_empty()
            }
            Some("discord.com" | "www.discord.com" | "discordapp.com" | "www.discordapp.com") => {
                url.path()
                    .strip_prefix("/invite/")
                    .is_some_and(|code| !code.is_empty() && !code.contains('/'))
            }
            _ => false,
        };
        if !invite
            || url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some_and(|port| port != 443)
        {
            return false;
        }
        decision.ignore();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = open(app.clone(), url.into()).await {
                use tauri::Emitter;
                crate::core::logger::event("ERROR", &error);
                let _ = app.emit("discord-invite-error", error);
            }
        });
        true
    });
}
#[tauri::command]
pub async fn open_browser_url(app: tauri::AppHandle, url: String) -> Result<(), String> {
    open(app, url).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn host_fallback_clears_missing_helper_and_passes_url_without_shell() {
        let mut command = std::process::Command::new("xdg-open");
        command.env("GIO_LAUNCH_DESKTOP", "/missing/helper");
        let url = "https://matrix.org/login?redirectUrl=http%3A%2F%2Flocalhost&literal=$(echo bad)";
        configure_host_browser(&mut command, url);
        assert_eq!(command.get_args().collect::<Vec<_>>(), vec![std::ffi::OsStr::new(url)]);
        for key in ["GIO_LAUNCH_DESKTOP", "LD_LIBRARY_PATH", "LD_PRELOAD"] {
            assert!(command.get_envs().any(|(name, value)| name == key && value.is_none()));
        }
    }
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
