//! Basic community chat. Tokens stay in libsecret; encrypted rooms are never sent plaintext.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::{Duration, Instant};
pub(super) const API: &str = "https://matrix-client.matrix.org";
const PREMIUM_ROOM: &str = "!d6uopxWbqNBgbKYC8uTxjgvzIP4F955uQi3rerKKX2Q";
static PREMIUM_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const ROOM: &str = "!pNHMqrnxSAmcVFqrgf:matrix.org";
const EXPIRED: &str = "Sua sessão Matrix expirou ou foi encerrada. Entre novamente.";
static AUTH: tokio::sync::Mutex<bool> = tokio::sync::Mutex::const_new(false);
static AUTH_ERROR: tokio::sync::Mutex<Option<String>> = tokio::sync::Mutex::const_new(None);
// Read once from the keyring; every request afterwards uses this copy.
static SESSION: tokio::sync::Mutex<Option<Session>> = tokio::sync::Mutex::const_new(None);
fn login_page(received: bool) -> String {
    // The token has only arrived at this point; account validation happens next
    // in the launcher. Never claim the account is connected before that check.
    include_str!("matrix-login.html")
        .replace("{{BRAND_STYLES}}", include_str!("connection-brand.css"))
        .replace("{{BRAND_MARKUP}}", include_str!("connection-brand-markup.html"))
        .replace("{{STATUS}}", if received { "" } else { "error" })
        .replace("{{ICON}}", if received { "<path d=\"M5 12h14m-6-6 6 6-6 6\" />" } else { "<path d=\"M12 5v8M12 18h.01\" />" })
        .replace("{{TITLE}}", if received { "Autorização recebida" } else { "Retorno de login inválido" })
        .replace("{{MESSAGE}}", if received { "Continue no PeliGames para acompanhar a conexão da sua conta Matrix." } else { "Não foi possível reconhecer este retorno do Matrix. Inicie uma nova conexão pelo PeliGames." })
        .replace("{{NEXT}}", "Você pode fechar esta aba e voltar ao aplicativo.")
}
fn write_login_page(stream: &mut std::net::TcpStream, received: bool) {
    let body = login_page(received);
    let _ = write!(stream,"HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:; frame-ancestors 'none'; base-uri 'none'; form-action 'none'\r\nConnection: close\r\n\r\n{}", if received { "200 OK" } else { "404 Not Found" }, body.len(),body);
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Session { pub access_token: String, pub user_id: String }
async fn stored(action: &'static str, value: Option<String>) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || super::nexus_account::credential_for(action, value.as_deref(), "matrix-community-chat", "PeliGames — Matrix"))
        .await.map_err(|_| "Chaveiro Matrix indisponível.".to_string())?
}
// One shared pool: a new client per request paid DNS, TCP and the TLS
// handshake (several round trips to matrix.org) on every message.
fn client() -> Result<reqwest::Client, String> {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    if let Some(client) = CLIENT.get() { return Ok(client.clone()); }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_| "Conexão Matrix indisponível.")?;
    Ok(CLIENT.get_or_init(|| client).clone())
}
async fn current() -> Result<Option<Session>, String> {
    let mut cached = SESSION.lock().await;
    if cached.is_none() {
        if let Some(value) = stored("lookup", None).await? {
            *cached = Some(serde_json::from_str(&value).map_err(|_| "Sessão Matrix inválida. Reconecte sua conta.")?);
        }
    }
    Ok(cached.clone())
}
pub(super) async fn session() -> Result<Session, String> {
    current().await?.ok_or_else(|| "Entre com Matrix para acessar o chat.".into())
}
/// The server no longer accepts the token (signed out elsewhere, revoked):
/// the local session goes away, so the chat offers the sign-in again.
pub(super) async fn forget() -> Result<(), String> {
    let _ = tauri::async_runtime::spawn_blocking(super::matrix_call::stop).await;
    stored("clear", None).await?;
    *SESSION.lock().await = None;
    Ok(())
}
async fn checked(response: reqwest::Response) -> Result<reqwest::Response, String> {
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        let _ = forget().await;
        return Err(EXPIRED.into());
    }
    Ok(response)
}
async fn get(path: &str, token: &str) -> Result<Option<Value>, String> {
    let response = checked(client()?.get(format!("{API}{path}")).bearer_auth(token).send().await.map_err(|_| "Não foi possível consultar o Matrix.")?).await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
    if !response.status().is_success() { return Err("Acesso Matrix indisponível. Confira o login e o acesso à sala.".into()); }
    response.json().await.map(Some).map_err(|_| "Resposta Matrix inválida.".into())
}
pub(super) fn room_path(room: &str) -> Result<String, String> {
    // Room version 12 IDs are opaque hashes without the legacy :server suffix.
    if !room.starts_with('!') || room.len() < 2 || room.len() > 512 || room.chars().any(|c| c.is_control() || c.is_whitespace()) { return Err("Sala Matrix inválida.".into()); }
    Ok(urlencoding::encode(room).into_owned())
}
fn event_path(event: &str) -> Result<String, String> {
    if !event.starts_with('$') || event.len() > 512 || event.chars().any(char::is_control) { return Err("Mensagem Matrix inválida.".into()); }
    Ok(urlencoding::encode(event).into_owned())
}
fn user_valid(user: &str) -> bool { user.starts_with('@') && user.contains(':') && user.len() <= 255 && !user.chars().any(char::is_control) }
// Encryption can never be turned off in a room, so "encrypted" is kept for
// good; "not encrypted" is checked again after a minute. This saves a full
// request before every message sent.
static ENCRYPTION: std::sync::Mutex<Option<std::collections::HashMap<String, (bool, Instant)>>> = std::sync::Mutex::new(None);
const UNENCRYPTED_RECHECK: Duration = Duration::from_secs(60);
fn known_encryption(cache: &std::collections::HashMap<String, (bool, Instant)>, room: &str) -> Option<bool> {
    cache.get(room).and_then(|&(encrypted, at)| (encrypted || at.elapsed() < UNENCRYPTED_RECHECK).then_some(encrypted))
}
async fn encrypted(encoded: &str, token: &str) -> Result<bool, String> {
    if let Some(known) = ENCRYPTION.lock().ok().and_then(|cache| cache.as_ref().and_then(|cache| known_encryption(cache, encoded))) {
        return Ok(known);
    }
    let encrypted = get(&format!("/_matrix/client/v3/rooms/{encoded}/state/m.room.encryption"), token).await?.is_some();
    if let Ok(mut cache) = ENCRYPTION.lock() {
        let cache = cache.get_or_insert_with(Default::default);
        if cache.len() > 256 { cache.clear(); }
        cache.insert(encoded.to_string(), (encrypted, Instant::now()));
    }
    Ok(encrypted)
}

