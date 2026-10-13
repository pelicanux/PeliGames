//! Isolated hosted Element client. Remote pages receive no launcher IPC capability.
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
const LABEL: &str = "matrix-community";
const COMMUNITY: &str = "https://app.element.io/#/room/!pNHMqrnxSAmcVFqrgf:matrix.org";
fn trusted_navigation(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
        && url.host_str().is_some_and(|host| host == "app.element.io" || host == "matrix.org" || host.ends_with(".matrix.org"))
}
#[tauri::command]
pub async fn open_matrix_community(app: tauri::AppHandle) -> Result<(), String> {
    let directory = crate::core::paths::app_root()?.join("matrix-community");
    if std::fs::symlink_metadata(&directory).is_ok_and(|meta| meta.file_type().is_symlink()) {
        return Err("Pasta da sessão Matrix inválida.".into());
    }
    std::fs::create_dir_all(&directory).map_err(|_| "Não foi possível criar a sessão Matrix.")?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).map_err(|_| "Não foi possível proteger a sessão Matrix.")?;
    }
    let (send, receive) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let result = (|| -> Result<(), String> {
            if let Some(window) = handle.get_webview_window(LABEL) {
                window.show().map_err(|_| "Não foi possível mostrar o Matrix.")?;
                window.unminimize().map_err(|_| "Não foi possível restaurar o Matrix.")?;
                return window.set_focus().map_err(|_| "Não foi possível focar o Matrix.".into());
            }
            let navigation = handle.clone();
            let popup = handle.clone();
            WebviewWindowBuilder::new(&handle, LABEL, WebviewUrl::External(reqwest::Url::parse(COMMUNITY).map_err(|_| "Endereço Matrix inválido.")?))
                .title("PeliGames · Matrix")
                .inner_size(1060., 760.).min_inner_size(640., 480.)
                .resizable(true).data_directory(directory).devtools(false)
                .disable_drag_drop_handler()
                .background_color(tauri::webview::Color(15, 20, 28, 255))
                .on_navigation(move |url| {
                    if trusted_navigation(url) { return true; }
                    // External help/SSO links use the system browser. Never allow
                    // file:, javascript:, custom application schemes or remote IPC.
                    if url.scheme() == "https" && url.username().is_empty() && url.password().is_none() {
                        let app = navigation.clone(); let target = url.to_string();
                        tauri::async_runtime::spawn(async move { let _ = super::browser::open(app, target).await; });
                    }
                    false
                })
                .on_new_window(move |url, _| {
                    if trusted_navigation(&url) {
                        if let Some(window) = popup.get_webview_window(LABEL) { let _ = window.navigate(url); }
                    } else if url.scheme() == "https" && url.username().is_empty() && url.password().is_none() {
                        let app = popup.clone(); let target = url.to_string();
                        tauri::async_runtime::spawn(async move { let _ = super::browser::open(app, target).await; });
                    }
                    tauri::webview::NewWindowResponse::Deny
                })
                .build().map_err(|_| "Não foi possível abrir o Matrix integrado. Tente abrir no navegador.".to_string())?;
            Ok(())
        })();
        let _ = send.send(result);
    }).map_err(|_| "Não foi possível iniciar a janela Matrix.")?;
    receive.await.map_err(|_| "A abertura do Matrix foi interrompida.".to_string())?
}
#[cfg(test)]
mod tests {
    use super::trusted_navigation;
    #[test]
    fn remote_client_navigation_stays_on_element_and_matrix() {
        for value in ["https://app.element.io/#/login", "https://account.matrix.org/", "https://matrix.org/"] {
            assert!(trusted_navigation(&reqwest::Url::parse(value).unwrap()));
        }
        for value in ["https://app.element.io.attacker.example/", "https://matrix.org.attacker.example/", "http://app.element.io/", "https://user@app.element.io/", "file:///tmp/test"] {
            assert!(!trusted_navigation(&reqwest::Url::parse(value).unwrap()));
        }
    }
}
