//! "Edit channel" of the community, Discord-style, written straight into the
//! room's Matrix state: name, topic, who may join, and permissions as power
//! levels. Element and every other client read the same state, so changes
//! made here and there stay in sync. The server checks every change again.
use super::{checked, client, get, list, power_of_user, room_path, session, user_valid, API, ROOM};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

const ADMIN: i64 = 100;
/// Creators of v12 rooms have unlimited power; sent to the interface as this.
const UNLIMITED: i64 = 1_000_000;

/// Where a permission lives in `m.room.power_levels`.
enum Key {
    /// A top-level field, with the spec default.
    Field(&'static str, i64),
    /// Event types under `events`; state events default to `state_default`.
    Events(&'static [&'static str], bool),
}
/// The permissions shown, by interface id; `true` = voice channels only.
const PERMISSIONS: [(&str, Key, bool); 10] = [
    ("send", Key::Events(&["m.room.message", "m.sticker"], false), false),
    ("react", Key::Events(&["m.reaction"], false), false),
    ("voice", Key::Events(&["org.matrix.msc3401.call.member", "m.call.member", "org.matrix.msc4143.rtc.member"], true), true),
    ("invite", Key::Field("invite", 0), false),
    ("redact", Key::Field("redact", 50), false),
    ("kick", Key::Field("kick", 50), false),
    ("ban", Key::Field("ban", 50), false),
    ("pin", Key::Events(&["m.room.pinned_events"], true), false),
    ("manage", Key::Events(&["m.room.name", "m.room.topic", "m.room.avatar"], true), false),
    ("permissions", Key::Events(&["m.room.power_levels"], true), false),
];

fn level_of(levels: &Value, key: &Key) -> i64 {
    match key {
        Key::Field(field, default) => levels[*field].as_i64().unwrap_or(*default),
        Key::Events(kinds, state) => levels["events"][kinds[0]].as_i64().unwrap_or_else(|| {
            if *state { levels["state_default"].as_i64().unwrap_or(50) } else { levels["events_default"].as_i64().unwrap_or(0) }
        }),
    }
}
fn set_level(levels: &mut Value, key: &Key, level: i64) {
    match key {
        Key::Field(field, _) => levels[*field] = json!(level),
        Key::Events(kinds, _) => {
            if !levels["events"].is_object() {
                levels["events"] = json!({});
            }
            for kind in *kinds {
                levels["events"][*kind] = json!(level);
            }
        }
    }
}
fn state_content<'a>(events: &'a [Value], kind: &str) -> Option<&'a Value> {
    events.iter().find(|event| event["type"] == kind && event["state_key"] == "").map(|event| &event["content"])
}
fn voice_room(events: &[Value]) -> bool {
    state_content(events, "m.room.create").is_some_and(|create| {
        matches!(create["type"].as_str(), Some("org.matrix.msc3417.call" | "io.element.video"))
    })
}
fn in_community(events: &[Value]) -> bool {
    events.iter().any(|event| event["type"] == "m.space.parent" && event["state_key"] == ROOM && event["content"].as_object().is_some_and(|c| !c.is_empty()))
}

#[derive(Serialize)]
pub struct ChannelSettings {
    name: String,
    topic: String,
    /// "restricted" (community members), "invite" or "public"; another rule as is.
    join_rule: String,
    voice: bool,
    my_power: i64,
    users_default: i64,
    permissions: HashMap<&'static str, i64>,
    /// Members with a level of their own (moderators, administrators).
    users: Vec<(String, i64)>,
    creators: Vec<String>,
    /// Levels needed to change each part (the server enforces them).
    needs: HashMap<&'static str, i64>,
}

fn settings_of(events: &[Value], me: &str) -> ChannelSettings {
    let (mine, levels) = power_of_user(events, me);
    let creators = super::privileged_creators(events).unwrap_or_default();
    let voice = voice_room(events);
    let permissions = PERMISSIONS.iter()
        .filter(|(_, _, voice_only)| voice || !voice_only)
        .map(|(id, key, _)| (*id, level_of(&levels, key)))
        .collect();
    let mut users: Vec<(String, i64)> = levels["users"].as_object().into_iter().flatten()
        .filter_map(|(user, level)| Some((user.clone(), level.as_i64()?)))
        .collect();
    users.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let state = |kind: &str| levels["events"][kind].as_i64().unwrap_or_else(|| levels["state_default"].as_i64().unwrap_or(50));
    ChannelSettings {
        name: state_content(events, "m.room.name").and_then(|c| c["name"].as_str()).unwrap_or_default().to_string(),
        topic: state_content(events, "m.room.topic").and_then(|c| c["topic"].as_str()).unwrap_or_default().to_string(),
        join_rule: state_content(events, "m.room.join_rules").and_then(|c| c["join_rule"].as_str()).unwrap_or("invite").to_string(),
        voice,
        my_power: mine.min(UNLIMITED),
        users_default: levels["users_default"].as_i64().unwrap_or(0),
        permissions,
        users,
        creators,
        needs: HashMap::from([
            ("name", state("m.room.name")),
            ("topic", state("m.room.topic")),
            ("join_rules", state("m.room.join_rules")),
            ("power_levels", state("m.room.power_levels")),
        ]),
    }
}