#[tauri::command]
pub async fn matrix_chat_status() -> Result<Value, String> {
    let pending = *AUTH.lock().await;
    let error = AUTH_ERROR.lock().await.clone();
    let user = current().await?.map(|data| data.user_id);
    Ok(json!({"pending":pending,"error":error,"user_id":user}))
}
#[tauri::command]
pub async fn connect_matrix_chat(app: tauri::AppHandle) -> Result<(), String> {
    let mut auth = AUTH.lock().await;
    if *auth { return Err("Um login Matrix já está em andamento.".into()); }
    if current().await?.is_some() { return Err("Desconecte a conta atual antes de iniciar outro login.".into()); }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|_| "Não foi possível preparar o retorno do login.")?;
    listener.set_nonblocking(true).map_err(|_| "Retorno de login indisponível.")?;
    let port = listener.local_addr().map_err(|_| "Porta local indisponível.")?.port();
    let nonce = uuid::Uuid::new_v4().to_string();
    let path = format!("/matrix/{nonce}");
    let redirect = format!("http://127.0.0.1:{port}{path}");
    let url = format!("{API}/_matrix/client/v3/login/sso/redirect?redirectUrl={}", urlencoding::encode(&redirect));
    super::browser::open(app, url).await?;
    *AUTH_ERROR.lock().await = None;
    *auth = true;
    drop(auth);
    tauri::async_runtime::spawn(async move {
        let received = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(180) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // Anything else reaching this port (a scan, a stray request) is ignored.
                        let _ = stream.set_nonblocking(false);
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                        let mut buffer = [0_u8; 8192];
                        let Ok(count) = stream.read(&mut buffer) else { continue; };
                        let first = String::from_utf8_lossy(&buffer[..count]).lines().next().unwrap_or("").to_string();
                        let mut parts = first.split_whitespace();
                        if parts.next() != Some("GET") { continue; }
                        let Some(target) = parts.next() else { continue; };
                        let Ok(url) = reqwest::Url::parse(&format!("http://127.0.0.1:{port}{target}")) else { continue; };
                        let token = url.query_pairs().find(|(key,_)| key == "loginToken").map(|(_,value)|value.into_owned());
                        let Some(token) = token.filter(|token| url.path() == path && !token.is_empty() && token.len() <= 4096) else {
                            write_login_page(&mut stream, false);
                            continue;
                        };
                        write_login_page(&mut stream, true);
                        return Ok(token);
                    }, Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
                    Err(_) => return Err("Não foi possível receber o login.".into())
                }
            }
            Err("Login Matrix expirado.".into())
        }).await;
        if let Ok(Ok(token)) = received {
            let result = async {
                let response = client()?.post(format!("{API}/_matrix/client/v3/login")).json(&json!({"type":"m.login.token","token":token,"initial_device_display_name":"PeliGames — Chat retrô"})).send().await.map_err(|_| "Login indisponível.")?;
                if !response.status().is_success() { return Err("Login não autorizado.".to_string()); }
                let value: Value = response.json().await.map_err(|_| "Login inválido.")?;
                let data = Session { access_token: value["access_token"].as_str().ok_or("Token ausente.")?.into(), user_id: value["user_id"].as_str().ok_or("Conta ausente.")?.into() };
                stored("store", Some(serde_json::to_string(&data).map_err(|_|"Conta inválida.")?)).await?;
                *SESSION.lock().await = Some(data);
                Ok::<(),String>(())
            }.await;
            if result.is_err() { *AUTH_ERROR.lock().await = Some("Não foi possível concluir o login Matrix. Tente novamente.".into()); }
        } else { *AUTH_ERROR.lock().await = Some("Login Matrix não concluído ou expirado. Tente novamente.".into()); }
        *AUTH.lock().await = false;
    });
    Ok(())
}
#[tauri::command]
/// Returns whether the server confirmed the sign-out. Without a connection the
/// account still leaves this computer; the server session can be ended in Element.
pub async fn disconnect_matrix_chat() -> Result<bool, String> {
    let _ = tauri::async_runtime::spawn_blocking(super::matrix_call::stop).await;
    if *AUTH.lock().await { return Err("Aguarde o login atual concluir ou expirar.".into()); }
    let data = session().await?;
    let _ = matrix_chat_game_activity(None, data.user_id.clone()).await;
    let response = client()?.post(format!("{API}/_matrix/client/v3/logout")).bearer_auth(&data.access_token).send().await;
    // A token the server no longer accepts is already signed out.
    let revoked = response.is_ok_and(|response| response.status().is_success() || response.status() == reqwest::StatusCode::UNAUTHORIZED);
    forget().await?;
    *GAME_ACTIVITY.lock().await = None;
    *AUTH_ERROR.lock().await = None;
    Ok(revoked)
}
#[tauri::command]
pub async fn matrix_chat_rooms() -> Result<Value, String> {
    let data = session().await?;
    let root = room_path(ROOM)?;
    let hierarchy = get(&format!("/_matrix/client/v1/rooms/{root}/hierarchy?limit=50&max_depth=1"),&data.access_token).await?;
    let mut value = hierarchy.unwrap_or_else(|| json!({"rooms":[]}));
    // Private rooms may be absent from the space hierarchy. Only add the
    // configured room after Matrix confirms membership, with its real name.
    if let Ok(Some(joined)) = get("/_matrix/client/v3/joined_rooms", &data.access_token).await {
        let joined = joined["joined_rooms"].as_array();
        let already_listed = value["rooms"].as_array().is_some_and(|rooms| rooms.iter().any(|room| room["room_id"] == PREMIUM_ROOM));
        if !already_listed && joined.is_some_and(|rooms| rooms.iter().any(|room| room == PREMIUM_ROOM)) {
            let encoded = room_path(PREMIUM_ROOM)?;
            if let Ok(Some(name)) = get(&format!("/_matrix/client/v3/rooms/{encoded}/state/m.room.name"), &data.access_token).await {
                let topic = get(&format!("/_matrix/client/v3/rooms/{encoded}/state/m.room.topic"), &data.access_token).await.ok().flatten();
                if let Some(rooms) = value["rooms"].as_array_mut() {
                    rooms.push(json!({"room_id":PREMIUM_ROOM,"name":name["name"],"topic":topic.as_ref().map(|value| &value["topic"])}));
                }
            }
        }
    }
    Ok(value)
}
#[tauri::command]
pub async fn matrix_chat_join(room: String) -> Result<(), String> {
    let data = session().await?;
    let response = checked(client()?.post(format!("{API}/_matrix/client/v3/join/{}",room_path(&room)?)).bearer_auth(data.access_token).json(&json!({})).send().await.map_err(|_| "Não foi possível entrar na sala.")?).await?;
    if !response.status().is_success() { return Err("Não foi possível entrar. A sala pode exigir convite.".into()); }
    Ok(())
}

