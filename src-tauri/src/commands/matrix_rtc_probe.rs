//! Local media proof, isolated from authentication and from remote pages.
use serde_json::{json, Value};
use tauri::{Manager, WebviewUrl, WebviewWindow};

#[tauri::command]
pub fn open_matrix_rtc_probe(app: tauri::AppHandle, window: WebviewWindow) -> Result<(), String> {
    if window.label() != "main" { return Err("Operação disponível somente no launcher.".into()); }
    if let Some(probe) = app.get_webview_window("matrix-rtc-probe") {
        probe.show().map_err(|e|e.to_string())?;
        return probe.set_focus().map_err(|e|e.to_string());
    }
    let mut expected_url = window.url().map_err(|e|e.to_string())?;
    expected_url.set_path("/rtc-test.html"); expected_url.set_query(None); expected_url.set_fragment(None);
    let expected = expected_url.to_string();
    let probe = tauri::WebviewWindowBuilder::new(&app,"matrix-rtc-probe",WebviewUrl::App("rtc-test.html".into()))
        .title("PeliGames · Teste de voz e captura Matrix").inner_size(850.0,720.0).build().map_err(|e|e.to_string())?;
    probe.on_window_event(|event| { if matches!(event,tauri::WindowEvent::Destroyed) { super::matrix_voice::stop(); super::matrix_call::stop_probe(); } });
    configure_permissions(&probe, expected)?;
    Ok(())
}

fn configure_permissions(probe: &WebviewWindow, expected: String) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        probe.with_webview(move |native| {
            use gtk::prelude::*;
            use webkit2gtk::{WebViewExt, SettingsExt, PermissionRequestExt};
            let view = native.inner();
            if let Some(settings) = WebViewExt::settings(&view) {
                settings.set_enable_media_stream(true);
                settings.set_enable_webrtc(true);
            }
            let reload_url = expected.clone();
            view.connect_permission_request(move |view, request| {
                if view.uri().as_deref() != Some(expected.as_str()) {
                    request.deny(); return true;
                }
                let kind = request.type_().name();
                if kind != "WebKitUserMediaPermissionRequest" && kind != "WebKitDisplayCapturePermissionRequest" {
                    request.deny(); return true;
                }
                // Native approval is per request, never remembered or automatically granted.
                let parent = view.toplevel().and_then(|widget|widget.downcast::<gtk::Window>().ok());
                let dialog = gtk::MessageDialog::new(parent.as_ref(),gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,gtk::MessageType::Question,gtk::ButtonsType::YesNo,
                    "Autorizar a captura solicitada para o teste local do PeliGames?\n\nO teste não envia áudio ou vídeo. Use Parar captura ao terminar. Para tela/janela, o portal do sistema também poderá pedir sua autorização.");
                let request = request.clone();
                let source = view.clone();
                let expected = expected.clone();
                dialog.connect_response(move |dialog, response| {
                    if response == gtk::ResponseType::Yes && source.uri().as_deref() == Some(expected.as_str()) { request.allow(); } else { request.deny(); }
                    dialog.close();
                });
                dialog.show_all(); true
            });
            // Media APIs are installed when a document is created. Reload the local
            // proof after applying settings instead of measuring the initial defaults.
            println!("MATRIX_RTC_NATIVE {}", json!({"configured_webrtc":WebViewExt::settings(&view).is_some_and(|s|s.enables_webrtc())}));
            view.load_uri(&reload_url);
        }).map_err(|e|e.to_string())?;
    }
    Ok(())
}

/// Standalone proof: no launcher lock, account load, migrations or update checks.
pub fn run_standalone() {
    #[cfg(target_os = "linux")]
    {
        use std::env;
        if env::var_os("WEBKIT_GST_ENABLE_WEBRTC").is_none() {
            env::set_var("WEBKIT_GST_ENABLE_WEBRTC", "1");
        }
    }
    let mut context = tauri::generate_context!();
    if let Some(window) = context.config_mut().app.windows.first_mut() {
        window.label = "matrix-rtc-probe".into();
        window.url = WebviewUrl::App("rtc-test.html".into());
        window.title = match std::env::var("PELIGAMES_RTC_TEST_ID").as_deref() { Ok("a") => "PeliGames · Sessão A · Chamada local", Ok("b") => "PeliGames · Sessão B · Chamada local", _ => "PeliGames · Teste local de mídia" }.into();
        window.width=850.;window.height=720.;window.min_width=Some(600.);window.min_height=Some(500.);
        window.decorations=true;window.transparent=false;
    }
    let result = tauri::Builder::default().setup(|app| {
        let probe = app.get_webview_window("matrix-rtc-probe").ok_or("Janela de teste indisponível")?;
        let expected = probe.url()?.to_string();
        configure_permissions(&probe, expected).map_err(std::io::Error::other)?;
        Ok(())
    }).on_window_event(|_,event| { if matches!(event,tauri::WindowEvent::Destroyed) { super::matrix_voice::stop(); super::matrix_call::stop_probe(); } }).invoke_handler(tauri::generate_handler![matrix_rtc_probe_environment,matrix_rtc_probe_report,super::matrix_voice::matrix_voice_start,super::matrix_voice::matrix_voice_stop,super::matrix_call::matrix_call_start,super::matrix_call::matrix_call_stop,super::matrix_call::matrix_call_controls]).run(context);
    if let Err(error) = result { eprintln!("Teste RTC: {error}"); }
}

#[tauri::command]
pub fn matrix_rtc_probe_report(window: WebviewWindow, report: Value) -> Result<(),String> {
    if window.label() != "matrix-rtc-probe" { return Err("Janela de teste inválida".into()); }
    // Log only capability flags, never device labels, credentials or media.
    println!("MATRIX_RTC_CAPABILITIES {}", json!({"WebRTC":report["WebRTC"].as_bool(),"microfoneCamera":report["microfoneCamera"].as_bool(),"capturaTela":report["capturaTela"].as_bool(),"contextoSeguro":report["contextoSeguro"].as_bool()}));
    Ok(())
}

#[tauri::command]
pub async fn matrix_rtc_probe_environment(window: WebviewWindow) -> Result<Value,String> {
    if window.label() != "matrix-rtc-probe" { return Err("Diagnóstico disponível somente na janela de teste.".into()); }
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(12)).build().map_err(|e|e.to_string())?;
    let discovery = async {
        let response = client.get("https://matrix.org/.well-known/matrix/client").send().await.map_err(|_|"Descoberta indisponível")?.error_for_status().map_err(|_|"Descoberta indisponível")?;
        if response.content_length().unwrap_or(0) > 65536 { return Err("Resposta grande demais"); }
        let bytes = response.bytes().await.map_err(|_|"Resposta inválida")?;
        if bytes.len() > 65536 { return Err("Resposta grande demais"); }
        let value:Value = serde_json::from_slice(&bytes).map_err(|_|"Resposta inválida")?;
        Ok::<_,&str>(value["org.matrix.msc4143.rtc_foci"].as_array().cloned().unwrap_or_default().into_iter()
            .filter(|focus| focus["type"] == "livekit")
            .filter_map(|focus|focus["livekit_service_url"].as_str().filter(|url|url.starts_with("https://")).map(str::to_owned)).collect::<Vec<_>>())
    }.await;
    let (transports,error) = match discovery { Ok(urls)=>(urls,None),Err(e)=>(vec![],Some(e)) };
    Ok(json!({"platform":std::env::consts::OS,"session":std::env::var("XDG_SESSION_TYPE").unwrap_or_default(),
        "desktop":std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),"rtc_transports":transports,"discovery_error":error}))
}