async fn room_state(room: &str, token: &str) -> Result<Vec<Value>, String> {
    let state = get(&format!("/_matrix/client/v3/rooms/{}/state", room_path(room)?), token).await?
        .ok_or("Não foi possível ler as configurações do canal. Entre no canal primeiro.")?;
    Ok(list(&state).to_vec())
}

#[tauri::command]
pub async fn matrix_channel_settings(room: String) -> Result<ChannelSettings, String> {
    let data = session().await?;
    let events = room_state(&room, &data.access_token).await?;
    if room != ROOM && !in_community(&events) {
        return Err("Este canal não faz parte da comunidade.".into());
    }
    Ok(settings_of(&events, &data.user_id))
}

#[derive(Deserialize, Default)]
pub struct ChannelChange {
    name: Option<String>,
    topic: Option<String>,
    join_rule: Option<String>,
    #[serde(default)]
    permissions: HashMap<String, i64>,
    /// New level per member; `null` takes the member back to the default.
    #[serde(default)]
    users: HashMap<String, Option<i64>>,
}

/// The new power levels, or why the change is not allowed for someone at `mine`.
fn changed_levels(mut levels: Value, voice: bool, mine: i64, change: &ChannelChange) -> Result<Value, String> {
    let allowed = |level: i64| (0..=ADMIN).contains(&level) && level <= mine;
    for (id, level) in &change.permissions {
        let (_, key, voice_only) = PERMISSIONS.iter().find(|(known, _, _)| known == id).ok_or("Permissão desconhecida.")?;
        if *voice_only && !voice {
            return Err("Esta permissão vale só para canais de voz.".into());
        }
        if !allowed(*level) || level_of(&levels, key) > mine {
            return Err("Você não pode definir uma permissão acima do seu próprio cargo.".into());
        }
        set_level(&mut levels, key, *level);
    }
    if !change.users.is_empty() && !levels["users"].is_object() {
        levels["users"] = json!({});
    }
    for (user, level) in &change.users {
        if !user_valid(user) {
            return Err("Conta Matrix inválida.".into());
        }
        let current = levels["users"][user.as_str()].as_i64().unwrap_or_else(|| levels["users_default"].as_i64().unwrap_or(0));
        if current >= mine || level.is_some_and(|level| !allowed(level) || level >= mine && mine < UNLIMITED) {
            return Err("Você só pode mudar o cargo de quem está abaixo de você, e até abaixo do seu.".into());
        }
        match level {
            Some(level) => levels["users"][user.as_str()] = json!(level),
            None => {
                if let Some(users) = levels["users"].as_object_mut() {
                    users.remove(user.as_str());
                }
            }
        }
    }
    Ok(levels)
}

async fn put_state(room: &str, kind: &str, content: &Value, token: &str, what: &str) -> Result<(), String> {
    let url = format!("{API}/_matrix/client/v3/rooms/{}/state/{kind}/", room_path(room)?);
    let response = checked(client()?.put(url).bearer_auth(token).json(content).send().await
        .map_err(|_| format!("Não foi possível salvar {what}."))?).await?;
    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(format!("O Matrix não autorizou mudar {what}: seu cargo não permite."));
    }
    if !response.status().is_success() {
        return Err(format!("O Matrix recusou {what} (HTTP {}).", response.status().as_u16()));
    }
    Ok(())
}

#[tauri::command]
pub async fn matrix_channel_update(room: String, change: ChannelChange) -> Result<ChannelSettings, String> {
    let data = session().await?;
    let token = &data.access_token;
    let events = room_state(&room, token).await?;
    if room != ROOM && !in_community(&events) {
        return Err("Este canal não faz parte da comunidade.".into());
    }
    if let Some(name) = &change.name {
        let name = super::channel_name(name)?;
        put_state(&room, "m.room.name", &json!({"name": name}), token, "o nome").await?;
    }
    if let Some(topic) = &change.topic {
        let topic = topic.trim();
        if topic.chars().count() > 1024 || topic.chars().any(|c| c.is_control() && c != '\n') {
            return Err("Use um tópico de até 1024 caracteres.".into());
        }
        put_state(&room, "m.room.topic", &json!({"topic": topic}), token, "o tópico").await?;
    }
    if let Some(rule) = &change.join_rule {
        let content = match rule.as_str() {
            "restricted" => json!({"join_rule":"restricted","allow":[{"type":"m.room_membership","room_id":ROOM}]}),
            "invite" => json!({"join_rule":"invite"}),
            "public" => json!({"join_rule":"public"}),
            _ => return Err("Regra de entrada inválida.".into()),
        };
        if room == ROOM {
            return Err("A entrada na comunidade se configura pelo Element.".into());
        }
        put_state(&room, "m.room.join_rules", &content, token, "quem pode entrar").await?;
    }
    if !change.permissions.is_empty() || !change.users.is_empty() {
        let (mine, levels) = power_of_user(&events, &data.user_id);
        let levels = changed_levels(levels, voice_room(&events), mine, &change)?;
        put_state(&room, "m.room.power_levels", &levels, token, "as permissões").await?;
    }
    Ok(settings_of(&room_state(&room, token).await?, &data.user_id))
}

