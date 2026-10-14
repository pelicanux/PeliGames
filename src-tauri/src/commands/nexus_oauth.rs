//! Public desktop OAuth client: no client secret and no credentials in the webview.
use super::nexus_account::{credential_for, with_account, NexusAccount};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri_plugin_opener::OpenerExt;

const SERVICE: &str = "nexusmods-oauth";
const USERS: &str = "https://users.nexusmods.com";
const REDIRECT: &str = "http://127.0.0.1:17849/callback";
static LOGIN: tokio::sync::Mutex<Option<Arc<AtomicBool>>> = tokio::sync::Mutex::const_new(None);

fn client_id() -> &'static str {
    option_env!("PELIGAMES_NEXUS_CLIENT_ID")
        .unwrap_or("")
        .trim()
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn random_secret() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| "Não foi possível preparar um login seguro.")?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("PeliGames/", env!("CARGO_PKG_VERSION")))
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("Application-Name", "PeliGames".parse().unwrap());
            headers.insert(
                "Application-Version",
                env!("CARGO_PKG_VERSION").parse().unwrap(),
            );
            headers
        })
        .build()
        .map_err(|_| "Não foi possível conectar ao Nexus Mods.".into())
}
#[derive(Serialize, Deserialize)]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: u64,
    client_id: String,
}
async fn storage(action: &'static str, data: Option<String>) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        credential_for(action, data.as_deref(), SERVICE, "PeliGames — Nexus OAuth")
    })
    .await
    .map_err(|_| "Chaveiro indisponível.".to_string())?
}
pub(super) async fn clear() -> Result<(), String> {
    storage("clear", None).await.map(|_| ())
}
async fn save(session: &Session) -> Result<(), String> {
    let data = serde_json::to_string(session).map_err(|_| "Credenciais Nexus inválidas.")?;
    if data.len() > 60000 {
        return Err("Credenciais Nexus excederam o limite local.".into());
    }
    storage("store", Some(data)).await.map(|_| ())
}
async fn bounded_json(mut response: reqwest::Response) -> Result<serde_json::Value, String> {
    super::nexus_account::observe_response(&response);
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            400 | 401 | 403 => "O Nexus recusou a autorização. Conecte sua conta novamente.",
            429 => "Limite de consultas Nexus atingido. Aguarde antes de tentar novamente.",
            _ => "O Nexus Mods não conseguiu concluir a autorização.",
        }
        .into());
    }
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Resposta Nexus indisponível.")?
    {
        if data.len() + chunk.len() > 65536 {
            return Err("Resposta Nexus excedeu o limite.".into());
        }
        data.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&data).map_err(|_| "Resposta Nexus inválida.".into())
}
enum TokenError {
    /// The token endpoint refused the grant: the saved session can never work again.
    Refused,
    Other(String),
}
impl From<String> for TokenError {
    fn from(error: String) -> Self {
        TokenError::Other(error)
    }
}
impl From<TokenError> for String {
    fn from(error: TokenError) -> Self {
        match error {
            TokenError::Refused => "O Nexus recusou a autorização. Conecte sua conta novamente.".into(),
            TokenError::Other(error) => error,
        }
    }
}
// RFC 6749 §5.2: a refused grant (revoked, expired or reused refresh token) is
// 400 invalid_grant; 401 is a refused client. Neither recovers by retrying.
fn grant_refused(status: u16) -> bool {
    matches!(status, 400 | 401)
}
async fn exchange(
    fields: &[(&str, &str)],
    previous_refresh: Option<&str>,
) -> Result<Session, TokenError> {
    super::nexus_account::before_request().await?;
    let response = client()?
        .post(format!("{USERS}/oauth/token"))
        .form(fields)
        .send()
        .await
        .map_err(|_| "Não foi possível concluir o login Nexus. Verifique sua conexão.".to_string())?;
    if grant_refused(response.status().as_u16()) {
        super::nexus_account::observe_response(&response);
        return Err(TokenError::Refused);
    }
    let data = bounded_json(response).await?;
    Ok(parse_session(data, previous_refresh)?)
}
fn parse_session(
    data: serde_json::Value,
    previous_refresh: Option<&str>,
) -> Result<Session, String> {
    if !data["token_type"]
        .as_str()
        .is_some_and(|v| v.eq_ignore_ascii_case("bearer"))
    {
        return Err("Tipo de autorização Nexus incompatível.".into());
    }
    let valid_token =
        |s: &&str| !s.is_empty() && s.len() <= 16384 && s.bytes().all(|b| b.is_ascii_graphic());
    let access = data["access_token"]
        .as_str()
        .filter(valid_token)
        .ok_or("Token Nexus ausente.")?;
    let refresh = data["refresh_token"]
        .as_str()
        .or(previous_refresh)
        .filter(valid_token)
        .ok_or("Renovação Nexus indisponível.")?;
    // Some Nexus responses expose expiry only in the JWT. This is scheduling
    // metadata, never proof of identity: the account is fetched from userinfo.
    let expiry = data["expires_in"]
        .as_u64()
        .map(|s| now().saturating_add(s))
        .or_else(|| {
            let payload = access.split('.').nth(1)?;
            let claims: serde_json::Value =
                serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
            claims["exp"].as_u64()
        })
        .ok_or("Validade do token Nexus ausente.")?;
    if expiry <= now() {
        return Err("A autorização Nexus já expirou.".into());
    }
    Ok(Session {
        access_token: access.into(),
        refresh_token: refresh.into(),
        expires_at: expiry,
        client_id: client_id().into(),
    })
}
// Called inside with_account: refresh-token rotation and persistence survive
// caller cancellation, and cannot race logout or another account operation.
pub(super) async fn access(force_refresh: bool) -> Result<Option<String>, String> {
    let Some(data) = storage("lookup", None).await? else {
        return Ok(None);
    };
    let mut session: Session =
        serde_json::from_str(&data).map_err(|_| "Reconecte sua conta Nexus.")?;
    if client_id().is_empty() || session.client_id != client_id() {
        return Err("O registro OAuth mudou. Reconecte sua conta Nexus.".into());
    }
    if force_refresh || session.expires_at <= now().saturating_add(60) {
        let renewed = exchange(
            &[
                ("grant_type", "refresh_token"),
                ("client_id", client_id()),
                ("refresh_token", &session.refresh_token),
            ],
            Some(&session.refresh_token),
        )
        .await;
        session = match renewed {
            Ok(session) => session,
            // Revoked on the site or expired: drop the dead session instead of
            // sending its refresh token again on every request.
            Err(TokenError::Refused) => {
                clear().await?;
                return Err("A autorização Nexus foi encerrada. Conecte sua conta novamente.".into());
            }
            Err(error) => return Err(error.into()),
        };
        save(&session).await?;
    }
    Ok(Some(session.access_token))
}
async fn account_response(token: &str, endpoint: &str) -> Result<reqwest::Response, String> {
    super::nexus_account::before_request().await?;
    client()?
        .get(endpoint)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| "Não foi possível verificar a conta Nexus.".into())
}
fn parse_account(data: serde_json::Value) -> Result<NexusAccount, String> {
    let id = data["sub"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| data["sub"].as_u64())
        .filter(|id| *id > 0)
        .ok_or("Identificação Nexus inválida.")?;
    let name = data["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Nome Nexus ausente.")?;
    let roles = data["membership_roles"]
        .as_array()
        .ok_or("Informações de assinatura Nexus ausentes.")?;
    Ok(NexusAccount {
        user_id: id,
        name: name.into(),
        is_premium: roles.iter().any(|v| v.as_str() == Some("premium")),
        is_supporter: roles.iter().any(|v| v.as_str() == Some("supporter")),
    })
}
async fn account(token: &str) -> Result<NexusAccount, String> {
    let response = account_response(token, &format!("{USERS}/oauth/userinfo")).await?;
    parse_account(bounded_json(response).await?)
}