/// Room type Element uses for call rooms; the chat lists them as voice channels.
const VOICE_ROOM_TYPE: &str = "org.matrix.msc3417.call";
/// Valid channel name: 1 to 80 visible characters.
fn channel_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) { return Err("Use um nome de 1 a 80 caracteres.".into()); }
    Ok(name.to_string())
}
/// Body of a new channel inside the community space, as Discord channels: any
/// member of the space may join (restricted join rule), the space's moderators
/// keep their power, and in voice channels everyone may publish call membership.
fn channel_body(name: &str, voice: bool, space_users: &serde_json::Map<String, Value>, creator: &str, room_version: &str) -> Value {
    let mut users = space_users.clone();
    // Since v12 creators have infinite power. Listing even an inherited entry
    // for the creator in power_levels makes createRoom fail authorization.
    if room_version.parse::<u64>().is_ok_and(|version| version >= 12) {
        users.remove(creator);
    } else {
        users.insert(creator.to_string(), json!(100));
    }
    let mut events = json!({"m.room.name":50,"m.room.power_levels":100,"m.room.history_visibility":100,"m.room.canonical_alias":50,
        "m.room.avatar":50,"m.room.tombstone":100,"m.room.server_acl":100,"m.room.encryption":100,"m.room.topic":50});
    if voice {
        for kind in ["org.matrix.msc3401.call.member", "m.call.member", "org.matrix.msc4143.rtc.member"] { events[kind] = json!(0); }
    }
    json!({
        "name": name, "visibility": "private", "room_version": room_version,
        "creation_content": if voice { json!({"type": VOICE_ROOM_TYPE}) } else { json!({}) },
        "initial_state": [
            {"type":"m.space.parent","state_key":ROOM,"content":{"canonical":true,"via":["matrix.org"]}},
            {"type":"m.room.join_rules","state_key":"","content":{"join_rule":"restricted","allow":[{"type":"m.room_membership","room_id":ROOM}]}},
            {"type":"m.room.history_visibility","state_key":"","content":{"history_visibility":"shared"}},
            {"type":"m.room.guest_access","state_key":"","content":{"guest_access":"forbidden"}},
        ],
        "power_level_content_override": {"users": users, "events": events},
    })
}
/// Keep diagnostics useful without displaying arbitrary server text or secrets.
fn channel_creation_error(status: u16, body: &Value) -> String {
    let code = body["errcode"].as_str().filter(|code| code.starts_with("M_") && code.len() <= 64
        && code.bytes().all(|byte| byte.is_ascii_uppercase() || byte == b'_'));
    let message = match code {
        Some("M_FORBIDDEN") => "O Matrix não autorizou a criação do canal. Confira as permissões da conta.",
        Some("M_LIMIT_EXCEEDED") => "O Matrix limitou temporariamente a criação de canais. Aguarde e tente novamente.",
        Some("M_UNSUPPORTED_ROOM_VERSION") => "O servidor não aceita a versão de sala informada. Tente novamente.",
        Some("M_BAD_JSON" | "M_INVALID_PARAM") => "O Matrix rejeitou as configurações do novo canal.",
        _ => "Não foi possível criar o canal no Matrix.",
    };
    format!("{message} (HTTP {status}{})", code.map(|code| format!(", {code}")).unwrap_or_default())
}
fn space_child_level(levels: &Value) -> i64 {
    levels["events"]["m.space.child"].as_i64().unwrap_or_else(|| levels["state_default"].as_i64().unwrap_or(50))
}
/// Sends a request again while the server answers 429, waiting as long as it
/// asks (at most 10 s each time, five tries). Matrix.org limits bursts such as
/// removing everyone from a channel.
async fn respecting_rate_limit(request: reqwest::RequestBuilder) -> Option<reqwest::Response> {
    for _ in 0..5 {
        let response = request.try_clone()?.send().await.ok()?;
        if response.status() != reqwest::StatusCode::TOO_MANY_REQUESTS { return Some(response); }
        let header = response.headers().get(reqwest::header::RETRY_AFTER).and_then(|value| value.to_str().ok()).and_then(|value| value.parse::<u64>().ok()).map(|seconds| seconds * 1000);
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        tokio::time::sleep(Duration::from_millis(rate_limit_wait(header, &body))).await;
    }
    None
}
fn rate_limit_wait(retry_after_ms: Option<u64>, body: &Value) -> u64 {
    retry_after_ms.or_else(|| body["retry_after_ms"].as_u64()).unwrap_or(1000).clamp(200, 10_000)
}
/// Deletes a channel as Discord does: it leaves the community, nobody new can
/// join, everyone in it is removed, and this account leaves and forgets it.
/// Matrix rooms cannot be erased from the server; this is the closest.
/// Returns how many people were removed.
#[tauri::command]
pub async fn matrix_chat_delete_channel(room: String) -> Result<usize, String> {
    let path = room_path(&room)?;
    if room == ROOM || room == PREMIUM_ROOM { return Err("Este canal não pode ser apagado pelo launcher.".into()); }
    let data = session().await?;
    let token = data.access_token.clone();
    let space = room_path(ROOM)?;
    let child = get(&format!("/_matrix/client/v3/rooms/{space}/state/m.space.child/{path}"), &token).await?;
    if !child.is_some_and(|content| content.as_object().is_some_and(|map| !map.is_empty())) { return Err("Este canal não faz parte da comunidade.".into()); }
    let (mine, levels) = power_in(ROOM, &token, &data.user_id).await?;
    if mine < space_child_level(&levels) { return Err("Você não tem permissão para apagar canais nesta comunidade.".into()); }
    // Out of the community first: the channel disappears even if a later step fails.
    let removed = checked(client()?.put(format!("{API}/_matrix/client/v3/rooms/{space}/state/m.space.child/{path}")).bearer_auth(&token).json(&json!({})).send().await.map_err(|_| "Não foi possível remover o canal da comunidade.")?).await?;
    if !removed.status().is_success() { return Err("A comunidade recusou remover o canal.".into()); }
    let _ = checked(client()?.post(format!("{API}/_matrix/client/v3/join/{path}")).bearer_auth(&token).json(&json!({})).send().await.map_err(|_| String::new())?).await;
    let put_state = |kind: &str, key: &str, body: Value| {
        let url = format!("{API}/_matrix/client/v3/rooms/{path}/state/{kind}/{}", urlencoding::encode(key));
        let token = token.clone();
        async move { let _ = client()?.put(url).bearer_auth(token).json(&body).send().await; Ok::<(), String>(()) }
    };
    put_state("m.room.join_rules", "", json!({"join_rule":"invite"})).await?;
    put_state("m.space.parent", ROOM, json!({})).await?;
    let mut kicked = 0;
    let (power, room_levels) = power_in(&room, &token, &data.user_id).await?;
    if power >= room_levels["kick"].as_i64().unwrap_or(50) {
        let members = get(&format!("/_matrix/client/v3/rooms/{path}/joined_members"), &token).await?.unwrap_or_else(|| json!({}));
        for user in members["joined"].as_object().into_iter().flatten().map(|(user, _)| user.clone()).filter(|user| *user != data.user_id).take(1000) {
            let request = client()?.post(format!("{API}/_matrix/client/v3/rooms/{path}/kick")).bearer_auth(&token)
                .json(&json!({"user_id": user, "reason": "Canal apagado pela administração da comunidade."}));
            if respecting_rate_limit(request).await.is_some_and(|response| response.status().is_success()) { kicked += 1; }
        }
    }
    let _ = client()?.post(format!("{API}/_matrix/client/v3/rooms/{path}/leave")).bearer_auth(&token).json(&json!({})).send().await;
    let _ = client()?.post(format!("{API}/_matrix/client/v3/rooms/{path}/forget")).bearer_auth(&token).json(&json!({})).send().await;
    Ok(kicked)
}
/// Creates a text or voice channel in the community space and lists it there.
/// Only someone allowed to add rooms to the space can; the server checks again.
#[tauri::command]
pub async fn matrix_chat_create_channel(name: String, kind: String) -> Result<String, String> {
    let name = channel_name(&name)?;
    let voice = match kind.as_str() { "voice" => true, "text" => false, _ => return Err("Tipo de canal inválido.".into()) };
    let data = session().await?;
    let space = room_path(ROOM)?;
    let (mine, levels) = power_in(ROOM, &data.access_token, &data.user_id).await?;
    let users = levels["users"].as_object().cloned().unwrap_or_default();
    if mine < space_child_level(&levels) { return Err("Você não tem permissão para criar canais nesta comunidade.".into()); }
    let capabilities = get("/_matrix/client/v3/capabilities", &data.access_token).await?
        .ok_or("Não foi possível consultar a versão de sala do servidor.")?;
    let room_version = capabilities["capabilities"]["m.room_versions"]["default"].as_str()
        .ok_or("O Matrix não informou a versão de sala para criar o canal.")?;
    let created = checked(client()?.post(format!("{API}/_matrix/client/v3/createRoom")).bearer_auth(&data.access_token)
        .json(&channel_body(&name, voice, &users, &data.user_id, room_version)).send().await.map_err(|_| "Não foi possível criar o canal.")?).await?;
    if !created.status().is_success() {
        let status = created.status().as_u16();
        let body = created.json::<Value>().await.unwrap_or(Value::Null);
        return Err(channel_creation_error(status, &body));
    }
    let room: Value = created.json().await.map_err(|_| "Resposta Matrix inválida.")?;
    let id = room["room_id"].as_str().ok_or("Canal criado sem identificador.")?.to_string();
    let listed = checked(client()?.put(format!("{API}/_matrix/client/v3/rooms/{space}/state/m.space.child/{}", room_path(&id)?)).bearer_auth(&data.access_token)
        .json(&json!({"via":["matrix.org"],"suggested":true})).send().await.map_err(|_| "Canal criado, mas não foi possível adicioná-lo à comunidade.")?).await?;
    if !listed.status().is_success() { return Err("Canal criado, mas a comunidade recusou adicioná-lo. Adicione-o pelo Element.".into()); }
    Ok(id)
}

