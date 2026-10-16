//! Last play and total playtime of every library, read-only from each
//! launcher's own records: Steam (localconfig.vdf), Heroic (timestamp.json),
//! Lutris (pga.db) and the launcher's own playtime.json.
use crate::core::playtime::Record;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Keys: `Steam:<appid>`, `Heroic:<native|flatpak>:<appName>`,
/// `Lutris:<native|flatpak>:<id>` and `PeliGames:<entry path>`.
type Activity = BTreeMap<String, Record>;

fn merge(activity: &mut Activity, key: String, record: Record) {
    if record.last_played == 0 && record.minutes == 0 { return; }
    let item = activity.entry(key).or_default();
    item.last_played = item.last_played.max(record.last_played);
    item.minutes = item.minutes.max(record.minutes);
}

/// Tokens of a text VDF file: quoted strings and braces.
fn vdf_tokens(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' | b'}' => { tokens.push(&text[i..i + 1]); i += 1; }
            b'"' => {
                let start = i + 1;
                i = start;
                while i < bytes.len() && bytes[i] != b'"' { i += if bytes[i] == b'\\' { 2 } else { 1 }; }
                tokens.push(&text[start..i.min(bytes.len())]);
                i += 1;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => { while i < bytes.len() && bytes[i] != b'\n' { i += 1; } }
            _ => i += 1,
        }
    }
    tokens
}

/// `LastPlayed` (unix seconds) and `Playtime` (minutes) of every block under an `apps` block.
fn steam_records(text: &str) -> BTreeMap<String, Record> {
    let mut result = BTreeMap::<String, Record>::new();
    let mut path: Vec<&str> = Vec::new();
    let mut pending: Option<&str> = None;
    for token in vdf_tokens(text) {
        match token {
            "{" => path.push(pending.take().unwrap_or_default()),
            "}" => { path.pop(); pending = None; }
            value => match pending.take() {
                None => pending = Some(value),
                Some(key) => {
                    let [.., apps, id] = path.as_slice() else { continue };
                    if !apps.eq_ignore_ascii_case("apps") || id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) { continue; }
                    let number = value.parse::<u64>().unwrap_or(0);
                    let item = result.entry(id.to_string()).or_default();
                    if key.eq_ignore_ascii_case("LastPlayed") { item.last_played = item.last_played.max(number); }
                    if key.eq_ignore_ascii_case("Playtime") { item.minutes = item.minutes.max(number); }
                }
            },
        }
    }
    result.retain(|_, record| record.last_played > 0 || record.minutes > 0);
    result
}

fn steam(home: &Path, activity: &mut Activity) {
    for root in [".local/share/Steam", ".steam/steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam", ".var/app/com.valvesoftware.Steam/data/Steam"] {
        let Ok(users) = fs::read_dir(home.join(root).join("userdata")) else { continue };
        for user in users.flatten() {
            let Ok(text) = fs::read_to_string(user.path().join("config/localconfig.vdf")) else { continue };
            for (id, record) in steam_records(&text) { merge(activity, format!("Steam:{id}"), record); }
        }
    }
}

/// Unix seconds of an ISO 8601 UTC date (`2026-10-14T01:02:37.545Z`).
fn iso_seconds(value: &str) -> Option<u64> {
    let number = |range: std::ops::Range<usize>| value.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13).unwrap_or(0), number(14..16).unwrap_or(0), number(17..19).unwrap_or(0));
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) { return None; }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hour * 3_600 + minute * 60 + second).ok()
}

fn heroic_records(text: &str) -> BTreeMap<String, Record> {
    let Ok(serde_json::Value::Object(games)) = serde_json::from_str(text) else { return BTreeMap::new() };
    games.into_iter().filter_map(|(id, game)| {
        let last_played = game.get("lastPlayed").and_then(|v| v.as_str()).and_then(iso_seconds).unwrap_or(0);
        let minutes = game.get("totalPlayed").and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0) as u64;
        Some((id, Record { last_played, minutes }))
    }).collect()
}

fn heroic(home: &Path, activity: &mut Activity) {
    for (root, kind) in [(".config/heroic", "native"), (".var/app/com.heroicgameslauncher.hgl/config/heroic", "flatpak")] {
        let Ok(text) = fs::read_to_string(home.join(root).join("store/timestamp.json")) else { continue };
        for (id, record) in heroic_records(&text) { merge(activity, format!("Heroic:{kind}:{id}"), record); }
    }
}

/// Lutris keeps `lastplayed` in unix seconds and `playtime` in hours.
fn lutris_records(db: &Path) -> rusqlite::Result<Vec<(String, Record)>> {
    use rusqlite::{types::Value, Connection, OpenFlags};
    let connection = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    connection.busy_timeout(std::time::Duration::from_secs(1))?;
    connection.pragma_update(None, "query_only", true)?;
    let number = |value: Value| match value {
        Value::Integer(number) => number as f64,
        Value::Real(number) => number,
        Value::Text(text) => text.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    };
    let mut query = connection.prepare("SELECT id, lastplayed, playtime FROM games LIMIT 5000")?;
    let rows = query.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?.to_string(), Record {
            last_played: number(row.get(1)?).max(0.0) as u64,
            minutes: (number(row.get(2)?).max(0.0) * 60.0).round() as u64,
        }))
    })?;
    Ok(rows.flatten().collect())
}

fn lutris(home: &Path, activity: &mut Activity) {
    for (db, kind) in [(".local/share/lutris/pga.db", "native"), (".var/app/net.lutris.Lutris/data/lutris/pga.db", "flatpak")] {
        let path: PathBuf = home.join(db);
        if !path.is_file() { continue; }
        for (id, record) in lutris_records(&path).unwrap_or_default() { merge(activity, format!("Lutris:{kind}:{id}"), record); }
    }
}

fn collect() -> Activity {
    let mut activity = Activity::new();
    if let Some(home) = dirs::home_dir() {
        steam(&home, &mut activity);
        heroic(&home, &mut activity);
        lutris(&home, &mut activity);
    }
    for (path, record) in crate::core::playtime::read() { merge(&mut activity, format!("PeliGames:{path}"), record); }
    activity
}

#[tauri::command]
pub async fn game_activity() -> Result<Activity, String> {
    tauri::async_runtime::spawn_blocking(collect).await.map_err(|_| "Não foi possível ler o tempo de jogo.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steam_reads_apps_blocks_only() {
        let text = r#""UserLocalConfigStore" { "Software" { "Valve" { "Steam" { "apps" {
            "240" { "LastPlayed" "1788099175" "Playtime" "599" "cloud" { "last_sync_state" "synchronized" } }
            "480" { "LastPlayed"		"1781835908" }
            "999" { "badge" "1" }
        } } } } "friends" { "LastPlayed" "5" } }"#;
        let records = steam_records(text);
        assert_eq!(records["240"], Record { last_played: 1_788_099_175, minutes: 599 });
        assert_eq!(records["480"], Record { last_played: 1_781_835_908, minutes: 0 });
        assert_eq!(records.len(), 2);
    }
    #[test]
    fn heroic_dates_become_unix_seconds() {
        assert_eq!(iso_seconds("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(iso_seconds("2026-10-14T01:02:37.545Z"), Some(1_791_939_757));
        assert_eq!(iso_seconds(""), None);
        let records = heroic_records(r#"{"a":{"firstPlayed":"","lastPlayed":"2000-03-01T00:00:00Z","totalPlayed":1145},"b":{"lastPlayed":""}}"#);
        assert_eq!(records["a"], Record { last_played: 951_868_800, minutes: 1145 });
        assert_eq!(records["b"], Record::default());
    }
}
