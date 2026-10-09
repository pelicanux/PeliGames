//! Launcher-only IPC. Installers and background game monitors remain independent.
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{FileTypeExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct LauncherInstance {
    listener: UnixListener,
    _lock: File,
    socket: PathBuf,
}

fn send_at(socket: &Path, entry: Option<&str>) -> Result<bool, String> {
    let mut stream = match UnixStream::connect(socket) {
        Ok(stream) => stream,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(false)
        }
        Err(error) => return Err(format!("Não foi possível contatar o launcher: {error}")),
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let payload = serde_json::to_vec(&entry).map_err(|e| e.to_string())?;
    if payload.len() > 65536 {
        return Err("Pedido de abertura muito longo.".into());
    }
    stream.write_all(&payload).map_err(|e| e.to_string())?;
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|e| e.to_string())?;
    let mut response = Vec::new();
    stream
        .take(4096)
        .read_to_end(&mut response)
        .map_err(|e| format!("O launcher não respondeu: {e}"))?;
    let result: Result<(), String> = serde_json::from_slice(&response)
        .map_err(|e| format!("Resposta inválida do launcher: {e}"))?;
    result.map(|()| true)
}

pub fn focus_running(entry: Option<&str>) -> Result<bool, String> {
    send_at(&super::paths::app_root()?.join("launcher.sock"), entry)
}

impl LauncherInstance {
    pub fn claim_at(root: &Path, entry: Option<&str>) -> Result<Option<Self>, String> {
        let socket = root.join("launcher.sock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("launcher.lock"))
            .map_err(|e| e.to_string())?;
        // Atomic ownership also handles simultaneous launches from different buttons.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
                return Err(error.to_string());
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if send_at(&socket, entry)? {
                    return Ok(None);
                }
                if Instant::now() >= deadline {
                    return Err(
                        "O launcher existente ainda não está pronto para receber pedidos.".into(),
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        match fs::symlink_metadata(&socket) {
            Ok(meta) if meta.file_type().is_socket() => {
                fs::remove_file(&socket).map_err(|e| e.to_string())?
            }
            Ok(_) => return Err("O endereço do launcher não é um socket regular.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        Ok(Some(Self {
            listener,
            _lock: lock,
            socket,
        }))
    }

    pub fn serve(self, mut receive: impl FnMut(Option<String>) -> Result<(), String>) {
        for stream in self.listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let result = (|| {
                let mut bytes = Vec::new();
                (&mut stream)
                    .take(65537)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 65536 {
                    return Err("Pedido de abertura muito longo.".into());
                }
                let entry: Option<String> =
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                receive(entry)
            })();
            if let Ok(response) = serde_json::to_vec(&result) {
                let _ = stream.write_all(&response);
            }
        }
    }
}
impl Drop for LauncherInstance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket);
    }
}

// A deliberate restart must wait for the old process to release its ownership.
// Ordinary open requests never use this path and always reuse the existing UI.
pub fn wait_for_restart_parent() -> Result<(), String> {
    let Some(value) = std::env::var_os("PELIGAMES_RESTART_PARENT_PID") else {
        return Ok(());
    };
    std::env::remove_var("PELIGAMES_RESTART_PARENT_PID");
    let pid: i32 = value
        .to_string_lossy()
        .parse()
        .map_err(|_| "PID de reinício inválido.")?;
    if pid <= 0 || pid as u32 == std::process::id() {
        return Err("PID de reinício inválido.".into());
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    while unsafe { libc::kill(pid, 0) } == 0
        || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    {
        if Instant::now() >= deadline {
            return Err("O launcher anterior não encerrou para concluir o reinício.".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}
