//! Basic community chat. Tokens stay in libsecret; encrypted rooms are never sent plaintext.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::{Duration, Instant};
const API: &str = "https://matrix-client.matrix.org";
const PREMIUM_ROOM: &str = "!d6uopxWbqNBgbKYC8uTxjgvzIP4F955uQi3rerKKX2Q";
static PREMIUM_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const ROOM: &str = "!pNHMqrnxSAmcVFqrgf:matrix.org";
const EXPIRED: &str = "Sua sessão Matrix expirou ou foi encerrada. Entre novamente.";
static AUTH: tokio::sync::Mutex<bool> = tokio::sync::Mutex::const_new(false);
static AUTH_ERROR: tokio::sync::Mutex<Option<String>> = tokio::sync::Mutex::const_new(None);
// Read once from the keyring; every request afterwards uses this copy.
static SESSION: tokio::sync::Mutex<Option<Session>> = tokio::sync::Mutex::const_new(None);
#[derive(Clone, Serialize, Deserialize)]
struct Session { access_token: String, user_id: String }
async fn stored(action: &'static str, value: Option<String>) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || super::nexus_account::credential_for(action, value.as_deref(), "matrix-community-chat", "PeliGames — Matrix"))
        .await.map_err(|_| "Chaveiro Matrix indisponível.".to_string())?
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().timeout(Duration::from_secs(20)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_| "Conexão Matrix indisponível.".into())
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
async fn session() -> Result<Session, String> {
    current().await?.ok_or_else(|| "Entre com Matrix para acessar o chat.".into())
}
/// The server no longer accepts the token (signed out elsewhere, revoked):
/// the local session goes away, so the chat offers the sign-in again.
async fn forget() -> Result<(), String> {
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
fn room_path(room: &str) -> Result<String, String> {
    // Room version 12 IDs are opaque hashes without the legacy :server suffix.
    if !room.starts_with('!') || room.len() < 2 || room.len() > 512 || room.chars().any(|c| c.is_control() || c.is_whitespace()) { return Err("Sala Matrix inválida.".into()); }
    Ok(urlencoding::encode(room).into_owned())
}
fn event_path(event: &str) -> Result<String, String> {
    if !event.starts_with('$') || event.len() > 512 || event.chars().any(char::is_control) { return Err("Mensagem Matrix inválida.".into()); }
    Ok(urlencoding::encode(event).into_owned())
}
fn user_valid(user: &str) -> bool { user.starts_with('@') && user.contains(':') && user.len() <= 255 && !user.chars().any(char::is_control) }
async fn encrypted(encoded: &str, token: &str) -> Result<bool, String> {
    Ok(get(&format!("/_matrix/client/v3/rooms/{encoded}/state/m.room.encryption"), token).await?.is_some())
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
                        let mut buffer = [0_u8; 8192];
                        let Ok(count) = stream.read(&mut buffer) else { continue; };
                        let first = String::from_utf8_lossy(&buffer[..count]).lines().next().unwrap_or("").to_string();
                        let mut parts = first.split_whitespace();
                        if parts.next() != Some("GET") { continue; }
                        let Some(target) = parts.next() else { continue; };
                        let Ok(url) = reqwest::Url::parse(&format!("http://127.0.0.1:{port}{target}")) else { continue; };
                        let token = url.query_pairs().find(|(key,_)| key == "loginToken").map(|(_,value)|value.into_owned());
                        let Some(token) = token.filter(|token| url.path() == path && !token.is_empty() && token.len() <= 4096) else {
                            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                            continue;
                        };
                        let body = "<!doctype html><meta charset=utf-8><title>PeliGames Matrix</title><p>Retorno recebido. Volte ao PeliGames para acompanhar a conexão. Esta página pode ser fechada.</p>";
                        let _ = write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{}",body.len(),body);
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
pub async fn disconnect_matrix_chat() -> Result<(), String> {
    if *AUTH.lock().await { return Err("Aguarde o login atual concluir ou expirar.".into()); }
    let data = session().await?;
    let response = client()?.post(format!("{API}/_matrix/client/v3/logout")).bearer_auth(&data.access_token).send().await.map_err(|_| "Sem conexão com o Matrix. Tente desconectar novamente.")?;
    // A token the server no longer accepts is already signed out.
    if !response.status().is_success() && response.status() != reqwest::StatusCode::UNAUTHORIZED { return Err("Não foi possível revogar a sessão Matrix.".into()); }
    forget().await?;
    *AUTH_ERROR.lock().await = None;
    Ok(())
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

const TYPES: [&str; 5] = ["m.room.message", "m.room.redaction", "m.room.member", "m.room.encryption", "m.room.power_levels"];
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
/// Who may remove messages, remove or ban people: the latest power levels of a room.
fn power_levels(events: &[Value]) -> Option<Value> {
    let content = &events.iter().rev().find(|event| event["type"] == "m.room.power_levels")?["content"];
    let level = |key: &str, default: i64| content[key].as_i64().unwrap_or(default);
    Some(json!({"users": content["users"].as_object().cloned().unwrap_or_default(), "users_default": level("users_default", 0),
        "redact": level("redact", 50), "kick": level("kick", 50), "ban": level("ban", 50)}))
}
fn has_encryption(events: &[Value]) -> bool { events.iter().any(|event| event["type"] == "m.room.encryption") }
fn list(value: &Value) -> &[Value] { value.as_array().map(Vec::as_slice).unwrap_or(&[]) }

/// Long-polled `/sync` limited to the community rooms. The first call (no
/// `since`) returns recent history; later ones wait up to `timeout` ms for news.
#[tauri::command]
pub async fn matrix_chat_sync(since: Option<String>, rooms: Vec<String>, timeout: u32) -> Result<Value, String> {
    let data = session().await?;
    if rooms.len() > 60 { return Err("Salas demais na comunidade.".into()); }
    for room in &rooms { room_path(room)?; }
    let filter = json!({
        "presence":{"not_types":["*"]},
        "account_data":{"types":["m.ignored_user_list"]},
        "room":{"rooms":rooms,"timeline":{"limit":50,"types":TYPES},"state":{"lazy_load_members":true,"types":["m.room.member","m.room.encryption","m.room.power_levels"]},
            "ephemeral":{"not_types":["*"]},"account_data":{"not_types":["*"]}}
    });
    let timeout = timeout.min(30000);
    let mut url = format!("{API}/_matrix/client/v3/sync?timeout={timeout}&filter={}", urlencoding::encode(&filter.to_string()));
    if let Some(since) = since.filter(|value| !value.is_empty() && value.len() < 1024) { url.push_str(&format!("&since={}", urlencoding::encode(&since))); }
    let response = checked(client()?.get(url).bearer_auth(&data.access_token).timeout(Duration::from_millis(u64::from(timeout) + 20000)).send().await.map_err(|_| "Sem conexão com o Matrix.")?).await?;
    if !response.status().is_success() { return Err("Não foi possível atualizar o chat Matrix.".into()); }
    let value: Value = response.json().await.map_err(|_| "Resposta Matrix inválida.")?;
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
            "power": power,
        }));
    }
    let left: Vec<&String> = value["rooms"]["leave"].as_object().map(|rooms| rooms.keys().collect()).unwrap_or_default();
    let ignored = list(&value["account_data"]["events"]).iter().find(|event| event["type"] == "m.ignored_user_list")
        .map(|event| event["content"]["ignored_users"].as_object().map(|users| users.keys().cloned().collect::<Vec<_>>()).unwrap_or_default());
    Ok(json!({"next_batch": value["next_batch"], "user_id": data.user_id, "join": joined, "leave": left, "ignored": ignored}))
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
    let response = checked(client()?.post(format!("{API}/_matrix/media/v3/upload?filename={}",urlencoding::encode(&name))).bearer_auth(&data.access_token).header(reqwest::header::CONTENT_TYPE,&mime).body(bytes).send().await.map_err(|_| "Não foi possível enviar o arquivo.")?).await?;
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
    use super::*;
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