/// Invites someone to a channel, and to the community so it shows up for them.
#[tauri::command]
pub async fn matrix_channel_invite(room: String, user: String) -> Result<(), String> {
    let user = user.trim();
    if !user_valid(user) {
        return Err("Use a conta Matrix completa, como @nome:matrix.org.".into());
    }
    let data = session().await?;
    let invite = |target: String| {
        let token = data.access_token.clone();
        let user = user.to_string();
        async move {
            let url = format!("{API}/_matrix/client/v3/rooms/{}/invite", room_path(&target)?);
            checked(client()?.post(url).bearer_auth(token).json(&json!({"user_id": user})).send().await
                .map_err(|_| "Não foi possível enviar o convite.".to_string())?).await
        }
    };
    let response = invite(room.clone()).await?;
    if response.status() == reqwest::StatusCode::FORBIDDEN {
        let body = response.json::<Value>().await.unwrap_or(Value::Null);
        // Already in the room, or already invited: nothing to do.
        if body["error"].as_str().is_some_and(|text| text.contains("already")) {
            return Ok(());
        }
        return Err("O Matrix não autorizou o convite: seu cargo não permite convidar neste canal.".into());
    }
    if !response.status().is_success() {
        return Err("O Matrix recusou o convite. Confira a conta informada.".into());
    }
    if room != ROOM {
        let _ = invite(ROOM.to_string()).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> Value {
        json!({"users":{"@admin:x":100,"@mod:x":50},"users_default":0,"kick":50,"ban":50,"redact":50,"state_default":50,
            "events":{"m.room.power_levels":100,"m.room.name":50,"org.matrix.msc3401.call.member":0}})
    }

    #[test]
    fn permissions_read_like_matrix_defaults() {
        let levels = levels();
        let find = |id: &str| &PERMISSIONS.iter().find(|(known, _, _)| *known == id).unwrap().1;
        assert_eq!(level_of(&levels, find("send")), 0);
        assert_eq!(level_of(&levels, find("invite")), 0);
        assert_eq!(level_of(&levels, find("pin")), 50);
        assert_eq!(level_of(&levels, find("permissions")), 100);
        assert_eq!(level_of(&levels, find("voice")), 0);
    }

    #[test]
    fn changes_stay_below_the_editor() {
        let change = ChannelChange { permissions: HashMap::from([("send".into(), 50)]), ..Default::default() };
        let next = changed_levels(levels(), false, 50, &change).unwrap();
        assert_eq!(next["events"]["m.room.message"], 50);
        assert_eq!(next["events"]["m.sticker"], 50);
        // Kept untouched: what the launcher does not show.
        assert_eq!(next["events"]["m.room.power_levels"], 100);
        let above = ChannelChange { permissions: HashMap::from([("kick".into(), 100)]), ..Default::default() };
        assert!(changed_levels(levels(), false, 50, &above).is_err());
        let voice = ChannelChange { permissions: HashMap::from([("voice".into(), 50)]), ..Default::default() };
        assert!(changed_levels(levels(), false, 100, &voice).is_err());
        assert_eq!(changed_levels(levels(), true, 100, &voice).unwrap()["events"]["m.call.member"], 50);
    }

    #[test]
    fn roles_change_only_below_the_editor() {
        let promote = ChannelChange { users: HashMap::from([("@new:x".into(), Some(50))]), ..Default::default() };
        assert_eq!(changed_levels(levels(), false, 100, &promote).unwrap()["users"]["@new:x"], 50);
        // A moderator cannot make another moderator, nor touch one.
        assert!(changed_levels(levels(), false, 50, &promote).is_err());
        let demote = ChannelChange { users: HashMap::from([("@mod:x".into(), None)]), ..Default::default() };
        assert!(changed_levels(levels(), false, 50, &demote).is_err());
        assert!(changed_levels(levels(), false, 100, &demote).unwrap()["users"].get("@mod:x").is_none());
        // A room creator (unlimited) may make administrators.
        let admin = ChannelChange { users: HashMap::from([("@new:x".into(), Some(100))]), ..Default::default() };
        assert!(changed_levels(levels(), false, 100, &admin).is_err());
        assert!(changed_levels(levels(), false, UNLIMITED, &admin).is_ok());
    }
}
