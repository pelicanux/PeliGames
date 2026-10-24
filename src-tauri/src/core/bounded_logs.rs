//! Bounded diagnostic tails. Never use this for state files such as winetricks.log.
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
const LINE_BYTES: usize = 64 * 1024;
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
            while self.bytes + self.partial.len() > MAX_BYTES
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
    if file.metadata()?.len() <= MAX_BYTES as u64 { return Ok(()); }
    let tail = load(&mut file)?;
    tail.write(&mut file)
}
/// Append in batches. Only compact when crossing the size cap, retaining the
/// newest half of the file so subsequent events do not repeatedly rewrite it.
fn append_file(file: &mut File, bytes: &[u8]) -> io::Result<()> {
    if file.metadata()?.len() + bytes.len() as u64 > MAX_BYTES as u64 {
        let size=file.metadata()?.len();
        let budget=MAX_BYTES.saturating_sub(bytes.len()).min(MAX_BYTES/2);
        let start=size.saturating_sub(budget as u64);
        file.seek(SeekFrom::Start(start))?;
        let mut kept=Vec::new();file.take(budget as u64).read_to_end(&mut kept)?;
        if start>0 { if let Some(end)=kept.iter().position(|b|*b==b'\n') {kept.drain(..=end);} else {kept.clear();} }
        file.seek(SeekFrom::Start(0))?;file.write_all(&kept)?;file.set_len(kept.len() as u64)?;
    }
    file.seek(SeekFrom::End(0))?;file.write_all(bytes)
}
pub fn append(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut writer=LogWriter::existing(path)?;
    writer.write_all(data)?;writer.flush()
}
pub struct LogWriter {
    file: File,
    pending: Vec<u8>,
    last: Instant,
    line_bytes: usize,
}
impl LogWriter {
    fn existing(path: &Path) -> io::Result<Self> {
        let mut file=open(path)?;
        if file.metadata()?.len()>MAX_BYTES as u64 {load(&mut file)?.write(&mut file)?;}
        Ok(Self{file,pending:Vec::with_capacity(64*1024),last:Instant::now(),line_bytes:0})
    }
    pub fn create(path: &Path) -> io::Result<Self> {
        if let Some(parent)=path.parent().filter(|p|p.file_name().is_some_and(|name|matches!(name.to_str(),Some("logs"|"current"|"previous"|"MangoHUD")))) {sweep(parent);}
        let mut writer=Self::existing(path)?;writer.file.set_len(0)?;writer.file.seek(SeekFrom::Start(0))?;Ok(writer)
    }
}
impl Write for LogWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        for &byte in data {
            if matches!(byte,b'\n'|b'\r') {self.pending.push(b'\n');self.line_bytes=0;}
            else if self.line_bytes<LINE_BYTES {self.pending.push(byte);self.line_bytes+=1;}
            else if self.line_bytes==LINE_BYTES {self.pending.extend_from_slice(b" [record truncated]");self.line_bytes+=1;}
            if self.pending.len()>=64*1024 {self.flush()?;}
        }
        if self.last.elapsed()>=Duration::from_millis(250) {self.flush()?;}
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if !self.pending.is_empty() {append_file(&mut self.file,&self.pending)?;self.pending.clear();}
        self.last=Instant::now();Ok(())
    }
}
impl Drop for LogWriter {fn drop(&mut self){let _=self.flush();}}
/// Read a bounded suffix without rewriting the log or taking its writer's lock.
pub fn read(path: &Path) -> io::Result<String> {
    let mut opts=OpenOptions::new();opts.read(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;opts.custom_flags(libc::O_NOFOLLOW|libc::O_NONBLOCK);}
    let mut file=match opts.open(path){Ok(f)=>f,Err(e)if e.kind()==io::ErrorKind::NotFound=>return Ok(String::new()),Err(e)=>return Err(e)};
    if !file.metadata()?.is_file(){return Err(io::Error::other("Invalid log file"));}
    let size=file.metadata()?.len();let start=size.saturating_sub(MAX_BYTES as u64);file.seek(SeekFrom::Start(start))?;
    let mut bytes=Vec::new();file.take(MAX_BYTES as u64).read_to_end(&mut bytes)?;
    if start>0 {if let Some(end)=bytes.iter().position(|b|*b==b'\n'){bytes.drain(..=end);}else{bytes.clear();}}
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
/// Refuse deletion while a collector owns a file. Never follow linked folders.
pub fn remove_directory(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path){Err(e)if e.kind()==io::ErrorKind::NotFound=>return Ok(()),Err(e)=>return Err(e),Ok(m)if !m.is_dir()||m.file_type().is_symlink()=>return Err(io::Error::other("Invalid log directory")),_=>{}}
    let mut leases=Vec::new();
    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        let entry=entry.map_err(io::Error::other)?;
        if entry.file_type().is_symlink(){continue;}
        if entry.file_type().is_file(){
            let mut options=OpenOptions::new();options.read(true);
            #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.custom_flags(libc::O_NOFOLLOW|libc::O_NONBLOCK);}
            let file=options.open(entry.path())?;
            #[cfg(unix)] {use std::os::fd::AsRawFd;if unsafe{libc::flock(file.as_raw_fd(),libc::LOCK_EX|libc::LOCK_NB)}!=0{return Err(io::Error::last_os_error());}}
            leases.push(file);
        }
    }
    std::fs::remove_dir_all(path)
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
    let mut writer=LogWriter::existing(path)?;
    io::stdout().write_all(&[1])?;io::stdout().flush()?;
    let mut buffer=[0;8192];let mut stdin=io::stdin();
    loop {
        #[cfg(unix)] {
            use std::os::fd::AsRawFd;
            let mut fd=libc::pollfd{fd:stdin.as_raw_fd(),events:libc::POLLIN,revents:0};
            let ready=unsafe{libc::poll(&mut fd,1,200)};
            if ready==0 {writer.flush()?;continue;}
            if ready<0 {let e=io::Error::last_os_error();if e.kind()==io::ErrorKind::Interrupted{continue;}return Err(e);}
        }
        match stdin.read(&mut buffer) {Ok(0)=>break,Ok(n)=>writer.write_all(&buffer[..n])?,Err(e)if e.kind()==io::ErrorKind::Interrupted=>continue,Err(e)=>return Err(e)}
    }
    writer.flush()
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
    fn keeps_recent_records_within_byte_budget() {
        let mut tail = Tail::default();
        for n in 0..50_000 {
            tail.push(format!("line {n}\n").as_bytes());
        }
        assert_eq!(tail.lines.len(), 50_000);
        assert_eq!(tail.lines.front().unwrap(), b"line 0\n");
        tail.push(&vec![b'x'; 5 * MAX_BYTES]);
        assert!(tail.partial.len() <= LINE_BYTES);
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
                5000
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

#[cfg(test)]
mod append_tests {
    use super::*;
    #[test]
    fn writer_keeps_more_than_a_thousand_lines_and_compacts_only_at_the_byte_cap() {
        let path=std::env::temp_dir().join(format!("peligames-log-batch-{}",uuid::Uuid::new_v4()));
        let mut writer=LogWriter::create(&path).unwrap();
        let lines="normal message\n".repeat(5000);writer.write_all(lines.as_bytes()).unwrap();writer.flush().unwrap();
        assert_eq!(read(&path).unwrap().lines().count(),5000);
        let before=std::fs::read(&path).unwrap();writer.write_all(b"last message\n").unwrap();writer.flush().unwrap();
        assert!(std::fs::read(&path).unwrap().starts_with(&before));
        for n in 0..50_000 {writeln!(writer,"{n}: {}","x".repeat(80)).unwrap();}
        writer.flush().unwrap();let content=std::fs::read(&path).unwrap();
        assert!(content.len()<=MAX_BYTES);assert!(String::from_utf8_lossy(&content).contains("49999:"));
        assert!(!content.starts_with(b"normal message"));drop(writer);std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn reading_does_not_rewrite_an_active_log() {
        let path=std::env::temp_dir().join(format!("peligames-read-log-{}",uuid::Uuid::new_v4()));
        let mut writer=LogWriter::create(&path).unwrap();writer.write_all(b"one\ntwo\n").unwrap();writer.flush().unwrap();
        let stamp=std::fs::metadata(&path).unwrap().modified().unwrap();assert_eq!(read(&path).unwrap(),"one\ntwo\n");
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(),stamp);drop(writer);std::fs::remove_file(path).unwrap();
    }
}
