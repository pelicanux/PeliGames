use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

/// All cooperating writers lock the same game directory before reading its INIs.
/// The lock file remains in place: unlinking it would allow two different inodes
/// to be locked at the same pathname. Closing the handle releases the OS lock.
pub struct DirectoryLock {
    _file: File,
}
impl DirectoryLock {
    pub fn acquire(directory: &Path) -> Result<Self, String> {
        Self::with_timeout(directory, Duration::from_secs(3))
    }

    pub fn with_timeout(directory: &Path, timeout: Duration) -> Result<Self, String> {
        if !directory.is_absolute() || !directory.is_dir() {
            return Err("Choose an existing absolute configuration directory".into());
        }
        let path = directory.join(".peligames-config.lock");
        reject_redirect(&path).map_err(|e| e.to_string())?;
        let file = options()
            .create(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("Invalid configuration lock file".into());
        }
        let start = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::Error(error)) => return Err(error.to_string()),
                Err(TryLockError::WouldBlock) if start.elapsed() < timeout => {
                    std::thread::sleep(
                        Duration::from_millis(10).min(timeout.saturating_sub(start.elapsed())),
                    );
                }
                Err(TryLockError::WouldBlock) => {
                    return Err(
                        "Configuration is being changed by another process; try again".into(),
                    );
                }
            }
        }
    }
}

fn options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    options
}

fn reject_redirect(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => Err(
            io::Error::other("Redirected or non-regular configuration files are not supported"),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

static TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// Replace one file without exposing partially written bytes. The caller holds
/// DirectoryLock throughout the read/modify/write operation, not just this call.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    write(path, text.as_bytes(), None).map_err(|error| error.to_string())
}
/// Publish managed data or an executable with explicit private permissions.
/// The caller chooses a managed path and coordinates concurrent writers.
pub fn write_atomic_bytes(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    if mode & !0o777 != 0 {
        return Err("Invalid managed file permissions".into());
    }
    let parent = path.parent().ok_or("Invalid configuration path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    write(path, bytes, Some(mode)).map_err(|error| error.to_string())
}
fn write(path: &Path, bytes: &[u8], mode: Option<u32>) -> io::Result<()> {
    reject_redirect(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Invalid configuration path"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("Invalid configuration name"))?;
    let (temporary, mut output) = loop {
        let mut filename = OsString::from(".peligames-");
        filename.push(name);
        filename.push(format!(
            "-{}-{}.tmp",
            std::process::id(),
            TEMPORARY.fetch_add(1, Ordering::Relaxed)
        ));
        let temporary = parent.join(filename);
        match options().create_new(true).open(&temporary) {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        output.write_all(bytes)?;
        if let Some(mode) = mode {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                output.set_permissions(fs::Permissions::from_mode(mode))?;
            }
            #[cfg(not(unix))]
            let _ = mode;
        } else if let Ok(metadata) = fs::metadata(path) {
            output.set_permissions(metadata.permissions())?;
        }
        output.sync_all()?;
        drop(output);
        reject_redirect(path)?;
        fs::rename(&temporary, path)
    })();
    let _ = fs::remove_file(&temporary);
    result
}
