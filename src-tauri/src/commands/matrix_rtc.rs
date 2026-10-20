//! Matrix SDK HTTP bridge: the account access token never leaves Rust.
use super::matrix_chat::{self, API};
use serde_json::{json, Value};
use sha2::Digest;
use std::time::Duration;
use tauri::WebviewWindow;

static STORE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn guard(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Janela Matrix inválida.".into())
    }
}
// Shared pool for /sync and every SDK request: reuses the TLS connections
// instead of a new handshake with matrix.org on each one.
fn client() -> Result<reqwest::Client, String> {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client.clone());
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(55))
        .build()
        .map_err(|_| "Conexão Matrix indisponível.")?;
    Ok(CLIENT.get_or_init(|| client).clone())
}
// whoami never changes for an access token: one lookup per sign-in instead of
// two on every voice join.
static WHOAMI: tokio::sync::Mutex<Option<(String, Value)>> = tokio::sync::Mutex::const_new(None);
async fn identity(token: &str) -> Result<Value, String> {
    if let Some((cached_token, value)) = WHOAMI.lock().await.as_ref() {
        if cached_token == token {
            return Ok(value.clone());
        }
    }
    let value = whoami(token).await?;
    *WHOAMI.lock().await = Some((token.to_string(), value.clone()));
    Ok(value)
}
async fn whoami(token: &str) -> Result<Value, String> {
    let response = client()?
        .get(format!("{API}/_matrix/client/v3/account/whoami"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "Não foi possível verificar a conta Matrix.")?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        if matrix_chat::session()
            .await
            .is_ok_and(|account| account.access_token == token)
        {
            let _ = matrix_chat::forget().await;
        }
        return Err("Sessão Matrix expirada. Reconecte sua conta.".into());
    }
    if !response.status().is_success() {
        return Err("Não foi possível validar a conta Matrix.".into());
    }
    response
        .json()
        .await
        .map_err(|_| "Identidade Matrix inválida.".into())
}
#[tauri::command]
pub async fn matrix_rtc_identity(window: WebviewWindow) -> Result<Value, String> {
    guard(&window)?;
    let account = matrix_chat::session().await?;
    let value = identity(&account.access_token).await?;
    if value["user_id"] != account.user_id || value["device_id"].as_str().is_none() {
        return Err("Reconecte sua conta para registrar o dispositivo de voz.".into());
    }
    let _store_lock = STORE_LOCK.lock().await;
    let password = tauri::async_runtime::spawn_blocking(|| -> Result<String, String> {
        let purpose = "matrix-rtc-crypto-store";
        if let Some(secret) =
            super::nexus_account::credential_for("lookup", None, purpose, "PeliGames — Matrix RTC")?
        {
            return Ok(secret);
        }
        let secret = format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        super::nexus_account::credential_for(
            "store",
            Some(&secret),
            purpose,
            "PeliGames — Matrix RTC",
        )?;
        Ok(secret)
    })
    .await
    .map_err(|_| "Chaveiro de criptografia indisponível.")??;
    let storage_key = sha2::Sha256::digest(password.as_bytes()).to_vec();
    Ok(
        json!({"user_id":account.user_id,"device_id":value["device_id"],"base_url":API,"storage_key":storage_key,"temporary":super::credential_storage::temporary("matrix")?}),
    )
}