// The profile endpoint uses the same one-refresh limit as catalog requests.
// Fresh login uses account() instead, so it never refreshes a different saved account.
async fn account_with_refresh<F, R>(
    token: String,
    endpoint: &str,
    refresh: F,
) -> Result<NexusAccount, String>
where
    F: FnOnce() -> R,
    R: std::future::Future<Output = Result<String, String>>,
{
    let mut response = account_response(&token, endpoint).await?;
    super::nexus_account::observe_response(&response);
    if response.status().as_u16() == 401 {
        drop(response);
        let renewed = refresh().await?;
        response = account_response(&renewed, endpoint).await?;
    }
    parse_account(bounded_json(response).await?)
}

pub(super) async fn load_account() -> Result<Option<NexusAccount>, String> {
    let Some(token) = access(false).await? else {
        return Ok(None);
    };
    account_with_refresh(token, &format!("{USERS}/oauth/userinfo"), || async {
        access(true)
            .await?
            .ok_or_else(|| "Reconecte sua conta Nexus.".into())
    })
    .await
    .map(Some)
}
#[derive(Serialize)]
pub struct Status {
    configured: bool,
    pending: bool,
}
#[tauri::command]
pub async fn nexus_oauth_status() -> Status {
    Status {
        configured: !client_id().is_empty(),
        pending: LOGIN.lock().await.is_some(),
    }
}
#[tauri::command]
pub async fn cancel_nexus_oauth() {
    if let Some(cancel) = LOGIN.lock().await.as_ref() {
        cancel.store(true, Ordering::SeqCst);
    }
}