const TYPES: [&str; 6] = ["m.room.message", "m.room.redaction", "m.room.member", "m.room.encryption", "m.room.power_levels", "m.reaction"];
/// One timeline event, reduced to what the chat shows: a message, an edit of
/// one, or a removal. Already-removed messages (empty content) are skipped.
fn chat_event(event: &Value) -> Option<Value> {
    let id = event["event_id"].as_str()?;
    let sender = event["sender"].as_str()?;
    let ts = event["origin_server_ts"].as_u64().unwrap_or(0);
    match event["type"].as_str()? {
        "m.room.redaction" => {
            let target = event["redacts"].as_str().or_else(|| event["content"]["redacts"].as_str())?;
            Some(json!({"kind":"redaction","id":id,"target":target}))
        }
        "m.reaction" => {
            let relation = &event["content"]["m.relates_to"];
            if relation["rel_type"] != "m.annotation" || event["unsigned"]["redacted_because"].is_object() { return None; }
            let key: String = relation["key"].as_str()?.chars().take(32).collect();
            Some(json!({"kind":"reaction","id":id,"target":relation["event_id"].as_str()?,"key":key,"sender":sender}))
        }
        "m.room.message" => {
            let content = &event["content"];
            if event["unsigned"]["redacted_because"].is_object() || content.as_object().is_none_or(|map| map.is_empty()) { return None; }
            let relation = &content["m.relates_to"];
            if relation["rel_type"] == "m.replace" {
                let target = relation["event_id"].as_str()?;
                let body = content["m.new_content"]["body"].as_str().or_else(|| content["body"].as_str().map(|body| body.strip_prefix("* ").unwrap_or(body)))?;
                return Some(json!({"kind":"edit","id":id,"target":target,"body":body}));
            }
            let reply = relation["m.in_reply_to"]["event_id"].as_str();
            let mut body = content["body"].as_str().unwrap_or("").to_string();
            if reply.is_some() { body = strip_reply_fallback(&body); }
            let mentions: Vec<&str> = content["m.mentions"]["user_ids"].as_array().map(|users| users.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            Some(json!({"kind":"message","id":id,"sender":sender,"ts":ts,"msgtype":content["msgtype"].as_str().unwrap_or("m.text"),"body":body,
                "url":content["url"].as_str(),"reply_to":reply,"mentions":mentions}))
        }
        _ => None,
    }
}
/// Older clients quote the replied message as "> <@user> text" lines before the answer.
fn strip_reply_fallback(body: &str) -> String {
    if !body.starts_with("> ") { return body.to_string(); }
    let rest: Vec<&str> = body.lines().skip_while(|line| line.starts_with("> ") || *line == ">").collect();
    rest.iter().skip_while(|line| line.is_empty()).copied().collect::<Vec<_>>().join("\n")
}
fn display_names(events: &[Value], names: &mut serde_json::Map<String, Value>) {
    for event in events.iter().filter(|event| event["type"] == "m.room.member") {
        let (Some(user), Some(name)) = (event["state_key"].as_str(), event["content"]["displayname"].as_str()) else { continue; };
        if !name.trim().is_empty() { names.insert(user.into(), json!(name.chars().take(64).collect::<String>())); }
    }
}
/// Current membership changes in state/timeline order; never infer membership from messages.
fn membership_updates(state: &[Value], timeline: &[Value]) -> serde_json::Map<String, Value> {
    let mut members = serde_json::Map::new();
    for event in state.iter().chain(timeline) {
        if event["type"] != "m.room.member" { continue; }
        let (Some(user), Some(membership)) = (event["state_key"].as_str(), event["content"]["membership"].as_str()) else { continue; };
        if !matches!(membership, "join" | "leave" | "ban" | "invite" | "knock") { continue; }
        members.insert(user.into(), json!(membership == "join"));
    }
    members
}
/// Who may remove messages, remove or ban people: the latest power levels of a room.
fn power_levels(events: &[Value]) -> Option<Value> {
    let content = &events.iter().rev().find(|event| event["type"] == "m.room.power_levels")?["content"];
    let level = |key: &str, default: i64| content[key].as_i64().unwrap_or(default);
    Some(json!({"users": content["users"].as_object().cloned().unwrap_or_default(), "users_default": level("users_default", 0),
        "redact": level("redact", 50), "kick": level("kick", 50), "ban": level("ban", 50),
        // In the community space: who may add channels (text or voice rooms).
        "space_child": content["events"]["m.space.child"].as_i64().unwrap_or_else(|| level("state_default", 50))}))
}
/// Since room version 12 the creators (the sender of `m.room.create` and its
/// `additional_creators`) have unlimited power and are absent from
/// `power_levels`. `None` when the batch has no create event; empty before v12.
fn privileged_creators(events: &[Value]) -> Option<Vec<String>> {
    let create = events.iter().find(|event| event["type"] == "m.room.create" && event["state_key"] == "")?;
    if !create["content"]["room_version"].as_str().unwrap_or("1").parse::<u64>().is_ok_and(|version| version >= 12) { return Some(Vec::new()); }
    let mut creators: Vec<String> = create["sender"].as_str().map(str::to_string).into_iter().collect();
    creators.extend(create["content"]["additional_creators"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string));
    Some(creators)
}
/// The user's power in a room, counting the unlimited power of v12 creators,
/// with the room's power levels (one read of the room state).
async fn power_in(room: &str, token: &str, user: &str) -> Result<(i64, Value), String> {
    let state = get(&format!("/_matrix/client/v3/rooms/{}/state", room_path(room)?), token).await?.ok_or("Permissões da sala indisponíveis.")?;
    let events = list(&state);
    Ok(power_of_user(events, user))
}
fn power_of_user(events: &[Value], user: &str) -> (i64, Value) {
    let levels = events.iter().find(|event| event["type"] == "m.room.power_levels" && event["state_key"] == "").map(|event| event["content"].clone()).unwrap_or_else(|| json!({}));
    if privileged_creators(events).is_some_and(|creators| creators.iter().any(|creator| creator == user)) { return (i64::MAX, levels); }
    let mine = levels["users"][user].as_i64().unwrap_or_else(|| levels["users_default"].as_i64().unwrap_or(0));
    (mine, levels)
}
fn has_encryption(events: &[Value]) -> bool { events.iter().any(|event| event["type"] == "m.room.encryption") }
fn list(value: &Value) -> &[Value] { value.as_array().map(Vec::as_slice).unwrap_or(&[]) }

/// Long-polled `/sync` limited to the community rooms. The first call (no
/// `since`) returns recent history and the full membership state; later ones
/// deliver only state/presence deltas and wait up to `timeout` ms for news.
#[tauri::command]
pub async fn matrix_chat_sync(since: Option<String>, rooms: Vec<String>, timeout: u32) -> Result<Value, String> {
    let data = session().await?;
    if rooms.len() > 60 { return Err("Salas demais na comunidade.".into()); }
    for room in &rooms { room_path(room)?; }
    let filter = json!({
        "presence":{"types":["m.presence"]},
        "account_data":{"types":["m.ignored_user_list"]},
        "room":{"rooms":rooms,"timeline":{"limit":50,"types":TYPES},"state":{"lazy_load_members":false,"types":["m.room.member","m.room.encryption","m.room.power_levels"]},
            "ephemeral":{"types":["m.typing"]},"account_data":{"not_types":["*"]}}
    });
    let timeout = timeout.min(30000);
    let mut url = format!("{API}/_matrix/client/v3/sync?timeout={timeout}&filter={}", urlencoding::encode(&filter.to_string()));
    if let Some(since) = since.filter(|value| !value.is_empty() && value.len() < 1024) { url.push_str(&format!("&since={}", urlencoding::encode(&since))); }
    let response = checked(client()?.get(url).bearer_auth(&data.access_token).timeout(Duration::from_millis(u64::from(timeout) + 20000)).send().await.map_err(|_| "Sem conexão com o Matrix.")?).await?;
    if !response.status().is_success() { return Err("Não foi possível atualizar o chat Matrix.".into()); }
    let value: Value = response.json().await.map_err(|_| "Resposta Matrix inválida.")?;
    Ok(normalize_sync(&value, &data.user_id))
}
fn typing_users(events: &[Value], me: &str) -> Option<Vec<String>> {
    let event = events.iter().rev().find(|event| event["type"] == "m.typing")?;
    Some(list(&event["content"]["user_ids"]).iter().filter_map(Value::as_str).filter(|user| *user != me && user_valid(user)).take(20).map(str::to_string).collect())
}
pub(super) fn normalize_sync(value: &Value, user_id: &str) -> Value {
    let mut joined = serde_json::Map::new();
    for (room, content) in value["rooms"]["join"].as_object().into_iter().flatten() {
        let timeline = list(&content["timeline"]["events"]);
        let state = list(&content["state"]["events"]);
        let mut names = serde_json::Map::new();
        display_names(state, &mut names); display_names(timeline, &mut names);
        let encrypted = has_encryption(state) || has_encryption(timeline);
        let power = power_levels(timeline).or_else(|| power_levels(state));
        joined.insert(room.clone(), json!({
            "events": timeline.iter().filter_map(chat_event).collect::<Vec<_>>(),
            "prev_batch": content["timeline"]["prev_batch"].as_str(),
            "limited": content["timeline"]["limited"].as_bool().unwrap_or(false),
            // Absent from incremental syncs: unchanged.
            "encrypted": if encrypted { Some(true) } else { None },
            "names": names,
            "members": membership_updates(state, timeline),
            "power": power,
            "creators": privileged_creators(state).or_else(|| privileged_creators(timeline)),
            // Unread messages counted by the server (cleared by our read marker).
            "unread": content["unread_notifications"]["notification_count"].as_u64(),
            // Who is typing; absent when unchanged.
            "typing": typing_users(list(&content["ephemeral"]["events"]), user_id),
        }));
    }
    let left: Vec<&String> = value["rooms"]["leave"].as_object().map(|rooms| rooms.keys().collect()).unwrap_or_default();
    let ignored = list(&value["account_data"]["events"]).iter().find(|event| event["type"] == "m.ignored_user_list")
        .map(|event| event["content"]["ignored_users"].as_object().map(|users| users.keys().cloned().collect::<Vec<_>>()).unwrap_or_default());
    json!({"next_batch": value["next_batch"], "user_id": user_id, "join": joined, "leave": left, "ignored": ignored, "presence": presence_statuses(list(&value["presence"]["events"]))})
}
/// Older messages of a room, before `from` (a `prev_batch`/`end` token).
#[tauri::command]
pub async fn matrix_chat_history(room: String, from: String) -> Result<Value, String> {
    let data = session().await?;
    if from.is_empty() || from.len() > 1024 { return Err("Histórico indisponível.".into()); }
    let filter = json!({"types":TYPES,"lazy_load_members":true});
    let value = get(&format!("/_matrix/client/v3/rooms/{}/messages?dir=b&limit=50&from={}&filter={}", room_path(&room)?, urlencoding::encode(&from), urlencoding::encode(&filter.to_string())), &data.access_token).await?.ok_or("Histórico indisponível.")?;
    let mut names = serde_json::Map::new();
    display_names(list(&value["state"]), &mut names); display_names(list(&value["chunk"]), &mut names);
    // `chunk` comes newest first.
    let events: Vec<Value> = list(&value["chunk"]).iter().rev().filter_map(chat_event).collect();
    Ok(json!({"events": events, "end": value["end"].as_str(), "names": names}))
}
#[tauri::command]
pub async fn matrix_chat_send(room: String, text: String, reply_to: Option<String>, reply_sender: Option<String>, emote: Option<bool>) -> Result<(), String> {
    if text.trim().is_empty() || text.len() > 8000 { return Err("Escreva uma mensagem de até 8000 caracteres.".into()); }
    let data = session().await?;
    let encoded = room_path(&room)?;
    if encrypted(&encoded, &data.access_token).await? { return Err("Esta sala é criptografada. Use o Element para conversar nela.".into()); }
    let mut content = json!({"msgtype": if emote == Some(true) {"m.emote"} else {"m.text"}, "body": text, "m.mentions": {}});
    if let Some(reply) = reply_to.as_deref() {
        event_path(reply)?;
        content["m.relates_to"] = json!({"m.in_reply_to":{"event_id":reply}});
        if let Some(user) = reply_sender.filter(|user| user_valid(user) && *user != data.user_id) { content["m.mentions"] = json!({"user_ids":[user]}); }
    }
    let txn = uuid::Uuid::new_v4();
    let response = checked(client()?.put(format!("{API}/_matrix/client/v3/rooms/{encoded}/send/m.room.message/{txn}")).bearer_auth(data.access_token).json(&content).send().await.map_err(|_| "Não foi possível enviar a mensagem.")?).await?;
    if !response.status().is_success() { return Err("Envio não autorizado. Confira se entrou na sala.".into()); }
    Ok(())
}
/// Marks a room as read up to `event` (private receipt: others do not see it).
#[tauri::command]
pub async fn matrix_chat_read(room: String, event: String) -> Result<(), String> {
    let data = session().await?;
    let url = format!("{API}/_matrix/client/v3/rooms/{}/read_markers", room_path(&room)?);
    event_path(&event)?;
    let response = checked(client()?.post(url).bearer_auth(data.access_token).json(&json!({"m.fully_read":event,"m.read.private":event})).send().await.map_err(|_| "Sem conexão com o Matrix.")?).await?;
    if !response.status().is_success() { return Err("Não foi possível marcar a sala como lida.".into()); }
    Ok(())
}
/// "Typing…" for the others in the room; the server ends it after `timeout`.
#[tauri::command]
pub async fn matrix_chat_typing(room: String, typing: bool) -> Result<(), String> {
    let data = session().await?;
    let url = format!("{API}/_matrix/client/v3/rooms/{}/typing/{}", room_path(&room)?, urlencoding::encode(&data.user_id));
    let body = if typing { json!({"typing":true,"timeout":6000}) } else { json!({"typing":false}) };
    let _ = checked(client()?.put(url).bearer_auth(data.access_token).json(&body).send().await.map_err(|_| "Sem conexão com o Matrix.")?).await?;
    Ok(())
}
/// Replaces the text of one of the user's own messages (Matrix edit).
#[tauri::command]
pub async fn matrix_chat_edit(room: String, target: String, text: String) -> Result<(), String> {
    if text.trim().is_empty() || text.len() > 8000 { return Err("Escreva uma mensagem de até 8000 caracteres.".into()); }
    let data = session().await?;
    let encoded = room_path(&room)?;
    let original = get(&format!("/_matrix/client/v3/rooms/{encoded}/event/{}", event_path(&target)?), &data.access_token).await?.ok_or("Mensagem não encontrada.")?;
    if original["sender"] != data.user_id.as_str() || original["type"] != "m.room.message" { return Err("Só é possível editar as próprias mensagens.".into()); }
    if encrypted(&encoded, &data.access_token).await? { return Err("Esta sala é criptografada. Use o Element para conversar nela.".into()); }
    let msgtype = original["content"]["msgtype"].as_str().filter(|kind| matches!(*kind, "m.text" | "m.emote")).unwrap_or("m.text");
    let content = json!({"msgtype":msgtype,"body":format!("* {text}"),"m.mentions":{},
        "m.new_content":{"msgtype":msgtype,"body":text,"m.mentions":{}},
        "m.relates_to":{"rel_type":"m.replace","event_id":target}});
    let response = checked(client()?.put(format!("{API}/_matrix/client/v3/rooms/{encoded}/send/m.room.message/{}", uuid::Uuid::new_v4())).bearer_auth(data.access_token).json(&content).send().await.map_err(|_| "Não foi possível editar a mensagem.")?).await?;
    if !response.status().is_success() { return Err("A edição não foi aceita pelo servidor.".into()); }
    Ok(())
}
/// Adds an emoji reaction to a message; removing one redacts the reaction event.
#[tauri::command]
pub async fn matrix_chat_react(room: String, target: String, key: String) -> Result<(), String> {
    if key.is_empty() || key.chars().count() > 16 || key.chars().any(char::is_control) { return Err("Reação inválida.".into()); }
    let data = session().await?;
    let encoded = room_path(&room)?;
    let content = json!({"m.relates_to":{"rel_type":"m.annotation","event_id":event_path(&target).map(|_| target.clone())?,"key":key}});
    let response = checked(client()?.put(format!("{API}/_matrix/client/v3/rooms/{encoded}/send/m.reaction/{}", uuid::Uuid::new_v4())).bearer_auth(data.access_token).json(&content).send().await.map_err(|_| "Não foi possível reagir.")?).await?;
    if !response.status().is_success() { return Err("A reação não foi aceita pelo servidor.".into()); }
    Ok(())
}
/// Changes the account's display name in every room.
#[tauri::command]
pub async fn matrix_profile_set_name(name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 || name.chars().any(char::is_control) { return Err("Use um nome de 1 a 64 caracteres.".into()); }
    let data = session().await?;
    let url = format!("{API}/_matrix/client/v3/profile/{}/displayname", urlencoding::encode(&data.user_id));
    let response = checked(client()?.put(url).bearer_auth(data.access_token).json(&json!({"displayname":name})).send().await.map_err(|_| "Sem conexão com o Matrix.")?).await?;
    if !response.status().is_success() { return Err("O Matrix não aceitou o novo nome.".into()); }
    Ok(())
}
/// New profile picture: raw image bytes, type in the `x-matrix-mime` header.
#[tauri::command]
pub async fn matrix_profile_set_avatar(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(raw) = request.body() else { return Err("Escolha uma imagem de até 2 MB.".into()); };
    let mime = request.headers().get("x-matrix-mime").and_then(|value| value.to_str().ok()).unwrap_or("").to_string();
    if raw.is_empty() || raw.len() > 2 * 1024 * 1024 || !matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/webp" | "image/gif") { return Err("Escolha uma imagem PNG, JPEG, WebP ou GIF de até 2 MB.".into()); }
    let data = session().await?;
    let response = checked(client()?.post(format!("{API}/_matrix/media/v3/upload?filename=avatar")).timeout(Duration::from_secs(120)).bearer_auth(&data.access_token).header(reqwest::header::CONTENT_TYPE, &mime).body(raw.clone()).send().await.map_err(|_| "Não foi possível enviar a imagem.")?).await?;
    if !response.status().is_success() { return Err("O Matrix não aceitou a imagem.".into()); }
    let uploaded: Value = response.json().await.map_err(|_| "Resposta do upload inválida.")?;
    let mxc = uploaded["content_uri"].as_str().ok_or("Imagem indisponível.")?;
    let url = format!("{API}/_matrix/client/v3/profile/{}/avatar_url", urlencoding::encode(&data.user_id));
    let response = checked(client()?.put(url).bearer_auth(data.access_token).json(&json!({"avatar_url":mxc})).send().await.map_err(|_| "Imagem enviada, mas não foi possível usá-la no perfil.")?).await?;
    if !response.status().is_success() { return Err("Imagem enviada, mas o Matrix recusou usá-la no perfil.".into()); }
    Ok(())
}
/// Hides (or shows again) everything a user sends, through the account's
/// ignore list: the server stops delivering their messages to every client.
#[tauri::command]
pub async fn matrix_chat_ignore(user: String, ignore: bool) -> Result<(), String> {
    if !user_valid(&user) { return Err("Usuário Matrix inválido.".into()); }
    let data = session().await?;
    if user == data.user_id { return Err("Você não pode ignorar a própria conta.".into()); }
    let path = format!("/_matrix/client/v3/user/{}/account_data/m.ignored_user_list", urlencoding::encode(&data.user_id));
    let mut list = get(&path, &data.access_token).await?.unwrap_or_else(|| json!({}));
    if !list["ignored_users"].is_object() { list["ignored_users"] = json!({}); }
    let users = list["ignored_users"].as_object_mut().ok_or("Lista de ignorados inválida.")?;
    if ignore { users.insert(user, json!({})); } else { users.remove(&user); }
    let response = checked(client()?.put(format!("{API}{path}")).bearer_auth(&data.access_token).json(&list).send().await.map_err(|_| "Não foi possível atualizar a lista de ignorados.")?).await?;
    if !response.status().is_success() { return Err("Não foi possível atualizar a lista de ignorados.".into()); }
    Ok(())
}
/// Room moderation: remove a message (anyone may remove their own), or remove
/// (`kick`) or ban a person. The server checks the room's power levels again.
#[tauri::command]
pub async fn matrix_chat_moderate(room: String, action: String, target: String, reason: String) -> Result<(), String> {
    let data = session().await?;
    let encoded = room_path(&room)?;
    let reason: String = reason.trim().chars().filter(|c| !c.is_control()).take(500).collect();
    let request = match action.as_str() {
        "redact" => client()?.put(format!("{API}/_matrix/client/v3/rooms/{encoded}/redact/{}/{}", event_path(&target)?, uuid::Uuid::new_v4())).json(&if reason.is_empty() { json!({}) } else { json!({"reason":reason}) }),
        "kick" | "ban" => {
            if !user_valid(&target) || target == data.user_id { return Err("Usuário Matrix inválido.".into()); }
            client()?.post(format!("{API}/_matrix/client/v3/rooms/{encoded}/{action}")).json(&json!({"user_id":target,"reason":reason}))
        }
        _ => return Err("Ação de moderação inválida.".into()),
    };
    let response = checked(request.bearer_auth(data.access_token).send().await.map_err(|_| "Sem conexão com o Matrix.")?).await?;
    if response.status() == reqwest::StatusCode::FORBIDDEN { return Err("Você não tem permissão para isso nesta sala.".into()); }
    if !response.status().is_success() { return Err("O servidor não aceitou a ação de moderação.".into()); }
    Ok(())
}
/// Reports a message to the server's moderators (and the room's, where supported).
#[tauri::command]
pub async fn matrix_chat_report(room: String, event: String, reason: String) -> Result<(), String> {
    let data = session().await?;
    let reason: String = reason.trim().chars().filter(|c| !c.is_control() || *c == '\n').take(1000).collect();
    let url = format!("{API}/_matrix/client/v3/rooms/{}/report/{}", room_path(&room)?, event_path(&event)?);
    let response = checked(client()?.post(url).bearer_auth(data.access_token).json(&json!({"reason": if reason.is_empty() {"Denúncia enviada pelo PeliGames".into()} else {reason}})).send().await.map_err(|_| "Não foi possível enviar a denúncia.")?).await?;
    if !response.status().is_success() { return Err("A denúncia não foi aceita pelo servidor.".into()); }
    Ok(())
}
#[tauri::command]
pub async fn matrix_chat_avatar(user: String) -> Result<Option<String>, String> {
    if !user_valid(&user) { return Err("Usuário Matrix inválido.".into()); }
    let data = session().await?;
    let profile = get(&format!("/_matrix/client/v3/profile/{}",urlencoding::encode(&user)),&data.access_token).await?;
    let Some(avatar) = profile.as_ref().and_then(|value|value["avatar_url"].as_str()) else { return Ok(None); };
    thumbnail(avatar, &data.access_token, 64).await
}
async fn thumbnail(avatar: &str, token: &str, width: u32) -> Result<Option<String>,String> {
    use base64::Engine;
    let Some(mxc) = avatar.strip_prefix("mxc://") else { return Ok(None); };
    let Some((server, media)) = mxc.split_once('/') else { return Ok(None); };
    if server.is_empty() || media.is_empty() || media.contains('/') || mxc.len() > 1024 { return Ok(None); }
    let url = format!("{API}/_matrix/client/v1/media/thumbnail/{}/{}?width={width}&height={width}&method=scale",urlencoding::encode(server),urlencoding::encode(media));
    let mut response = client()?.get(url).bearer_auth(token).send().await.map_err(|_| "Avatar indisponível.")?;
    if !response.status().is_success() { return Ok(None); }
    let mime = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("").to_string();
    if !matches!(mime.as_str(),"image/png"|"image/jpeg"|"image/webp"|"image/gif") { return Ok(None); }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "Avatar indisponível.")? { if bytes.len()+chunk.len() > 1024*1024 { return Ok(None); } bytes.extend_from_slice(&chunk); }
    Ok(Some(format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes))))
}
/// The file arrives as the raw request body; room, name and type come in headers
/// (URL-encoded), so a 5 MB image is not turned into a JSON array of numbers.
#[tauri::command]
pub async fn matrix_chat_upload(request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(raw) = request.body() else { return Err("Envie uma imagem ou log de até 5 MB.".into()); };
    let header = |name: &str| request.headers().get(name).and_then(|value| value.to_str().ok()).and_then(|value| urlencoding::decode(value).ok()).map(|value| value.into_owned()).unwrap_or_default();
    let (room, name, mime, bytes) = (header("x-matrix-room"), header("x-matrix-name"), header("x-matrix-mime"), raw.clone());
    if bytes.is_empty() || bytes.len() > 5*1024*1024 || name.is_empty() || name.len() > 200 || name.chars().any(char::is_control) || !matches!(mime.as_str(),"image/png"|"image/jpeg"|"image/webp"|"image/gif"|"text/plain") { return Err("Envie uma imagem ou log de até 5 MB.".into()); }
    let data = session().await?; let encoded = room_path(&room)?;
    if encrypted(&encoded, &data.access_token).await? { return Err("Use o Element para enviar arquivos nesta sala criptografada.".into()); }
    let size = bytes.len();
    // Up to 5 MB: five minutes, not the 20 s of other requests, so slow connections finish.
    let response = checked(client()?.post(format!("{API}/_matrix/media/v3/upload?filename={}",urlencoding::encode(&name))).timeout(Duration::from_secs(300)).bearer_auth(&data.access_token).header(reqwest::header::CONTENT_TYPE,&mime).body(bytes).send().await.map_err(|_| "Não foi possível enviar o arquivo.")?).await?;
    if !response.status().is_success() { return Err("Upload Matrix não autorizado.".into()); }
    let uploaded: Value = response.json().await.map_err(|_| "Resposta do upload inválida.")?;
    let mxc = uploaded["content_uri"].as_str().ok_or("Arquivo indisponível.")?;
    let response = checked(client()?.put(format!("{API}/_matrix/client/v3/rooms/{encoded}/send/m.room.message/{}",uuid::Uuid::new_v4())).bearer_auth(data.access_token).json(&json!({"msgtype":if mime.starts_with("image/") {"m.image"} else {"m.file"},"body":name,"url":mxc,"info":{"mimetype":mime,"size":size}})).send().await.map_err(|_| "Arquivo enviado, mas não foi possível publicá-lo na sala.")?).await?;
    if !response.status().is_success() { return Err("Arquivo enviado, mas a publicação não foi autorizada.".into()); }
    Ok(())
}

