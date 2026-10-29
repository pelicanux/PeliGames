use std::{path::Path, sync::Mutex};

static LOG_LOCK: Mutex<()> = Mutex::new(());

fn append_log(log_dir: &Path, level: &str, message: &str) -> std::io::Result<()> {
    let _guard = LOG_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    std::fs::create_dir_all(log_dir)?;
    let log_file = log_dir.join("launcher.log");
    let timestamp = timestamp();
    let level = match level {
        "ERROR" => "ERROR",
        "WARN" => "WARN",
        _ => "INFO",
    };
    // Keep each event on one bounded line, even for multiline installer errors.
    let message: String = message
        .chars()
        .take(4096)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    super::bounded_logs::append(
        &log_file,
        format!("[{timestamp}] [{level}] {message}\n").as_bytes(),
    )
}

fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    #[cfg(unix)]
    {
        let raw = now as libc::time_t;
        let mut time: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&raw, &mut time) }.is_null() {
            return format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                time.tm_year + 1900,
                time.tm_mon + 1,
                time.tm_mday,
                time.tm_hour,
                time.tm_min,
                time.tm_sec
            );
        }
    }
    now.to_string()
}
pub fn log_launcher(_app: &tauri::AppHandle, level: &str, message: &str) {
    event(level, message);
}
pub fn event(level: &str, message: &str) {
    let Ok(config_dir) = super::paths::app_path("logs") else {
        return;
    };
    let _ = append_log(&config_dir, level, message);
}

pub fn startup() {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "não informado".into());
    let session = std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "não informada".into());
    event("INFO",&format!("PeliGames {} iniciado; sistema={}/{}; interface={}; sessão={}; logs: até 2 MiB por arquivo, retenção por tamanho",env!("CARGO_PKG_VERSION"),std::env::consts::OS,std::env::consts::ARCH,desktop.chars().take(64).collect::<String>(),session.chars().take(32).collect::<String>()));
}
pub fn log_result<T>(app: &tauri::AppHandle, operation: &str, result: &Result<T, String>) {
    match result {
        Ok(_) => log_launcher(app, "INFO", &format!("{operation}: concluído")),
        Err(error) => log_launcher(app, "ERROR", &format!("{operation}: {error}")),
    }
}

#[derive(serde::Serialize)]
pub struct LauncherLog {
    pub path: String,
    pub content: String,
}

pub fn read_launcher_log() -> Result<LauncherLog, String> {
    let root = super::paths::app_root()?;
    let _guard = LOG_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let path = super::paths::layout::join(&root,"launcher.log");
    let content=super::bounded_logs::read(&path).map_err(|e|e.to_string())?;
    Ok(LauncherLog {
        path: path.to_string_lossy().into_owned(),
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn general_log_preserves_recent_events_with_readable_dates() {
        let root = std::env::temp_dir().join(format!(
            "peligames-general-log-test-{}",
            uuid::Uuid::new_v4()
        ));
        for n in 0..1005 {
            append_log(&root, "INFO", &format!("event {n}")).unwrap();
        }
        append_log(&root, "ERROR", "failure\nnext line").unwrap();
        let text = std::fs::read_to_string(root.join("launcher.log")).unwrap();
        assert_eq!(text.lines().count(), 1006);
        assert!(text.contains("event 0\n"));
        assert!(text.ends_with("[ERROR] failure next line\n"));
        assert!(text.lines().next().unwrap().as_bytes()[5] == b'-');
        assert!(text.len() <= super::super::bounded_logs::MAX_BYTES);
        std::fs::remove_dir_all(root).unwrap();
    }
}
