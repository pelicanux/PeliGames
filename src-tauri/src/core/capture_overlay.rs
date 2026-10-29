//! Private local bridge: the game renderer never launches or elevates the recorder.
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
pub struct Bridge {
    directory: PathBuf,
    descriptor: PathBuf,
    stopped: Arc<AtomicBool>,
}
impl Bridge {
    pub fn start(
        root: &Path,
        handle: Arc<dyn Fn(&str, u32) -> Value + Send + Sync>,
    ) -> Result<Self, String> {
        // The game can already read the descriptor in this folder. Keep its
        // socket here too: /tmp and XDG_RUNTIME_DIR may be isolated by Steam.
        // A short private child also stays within sockaddr_un's path limit.
        use std::os::unix::fs::MetadataExt;
        let metadata=fs::symlink_metadata(root).map_err(|e|e.to_string())?;
        if !metadata.is_dir() || metadata.uid()!=unsafe{libc::geteuid()} || metadata.mode()&0o022!=0 {
            return Err("Pasta da ponte de captura sem propriedade ou permissões seguras.".into());
        }
        let runtime=root.join("runtime");
        fs::DirBuilder::new().recursive(true).mode(0o700).create(&runtime).map_err(|e|e.to_string())?;
        let directory=(1..=1024).map(|n|runtime.join(format!("capture-{}-{n}",std::process::id()))).find(|p|p.symlink_metadata().is_err()).ok_or("Não foi possível reservar a ponte de captura.")?;
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            let socket = directory.join("control");
            let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
            let descriptor = crate::core::paths::layout::join(&root, "capture-overlay.json");
            let temporary = root.join(format!(".capture-overlay-{}", uuid::Uuid::new_v4()));
            let bytes = serde_json::to_vec(&json!({"socket":socket})).map_err(|e| e.to_string())?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(|e| e.to_string())?;
            if let Err(error) = file
                .write_all(&bytes)
                .and_then(|_| fs::rename(&temporary, &descriptor))
            {
                let _ = fs::remove_file(&temporary);
                return Err(error.to_string());
            }
            let stopped = Arc::new(AtomicBool::new(false));
            let stop = stopped.clone();
            let pending = Arc::new(AtomicUsize::new(0));
            thread::Builder::new()
                .name("peli-capture-bridge".into())
                .spawn(move || {
                    for stream in listener.incoming() {
                        if stop.load(Ordering::Acquire) {
                            break;
                        }
                        let Ok(mut stream) = stream else { break };
                        let Some(peer) = peer_pid(&stream) else { continue; };
                        if pending.fetch_add(1, Ordering::AcqRel) >= 4 {
                            pending.fetch_sub(1, Ordering::AcqRel);
                            continue;
                        }
                        let handle = handle.clone();
                        let completed = pending.clone();
                        let stop = stop.clone();
                        let worker = thread::Builder::new()
                            .name("peli-capture-request".into())
                            .spawn(move || {
                                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                                let mut line = String::new();
                                if BufReader::new((&mut stream).take(8193))
                                    .read_line(&mut line)
                                    .is_ok()
                                    && line.len() <= 8192
                                {
                                    let response = match serde_json::from_str::<Value>(&line)
                                        .ok()
                                        .and_then(|v| v["command"].as_str().map(str::to_owned))
                                    {
                                        Some(command)
                                            if matches!(
                                                command.as_str(),
                                                "status"
                                                    | "toggle-recording"
                                                    | "toggle-replay"
                                                    | "sources"
                                                    | "record"
                                                    | "replay"
                                                    | "save-replay"
                                                    | "start-recording"
                                                    | "stop-recording"
                                                    | "stop"
                                            ) || command.starts_with("configure:") =>
                                        {
                                            if stop.load(Ordering::Acquire) {
                                                json!({"error":"Launcher encerrando."})
                                            } else {
                                                handle(&command, peer)
                                            }
                                        }
                                        _ => json!({"error":"Comando de captura inválido."}),
                                    };
                                    let _ = writeln!(stream, "{response}");
                                }
                                completed.fetch_sub(1, Ordering::AcqRel);
                            });
                        if worker.is_err() {
                            pending.fetch_sub(1, Ordering::AcqRel);
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
            Ok(Self {
                directory: directory.clone(),
                descriptor,
                stopped,
            })
        })();
        if result.is_err() {
            let _ = fs::remove_dir_all(&directory);
        }
        result
    }
    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        let socket = self.directory.join("control");
        let _ = UnixStream::connect(&socket);
        // Only remove our own descriptor; another launcher may have replaced it.
        if fs::read(&self.descriptor)
            .ok()
            .and_then(|v| serde_json::from_slice::<Value>(&v).ok())
            .is_some_and(|v| v["socket"] == socket.to_string_lossy().as_ref())
        {
            let _ = fs::remove_file(&self.descriptor);
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.shutdown();
    }
}
fn peer_pid(stream: &UnixStream) -> Option<u32> {
    use std::os::fd::AsRawFd;
    let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of_val(&cred) as libc::socklen_t;
    let valid = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            &mut cred as *mut _ as *mut _,
            &mut size,
        ) == 0
            && cred.uid == libc::geteuid() && cred.pid > 1
    };
    valid.then_some(cred.pid as u32)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;
    #[test]
    fn bridge_limits_commands_and_cleans_its_private_endpoint() {
        let root =
            std::env::temp_dir().join(format!("capture-bridge-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let bridge = Bridge::start(&root, Arc::new(|command, peer| json!({"received":command,"peer":peer}))).unwrap();
        let descriptor: Value =
            serde_json::from_slice(&fs::read(crate::core::paths::layout::join(&root, "capture-overlay.json")).unwrap()).unwrap();
        let socket = descriptor["socket"].as_str().unwrap();
        assert_eq!(Path::new(socket).parent().unwrap().parent(),Some(root.join("runtime").as_path()));
        assert_eq!(
            fs::metadata(&bridge.directory).unwrap().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(crate::core::paths::layout::join(&root, "capture-overlay.json"))
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );
        for (command, valid) in [
            ("status", true),
            ("record", true),
            ("save-replay", true),
            ("sources", true),
            ("configure:{\"format\":\"mkv\"}", true),
            ("shell", false),
        ] {
            let mut stream = UnixStream::connect(socket).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            writeln!(stream, "{}", json!({"command":command})).unwrap();
            let mut reply = String::new();
            BufReader::new(stream).read_line(&mut reply).unwrap();
            let reply: Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(reply["received"].as_str() == Some(command), valid);
            assert_eq!(reply["error"].is_string(), !valid);
            if valid { assert_eq!(reply["peer"], std::process::id()); }
        }
        bridge.shutdown();
        assert!(!Path::new(socket).exists());
        assert!(!crate::core::paths::layout::join(&root, "capture-overlay.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