#[tauri::command]
pub async fn matrix_chat_image(mxc: String) -> Result<Option<String>,String> {
    let data = session().await?;
    thumbnail(&mxc,&data.access_token,480).await
}

#[cfg(test)]
mod tests {
    #[test]
    fn rate_limit_wait_follows_the_server_within_bounds() {
        use serde_json::json;
        assert_eq!(super::rate_limit_wait(Some(3000), &json!({"retry_after_ms": 500})), 3000);
        assert_eq!(super::rate_limit_wait(None, &json!({"retry_after_ms": 2500})), 2500);
        assert_eq!(super::rate_limit_wait(None, &json!({})), 1000);
        assert_eq!(super::rate_limit_wait(Some(600_000), &json!({})), 10_000);
        assert_eq!(super::rate_limit_wait(None, &json!({"retry_after_ms": 1})), 200);
    }
    #[test]
    fn encryption_cache_keeps_encrypted_rooms_and_rechecks_the_others() {
        let mut cache = std::collections::HashMap::new();
        let old = Instant::now() - UNENCRYPTED_RECHECK - Duration::from_secs(1);
        cache.insert("fresh".to_string(), (false, Instant::now()));
        cache.insert("stale".to_string(), (false, old));
        cache.insert("secure".to_string(), (true, old));
        assert_eq!(known_encryption(&cache, "fresh"), Some(false));
        assert_eq!(known_encryption(&cache, "stale"), None);
        assert_eq!(known_encryption(&cache, "secure"), Some(true));
        assert_eq!(known_encryption(&cache, "unknown"), None);
    }
    use super::*;
    #[test]
    fn v12_creators_have_unlimited_power_and_older_rooms_use_the_list() {
        let create = |version: &str| json!({"type":"m.room.create","state_key":"","sender":"@dono:matrix.org","content":{"room_version":version,"additional_creators":["@socio:matrix.org"]}});
        let levels = json!({"type":"m.room.power_levels","state_key":"","content":{"users":{"@mod:matrix.org":50},"users_default":0}});
        let v12 = [create("12"), levels.clone()];
        assert_eq!(power_of_user(&v12, "@dono:matrix.org").0, i64::MAX);
        assert_eq!(power_of_user(&v12, "@socio:matrix.org").0, i64::MAX);
        assert_eq!(power_of_user(&v12, "@mod:matrix.org").0, 50);
        assert_eq!(power_of_user(&[create("10"), levels], "@dono:matrix.org").0, 0, "antes da v12 vale só a lista");
        assert_eq!(privileged_creators(&v12).unwrap().len(), 2);
        assert!(privileged_creators(&[json!({"type":"m.room.message"})]).is_none());
    }
    #[test]
    fn channels_join_the_space_and_voice_channels_let_everyone_call() {
        let mods = serde_json::Map::from_iter([("@mod:matrix.org".to_string(), json!(50))]);
        let voice = channel_body("Geral de voz", true, &mods, "@dono:matrix.org", "12");
        assert_eq!(voice["creation_content"]["type"], VOICE_ROOM_TYPE);
        assert_eq!(voice["initial_state"][1]["content"]["allow"][0]["room_id"], ROOM, "membros da comunidade podem entrar");
        assert_eq!(voice["power_level_content_override"]["events"]["org.matrix.msc3401.call.member"], 0);
        assert!(voice["power_level_content_override"]["users"].get("@dono:matrix.org").is_none());
        assert_eq!(voice["power_level_content_override"]["users"]["@mod:matrix.org"], 50, "moderadores da comunidade continuam moderadores");
        let text = channel_body("ajuda", false, &mods, "@dono:matrix.org", "12");
        assert!(text["creation_content"].get("type").is_none());
        assert!(text["power_level_content_override"]["events"].get("org.matrix.msc3401.call.member").is_none());
        assert!(channel_name("  ").is_err() && channel_name(&"x".repeat(81)).is_err() && channel_name(" bate-papo ").unwrap() == "bate-papo");
    }
    #[test]
    fn channel_creator_permissions_follow_the_selected_room_version() {
        let users = serde_json::Map::from_iter([("@dono:matrix.org".to_string(), json!(100)), ("@mod:matrix.org".to_string(), json!(50))]);
        for voice in [false, true] {
            for version in ["11", "12"] {
                let body = channel_body("Teste", voice, &users, "@dono:matrix.org", version);
                assert_eq!(body["room_version"], version);
                assert_eq!(body["power_level_content_override"]["users"]["@mod:matrix.org"], 50);
                assert_eq!(body["power_level_content_override"]["users"].get("@dono:matrix.org").and_then(Value::as_i64), if version == "11" { Some(100) } else { None });
            }
        }
    }
    #[test]
    fn channel_errors_show_status_and_safe_codes_without_server_text() {
        let error = channel_creation_error(403, &json!({"errcode":"M_FORBIDDEN","error":"secret-server-text"}));
        assert!(error.contains("HTTP 403, M_FORBIDDEN") && !error.contains("secret-server-text"));
        assert!(channel_creation_error(429, &json!({"errcode":"M_LIMIT_EXCEEDED"})).contains("Aguarde"));
        assert_eq!(channel_creation_error(502, &json!({"errcode":"M_SECRET\nunsafe"})), "Não foi possível criar o canal no Matrix. (HTTP 502)");
    }
    #[test]
    fn room_ids_accept_legacy_and_domainless_formats() {
        assert!(room_path("!RFQsmEfiISJgSfldHh:matrix.org").is_ok());
        assert!(room_path("!_HDT87EAvHDgjlVDmZT0Ny2nRLbBwRHjGw6xLoE3sdY").is_ok());
        for invalid in ["", "!", "#teste2323:matrix.org", "!room with spaces", "!room\n"] {
            assert!(room_path(invalid).is_err(), "accepted invalid room ID: {invalid:?}");
        }
        assert!(room_path(&format!("!{}", "a".repeat(512))).is_err());
    }
    #[test]
    fn timeline_events_become_messages_edits_and_removals() {
        let message = json!({"type":"m.room.message","event_id":"$1","sender":"@ana:matrix.org","origin_server_ts":5,
            "content":{"msgtype":"m.text","body":"> <@bia:matrix.org> testou o mod?\n\nTestei, funciona!","m.relates_to":{"m.in_reply_to":{"event_id":"$0"}}}});
        let parsed = chat_event(&message).unwrap();
        assert_eq!(parsed["kind"], "message");
        assert_eq!(parsed["body"], "Testei, funciona!", "a citação antiga da resposta sai do texto");
        assert_eq!(parsed["reply_to"], "$0");
        let edit = json!({"type":"m.room.message","event_id":"$2","sender":"@ana:matrix.org","content":{"body":"* Testei, funciona bem!","m.new_content":{"body":"Testei, funciona bem!"},"m.relates_to":{"rel_type":"m.replace","event_id":"$1"}}});
        assert_eq!(chat_event(&edit).unwrap(), json!({"kind":"edit","id":"$2","target":"$1","body":"Testei, funciona bem!"}));
        let removal = json!({"type":"m.room.redaction","event_id":"$3","sender":"@mod:matrix.org","redacts":"$1","content":{}});
        assert_eq!(chat_event(&removal).unwrap()["target"], "$1");
        // Already removed: no "shared file" placeholder.
        assert!(chat_event(&json!({"type":"m.room.message","event_id":"$4","sender":"@x:matrix.org","content":{},"unsigned":{"redacted_because":{}}})).is_none());
        let mut names = serde_json::Map::new();
        display_names(&[json!({"type":"m.room.member","state_key":"@ana:matrix.org","content":{"displayname":"Ana"}})], &mut names);
        assert_eq!(names["@ana:matrix.org"], "Ana");
        let power = power_levels(&[json!({"type":"m.room.power_levels","content":{"users":{"@mod:matrix.org":50},"kick":75}})]).unwrap();
        assert_eq!((power["kick"].as_i64(), power["ban"].as_i64(), power["users"]["@mod:matrix.org"].as_i64()), (Some(75), Some(50), Some(50)));
        assert!(room_path("#alias:matrix.org").is_err() && event_path("$abc").is_ok() && !user_valid("ana"));
    }
    #[test]
    fn reactions_unread_counts_and_typing_reach_the_chat() {
        let reaction = json!({"type":"m.reaction","event_id":"$5","sender":"@bia:matrix.org","content":{"m.relates_to":{"rel_type":"m.annotation","event_id":"$1","key":"👍"}}});
        assert_eq!(chat_event(&reaction).unwrap(), json!({"kind":"reaction","id":"$5","target":"$1","key":"👍","sender":"@bia:matrix.org"}));
        assert!(chat_event(&json!({"type":"m.reaction","event_id":"$6","sender":"@bia:matrix.org","content":{"m.relates_to":{"rel_type":"m.reference","event_id":"$1","key":"x"}}})).is_none());
        let sync = json!({"next_batch":"n","rooms":{"join":{"!a:matrix.org":{
            "timeline":{"events":[]},"state":{"events":[]},
            "unread_notifications":{"notification_count":3},
            "ephemeral":{"events":[{"type":"m.typing","content":{"user_ids":["@bia:matrix.org","@me:matrix.org","bad"]}}]}},
            "!b:matrix.org":{"timeline":{"events":[]},"state":{"events":[]}}}}});
        let normalized = normalize_sync(&sync, "@me:matrix.org");
        let room = &normalized["join"]["!a:matrix.org"];
        assert_eq!(room["unread"], 3);
        assert_eq!(room["typing"], json!(["@bia:matrix.org"]), "nem a própria conta nem IDs inválidos");
        assert!(normalized["join"]["!b:matrix.org"]["typing"].is_null(), "sem evento: lista inalterada");
    }
}

