//! A private process group plus an inherited operation identity; never global Wine kills.
use super::{output, Progress};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    os::unix::{fs::MetadataExt, io::AsRawFd, process::CommandExt},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Clone)]
struct Process {
    pid: i32,
    parent: i32,
    group: i32,
    birth: u64,
    marked: bool,
}
fn processes(marker: &str) -> Vec<Process> {
    let identity = format!("PELIGAMES_WINE_TOOL_ID={marker}");
    let mut result = vec![];
    let Ok(entries) = fs::read_dir("/proc") else {
        return result;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let root = entry.path();
        if fs::metadata(&root)
            .map(|m| m.uid() != unsafe { libc::geteuid() })
            .unwrap_or(true)
        {
            continue;
        }
        let Ok(stat) = fs::read_to_string(root.join("stat")) else {
            continue;
        };
        let Some((_, fields)) = stat.rsplit_once(") ") else {
            continue;
        };
        let fields: Vec<_> = fields.split_whitespace().collect();
        if fields.len() < 20 || fields[0] == "Z" {
            continue;
        }
        let (Ok(parent), Ok(group), Ok(birth)) =
            (fields[1].parse(), fields[2].parse(), fields[19].parse())
        else {
            continue;
        };
        let marked = fs::read(root.join("environ"))
            .unwrap_or_default()
            .split(|b| *b == 0)
            .any(|v| v == identity.as_bytes());
        result.push(Process {
            pid,
            parent,
            group,
            birth,
            marked,
        });
    }
    result
}
fn signal_process(process: &Process, signal: i32) {
    use std::os::fd::{FromRawFd, OwnedFd};
    // Bind the signal to this process, even if the numeric PID is subsequently reused.
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, process.pid, 0) };
    if raw < 0 {
        return;
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw as i32) };
    let birth = fs::read_to_string(format!("/proc/{}/stat", process.pid))
        .ok()
        .and_then(|s| {
            s.rsplit_once(") ")
                .and_then(|(_, fields)| fields.split_whitespace().nth(19)?.parse::<u64>().ok())
        });
    if birth == Some(process.birth) {
        unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd.as_raw_fd(),
                signal,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            );
        }
    }
}
fn reader(
    pipe: impl Read + AsRawFd + Send + 'static,
    stop: Arc<AtomicBool>,
    progress: Progress,
    log: Arc<Mutex<crate::core::bounded_logs::LogWriter>>,
) -> Result<std::thread::JoinHandle<()>, String> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(std::thread::spawn(move || {
        let _ = output::records_until(pipe, &stop, |line| {
            if let Ok(mut file) = log.lock() {
                let _ = writeln!(file, "{line}");
            }
            progress(line);
        });
    }))
}
pub fn run(
    mut cmd: Command,
    marker: String,
    cancel: Arc<AtomicBool>,
    progress: Progress,
    log: Arc<Mutex<crate::core::bounded_logs::LogWriter>>,
) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        return Err("PELIGAMES_WINE_TOOL_CANCELLED".into());
    }
    cmd.process_group(0).env("PELIGAMES_WINE_TOOL_ID", &marker);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Não foi possível iniciar a ferramenta: {e}"))?;
    let root = child.id() as i32;
    let stop = Arc::new(AtomicBool::new(false));
    let out = reader(
        child.stdout.take().ok_or("Saída indisponível.")?,
        stop.clone(),
        progress.clone(),
        log.clone(),
    );
    let err = reader(
        child.stderr.take().ok_or("Registro indisponível.")?,
        stop.clone(),
        progress.clone(),
        log,
    );
    let (out, err) = match (out, err) {
        (Ok(out), Ok(err)) => (out, err),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            stop.store(true, Ordering::Release);
            return Err("Não foi possível acompanhar a ferramenta.".into());
        }
    };
    let mut tracked = BTreeSet::new();
    let mut exit = None;
    let mut stopping = None;
    let result = loop {
        let all = processes(&marker);
        loop {
            let before = tracked.len();
            for p in &all {
                if p.marked
                    || (exit.is_none() && p.group == root)
                    || all.iter().any(|parent| {
                        parent.pid == p.parent && tracked.contains(&(parent.pid, parent.birth))
                    })
                {
                    tracked.insert((p.pid, p.birth));
                }
            }
            if tracked.len() == before {
                break;
            }
        }
        let owned: Vec<_> = all
            .iter()
            .filter(|p| tracked.contains(&(p.pid, p.birth)))
            .collect();
        if cancel.load(Ordering::Acquire) {
            let since = stopping.get_or_insert_with(Instant::now);
            let signal = if since.elapsed() < Duration::from_secs(2) {
                libc::SIGTERM
            } else {
                libc::SIGKILL
            };
            for p in &owned {
                signal_process(p, signal);
            }
            if exit.is_some() && owned.is_empty() {
                break Err("PELIGAMES_WINE_TOOL_CANCELLED".into());
            }
            if since.elapsed() > Duration::from_secs(8) {
                break Err("Não foi possível encerrar todos os processos da ferramenta. Consulte o registro.".into());
            }
        } else if exit.is_some() && out.is_finished() && err.is_finished() {
            break Ok(());
        }
        if exit.is_none() {
            match child.try_wait() {
                Ok(status) => exit = status,
                Err(error) => break Err(error.to_string()),
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    stop.store(true, Ordering::Release);
    let _ = out.join();
    let _ = err.join();
    result?;
    if exit.is_some_and(|s| s.success()) {
        Ok(())
    } else {
        Err(format!(
            "A ferramenta encerrou com código {:?}.",
            exit.and_then(|s| s.code())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            Mutex,
        },
        time::Duration,
    };
    fn run(script: &str, cancel_after: Option<Duration>) -> Result<(), String> {
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(delay) = cancel_after {
            let cancel = cancel.clone();
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                cancel.store(true, Ordering::Release);
            });
        }
        let marker = format!(
            "cancel-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(&marker);
        let log = Arc::new(Mutex::new(crate::core::bounded_logs::LogWriter::create(&path).unwrap()));
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", script])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let result = super::run(cmd, marker, cancel, Arc::new(|_| {}), log);
        let _ = fs::remove_file(path);
        result
    }
    #[test]
    fn normal_completion_and_failure() {
        assert!(run("printf 'done\\n'", None).is_ok());
        assert!(run("exit 7", None).unwrap_err().contains("7"));
    }
    #[test]
    fn cancellation_stops_descendants_and_preserves_unrelated_processes() {
        let mut unrelated = Command::new("sleep").arg("60").spawn().unwrap();
        let result = run("sleep 60 & wait", Some(Duration::from_millis(250)));
        let unaffected = unrelated.try_wait().unwrap().is_none();
        let _ = unrelated.kill();
        let _ = unrelated.wait();
        assert!(unaffected);
        assert_eq!(result.unwrap_err(), "PELIGAMES_WINE_TOOL_CANCELLED");
    }
    #[test]
    fn cancellation_escalates_for_an_unresponsive_installer() {
        let start = std::time::Instant::now();
        let result = run(
            "trap '' TERM; sleep 60 & wait",
            Some(Duration::from_millis(250)),
        );
        assert_eq!(result.unwrap_err(), "PELIGAMES_WINE_TOOL_CANCELLED");
        assert!(start.elapsed() < Duration::from_secs(6));
    }
    #[test]
    fn cancellation_handles_a_child_after_the_parent_exits() {
        assert_eq!(
            run("sleep 60 & exit 0", Some(Duration::from_millis(250))).unwrap_err(),
            "PELIGAMES_WINE_TOOL_CANCELLED"
        );
    }
}
