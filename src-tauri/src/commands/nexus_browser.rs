//! Isolated website session: no remote IPC capability, credentials or page scripts.
use std::sync::Mutex;
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl};

#[derive(Default)]
pub struct NexusBrowser(pub Mutex<Option<Session>>);
pub struct Session {
    label: String,
    job: Option<String>,
}

fn nexus_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Link Nexus inválido.")?;
    let host = url.host_str().unwrap_or_default();
    if url.scheme() != "https"
        || !(host == "nexusmods.com" || host.ends_with(".nexusmods.com"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|p| p != 443)
    {
        return Err("O navegador interno aceita somente páginas HTTPS do Nexus Mods.".into());
    }
    Ok(url)
}

fn navigation_allowed(url: &reqwest::Url) -> bool {
    if nexus_url(url.as_str()).is_ok() {
        return true;
    }
    // WebKitGTK applies the navigation callback to iframe navigations too.
    // Turnstile needs its exact HTTPS origin and internal blank/srcdoc frames.
    if matches!(url.as_str(), "about:blank" | "about:srcdoc") {
        return true;
    }
    url.scheme() == "https"
        && url.host_str() == Some("challenges.cloudflare.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none_or(|p| p == 443)
}

fn capture(app: &tauri::AppHandle, url: &reqwest::Url) {
    let pending = app
        .state::<NexusBrowser>()
        .0
        .lock()
        .ok()
        .and_then(|s| s.as_ref().and_then(|s| s.job.clone()));
    if super::nexus_downloads::receive(app, url.as_str()).is_ok() {
        dismiss(app, None);
        // If the user browsed to a different file, leave the original request
        // retryable instead of silently stranding the authorization queue.
        if let Some(job) = pending {
            super::nexus_downloads::browser_closed(app, &job);
        }
    }
}

pub async fn open(app: tauri::AppHandle, value: String, job: Option<String>) -> Result<(), String> {
    let url = nexus_url(&value)?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = send.send(build(&handle, url, job));
    })
    .map_err(|e| e.to_string())?;
    receive
        .await
        .map_err(|_| "A abertura do navegador Nexus foi interrompida.".to_string())?
}