/// Prove ownership by sending a short-lived nonce with the local Matrix session.
/// Neither the personal Matrix token nor the Discord session secret enters JS.
#[tauri::command]
pub async fn matrix_chat_premium_access() -> Result<Value, String> {
    let _guard = PREMIUM_LOCK.lock().await;
    let data = session().await?;
    let response = super::discord_account::matrix_premium_request("matrix/challenge", &json!({"user_id":data.user_id})).await?;
    let mut proof_room: Option<String> = None;
    let result: Result<Value, String> = async {
        let grant = if response["granted"] == true { response } else {
            let challenge = &response["challenge"];
            let challenge_room = challenge["room_id"].as_str().ok_or("Confirmação Matrix inválida.")?;
            if challenge["user_id"].as_str() != Some(data.user_id.as_str()) || challenge_room == PREMIUM_ROOM || challenge_room == ROOM {
                return Err("A sessão Discord está vinculada a outra conta Matrix. Reconecte o Discord para trocar a conta.".into());
            }
            let encoded = room_path(challenge_room)?;
            let nonce = challenge["body"].as_str().filter(|value| value.starts_with("PELiGAMES_VERIFY_") && value.len() < 128).ok_or("Confirmação Matrix inválida.")?;
            matrix_chat_join(challenge_room.to_owned()).await?;
            proof_room = Some(encoded.clone());
            if encrypted(&encoded, &data.access_token).await? { return Err("A sala de confirmação não pode ser criptografada.".into()); }
            let sent = checked(client()?.put(format!("{API}/_matrix/client/v3/rooms/{encoded}/send/m.room.message/{}",uuid::Uuid::new_v4())).bearer_auth(&data.access_token).json(&json!({"msgtype":"m.text","body":nonce})).send().await.map_err(|_| "Não foi possível confirmar sua conta Matrix.")?).await?;
            if !sent.status().is_success() { return Err("Não foi possível confirmar sua conta Matrix.".into()); }
            let event: Value = sent.json().await.map_err(|_| "Confirmação Matrix inválida.")?;
            let event_id = event["event_id"].as_str().ok_or("Confirmação Matrix inválida.")?;
            super::discord_account::matrix_premium_request("matrix/confirm", &json!({"event_id":event_id})).await?
        };
        if grant["granted"] != true || grant["room_id"].as_str() != Some(PREMIUM_ROOM) || grant["user_id"].as_str() != Some(data.user_id.as_str()) {
            return Err("A autorização premium pertence a outra sala ou conta Matrix. Reconecte o Discord em Conexões.".into());
        }
        if current().await?.as_ref().map(|session| &session.user_id) != Some(&data.user_id) { return Err("A conta Matrix mudou durante a confirmação. Tente novamente.".into()); }
        matrix_chat_join(PREMIUM_ROOM.to_owned()).await?;
        Ok(json!({"room_id":PREMIUM_ROOM,"user_id":data.user_id,"granted":true}))
    }.await;
    if let Some(encoded) = proof_room {
        // Best-effort cleanup uses the captured session, not a newly signed-in account.
        for action in ["leave", "forget"] {
            let _ = client()?.post(format!("{API}/_matrix/client/v3/rooms/{encoded}/{action}")).bearer_auth(&data.access_token).json(&json!({})).send().await;
        }
    }
    result
}