fn allowed(url: &reqwest::Url, method: &str) -> bool {
    if url.origin().ascii_serialization() != API
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }
    let path = url.path();
    if !path.starts_with("/_matrix/client/") || url.query_pairs().any(|(k, _)| k == "access_token")
    {
        return false;
    }
    // SDK discovery, sync, room state, device keys. No login, logout or token export.
    if path.contains("/login")
        || path.contains("/logout")
        || path.contains("/openid/")
        || path.contains("/register")
    {
        return false;
    }
    match method {
        "GET" => true,
        "POST" => {
            path.contains("/keys/")
                || path.ends_with("/filter")
                || path.contains("/delayed_events/")
        }
        "PUT" => {
            path.contains("/keys/")
                || path.contains("/sendToDevice/m.room.encrypted/")
                || path.contains("/state/org.matrix.msc3401.call.member/")
                || path.contains("/state/m.call.member/")
                || path.contains("/state/org.matrix.msc4143.rtc.member/")
                || path.contains("/delayed_events/")
        }
        _ => false,
    }
}
#[tauri::command]
pub async fn matrix_rtc_request(
    window: WebviewWindow,
    url: String,
    method: String,
    body: Option<String>,
    user_id: String,
) -> Result<Value, String> {
    guard(&window)?;
    let url = reqwest::Url::parse(&url).map_err(|_| "Endereço Matrix inválido.")?;
    if !allowed(&url, &method) || body.as_ref().is_some_and(|b| b.len() > 2 * 1024 * 1024) {
        return Err("Operação Matrix não permitida.".into());
    }
    let account = matrix_chat::session().await?;
    if account.user_id != user_id {
        return Err("A conta Matrix mudou.".into());
    }
    let is_sync = url.path().ends_with("/sync");
    let mut request = client()?
        .request(
            reqwest::Method::from_bytes(method.as_bytes()).map_err(|_| "Método inválido.")?,
            url,
        )
        .bearer_auth(&account.access_token);
    if let Some(body) = body {
        request = request
            .header("Content-Type", "application/json")
            .body(body);
    }
    let mut response = request
        .send()
        .await
        .map_err(|_| "Sem conexão com o Matrix.")?;
    let status = response.status().as_u16();
    if status == 401 {
        if matrix_chat::session().await?.access_token == account.access_token {
            let _ = matrix_chat::forget().await;
        }
        return Ok(
            json!({"status":401,"body":{"errcode":"M_UNKNOWN_TOKEN","error":"Sessão Matrix expirada. Reconecte sua conta."},"chat":null}),
        );
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Resposta Matrix interrompida.")?
    {
        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
            return Err("Resposta Matrix muito grande.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if matrix_chat::session().await?.user_id != user_id {
        return Err("A conta Matrix mudou.".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| "Resposta Matrix inválida.")?;
    let chat = if is_sync && status == 200 {
        Some(matrix_chat::normalize_sync(&value, &user_id))
    } else {
        None
    };
    Ok(json!({"status":status,"body":value,"chat":chat}))
}

const SERVICE: &str = "https://livekit-jwt.call.matrix.org";
/// Matrix.org's announced voice service, checked at most every ten minutes.
static DISCOVERY: tokio::sync::Mutex<Option<std::time::Instant>> = tokio::sync::Mutex::const_new(None);
async fn check_discovery(http: &reqwest::Client) -> Result<(), String> {
    let mut checked = DISCOVERY.lock().await;
    if checked.is_some_and(|at| at.elapsed() < Duration::from_secs(600)) {
        return Ok(());
    }
    let discovery: Value = http
        .get("https://matrix.org/.well-known/matrix/client")
        .send()
        .await
        .map_err(|_| "Descoberta de voz indisponível.")?
        .error_for_status()
        .map_err(|_| "Descoberta de voz indisponível.")?
        .json()
        .await
        .map_err(|_| "Descoberta inválida.")?;
    if !discovery["org.matrix.msc4143.rtc_foci"]
        .as_array()
        .is_some_and(|items| {
            items
                .iter()
                .any(|x| x["type"] == "livekit" && x["livekit_service_url"] == SERVICE)
        })
    {
        return Err("O serviço de voz anunciado mudou. Atualize o PeliGames.".into());
    }
    *checked = Some(std::time::Instant::now());
    Ok(())
}
async fn check_membership(http: &reqwest::Client, path: &str, account: &matrix_chat::Session) -> Result<(), String> {
    let membership = http
        .get(format!(
            "{API}/_matrix/client/v3/rooms/{path}/state/m.room.member/{}",
            urlencoding::encode(&account.user_id)
        ))
        .bearer_auth(&account.access_token)
        .send()
        .await
        .map_err(|_| "Não foi possível validar a sala.")?;
    if !membership.status().is_success()
        || membership
            .json::<Value>()
            .await
            .map_err(|_| "Sala inválida.")?["membership"]
            != "join"
    {
        return Err("Entre na sala Matrix antes de conectar à voz.".into());
    }
    Ok(())
}

/// Credentials fetched ahead, while Matrix confirms the call membership:
/// joining then skips the whole authorization round.
struct Prepared {
    room: String,
    user: String,
    at: std::time::Instant,
    credentials: (String, String),
}
static PREPARED: tokio::sync::Mutex<Option<Prepared>> = tokio::sync::Mutex::const_new(None);
// Held while preparing, so a join arriving meanwhile waits for that result
// instead of authorizing a second time.
static PREPARING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const PREPARED_FOR: Duration = Duration::from_secs(60);
#[tauri::command]
pub async fn matrix_room_call_prepare(window: WebviewWindow, room: String) -> Result<(), String> {
    guard(&window)?;
    let _preparing = PREPARING.lock().await;
    let user = matrix_chat::session().await?.user_id;
    let credentials = fetch_credentials(&room).await?;
    *PREPARED.lock().await = Some(Prepared { room, user, at: std::time::Instant::now(), credentials });
    Ok(())
}
/// Resolve only the configured Matrix.org service. Never forward credentials to a URL from room state.
pub(super) async fn credentials(room: &str) -> Result<(String, String), String> {
    let user = matrix_chat::session().await?.user_id;
    drop(PREPARING.lock().await);
    // Used once: a second join asks again.
    if let Some(prepared) = PREPARED.lock().await.take() {
        if prepared.room == room && prepared.user == user && prepared.at.elapsed() < PREPARED_FOR {
            return Ok(prepared.credentials);
        }
    }
    fetch_credentials(room).await
}
async fn fetch_credentials(room: &str) -> Result<(String, String), String> {
    let account = matrix_chat::session().await?;
    let path = matrix_chat::room_path(room)?;
    let http = client()?;
    // Independent checks run together instead of one after another.
    let (_, who, _) = tokio::try_join!(
        check_membership(&http, &path, &account),
        identity(&account.access_token),
        check_discovery(&http),
    )?;
    let device = who["device_id"]
        .as_str()
        .ok_or("Dispositivo Matrix indisponível.")?;
    let openid: Value = http
        .post(format!(
            "{API}/_matrix/client/v3/user/{}/openid/request_token",
            urlencoding::encode(&account.user_id)
        ))
        .bearer_auth(&account.access_token)
        .json(&json!({}))
        .send()
        .await
        .map_err(|_| "Autorização de voz indisponível.")?
        .error_for_status()
        .map_err(|_| "O Matrix recusou a autorização de voz.")?
        .json()
        .await
        .map_err(|_| "Autorização inválida.")?;
    let response = http
        .post(format!("{SERVICE}/sfu/get"))
        .json(&json!({"room":room,"device_id":device,"openid_token":openid}))
        .send()
        .await
        .map_err(|_| "O serviço de voz não respondeu.")?;
    if !response.status().is_success() {
        return Err(format!(
            "O serviço Matrix recusou a chamada (HTTP {}).",
            response.status().as_u16()
        ));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|_| "Resposta de voz inválida.")?;
    let url = value["url"].as_str().ok_or("Servidor de voz ausente.")?;
    let parsed = reqwest::Url::parse(url).map_err(|_| "Servidor de voz inválido.")?;
    if parsed.scheme() != "wss" || parsed.host_str().is_none() || !parsed.username().is_empty() {
        return Err("O serviço não forneceu conexão de voz segura.".into());
    }
    if matrix_chat::session().await?.user_id != account.user_id {
        return Err("A conta Matrix mudou.".into());
    }
    Ok((
        url.into(),
        value["jwt"]
            .as_str()
            .ok_or("Autorização de voz ausente.")?
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an existing signed-in Matrix account and PELIGAMES_MATRIX_VOICE_TEST_ROOM"]
    fn deployed_matrix_service_authorizes_joined_room_without_microphone() {
        let room =
            std::env::var("PELIGAMES_MATRIX_VOICE_TEST_ROOM").expect("explicit room required");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime.block_on(credentials(&room));
        let (url, token) = result.expect("Matrix voice authorization failed");
        assert!(url.starts_with("wss://"));
        assert_eq!(token.split('.').count(), 3);
    }
    #[test]
    fn bridge_never_exports_tokens_or_forwards_to_other_hosts() {
        for path in [
            "https://evil.test/_matrix/client/v3/sync",
            "https://matrix-client.matrix.org/_matrix/client/v3/user/u/openid/request_token",
            "https://matrix-client.matrix.org/_matrix/client/v3/sync?access_token=x",
        ] {
            assert!(!allowed(&reqwest::Url::parse(path).unwrap(), "GET"));
        }
        assert!(allowed(
            &reqwest::Url::parse(&format!(
                "{API}/_matrix/client/v3/sendToDevice/m.room.encrypted/1"
            ))
            .unwrap(),
            "PUT"
        ));
        assert!(!allowed(
            &reqwest::Url::parse(&format!(
                "{API}/_matrix/client/v3/sendToDevice/io.element.call.encryption_keys/1"
            ))
            .unwrap(),
            "PUT"
        ));
        assert!(!allowed(
            &reqwest::Url::parse(&format!(
                "{API}/_matrix/client/v3/rooms/room/send/m.room.message/1"
            ))
            .unwrap(),
            "PUT"
        ));
        assert!(allowed(
            &reqwest::Url::parse(&format!("{API}/_matrix/client/v3/sync")).unwrap(),
            "GET"
        ));
    }
}