fn callback(target: &str, state: &str) -> Result<Option<String>, String> {
    if !target.starts_with("/callback?") {
        return Ok(None);
    }
    let url = reqwest::Url::parse(&format!("http://127.0.0.1:17849{target}"))
        .map_err(|_| "Retorno Nexus inválido.")?;
    let values: Vec<_> = url.query_pairs().collect();
    // Reject duplicates and mismatched state without consuming the real callback.
    let get = |key: &str| {
        let mut found = values.iter().filter(|(k, _)| k == key);
        let value = found.next().map(|(_, v)| v.to_string());
        if found.next().is_some() {
            None
        } else {
            value
        }
    };
    if get("state").as_deref() != Some(state) {
        return Ok(None);
    }
    if get("error").is_some() {
        return Err("A autorização Nexus foi recusada ou cancelada.".into());
    }
    Ok(get("code").filter(|s| !s.is_empty() && s.len() <= 4096))
}
fn receive(
    listener: TcpListener,
    state: String,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(180) && !cancel.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let read_started = Instant::now();
                let mut request = Vec::new();
                let mut byte = [0u8; 1];
                while request.len() < 8192
                    && read_started.elapsed() < Duration::from_secs(2)
                    && !cancel.load(Ordering::SeqCst)
                    && stream.read_exact(&mut byte).is_ok()
                {
                    request.push(byte[0]);
                    if request.ends_with(b"\r\n\r\n") {
                        break;
                    }
                }
                let text = String::from_utf8_lossy(&request);
                let mut first = text.lines().next().unwrap_or("").split_whitespace();
                let result = if request.ends_with(b"\r\n\r\n") && first.next() == Some("GET") {
                    callback(first.next().unwrap_or(""), &state)
                } else {
                    Ok(None)
                };
                let accepted = !matches!(result, Ok(None));
                let body = if accepted {
                    "Retorno recebido. Volte ao PeliGames para acompanhar a autorização. Esta página pode ser fechada."
                } else {
                    "Retorno inválido."
                };
                let _ = write!(stream, "HTTP/1.1 {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{}", if accepted { "200 OK" } else { "400 Bad Request" }, body.len(), body);
                match result {
                    Ok(Some(code)) => return Ok(code),
                    Err(e) => return Err(e),
                    _ => {}
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(_) => return Err("Falha ao receber a autorização Nexus.".into()),
        }
    }
    Err(if cancel.load(Ordering::SeqCst) {
        "Login Nexus cancelado."
    } else {
        "O tempo para autorizar o Nexus expirou. Tente novamente."
    }
    .into())
}
#[tauri::command]
pub async fn connect_nexus_oauth(app: tauri::AppHandle) -> Result<NexusAccount, String> {
    if client_id().is_empty() {
        return Err("O login Nexus aguarda o registro do PeliGames e o client_id oficial.".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut login = LOGIN.lock().await;
        if login.is_some() {
            return Err("Um login Nexus já está em andamento.".into());
        }
        *login = Some(cancel.clone());
    }
    // Keep the worker (including cleanup of LOGIN) alive if the invoking webview
    // disappears. Explicit cancellation is handled by the shared flag.
    tauri::async_runtime::spawn(async move {
        let result = async {
            let verifier = random_secret()?;
            let state = random_secret()?;
            let listener = TcpListener::bind("127.0.0.1:17849").map_err(|_| {
            "A porta de retorno Nexus (17849) está ocupada. Feche outro login e tente novamente."
        })?;
            listener
                .set_nonblocking(true)
                .map_err(|_| "Retorno Nexus indisponível.")?;
            let mut url = reqwest::Url::parse(&format!("{USERS}/oauth/authorize"))
                .map_err(|_| "Endereço Nexus inválido.")?;
            url.query_pairs_mut().extend_pairs([
                ("client_id", client_id()),
                ("response_type", "code"),
                ("redirect_uri", REDIRECT),
                ("state", &state),
                ("code_challenge_method", "S256"),
                ("code_challenge", &challenge(&verifier)),
            ]);
            // System browser, never an embedded credential form or developer-console token.
            app.opener()
                .open_url(url.as_str(), None::<&str>)
                .map_err(|_| "Não foi possível abrir o navegador para autorizar o Nexus.")?;
            let callback_cancel = cancel.clone();
            let code = tauri::async_runtime::spawn_blocking(move || {
                receive(listener, state, callback_cancel)
            })
            .await
            .map_err(|_| "Retorno Nexus indisponível.")??;
            let session = exchange(
                &[
                    ("grant_type", "authorization_code"),
                    ("client_id", client_id()),
                    ("redirect_uri", REDIRECT),
                    ("code", &code),
                    ("code_verifier", &verifier),
                ],
                None,
            )
            .await
            .map_err(String::from)?;
            let account = account(&session.access_token).await?;
            with_account(async move {
                if cancel.load(Ordering::SeqCst) {
                    return Err("Login Nexus cancelado.".into());
                }
                let previous = storage("lookup", None).await?;
                save(&session).await?;
                // Cancel may arrive while secret-tool is waiting for an unlock.
                // Roll back under the same lock so logout cannot be undone later.
                if cancel.load(Ordering::SeqCst) {
                    match previous {
                        Some(data) => {
                            storage("store", Some(data)).await?;
                        }
                        None => {
                            clear().await?;
                        }
                    }
                    return Err("Login Nexus cancelado.".into());
                }
                Ok(account)
            })
            .await
        }
        .await;
        *LOGIN.lock().await = None;
        result
    })
    .await
    .map_err(|_| "Não foi possível concluir o login Nexus.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc7636_challenge() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn callback_requires_matching_unique_state() {
        assert_eq!(
            callback("/callback?state=expected&code=a%2Bb", "expected").unwrap(),
            Some("a+b".into())
        );
        for target in [
            "/callback?state=wrong&code=x",
            "/callback?state=expected&state=wrong&code=x",
            "/other?state=expected&code=x",
            "/callback?state=expected&code=x&code=y",
        ] {
            assert!(callback(target, "expected").unwrap().is_none());
        }
        assert!(callback("/callback?state=expected&error=access_denied", "expected").is_err());
    }
    #[test]
    fn token_response_requires_expiry_and_preserves_refresh_rotation() {
        let session = parse_session(serde_json::json!({"token_type":"Bearer","access_token":"access","refresh_token":"rotated","expires_in":3600}), Some("old")).unwrap();
        assert_eq!(session.refresh_token, "rotated");
        assert!(session.expires_at > now());
        let session = parse_session(
            serde_json::json!({"token_type":"bearer","access_token":"access","expires_in":3600}),
            Some("old"),
        )
        .unwrap();
        assert_eq!(session.refresh_token, "old");
        assert!(parse_session(serde_json::json!({"token_type":"Bearer","access_token":"access","refresh_token":"refresh"}), None).is_err());
        assert!(parse_session(serde_json::json!({"token_type":"Bearer","access_token":"bad\nheader","refresh_token":"refresh","expires_in":3600}), None).is_err());
        assert!(parse_session(serde_json::json!({"token_type":"Bearer","access_token":"access","refresh_token":"refresh","expires_in":0}), None).is_err());
    }
    #[test]
    fn only_refused_grants_drop_the_saved_session() {
        assert!(grant_refused(400) && grant_refused(401));
        for status in [200, 403, 429, 500, 503] {
            assert!(!grant_refused(status), "{status} must keep the session");
        }
        assert!(String::from(TokenError::Refused).contains("Conecte sua conta"));
    }
    #[test]
    fn cancelled_listener_exits_without_callback() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        assert!(
            receive(listener, "state".into(), Arc::new(AtomicBool::new(true)))
                .unwrap_err()
                .contains("cancelado")
        );
    }
    #[test]
    fn invalid_callback_does_not_consume_listener() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            receive(
                listener,
                "expected".into(),
                Arc::new(AtomicBool::new(false)),
            )
        });
        for state in ["wrong", "expected"] {
            let mut stream = std::net::TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            write!(
                stream,
                "GET /callback?state={state}&code=valid HTTP/1.1\r\nHost: {address}\r\n\r\n"
            )
            .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.contains(if state == "expected" {
                "200 OK"
            } else {
                "400 Bad Request"
            }));
        }
        assert_eq!(handle.join().unwrap().unwrap(), "valid");
    }
    fn profile_server(statuses: Vec<u16>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}/oauth/userinfo", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for status in statuses {
                let start = Instant::now();
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e)
                            if e.kind() == std::io::ErrorKind::WouldBlock
                                && start.elapsed() < Duration::from_secs(5) =>
                        {
                            std::thread::sleep(Duration::from_millis(5))
                        }
                        Err(e) => panic!("profile test server: {e}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut byte = [0u8; 1];
                while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 8192 {
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                requests.push(String::from_utf8(bytes).unwrap());
                let body = if status == 200 {
                    r#"{"sub":"123","name":"Test user","membership_roles":["premium"]}"#
                } else {
                    r#"{"access_token":"do-not-expose-response-body"}"#
                };
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
            requests
        });
        (endpoint, server)
    }

    #[test]
    fn profile_401_refreshes_and_retries_with_new_token() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let (endpoint, server) = profile_server(vec![401, 200]);
            let refreshes = std::sync::atomic::AtomicUsize::new(0);
            let account = account_with_refresh("stale-token".into(), &endpoint, || async {
                refreshes.fetch_add(1, Ordering::SeqCst);
                Ok("renewed-token".into())
            })
            .await
            .unwrap();
            assert_eq!(account.user_id, 123);
            assert!(account.is_premium);
            assert_eq!(refreshes.load(Ordering::SeqCst), 1);
            let requests = server.join().unwrap();
            assert!(requests[0].contains("Bearer stale-token"));
            assert!(requests[1].contains("Bearer renewed-token"));
            assert!(!requests.iter().any(|r| r.contains("apikey")));
        });
    }

    #[test]
    fn repeated_profile_401_stops_after_one_refresh() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let (endpoint, server) = profile_server(vec![401, 401]);
            let refreshes = std::sync::atomic::AtomicUsize::new(0);
            let error = account_with_refresh("stale-token".into(), &endpoint, || async {
                refreshes.fetch_add(1, Ordering::SeqCst);
                Ok("renewed-token".into())
            })
            .await
            .err()
            .unwrap();
            assert_eq!(refreshes.load(Ordering::SeqCst), 1);
            assert!(!error.contains("do-not-expose-response-body"));
            assert_eq!(server.join().unwrap().len(), 2);
        });
    }

    #[test]
    fn profile_403_does_not_refresh_and_refresh_failure_does_not_retry() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            for status in [403, 401] {
                let (endpoint, server) = profile_server(vec![status]);
                let refreshes = std::sync::atomic::AtomicUsize::new(0);
                assert!(
                    account_with_refresh("stale-token".into(), &endpoint, || async {
                        refreshes.fetch_add(1, Ordering::SeqCst);
                        Err("Refresh refused".into())
                    })
                    .await
                    .is_err()
                );
                assert_eq!(refreshes.load(Ordering::SeqCst), usize::from(status == 401));
                assert_eq!(server.join().unwrap().len(), 1);
            }
        });
    }
}