#[derive(Clone)]
struct PublishedGameActivity { user: String, previous: String, message: String }
static GAME_ACTIVITY: tokio::sync::Mutex<Option<PublishedGameActivity>> = tokio::sync::Mutex::const_new(None);
fn game_status(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 180 || name.chars().any(char::is_control) {
        return Err("Nome de jogo inválido para a atividade Matrix.".into());
    }
    Ok(format!("Jogando {name} · PeliGames"))
}
fn published_game_status(status: &str) -> bool {
    status.starts_with("Jogando ") && status.ends_with(" · PeliGames")
}
/// A standard status message, visible in clients/homeservers supporting presence.
/// Clearing only restores our own message, preserving changes made elsewhere.
#[tauri::command]
pub async fn matrix_chat_game_activity(game_name: Option<String>, user_id: String) -> Result<(), String> {
    let message = game_name.as_deref().map(game_status).transpose()?;
    let mut published = GAME_ACTIVITY.lock().await;
    let data = session().await?;
    if data.user_id != user_id { return Err("A conta Matrix mudou. Atualize a conexão.".into()); }
    let path = format!("/_matrix/client/v3/presence/{}/status", urlencoding::encode(&user_id));
    let status = match get(&path, &data.access_token).await? {
        Some(status) => status,
        None if message.is_none() => return Ok(()),
        None => return Err("Este servidor não disponibiliza atividade Matrix.".into()),
    };
    let existing = status["status_msg"].as_str().unwrap_or("");
    let previous = published.as_ref().filter(|p| p.user == user_id && p.message == existing)
        .map(|p| p.previous.clone()).unwrap_or_else(|| if published_game_status(existing) { String::new() } else { existing.into() });
    if message.is_none() && !published.as_ref().is_some_and(|p| p.user == user_id && p.message == existing)
        && !published_game_status(existing) {
        *published = None;
        return Ok(());
    }
    let next = message.as_deref().unwrap_or(&previous);
    let response = checked(client()?.put(format!("{API}{path}")).bearer_auth(&data.access_token)
        .json(&json!({"presence":"online","status_msg":next})).send().await
        .map_err(|_| "Sem conexão para atualizar a atividade Matrix.")?).await?;
    if !response.status().is_success() { return Err("Não foi possível compartilhar a atividade. O servidor pode não oferecer presença Matrix.".into()); }
    *published = message.map(|message| PublishedGameActivity { user: user_id, previous, message });
    Ok(())
}

