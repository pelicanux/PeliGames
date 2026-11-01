//! Opt-in local Rich Presence. No user tokens, bot tokens or OAuth credentials.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub const APPLICATION_ID: &str = match option_env!("PELIGAMES_DISCORD_APPLICATION_ID") {
    Some(id) => id,
    None => "1559353844239175802",
};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "SavedPreferences")]
pub struct Preferences {
    pub enabled: bool,
}
/// Honor the old capture opt-out without retaining game sharing in the new format.
#[derive(Deserialize)]
struct SavedPreferences {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "previous_capture_default")]
    share_capture: bool,
}
fn previous_capture_default() -> bool {
    true
}
impl From<SavedPreferences> for Preferences {
    fn from(saved: SavedPreferences) -> Self {
        Self {
            enabled: saved.enabled && saved.share_capture,
        }
    }
}
impl Default for Preferences {
    fn default() -> Self {
        Self { enabled: false }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaptureActivity {
    pub active: bool,
    pub recording: bool,
    pub replay: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Status {
    pub preferences: Preferences,
    pub connected: bool,
    pub active: bool,
    pub error: Option<String>,
    pub application_id: String,
}
#[derive(Default)]
struct Inner {
    preferences: Preferences,
    capture: CaptureActivity,
    revision: u64,
    connected: bool,
    error: Option<String>,
    stop: bool,
}
type Shared = Arc<(Mutex<Inner>, Condvar)>;
type Notify = Arc<dyn Fn(Status) + Send + Sync>;
pub struct Presence {
    state: Shared,
    path: PathBuf,
}
fn selection(preferences: &Preferences, capture: &CaptureActivity) -> Option<&'static str> {
    if !preferences.enabled || !capture.active {
        return None;
    }
    match (capture.recording, capture.replay) {
        (true, true) => Some("Gravando tela · replay ativo"),
        (true, false) => Some("Gravando tela"),
        (false, true) => Some("Replay instantâneo ativo"),
        (false, false) => None,
    }
}
fn snapshot(inner: &Inner) -> Status {
    Status {
        preferences: inner.preferences.clone(),
        connected: inner.connected,
        active: selection(&inner.preferences, &inner.capture).is_some(),
        error: inner.error.clone(),
        application_id: APPLICATION_ID.into(),
    }
}
fn frame(stream: &mut UnixStream, opcode: u32, payload: &[u8]) -> Result<(), String> {
    if payload.len() > 65536 {
        return Err("Mensagem Discord muito grande.".into());
    }
    let mut bytes = Vec::with_capacity(8 + payload.len());
    bytes.extend(opcode.to_le_bytes());
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
    stream
        .write_all(&bytes)
        .map_err(|e| format!("Discord IPC: {e}"))
}
fn receive(stream: &mut UnixStream) -> Result<(u32, Vec<u8>), String> {
    let mut header = [0; 8];
    stream
        .read_exact(&mut header)
        .map_err(|e| format!("Discord desconectado: {e}"))?;
    let opcode = u32::from_le_bytes(header[..4].try_into().unwrap());
    let size = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
    if size > 65536 {
        return Err("Resposta Discord excedeu o limite.".into());
    }
    let mut payload = vec![0; size];
    stream.read_exact(&mut payload).map_err(|e| e.to_string())?;
    Ok((opcode, payload))
}
struct Rpc {
    stream: UnixStream,
}
impl Rpc {
    fn from_stream(stream: UnixStream, id: &str) -> Result<Self, String> {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        let mut rpc = Self { stream };
        frame(
            &mut rpc.stream,
            0,
            json!({"v":1,"client_id":id}).to_string().as_bytes(),
        )?;
        let ready = rpc.reply(None)?;
        if ready["evt"] != "READY" {
            return Err("Discord não confirmou a conexão Rich Presence.".into());
        }
        Ok(rpc)
    }
    fn connect(paths: &[PathBuf]) -> Result<Self, String> {
        let mut error = None;
        for path in paths {
            if !fs::metadata(path)
                .is_ok_and(|m| m.file_type().is_socket() && m.uid() == unsafe { libc::geteuid() })
            {
                continue;
            }
            match UnixStream::connect(path)
                .map_err(|e| e.to_string())
                .and_then(|s| Self::from_stream(s, APPLICATION_ID))
            {
                Ok(rpc) => return Ok(rpc),
                Err(e) => error = Some(e),
            }
        }
        Err(error.unwrap_or_else(|| {
            "Abra o Discord desktop para compartilhar sua atividade. A reconexão é automática."
                .into()
        }))
    }
    fn reply(&mut self, nonce: Option<&str>) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            let (opcode, payload) = receive(&mut self.stream)?;
            if opcode == 3 {
                frame(&mut self.stream, 4, &payload)?;
                continue;
            }
            if opcode == 2 {
                return Err("O Discord encerrou a conexão.".into());
            }
            if opcode != 1 {
                continue;
            }
            let value: Value = serde_json::from_slice(&payload).map_err(|e| e.to_string())?;
            if value["evt"] == "ERROR" {
                return Err(value["data"]["message"]
                    .as_str()
                    .unwrap_or("Discord recusou a atividade.")
                    .to_string());
            }
            if nonce.is_none_or(|id| value["nonce"].as_str() == Some(id)) {
                return Ok(value);
            }
        }
        Err("O Discord não respondeu à atualização da atividade.".into())
    }
    fn activity(&mut self, activity: Value) -> Result<(), String> {
        let nonce = uuid::Uuid::new_v4().to_string();
        frame(&mut self.stream,1,json!({"cmd":"SET_ACTIVITY","args":{"pid":std::process::id(),"activity":activity},"nonce":nonce}).to_string().as_bytes())?;
        let reply = self.reply(Some(&nonce))?;
        if reply["cmd"] != "SET_ACTIVITY" {
            return Err("Resposta Discord inválida.".into());
        }
        Ok(())
    }
    fn ping(&mut self) -> Result<(), String> {
        frame(&mut self.stream, 3, b"{}")?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            let (op, payload) = receive(&mut self.stream)?;
            if op == 4 {
                return Ok(());
            }
            if op == 3 {
                frame(&mut self.stream, 4, &payload)?;
            }
            if op == 2 {
                return Err("Discord desconectado.".into());
            }
        }
        Err("Discord sem resposta.".into())
    }
}
impl Presence {
    pub fn new(path: PathBuf, paths: Vec<PathBuf>, notify: Notify) -> Result<Self, String> {
        let preferences = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "Preferências de presença Discord inválidas.")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Preferences::default(),
            Err(e) => return Err(e.to_string()),
        };
        let state = Arc::new((
            Mutex::new(Inner {
                preferences,
                ..Default::default()
            }),
            Condvar::new(),
        ));
        let shared = state.clone();
        thread::spawn(move || run(shared, paths, notify));
        Ok(Self { state, path })
    }
    pub fn status(&self) -> Status {
        snapshot(&self.state.0.lock().unwrap())
    }
    pub fn preferences(&self, preferences: Preferences) -> Result<Status, String> {
        let mut inner = self.state.0.lock().unwrap();
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        fs::write(
            &temporary,
            serde_json::to_vec(&preferences).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if let Err(e) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(temporary);
            return Err(e.to_string());
        }
        inner.preferences = preferences;
        inner.error = None;
        inner.revision += 1;
        let status = snapshot(&inner);
        self.state.1.notify_one();
        Ok(status)
    }
    pub fn capture(&self, activity: CaptureActivity) {
        let mut inner = self.state.0.lock().unwrap();
        if inner.capture != activity {
            inner.capture = activity;
            inner.revision += 1;
            self.state.1.notify_one();
        }
    }
    pub fn shutdown(&self) {
        let mut inner = self.state.0.lock().unwrap();
        inner.stop = true;
        inner.revision += 1;
        self.state.1.notify_one();
        let _ = self
            .state
            .1
            .wait_timeout_while(inner, Duration::from_secs(3), |s| s.connected);
    }
}
fn run(shared: Shared, paths: Vec<PathBuf>, notify: Notify) {
    let mut connection: Option<Rpc> = None;
    let mut sent = None;
    let mut identity = None;
    let mut since = 0;
    let mut failures = 0u32;
    loop {
        let inner = shared.0.lock().unwrap();
        let revision = inner.revision;
        let stop = inner.stop;
        let selected = selection(&inner.preferences, &inner.capture);
        let enabled = selected.is_some() && !stop;
        drop(inner);
        let outcome = if !enabled {
            if let Some(mut rpc) = connection.take() {
                let _ = rpc.activity(Value::Null);
            }
            sent = None;
            identity = None;
            Ok(false)
        } else {
            if identity != selected {
                identity = selected;
                since = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
            }
            if connection.is_none() {
                match Rpc::connect(&paths) {
                    Ok(rpc) => {
                        connection = Some(rpc);
                        sent = None;
                    }
                    Err(error) => {
                        publish(&shared, false, Some(error), &notify);
                        failures = failures.saturating_add(1);
                        wait(
                            &shared,
                            revision,
                            Duration::from_secs((5 * failures as u64).min(30)),
                        );
                        continue;
                    }
                }
            }
            let activity = json!({"type":0,"details":"Gravação e replay","state":selected.unwrap(),"timestamps":{"start":since}});
            let rpc = connection.as_mut().unwrap();
            let result = if sent.as_ref() == Some(&activity) {
                rpc.ping()
            } else {
                rpc.activity(activity.clone())
            };
            result.map(|_| {
                sent = Some(activity);
                true
            })
        };
        match outcome {
            Ok(connected) => {
                failures = 0;
                publish(&shared, connected, None, &notify);
            }
            Err(error) => {
                connection = None;
                sent = None;
                publish(&shared, false, Some(error), &notify);
            }
        }
        if stop {
            return;
        }
        wait(
            &shared,
            revision,
            if enabled {
                Duration::from_secs(20)
            } else {
                Duration::from_secs(86400)
            },
        );
    }
}
fn wait(shared: &Shared, revision: u64, timeout: Duration) {
    let inner = shared.0.lock().unwrap();
    if inner.revision == revision && !inner.stop {
        let _ = shared.1.wait_timeout(inner, timeout);
    }
}
fn publish(shared: &Shared, connected: bool, error: Option<String>, notify: &Notify) {
    let mut inner = shared.0.lock().unwrap();
    let changed = inner.connected != connected || inner.error != error;
    inner.connected = connected;
    inner.error = error;
    let status = snapshot(&inner);
    shared.1.notify_all();
    drop(inner);
    if changed {
        notify(status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_active_recording_or_replay_is_published() {
        let enabled = Preferences { enabled: true };
        let capture = CaptureActivity {
            active: true,
            recording: true,
            replay: false,
        };
        assert_eq!(selection(&Preferences::default(), &capture), None);
        assert_eq!(selection(&enabled, &CaptureActivity::default()), None);
        assert_eq!(
            selection(
                &enabled,
                &CaptureActivity {
                    active: false,
                    ..capture.clone()
                }
            ),
            None
        );
        assert_eq!(
            selection(
                &enabled,
                &CaptureActivity {
                    active: true,
                    ..Default::default()
                }
            ),
            None
        );
        assert_eq!(selection(&enabled, &capture), Some("Gravando tela"));
        assert_eq!(
            selection(
                &enabled,
                &CaptureActivity {
                    replay: true,
                    ..capture
                }
            ),
            Some("Gravando tela · replay ativo")
        );
        assert_eq!(
            selection(
                &enabled,
                &CaptureActivity {
                    active: true,
                    recording: false,
                    replay: true
                }
            ),
            Some("Replay instantâneo ativo")
        );
    }
    #[test]
    fn old_capture_opt_out_is_preserved_and_game_sharing_is_removed() {
        let old = |text: &str| serde_json::from_str::<Preferences>(text).unwrap();
        assert!(!old(r#"{"enabled":true,"share_capture":false,"share_game":true}"#).enabled);
        assert!(old(r#"{"enabled":true,"share_capture":true,"share_game":false}"#).enabled);
        assert!(!old(r#"{"enabled":false,"share_capture":true}"#).enabled);
        assert!(old(r#"{"enabled":true}"#).enabled);
        assert!(!old("{}").enabled);
        assert_eq!(
            serde_json::to_value(old(r#"{"enabled":true,"share_game":true}"#)).unwrap(),
            json!({"enabled":true})
        );
    }
    #[test]
    fn rpc_handshake_ping_activity_and_clear() {
        let (client, mut server) = UnixStream::pair().unwrap();
        let worker = thread::spawn(move || {
            let (op, bytes) = receive(&mut server).unwrap();
            assert_eq!(op, 0);
            assert_eq!(
                serde_json::from_slice::<Value>(&bytes).unwrap()["client_id"],
                "123"
            );
            frame(&mut server, 1, b"{\"evt\":\"READY\"}").unwrap();
            for expected in [false, true] {
                let (op, bytes) = receive(&mut server).unwrap();
                assert_eq!(op, 1);
                let request: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(request["cmd"], "SET_ACTIVITY");
                assert_eq!(request["args"]["activity"].is_null(), expected);
                frame(&mut server, 3, b"{\"ping\":1}").unwrap();
                let (op, _) = receive(&mut server).unwrap();
                assert_eq!(op, 4);
                frame(
                    &mut server,
                    1,
                    json!({"cmd":"SET_ACTIVITY","nonce":request["nonce"]})
                        .to_string()
                        .as_bytes(),
                )
                .unwrap();
            }
        });
        let mut rpc = Rpc::from_stream(client, "123").unwrap();
        rpc.activity(json!({"details":"Gravação e replay"}))
            .unwrap();
        rpc.activity(Value::Null).unwrap();
        worker.join().unwrap();
    }
    #[test]
    fn rejects_oversized_frame() {
        let (mut client, mut server) = UnixStream::pair().unwrap();
        server.write_all(&[1, 0, 0, 0, 1, 0, 1, 0]).unwrap();
        assert!(receive(&mut client).is_err());
    }
    #[test]
    fn opt_in_updates_privacy_and_clears_own_activity() {
        check_capture_activity_delivery(false);
    }
    #[test]
    fn flatpak_socket_delivers_recording_replay_and_combined_activity() {
        check_capture_activity_delivery(true);
    }
    fn check_capture_activity_delivery(flatpak: bool) {
        use std::os::unix::net::UnixListener;
        use std::sync::mpsc;
        let root = std::env::temp_dir().join(format!("peli-presence-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let socket_folder = if flatpak {
            root.join("app/com.discordapp.Discord")
        } else {
            root.clone()
        };
        fs::create_dir_all(&socket_folder).unwrap();
        let socket = socket_folder.join("discord-ipc-0");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let service = Presence::new(
            root.join("preferences.json"),
            super::super::discord_socket::candidates(&root),
            Arc::new(|_| {}),
        )
        .unwrap();
        thread::sleep(Duration::from_millis(30));
        assert!(matches!(listener.accept(),Err(e)if e.kind()==std::io::ErrorKind::WouldBlock));
        listener.set_nonblocking(false).unwrap();
        let (tx, rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let (op, _) = receive(&mut stream).unwrap();
            assert_eq!(op, 0);
            frame(&mut stream, 1, b"{\"evt\":\"READY\"}").unwrap();
            for index in 0..8 {
                // Stopping capture and disabling sharing close only our session.
                if index == 4 || index == 6 {
                    let (next, _) = listener.accept().unwrap();
                    stream = next;
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .unwrap();
                    let (op, _) = receive(&mut stream).unwrap();
                    assert_eq!(op, 0);
                    frame(&mut stream, 1, b"{\"evt\":\"READY\"}").unwrap();
                }
                let (op, payload) = receive(&mut stream).unwrap();
                assert_eq!(op, 1);
                let request: Value = serde_json::from_slice(&payload).unwrap();
                assert_eq!(request["cmd"], "SET_ACTIVITY");
                frame(
                    &mut stream,
                    1,
                    json!({"cmd":"SET_ACTIVITY","nonce":request["nonce"]})
                        .to_string()
                        .as_bytes(),
                )
                .unwrap();
                tx.send(request["args"]["activity"].clone()).unwrap();
            }
        });
        let enabled = Preferences { enabled: true };
        service.preferences(enabled.clone()).unwrap();
        // Enabling sharing while idle must not even open the Discord socket.
        assert!(rx.recv_timeout(Duration::from_millis(80)).is_err());
        service.capture(CaptureActivity {
            active: true,
            recording: true,
            replay: false,
        });
        let next = || rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let recording = next();
        assert_eq!(recording["state"], "Gravando tela");
        assert_eq!(recording["details"], "Gravação e replay");
        assert_eq!(recording["timestamps"]["start"].as_u64().is_some(), true);
        service.capture(CaptureActivity {
            active: true,
            recording: false,
            replay: true,
        });
        assert_eq!(next()["state"], "Replay instantâneo ativo");
        service.capture(CaptureActivity {
            active: true,
            recording: true,
            replay: true,
        });
        assert_eq!(next()["state"], "Gravando tela · replay ativo");
        // Removing PeliGames' own activity leaves other applications untouched.
        service.capture(CaptureActivity::default());
        assert!(next().is_null());
        service.capture(CaptureActivity {
            active: true,
            recording: true,
            replay: false,
        });
        assert_eq!(next()["state"], "Gravando tela");
        service.preferences(Preferences::default()).unwrap();
        assert!(next().is_null());
        service.preferences(enabled).unwrap();
        assert_eq!(next()["state"], "Gravando tela");
        service.shutdown();
        assert!(next().is_null());
        server.join().unwrap();
        service.shutdown();
        let stored: Preferences =
            serde_json::from_slice(&fs::read(root.join("preferences.json")).unwrap()).unwrap();
        assert!(stored.enabled);
        fs::remove_dir_all(root).unwrap();
    }
}
