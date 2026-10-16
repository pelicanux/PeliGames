//! Playtime of the launcher's own games (PeliGames library): total minutes and
//! the last start, recorded when a game session ends. Other libraries keep their
//! own records (Steam, Heroic, Lutris), read by commands::game_activity.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// Unix seconds of the last session start.
    pub last_played: u64,
    pub minutes: u64,
}

static WRITE: Mutex<()> = Mutex::new(());

fn file() -> Result<PathBuf, String> {
    Ok(super::paths::app_root()?.join("playtime.json"))
}

fn read_at(path: &Path) -> BTreeMap<String, Record> {
    fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

/// Every recorded game, keyed by its library entry path.
pub fn read() -> BTreeMap<String, Record> {
    file().map(|path| read_at(&path)).unwrap_or_default()
}

fn record_at(path: &Path, entry: &str, started: SystemTime, played: Duration) -> Result<(), String> {
    let _guard = WRITE.lock().map_err(|_| "Registro de tempo de jogo indisponível.")?;
    let mut records = read_at(path);
    let item = records.entry(entry.to_string()).or_default();
    item.last_played = item.last_played.max(started.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
    item.minutes += played.as_secs() / 60;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&records).map_err(|e| e.to_string())?)
        .and_then(|_| fs::rename(&temporary, path))
        .map_err(|e| e.to_string())
}

/// Adds a finished session. Sessions shorter than a minute still update the
/// last start, so a game opened briefly shows as recently played.
pub fn record(entry: &str, started: SystemTime, played: Duration) {
    if let Err(error) = file().and_then(|path| record_at(&path, entry, started, played)) {
        eprintln!("Tempo de jogo de {entry}: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sessions_add_minutes_and_keep_the_latest_start() {
        let path = std::env::temp_dir().join(format!("peligames-playtime-{}.json", uuid::Uuid::new_v4()));
        let earlier = UNIX_EPOCH + Duration::from_secs(1_000);
        let later = UNIX_EPOCH + Duration::from_secs(5_000);
        record_at(&path, "peligames:a", later, Duration::from_secs(90 * 60)).unwrap();
        record_at(&path, "peligames:a", earlier, Duration::from_secs(30 * 60 + 59)).unwrap();
        record_at(&path, "peligames:b", earlier, Duration::from_secs(20)).unwrap();
        let records = read_at(&path);
        assert_eq!(records["peligames:a"], Record { last_played: 5_000, minutes: 120 });
        assert_eq!(records["peligames:b"], Record { last_played: 1_000, minutes: 0 });
        fs::remove_file(path).unwrap();
    }
}