fn presence_statuses(events: &[Value]) -> serde_json::Map<String, Value> {
    let mut statuses = serde_json::Map::new();
    for event in events.iter().take(500) {
        let Some(user) = event["sender"].as_str().filter(|user| user_valid(user)) else { continue; };
        if event["type"] != "m.presence" { continue; }
        let content = &event["content"];
        let status = if content["presence"] == "offline" { "" } else { content["status_msg"].as_str().unwrap_or("") };
        statuses.insert(user.into(), status.chars().take(300).collect::<String>().into());
    }
    statuses
}

#[cfg(test)]
mod game_activity_tests {
    use super::*;
    #[test]
    fn validates_activity_and_keeps_plain_text() {
        assert_eq!(game_status("Clair Obscur").unwrap(), "Jogando Clair Obscur · PeliGames");
        assert!(game_status("\n").is_err());
        assert!(game_status(&"x".repeat(181)).is_err());
        assert!(!published_game_status("Meu status pessoal"));
    }
    #[test]
    fn clears_offline_activity_and_preserves_other_statuses() {
        let events = vec![json!({"type":"m.presence","sender":"@player:matrix.org","content":{"presence":"online","status_msg":"Jogando Cars · PeliGames"}}), json!({"type":"m.presence","sender":"@offline:matrix.org","content":{"presence":"offline","status_msg":"Jogando Cars · PeliGames"}})];
        let statuses = presence_statuses(&events);
        assert_eq!(statuses["@player:matrix.org"], "Jogando Cars · PeliGames");
        assert_eq!(statuses["@offline:matrix.org"], "");
    }
}

#[cfg(test)]
mod membership_tests {
    use super::*;
    #[test]
    fn includes_silent_members_and_timeline_leave_wins() {
        let state = vec![json!({"type":"m.room.member","state_key":"@silent:matrix.org","content":{"membership":"join"}}),
            json!({"type":"m.room.member","state_key":"@left:matrix.org","content":{"membership":"join"}})];
        let timeline = vec![json!({"type":"m.room.member","state_key":"@left:matrix.org","content":{"membership":"leave"}}),
            json!({"type":"m.room.member","state_key":"@new:matrix.org","content":{"membership":"join"}})];
        let members = membership_updates(&state, &timeline);
        assert_eq!(members["@silent:matrix.org"], true);
        assert_eq!(members["@left:matrix.org"], false);
        assert_eq!(members["@new:matrix.org"], true);
    }
    #[test]
    fn excludes_invites_bans_and_malformed_events() {
        let events = vec![json!({"type":"m.room.member","state_key":"@invite:matrix.org","content":{"membership":"invite"}}),
            json!({"type":"m.room.member","state_key":"@banned:matrix.org","content":{"membership":"ban"}}),
            json!({"type":"m.room.member","state_key":"@invalid:matrix.org","content":{}})];
        let members = membership_updates(&events, &[]);
        assert_eq!(members["@invite:matrix.org"], false); assert_eq!(members["@banned:matrix.org"], false);
        assert!(!members.contains_key("@invalid:matrix.org")); assert!(membership_updates(&[], &[]).is_empty());
    }
}
