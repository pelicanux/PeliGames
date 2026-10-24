//! Matrix voice shortcuts through the desktop portal; no keyboard interception.
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio_stream::StreamExt;
use zbus::{
    zvariant::{DynamicType, OwnedObjectPath, OwnedValue, Value},
    Connection, Proxy,
};
const DEST: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const IFACE: &str = "org.freedesktop.portal.GlobalShortcuts";
static CONFIGURE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct Session {
    connection: Connection,
    path: OwnedObjectPath,
    cancel: Arc<AtomicBool>,
}
static ACTIVE: Mutex<Option<Session>> = Mutex::new(None);
#[derive(Deserialize)]
pub struct Preferences {
    microphone: String,
    deafen: String,
}
fn validate(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    if value.len() > 80 {
        return Err("Atalho muito longo.".into());
    }
    let mut parts: Vec<_> = value.split('+').collect();
    let key = parts.pop().unwrap_or("");
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || parts
            .iter()
            .any(|part| !["CTRL", "ALT", "SHIFT", "LOGO"].contains(part))
        || parts
            .iter()
            .enumerate()
            .any(|(i, part)| parts[..i].contains(part))
    {
        return Err("Atalho inválido.".into());
    }
    let function = key
        .strip_prefix('F')
        .and_then(|n| n.parse::<u8>().ok())
        .is_some_and(|n| (1..=24).contains(&n));
    if !function
        && !parts
            .iter()
            .any(|part| ["CTRL", "ALT", "LOGO"].contains(part))
    {
        return Err("Use Ctrl, Alt ou Super com uma tecla.".into());
    }
    if ["ALT+F4", "CTRL+k", "LOGO+k"].contains(&value) {
        return Err("Atalho reservado pelo aplicativo ou sistema.".into());
    }
    Ok(())
}
async fn close(connection: &Connection, path: &OwnedObjectPath) {
    if let Ok(proxy) = Proxy::new(
        connection,
        DEST,
        path.as_str(),
        "org.freedesktop.portal.Session",
    )
    .await
    {
        let _: Result<(), _> = proxy.call("Close", &()).await;
    }
}
async fn stop_old() {
    let old = ACTIVE.lock().ok().and_then(|mut active| active.take());
    if let Some(old) = old {
        old.cancel.store(true, Ordering::Release);
        close(&old.connection, &old.path).await;
    }
}
async fn request<B: serde::Serialize + DynamicType>(
    connection: &Connection,
    proxy: &Proxy<'_>,
    method: &str,
    body: &B,
    token: &str,
) -> Result<HashMap<String, OwnedValue>, String> {
    let sender = connection
        .unique_name()
        .ok_or("Conexão D-Bus indisponível.")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let response = Proxy::new(
        connection,
        DEST,
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await
    .map_err(|e| e.to_string())?;
    // Subscribe first: a cached portal grant may answer before the method returns.
    let mut responses = response
        .receive_signal("Response")
        .await
        .map_err(|e| e.to_string())?;
    let returned: OwnedObjectPath = proxy
        .call(method, body)
        .await
        .map_err(|e| format!("Portal de atalhos globais indisponível: {e}"))?;
    if returned.as_str() != path {
        return Err("Resposta inesperada do portal de atalhos.".into());
    }
    let message = tokio::time::timeout(Duration::from_secs(120), responses.next())
        .await
        .map_err(|_| "O sistema não respondeu à configuração dos atalhos.")?
        .ok_or("O portal encerrou a configuração dos atalhos.")?;
    let (code, results): (u32, HashMap<String, OwnedValue>) =
        message.body().deserialize().map_err(|e| e.to_string())?;
    if code != 0 {
        return Err("A configuração global não foi autorizada pelo sistema.".into());
    }
    Ok(results)
}
#[tauri::command]
pub async fn configure_matrix_voice_shortcuts(
    app: tauri::AppHandle,
    preferences: Preferences,
) -> Result<HashMap<String, String>, String> {
    let _guard = CONFIGURE.lock().await;
    validate(&preferences.microphone)?;
    validate(&preferences.deafen)?;
    if !preferences.microphone.is_empty() && preferences.microphone == preferences.deafen {
        return Err("Escolha dois atalhos diferentes.".into());
    }
    let selected: Vec<_> = [
        (
            "microphone",
            "Matrix: alternar microfone",
            preferences.microphone,
        ),
        (
            "deafen",
            "Matrix: alternar silenciamento geral",
            preferences.deafen,
        ),
    ]
    .into_iter()
    .filter(|(_, _, trigger)| !trigger.is_empty())
    .collect();
    if selected.is_empty() {
        stop_old().await;
        return Ok(HashMap::new());
    }
    stop_old().await;
    let connection = Connection::session().await.map_err(|e| e.to_string())?;
    let proxy = Proxy::new(&connection, DEST, PATH, IFACE)
        .await
        .map_err(|e| e.to_string())?;
    let token = format!("matrix_{}", uuid::Uuid::new_v4().simple());
    let options = HashMap::from([
        ("handle_token", Value::from(token.as_str())),
        ("session_handle_token", Value::from(token.as_str())),
    ]);
    let mut result = request(&connection, &proxy, "CreateSession", &(options,), &token).await?;
    let session = String::try_from(
        result
            .remove("session_handle")
            .ok_or("Sessão de atalhos ausente.")?,
    )
    .map_err(|e| e.to_string())?;
    let path = OwnedObjectPath::try_from(session).map_err(|e| e.to_string())?;
    let token = format!("bind_{}", uuid::Uuid::new_v4().simple());
    let shortcuts: Vec<_> = selected
        .iter()
        .map(|(id, description, trigger)| {
            (
                *id,
                HashMap::from([
                    ("description", Value::from(*description)),
                    ("preferred_trigger", Value::from(trigger.as_str())),
                ]),
            )
        })
        .collect();
    let options = HashMap::from([("handle_token", Value::from(token.as_str()))]);
    let result = match request(
        &connection,
        &proxy,
        "BindShortcuts",
        &(path.clone(), shortcuts, "", options),
        &token,
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            close(&connection, &path).await;
            return Err(error);
        }
    };
    let bound: Vec<(String, HashMap<String, OwnedValue>)> = result
        .get("shortcuts")
        .ok_or("O sistema não informou os atalhos ativados.")?
        .try_clone()
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|e: zbus::zvariant::Error| e.to_string())?;
    let mut triggers = HashMap::new();
    for (id, mut details) in bound {
        if selected.iter().any(|(selected, _, _)| *selected == id) {
            if let Some(value) = details.remove("trigger_description") {
                if let Ok(label) = String::try_from(value) {
                    triggers.insert(id, label);
                }
            }
        }
    }
    if selected
        .iter()
        .any(|(id, _, _)| !triggers.contains_key(*id))
    {
        close(&connection, &path).await;
        return Err("O sistema não ativou todos os atalhos solicitados.".into());
    }
    drop(proxy);
    let cancel = Arc::new(AtomicBool::new(false));
    *ACTIVE.lock().map_err(|e| e.to_string())? = Some(Session {
        connection: connection.clone(),
        path: path.clone(),
        cancel: cancel.clone(),
    });
    tauri::async_runtime::spawn(async move {
        let result = async {
            let proxy = Proxy::new(&connection, DEST, PATH, IFACE).await?;
            let mut signals = proxy.receive_signal("Activated").await?;
            while !cancel.load(Ordering::Acquire) {
                let message =
                    match tokio::time::timeout(Duration::from_millis(500), signals.next()).await {
                        Ok(Some(message)) => message,
                        Ok(None) => break,
                        Err(_) => continue,
                    };
                if cancel.load(Ordering::Acquire) {
                    break;
                }
                let (session, id, _timestamp, _options): (
                    OwnedObjectPath,
                    String,
                    u64,
                    HashMap<String, OwnedValue>,
                ) = message.body().deserialize()?;
                if session == path && ["microphone", "deafen"].contains(&id.as_str()) {
                    use tauri::Emitter;
                    let _ = app.emit("matrix-voice-shortcut", id);
                }
            }
            Ok::<(), zbus::Error>(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("Matrix: atalho global encerrado: {error}");
        }
        if !cancel.load(Ordering::Acquire) {
            use tauri::Emitter;
            let _=app.emit("matrix-voice-shortcuts-unavailable", "A conexão com o portal de atalhos foi encerrada. Reaplique os atalhos nas configurações do Matrix.");
        }
        close(&connection, &path).await;
    });
    Ok(triggers)
}