fn build(app: &tauri::AppHandle, url: reqwest::Url, job: Option<String>) -> Result<(), String> {
    if job.is_none() {
        let label = app
            .state::<NexusBrowser>()
            .0
            .lock()
            .map_err(|_| "Sessão Nexus indisponível.")?
            .as_ref()
            .map(|s| s.label.clone());
        if let Some(window) = label.and_then(|label| app.get_webview(&label)) {
            window.navigate(url).map_err(|e| e.to_string())?;
            let _ = window.show();
            let _ = app.emit_to(
                "main",
                "nexus-browser-session",
                Some(window.label().to_string()),
            );
            super::browser::focus_launcher(app);
            return Ok(());
        }
    }
    // Close an earlier window without marking its request as cancelled.
    dismiss(app, None);
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())?
        .join("nexus-browser");
    std::fs::create_dir_all(&directory)
        .map_err(|_| "Não foi possível criar a sessão Nexus.".to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| "Não foi possível proteger a sessão Nexus.".to_string())?;
    }
    let label = format!("nexus-browser-{}", uuid::Uuid::new_v4());
    let navigation = app.clone();
    let popup = app.clone();
    let popup_label = label.clone();
    // Configure session storage before requesting the login page.
    let main = app
        .get_window("main")
        .ok_or("Janela principal indisponível.")?;
    let builder = WebviewBuilder::new(
        &label,
        WebviewUrl::External(reqwest::Url::parse("about:blank").expect("valid blank page")),
    )
    // Keep the initial blank page and navigation canvas dark before Nexus paints.
    .background_color(tauri::webview::Color(32, 32, 32, 255))
    .data_directory(directory.clone())
    .devtools(false)
    .on_navigation(move |url| {
        if url.scheme() == "nxm" {
            capture(&navigation, url);
            return false;
        }
        navigation_allowed(url)
    })
    .on_new_window(move |url, _| {
        if url.scheme() == "nxm" {
            capture(&popup, &url);
        } else if nexus_url(url.as_str()).is_ok() {
            if let Some(window) = popup.get_webview(&popup_label) {
                let _ = window.navigate(url);
            }
        }
        tauri::webview::NewWindowResponse::Deny
    });
    let window = main
        .add_child(
            builder,
            LogicalPosition::new(0., 0.),
            LogicalSize::new(1., 1.),
        )
        .map_err(|e| format!("Não foi possível abrir o navegador interno: {e}"))?;
    *app.state::<NexusBrowser>()
        .0
        .lock()
        .map_err(|_| "Sessão Nexus indisponível.")? = Some(Session {
        label: label.clone(),
        job,
    });
    let _ = window.hide();
    #[cfg(target_os = "linux")]
    {
        use webkit2gtk::{CookieManagerExt, SettingsExt, WebContextExt, WebViewExt};
        let cookies = directory
            .join("cookies.sqlite")
            .to_string_lossy()
            .into_owned();
        window
            .with_webview(move |view| {
                let widget = view.inner();
                // Tauri's GTK child webviews are packed into a vertical Box. Move the
                // remote widget to a Fixed overlay so it cannot shrink the launcher.
                use gtk::prelude::*;
                let Some(container) = widget.parent().and_then(|p| p.downcast::<gtk::Box>().ok())
                else {
                    return;
                };
                let overlay = container
                    .children()
                    .into_iter()
                    .find_map(|widget| widget.downcast::<gtk::Overlay>().ok())
                    .unwrap_or_else(|| {
                        let overlay = gtk::Overlay::new();
                        for child in container.children() {
                            // Keep the local UI as the expanding base child.
                            if child != widget.clone().upcast::<gtk::Widget>() {
                                container.remove(&child);
                                overlay.add(&child);
                                break;
                            }
                        }
                        container.pack_start(&overlay, true, true, 0);
                        overlay.show();
                        overlay
                    });
                // Give each browser its own layer: an empty visible Fixed can
                // continue intercepting input after WebKit destroys its child.
                let layer = gtk::Fixed::new();
                layer.set_halign(gtk::Align::Start);
                layer.set_valign(gtk::Align::Start);
                layer.set_size_request(1, 1);
                overlay.add_overlay(&layer);
                layer.show();
                let weak_layer = layer.downgrade();
                widget.connect_destroy(move |_| {
                    if let Some(layer) = weak_layer.upgrade() {
                        layer.hide();
                        if let Some(parent) = layer
                            .parent()
                            .and_then(|p| p.downcast::<gtk::Container>().ok())
                        {
                            parent.remove(&layer);
                        }
                    }
                });

                if let Some(parent) = widget
                    .parent()
                    .and_then(|p| p.downcast::<gtk::Container>().ok())
                {
                    parent.remove(&widget);
                }
                layer.put(&widget, 0, 0);
                if let Some(settings) = WebViewExt::settings(&widget) {
                    settings.set_enable_javascript(true);
                    settings.set_enable_html5_local_storage(true);
                }
                if let Some(context) = view.inner().context() {
                    if let Some(manager) = context.cookie_manager() {
                        manager.set_persistent_storage(
                            &cookies,
                            webkit2gtk::CookiePersistentStorage::Sqlite,
                        );
                        manager.set_accept_policy(webkit2gtk::CookieAcceptPolicy::Always);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
    }
    if let Err(error) = window.navigate(url) {
        dismiss(app, None);
        return Err(error.to_string());
    }
    app.emit_to(
        "main",
        "nexus-browser-session",
        Some(window.label().to_string()),
    )
    .map_err(|e| e.to_string())?;
    super::browser::focus_launcher(app);
    Ok(())
}

// Take the state first so programmatic close is never treated as a user cancel.
pub fn dismiss(app: &tauri::AppHandle, job: Option<&str>) {
    let session = {
        let state = app.state::<NexusBrowser>();
        let Ok(mut session) = state.0.lock() else {
            return;
        };
        if job.is_some() && !session.as_ref().is_some_and(|s| s.job.as_deref() == job) {
            return;
        }
        session.take()
    };
    if let Some(session) = session {
        let _ = app.emit_to("main", "nexus-browser-session", Option::<String>::None);
        if let Some(window) = app.get_webview(&session.label) {
            #[cfg(target_os = "linux")]
            let _ = window.with_webview(|view| {
                use gtk::prelude::*;
                let widget = view.inner();
                widget.hide();
                if let Some(layer) = widget
                    .parent()
                    .and_then(|p| p.downcast::<gtk::Fixed>().ok())
                {
                    layer.hide();
                    if let Some(parent) = layer
                        .parent()
                        .and_then(|p| p.downcast::<gtk::Container>().ok())
                    {
                        parent.remove(&layer);
                    }
                }
            });
            let _ = window.close();
        }
    }
}

fn session_label(app: &tauri::AppHandle) -> Result<Option<String>, String> {
    Ok(app
        .state::<NexusBrowser>()
        .0
        .lock()
        .map_err(|_| "Sessão indisponível.")?
        .as_ref()
        .map(|s| s.label.clone()))
}

#[tauri::command]
pub fn nexus_browser_session(app: tauri::AppHandle) -> Result<Option<String>, String> {
    session_label(&app)
}

fn within_launcher(x: f64, y: f64, width: f64, height: f64, size: LogicalSize<f64>) -> bool {
    [x, y, width, height].iter().all(|v| v.is_finite())
        && x >= 0.
        && y >= 0.
        && width >= 1.
        && height >= 1.
        && x + width <= size.width + 1.
        && y + height <= size.height + 1.
}

#[tauri::command]
pub fn resize_nexus_browser(
    app: tauri::AppHandle,
    label: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if session_label(&app)?.as_deref() != Some(&label) {
        return Ok(());
    }
    let main = app.get_window("main").ok_or("Janela indisponível.")?;
    let size = main
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(main.scale_factor().map_err(|e| e.to_string())?);
    if !within_launcher(x, y, width, height, size) {
        return Err("Área do navegador inválida.".into());
    }
    if let Some(view) = app.get_webview(&label) {
        #[cfg(target_os = "linux")]
        view.with_webview(move |native| {
            use gtk::prelude::*;
            let widget = native.inner();
            if let Some(layer) = widget
                .parent()
                .and_then(|p| p.downcast::<gtk::Fixed>().ok())
            {
                // Bound the input layer too, rather than covering the entire
                // launcher with a transparent native widget.
                layer.set_margin_start(x.round() as i32);
                layer.set_margin_top(y.round() as i32);
                layer.set_size_request(width.round() as i32, height.round() as i32);
                layer.move_(&widget, 0, 0);
                widget.set_size_request(width.round() as i32, height.round() as i32);
                widget.show();
            }
        })
        .map_err(|e| e.to_string())?;
        #[cfg(not(target_os = "linux"))]
        view.set_bounds(tauri::Rect {
            position: LogicalPosition::new(x, y).into(),
            size: LogicalSize::new(width, height).into(),
        })
        .map_err(|e| e.to_string())?;
        view.show().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn close_nexus_browser(app: tauri::AppHandle, label: String) -> Result<(), String> {
    let pending = {
        let state = app.state::<NexusBrowser>();
        let session = state.0.lock().map_err(|_| "Sessão indisponível.")?;
        if session.as_ref().map(|s| s.label.as_str()) != Some(label.as_str()) {
            return Ok(());
        }
        session.as_ref().and_then(|s| s.job.clone())
    };
    dismiss(&app, None);
    if let Some(job) = pending {
        super::nexus_downloads::browser_closed(&app, &job);
    }
    super::browser::focus_launcher(&app);
    Ok(())
}

#[tauri::command]
pub async fn nexus_browser_action(
    app: tauri::AppHandle,
    label: String,
    action: String,
) -> Result<(), String> {
    if session_label(&app)?.as_deref() != Some(&label) {
        return Ok(());
    }
    let view = app.get_webview(&label).ok_or("Navegador indisponível.")?;
    match action.as_str() {
        "reload" => view.reload().map_err(|e| e.to_string()),
        "external" => {
            let url = nexus_url(view.url().map_err(|e| e.to_string())?.as_str())?;
            super::nexus_downloads::register_nexus_handler().await?;
            super::browser::open(app.clone(), url.to_string()).await?;
            if session_label(&app)?.as_deref() == Some(&label) {
                dismiss(&app, None);
            }
            Ok(())
        }
        _ => Err("Ação de navegador inválida.".into()),
    }
}

#[tauri::command]
pub async fn open_nexus_browser(
    app: tauri::AppHandle,
    window: tauri::Webview,
    url: String,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Comando disponível somente no launcher.".into());
    }
    open(app, url, None).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_view_stays_inside_the_launcher_content_area() {
        let size = LogicalSize::new(1200., 850.);
        assert!(within_launcher(43., 170., 1114., 640., size));
        assert!(!within_launcher(-1., 170., 1114., 640., size));
        assert!(!within_launcher(43., 170., 1200., 640., size));
        assert!(!within_launcher(43., 170., 1114., 850., size));
        assert!(!within_launcher(f64::NAN, 0., 100., 100., size));
        assert!(!within_launcher(0., 0., f64::INFINITY, 100., size));
    }
    #[test]
    fn native_capabilities_only_apply_to_the_local_webview() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/default.json")).unwrap();
        assert!(capability.get("windows").is_none());
        assert_eq!(capability["webviews"], serde_json::json!(["main"]));
        assert!(capability.get("remote").is_none());
    }
    #[test]
    fn restricts_top_level_pages_to_real_nexus_https_hosts() {
        for url in [
            "https://www.nexusmods.com/cyberpunk2077/mods/4197",
            "https://users.nexusmods.com/auth/sign_in",
        ] {
            assert!(nexus_url(url).is_ok());
        }
        for url in [
            "http://www.nexusmods.com",
            "https://nexusmods.com.evil.test",
            "https://evilnexusmods.com",
            "https://user:secret@nexusmods.com",
            "https://nexusmods.com:8080",
            "file:///tmp/a",
            "javascript:alert(1)",
        ] {
            assert!(nexus_url(url).is_err());
        }
    }
    #[test]
    fn allows_turnstile_frames_without_opening_arbitrary_origins() {
        for value in [
            "https://challenges.cloudflare.com/turnstile/v0/api.js",
            "https://challenges.cloudflare.com/cdn-cgi/challenge-platform/h/g/turnstile/",
            "about:blank",
            "about:srcdoc",
            "https://users.nexusmods.com/auth/sign_in",
        ] {
            assert!(navigation_allowed(&reqwest::Url::parse(value).unwrap()));
        }
        for value in [
            "http://challenges.cloudflare.com/",
            "https://challenges.cloudflare.com.evil.test/",
            "https://evil.cloudflare.com/",
            "https://user:secret@challenges.cloudflare.com/",
            "https://challenges.cloudflare.com:8080/",
            "about:config",
            "file:///tmp/a",
            "javascript:alert(1)",
            "https://example.com/",
        ] {
            assert!(!navigation_allowed(&reqwest::Url::parse(value).unwrap()));
        }
        // Supporting embedded challenges does not allow opening them as a Nexus page.
        assert!(nexus_url("https://challenges.cloudflare.com/").is_err());
    }
}
