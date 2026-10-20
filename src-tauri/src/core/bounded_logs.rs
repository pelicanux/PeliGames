//! Bounded diagnostic tails. Never use this for state files such as winetricks.log.
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};
pub const MAX_LINES: usize = 1000;
pub const MAX_BYTES: usize = 1024 * 1024;
const LINE_BYTES: usize = 4096;
pub const FLAG: &str = "--peligames-log-collector";
#[derive(Default)]
struct Tail {
    lines: VecDeque<Vec<u8>>,
    partial: Vec<u8>,
    bytes: usize,
}
impl Tail {
    fn push(&mut self, data: &[u8]) {
        for &b in data {
            if b == b'\n' || b == b'\r' {
                if b == b'\r' && self.partial.is_empty() {
                    continue;
                }
                let mut line = std::mem::take(&mut self.partial);
                line.push(b'\n');
                self.bytes += line.len();
                self.lines.push_back(line);
            } else if self.partial.len() < LINE_BYTES {
                self.partial.push(b);
            }
            while self.lines.len() + usize::from(!self.partial.is_empty()) > MAX_LINES
                || self.bytes + self.partial.len() > MAX_BYTES
            {
                if let Some(old) = self.lines.pop_front() {
                    self.bytes -= old.len();
                } else {
                    break;
                }
            }
        }
    }
    fn write(&self, file: &mut File) -> io::Result<()> {
        file.seek(SeekFrom::Start(0))?;
        for line in &self.lines {
            file.write_all(line)?;
        }
        file.write_all(&self.partial)?;
        file.set_len((self.bytes + self.partial.len()) as u64)?;
        file.flush()
    }
}
fn open(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = opts.open(path)?;
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("Invalid log file"));
    }
    Ok(file)
}
fn load(file: &mut File) -> io::Result<Tail> {
    let length = file.metadata()?.len();
    let start = length.saturating_sub(MAX_BYTES as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES as u64).read_to_end(&mut bytes)?;
    // A bounded read may start halfway through a record.
    let bytes = if start > 0 {
        bytes
            .iter()
            .position(|b| *b == b'\n')
            .map(|p| &bytes[p + 1..])
            .unwrap_or(&[])
    } else {
        &bytes
    };
    let mut tail = Tail::default();
    tail.push(bytes);
    Ok(tail)
}
pub fn compact(path: &Path) -> io::Result<()> {
    let mut file = open(path)?;
    let tail = load(&mut file)?;
    tail.write(&mut file)
}
pub fn append(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut file = open(path)?;
    let mut tail = load(&mut file)?;
    tail.push(data);
    tail.write(&mut file)
}
pub struct LogWriter {
    file: File,
    tail: Tail,
    last: Instant,
}
impl LogWriter {
    pub fn create(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            if parent.file_name().is_some_and(|name| {
                matches!(
                    name.to_str(),
                    Some("logs" | "current" | "previous" | "MangoHUD")
                )
            }) {
                sweep(parent);
            }
        }
        let mut file = open(path)?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        Ok(Self {
            file,
            tail: Tail::default(),
            last: Instant::now(),
        })
    }
}
impl Write for LogWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.tail.push(data);
        if self.last.elapsed() >= Duration::from_millis(250) {
            self.flush()?;
        }
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.tail.write(&mut self.file)?;
        self.last = Instant::now();
        Ok(())
    }
}
impl Drop for LogWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}
/// A separate Rust collector survives closing the UI while a game remains open.
#[cfg(unix)]
pub fn capture(command: &mut std::process::Command, path: &Path) -> Result<(), String> {
    use std::os::{
        fd::OwnedFd,
        unix::{net::UnixStream, process::CommandExt},
    };
    use std::process::{Command, Stdio};
    if let Some(parent) = path.parent() {
        if parent.file_name().is_some_and(|name| {
            matches!(
                name.to_str(),
                Some("logs" | "current" | "previous" | "MangoHUD")
            )
        }) {
            sweep(parent);
        }
    }
    let mut retries = 0;
    loop {
        match compact(path) {
            Ok(()) => break,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock && retries < 50 => {
                retries += 1;
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let (reader, writer) = UnixStream::pair().map_err(|e| e.to_string())?;
    let mut helper = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    helper
        .arg(FLAG)
        .arg(path)
        .stdin(Stdio::from(OwnedFd::from(reader)))
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    unsafe {
        helper.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = helper.spawn().map_err(|e| e.to_string())?;
    let mut ready = child
        .stdout
        .take()
        .ok_or("Log collector handshake unavailable")?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    let mut ack = [0];
    ready
        .read_exact(&mut ack)
        .map_err(|_| "Não foi possível iniciar o coletor de logs.".to_string())?;
    let stderr = writer.try_clone().map_err(|e| e.to_string())?;
    command
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .stderr(Stdio::from(OwnedFd::from(stderr)));
    Ok(())
}
#[cfg(not(unix))]
pub fn capture(_: &mut std::process::Command, _: &Path) -> Result<(), String> {
    Err("Log capture requires Unix".into())
}
pub fn collect(path: &Path) -> io::Result<()> {
    let mut file = open(path)?;
    let mut tail = load(&mut file)?;
    tail.write(&mut file)?;
    io::stdout().write_all(&[1])?;
    io::stdout().flush()?;
    let mut last = Instant::now();
    let mut dirty = false;
    let mut buffer = [0; 8192];
    let mut stdin = io::stdin();
    loop {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let mut fd = libc::pollfd {
                fd: stdin.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut fd, 1, 200) };
            if ready == 0 {
                if dirty {
                    let _ = tail.write(&mut file);
                    dirty = false;
                    last = Instant::now();
                }
                continue;
            }
            if ready < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
        }
        let n = match stdin.read(&mut buffer) {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if n == 0 {
            break;
        }
        tail.push(&buffer[..n]);
        dirty = true;
        if last.elapsed() >= Duration::from_millis(200) {
            let _ = tail.write(&mut file);
            dirty = false;
            last = Instant::now();
        }
    }
    tail.write(&mut file)
}
/// Only diagnostic directories owned by PeliGames; never prefix/package contents.
pub fn sweep(root: &Path) {
    if !std::fs::symlink_metadata(root).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
        return;
    }
    let mut groups: std::collections::BTreeMap<
        std::path::PathBuf,
        Vec<(Option<std::time::SystemTime>, std::path::PathBuf)>,
    > = Default::default();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(5)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|e| e == "log") {
            let modified = entry.metadata().ok().and_then(|m| m.modified().ok());
            let _ = compact(entry.path());
            groups
                .entry(entry.path().parent().unwrap().to_path_buf())
                .or_default()
                .push((modified, entry.into_path()));
        }
    }
    for files in groups.values_mut() {
        files.sort_by_key(|p| p.0);
        for (_, path) in files.iter().take(files.len().saturating_sub(12)) {
            if let Ok(_file) = open(path) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}
pub fn startup_cleanup() {
    let Ok(root) = super::paths::app_root() else {
        return;
    };
    for name in ["launcher.log", "launcher.previous.log"] {
        let path = root.join(name);
        if path.exists() {
            let _ = compact(&path);
        }
    }
    for directory in ["game-logs", "Logs", "MangoHUD"] {
        sweep(&root.join(directory));
    }
    if let Ok(entries) = super::installed_library::list(false) {
        for entry in entries {
            sweep(&std::path::Path::new(&entry.directory).join("logs"));
        }
    }
    crate::commands::nexus_storage::clean_diagnostic_logs();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_latest_thousand_and_bounds_unbroken_records() {
        let mut tail = Tail::default();
        for n in 0..50_000 {
            tail.push(format!("line {n}\n").as_bytes());
        }
        assert_eq!(tail.lines.len(), 1000);
        assert_eq!(tail.lines.front().unwrap(), b"line 49000\n");
        tail.push(&vec![b'x'; 5 * MAX_BYTES]);
        assert!(tail.partial.len() <= LINE_BYTES);
        assert!(tail.lines.len() + 1 <= MAX_LINES);
        assert!(tail.bytes + tail.partial.len() <= MAX_BYTES);
    }
    #[test]
    fn huge_sparse_legacy_log_is_trimmed_without_full_read() {
        let path =
            std::env::temp_dir().join(format!("peligames-log-test-{}", uuid::Uuid::new_v4()));
        let mut file = open(&path).unwrap();
        file.seek(SeekFrom::Start(70 * 1024 * 1024 * 1024)).unwrap();
        file.write_all(b"\nlast error\n").unwrap();
        drop(file);
        compact(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"last error\n");
        append(&path, b"next error\n").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "last error\nnext error\n"
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn writer_limits_live_output_and_retention_stays_inside_diagnostic_directory() {
        let root =
            std::env::temp_dir().join(format!("peligames-log-retention-{}", uuid::Uuid::new_v4()));
        let logs = root.join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(root.join("winetricks.log"), "installed component state").unwrap();
        for n in 0..20 {
            std::fs::write(logs.join(format!("old-{n}.log")), "old\n").unwrap();
        }
        sweep(&logs);
        assert_eq!(std::fs::read_dir(&logs).unwrap().count(), 12);
        let path = logs.join("winecfg.log");
        {
            let mut writer = LogWriter::create(&path).unwrap();
            for n in 0..5000 {
                writeln!(writer, "event {n}").unwrap();
            }
            writer.flush().unwrap();
            assert_eq!(
                std::fs::read_to_string(&path).unwrap().lines().count(),
                1000
            );
            // Active diagnostic files must not be rewritten by background cleanup.
            assert!(compact(&path).is_err());
        }
        assert!(std::fs::read_to_string(path)
            .unwrap()
            .ends_with("event 4999\n"));
        assert_eq!(
            std::fs::read_to_string(root.join("winetricks.log")).unwrap(),
            "installed component state"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn does_not_follow_log_symlinks() {
        let root =
            std::env::temp_dir().join(format!("peligames-log-link-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("state"), b"preserve").unwrap();
        std::os::unix::fs::symlink(root.join("state"), root.join("test.log")).unwrap();
        assert!(compact(&root.join("test.log")).is_err());
        assert_eq!(std::fs::read(root.join("state")).unwrap(), b"preserve");
        std::fs::remove_dir_all(root).unwrap();
    }
}
