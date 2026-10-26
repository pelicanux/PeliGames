#[path = "capture_context.rs"]
pub mod context;
#[path = "capture_lifetime.rs"]
mod capture_lifetime;
#[path = "capture_library.rs"]
pub mod library;
#[path = "capture_storage.rs"]
pub mod storage;
#[path = "../../../tools/game-overlay/capture_selection.rs"]
pub mod selection;
#[path = "../../../tools/game-overlay/capture_shortcuts.rs"]
pub mod shortcuts;
#[path = "capture_game_audio.rs"]
mod capture_game_audio;
#[path = "capture_microphone.rs"]
mod capture_microphone;
// Optional native GPU Screen Recorder integration. The launcher never elevates itself.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::os::{fd::{AsRawFd,FromRawFd},unix::{ffi::OsStrExt,fs::DirBuilderExt,net::UnixStream}};
use std::{
    collections::VecDeque,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureOptions {
    pub game_key: String,
    pub game_name: String,
    #[serde(default)]
    pub game_directory: String,
    #[serde(default)]
    pub game_executable: String,
    #[serde(default)]
    pub steam_app_id: String,
    pub method: String,
    pub monitor: String,
    pub replay_seconds: u16,
    #[serde(flatten)]
    pub selection: selection::Selection,
}
fn settings_path(root: &Path, key: &str) -> PathBuf {
    root.join("CaptureSettings").join(format!("{:x}.json", Sha256::digest(key.as_bytes())))
}
fn write_settings(root: &Path, options: &CaptureOptions) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    validate(options)?;
    let path = settings_path(root, &options.game_key);
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary).map_err(|e|e.to_string())?;
        file.write_all(&serde_json::to_vec(options).map_err(|e|e.to_string())?).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;
        fs::rename(&temporary, &path).map_err(|e|e.to_string())
    })();
    if result.is_err() { let _ = fs::remove_file(temporary); }
    result
}
pub fn save_settings(options: &CaptureOptions) -> Result<(), String> {
    write_settings(&super::paths::app_root()?, options)
}
fn read_settings(root: &Path, key: &str) -> Result<Option<CaptureOptions>, String> {
    match fs::read(settings_path(root, key)) {
        Ok(bytes) => {
            let options: CaptureOptions = serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
            if options.game_key != key { return Err("Configuração de captura pertence a outro jogo.".into()); }
            validate(&options)?;
            Ok(Some(options))
        },
        Err(e) if e.kind()==std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
pub fn saved_settings(key: &str) -> Result<Option<CaptureOptions>, String> {
    read_settings(&super::paths::app_root()?, key)
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct CaptureStatus {
    pub phase: String,
    pub game_key: Option<String>,
    pub game_name: Option<String>,
    pub recording: bool,
    pub replay: bool,
    pub last_file: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct CaptureSupport {
    pub installed: bool,
    pub ipc: bool,
    pub portal: bool,
    pub monitors: Vec<String>,
    pub executable: Option<String>,
}
#[derive(Serialize)]
pub struct Clip {
    pub title: String,
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub modified: u64,
}
struct Session {
    child: Arc<Mutex<Child>>,
    socket: PathBuf,
    directory: PathBuf,
    id: String,
    _microphone: Option<capture_microphone::MicrophoneFilter>,
}
#[derive(Default)]
struct Inner {
    status: CaptureStatus,
    session: Option<Session>,
    startup_canceled: bool,
}
type Notify = Arc<dyn Fn(CaptureStatus) + Send + Sync>;
#[derive(Default)]
pub struct CaptureManager {
    inner: Arc<Mutex<Inner>>,
    operation: Mutex<()>,
}

fn executable() -> Option<PathBuf> {
    if let Some(path) = super::capture_runtime::executable() {
        return Some(path);
    }
    [
        "/usr/bin/gpu-screen-recorder",
        "/usr/local/bin/gpu-screen-recorder",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}
// Always drain pipes, retaining only a bounded tail, so verbose encoders cannot block.
fn drain(
    mut input: impl Read + Send + 'static,
    tail: Arc<Mutex<VecDeque<u8>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0; 2048];
        while let Ok(n) = input.read(&mut buf) {
            if n == 0 {
                break;
            }
            let mut log = tail.lock().unwrap();
            log.extend(&buf[..n]);
            while log.len() > 8192 {
                log.pop_front();
            }
        }
    })
}
fn probe(exe: &Path, argument: &str) -> Result<String, String> {
    let mut command = Command::new(exe);
    super::env_utils::host_environment(&mut command);
    let mut child = command
        .arg(argument)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let tail = Arc::new(Mutex::new(VecDeque::new()));
    let reader = drain(child.stdout.take().unwrap(), tail.clone());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_) => break,
            None if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("O gravador não respondeu à detecção.".into());
            }
            _ => thread::sleep(Duration::from_millis(40)),
        }
    }
    let _ = reader.join();
    let bytes: Vec<u8> = tail.lock().unwrap().iter().copied().collect();
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
pub(crate) fn check_runtime(exe: &Path) -> Result<(), String> {
    let help = probe(exe, "--help")?;
    if !help.contains("-ipc") || !help.contains("-ro") {
        return Err(
            "O backend de captura não oferece os controles exigidos pelo PeliGames.".into(),
        );
    }
    Ok(())
}
pub fn support() -> Result<CaptureSupport, String> {
    let Some(exe) = executable() else {
        return Ok(CaptureSupport {
            installed: false,
            ipc: false,
            portal: false,
            monitors: vec![],
            executable: None,
        });
    };
    let help = probe(&exe, "--help")?;
    let sources = probe(&exe, "--list-capture-options")?;
    let monitors = sources
        .lines()
        .filter_map(|line| {
            line.split_once('|')
                .map(|(name, _)| name.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .collect();
    Ok(CaptureSupport {
        installed: true,
        ipc: help.contains("-ipc"),
        portal: sources.lines().any(|l| l.trim() == "portal"),
        monitors,
        executable: Some(exe.to_string_lossy().into_owned()),
    })
}
pub fn encoder_support() -> Result<selection::EncoderSupport, String> {
    let exe = executable().ok_or("Instale o componente de captura.")?;
    let mut info = parse_encoder_info(&probe(&exe, "--info")?)?;
    let card = Path::new(&info.card_path)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("");
    if card.starts_with("card") && card[4..].bytes().all(|c| c.is_ascii_digit()) {
        info.pci_address = fs::canonicalize(Path::new("/sys/class/drm").join(card).join("device"))
            .ok()
            .and_then(|v| v.file_name().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_default();
    }
    Ok(info)
}
fn parse_encoder_info(text: &str) -> Result<selection::EncoderSupport, String> {
    let mut info = selection::EncoderSupport::default();
    let mut section = "";
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("section=") {
            section = name;
            continue;
        }
        match section {
            "gpu_info" => {
                if let Some(value) = line.strip_prefix("vendor|") {
                    info.gpu = value.to_uppercase();
                    info.vendor = value.to_lowercase();
                    info.encoder = if value.eq_ignore_ascii_case("nvidia") {
                        "NVENC"
                    } else {
                        "Hardware"
                    }
                    .into();
                }
                if let Some(value) = line.strip_prefix("card_path|") {
                    info.card_path = value.into();
                }
            }
            "video_codecs" if matches!(line, "h264" | "hevc" | "av1" | "vp9") => {
                if !info.codecs.iter().any(|v| v == line) {
                    info.codecs.push(line.into());
                }
            }
            "containers" if matches!(line, "mp4" | "mkv" | "webm") => {
                info.containers.push(line.into())
            }
            _ => (),
        }
    }
    if info.codecs.is_empty() {
        if info.vendor == "nvidia" {
            return Err("O NVENC não confirmou codecs disponíveis. Verifique se o driver NVIDIA e suas bibliotecas CUDA/NVENC estão instalados e compatíveis com a GPU.".into());
        }
        return Err("Não foi possível confirmar codecs de codificação por GPU. Verifique o driver e o componente de captura.".into());
    }
    if !info.card_path.is_empty() {
        info.gpu = format!("{} · {}", info.gpu, info.card_path);
    }
    Ok(info)
}
pub fn audio_sources() -> Result<selection::AudioSources, String> {
    let exe = executable().ok_or("Instale o componente de captura.")?;
    Ok(parse_audio_sources(
        &probe(&exe, "--list-audio-devices")?,
        &probe(&exe, "--list-application-audio")?,
    ))
}
fn parse_audio_sources(devices: &str, applications: &str) -> selection::AudioSources {
    use selection::{AudioSources, Source};
    let mut result = AudioSources {
        outputs: vec![Source {
            id: "default_output".into(),
            label: "Saída padrão do sistema".into(),
        }],
        inputs: vec![Source {
            id: "default_input".into(),
            label: "Microfone padrão".into(),
        }],
        applications: vec![],
    };
    for line in devices.lines().take(32) {
        let Some((id, label)) = line.split_once('|') else {
            continue;
        };
        if id.starts_with("default_") || id.is_empty() {
            continue;
        }
        let source = Source {
            id: format!("device:{id}"),
            label: label.chars().take(128).collect(),
        };
        if source.id.len() > 512 || source.id.chars().any(|c| c.is_control() || c == '|') {
            continue;
        }
        if id.ends_with(".monitor") {
            result.outputs.push(source);
        } else {
            result.inputs.push(source);
        }
    }
    for name in applications
        .lines()
        .take(32)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let id = format!("app:{name}");
        if id.len() <= 512
            && !name.chars().any(|c| c.is_control() || c == '|')
            && !result.applications.iter().any(|s| s.id == id)
        {
            result.applications.push(Source {
                id,
                label: name.chars().take(128).collect(),
            });
        }
    }
    result
}
pub(crate) fn validate(options: &CaptureOptions) -> Result<(), String> {
    if options.game_key.is_empty() || options.game_key.len() > 4096 || options.game_name.len() > 512
    {
        return Err("Identificação do jogo inválida.".into());
    }
    if !["portal", "direct"].contains(&options.method.as_str()) {
        return Err("Modo de captura inválido.".into());
    }
    if !(1..=900).contains(&options.replay_seconds) {
        return Err("A duração do replay deve ser de 1 a 900 segundos.".into());
    }
    options.selection.validate()?;
    Ok(())
}
fn game_folder(base: &Path, key: &str, name: &str) -> PathBuf {
    let slug: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(60)
        .collect();
    let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
    base.join(format!(
        "{}-{}",
        if slug.is_empty() { "Game" } else { &slug },
        &hash[..12]
    ))
}
pub fn output_folder(key: &str, name: &str) -> Result<PathBuf, String> {
    let base = storage::directory()?;
    Ok(game_folder(&base, key, name))
}
pub fn clips(key: &str, name: &str) -> Result<Vec<Clip>, String> {
    let folder = output_folder(key, name)?;
    if !folder.exists() {
        return Ok(vec![]);
    }
    let mut clips: Vec<Clip> = fs::read_dir(folder)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|entry| {
            if !["mp4", "mkv", "webm"].contains(&entry.path().extension()?.to_str()?) {
                return None;
            }
            let meta = entry.metadata().ok()?;
            if !meta.is_file() || entry.file_type().ok()?.is_symlink() {
                return None;
            }
            Some(Clip {
                title: library::title(&entry.path()),
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry.path().to_string_lossy().into_owned(),
                bytes: meta.len(),
                modified: meta
                    .modified()
                    .ok()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_secs(),
            })
        })
        .collect();
    clips.sort_by(|a, b| b.modified.cmp(&a.modified));
    clips.truncate(200);
    Ok(clips)
}
fn connect_ipc(socket:&Path,timeout:Duration)->Result<UnixStream,String>{
    let mut address:libc::sockaddr_un=unsafe{std::mem::zeroed()};
    address.sun_family=libc::AF_UNIX as _;
    let path=socket.as_os_str().as_bytes();
    if path.len()>=address.sun_path.len() || path.contains(&0){return Err("Caminho IPC inválido.".into());}
    for (dst,src) in address.sun_path.iter_mut().zip(path){*dst=*src as _;}
    let fd=unsafe{libc::socket(libc::AF_UNIX,libc::SOCK_STREAM|libc::SOCK_CLOEXEC|libc::SOCK_NONBLOCK,0)};
    if fd<0{return Err(std::io::Error::last_os_error().to_string());}
    let stream=unsafe{UnixStream::from_raw_fd(fd)};
    if unsafe{libc::connect(fd,(&address as *const libc::sockaddr_un).cast(),std::mem::size_of_val(&address) as _)}<0{
        let error=std::io::Error::last_os_error();
        if error.raw_os_error()!=Some(libc::EINPROGRESS){return Err(format!("Não foi possível falar com o gravador: {error}"));}
        let mut poll=libc::pollfd{fd,events:libc::POLLOUT,revents:0};
        if unsafe{libc::poll(&mut poll,1,timeout.as_millis().min(i32::MAX as u128) as i32)}<=0{return Err("O gravador não respondeu à conexão.".into());}
        let mut error:libc::c_int=0;let mut len=std::mem::size_of_val(&error) as libc::socklen_t;
        if unsafe{libc::getsockopt(stream.as_raw_fd(),libc::SOL_SOCKET,libc::SO_ERROR,(&mut error as *mut libc::c_int).cast(),&mut len)}<0||error!=0{return Err("Não foi possível conectar ao gravador.".into());}
    }
    stream.set_nonblocking(false).map_err(|e|e.to_string())?;Ok(stream)
}
fn ipc_reply(socket: &Path, name: &str, data: Value, timeout:Duration) -> Result<Value, String> {
    let mut stream=connect_ipc(socket,timeout)?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(timeout.min(Duration::from_secs(3))))
        .map_err(|e| e.to_string())?;
    let request = json!({"id":1,"name":name,"data":data}).to_string() + "\n";
    stream
        .write_all(request.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .take(65537)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if line.len() > 65536 || !line.ends_with('\n') {
        return Err("Resposta incompleta do gravador.".into());
    }
    let reply: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    if reply["id"]!=1 || !matches!(reply["result"].as_str(),Some("ok"|"error")) {return Err("Resposta inválida do gravador.".into());}
    Ok(reply)
}
fn recorder_ready(socket:&Path)->bool {
    // GSR 6.1.4 has no status/ping command. Its request thread starts only after
    // recorder creation (including the portal). A deliberate unknown request
    // proves that initialization completed without pausing or saving anything.
    ipc_reply(socket,"peligames-readiness",Value::Null,Duration::from_millis(250)).is_ok_and(|reply|
        reply["result"]=="error" && reply["data"]=="unknown request name 'peligames-readiness'")
}
fn ipc(socket: &Path, name: &str, data: Value) -> Result<Option<String>, String> {
    let reply=ipc_reply(socket,name,data,Duration::from_secs(30))?;
    if reply["result"] != "ok" {
        return Err(reply["data"]
            .as_str()
            .unwrap_or("O gravador recusou a operação.")
            .into());
    }
    Ok(reply["data"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_string))
}
fn args(
    options: &CaptureOptions,
    replay: bool,
    folder: &Path,
    socket: &Path,
    id: &str,
) -> Vec<String> {
    let source = if options.method == "portal" {
        "portal"
    } else {
        &options.monitor
    };
    let mut args = vec![
        "-w".into(),
        source.into(),
        "-f".into(),
        options.selection.fps.to_string(),
        "-c".into(),
        options.selection.format.clone(),
        "-k".into(),
        if options.selection.codec != "auto" {
            options.selection.codec.as_str()
        } else if options.selection.format == "webm" {
            "vp9"
        } else {
            "h264"
        }
        .into(),
        "-ac".into(),
        "opus".into(),
        "-fallback-cpu-encoding".into(),
        "no".into(),
        "-bm".into(),
        options.selection.rate_control.clone(),
        "-q".into(),
        if options.selection.rate_control == "cbr" {
            options.selection.bitrate_kbps.to_string()
        } else {
            options.selection.quality_preset.clone()
        },
        "-s".into(),
        options.selection.resolution.clone(),
        "-ipc".into(),
        socket.to_string_lossy().into_owned(),
    ];
    let mut audio = Vec::new();
    if options.selection.desktop_audio {
        audio.push(options.selection.output_source.as_str());
    }
    if options.selection.microphone {
        audio.push(options.selection.input_source.as_str());
    }
    if !audio.is_empty() {
        args.extend(["-a".into(), audio.join("|")]);
    }
    if replay {
        args.extend([
            "-r".into(),
            options.replay_seconds.to_string(),
            "-ro".into(),
            folder.to_string_lossy().into_owned(),
        ]);
    }
    args.extend([
        "-o".into(),
        if replay {
            folder.to_string_lossy().into_owned()
        } else {
            folder
                .join(format!("session-{id}.{}", options.selection.format))
                .to_string_lossy()
                .into_owned()
        },
    ]);
    args
}
// A single worker owns process reaping and capture finalization. The game
// watch observes exits; it never signals the game or another recorder.
fn monitor_session(
    inner: Arc<Mutex<Inner>>,
    child: Arc<Mutex<Child>>,
    log: Arc<Mutex<VecDeque<u8>>>,
    session_id: String,
    directory: PathBuf,
    mut game: capture_lifetime::Game,
    microphone_failure: Option<Arc<Mutex<Option<String>>>>,
    callback: Notify,
) {
    let mut microphone_failed_at = None;
    let mut game_closed_at = None;
    loop {
        if game.ended() {
            if game_closed_at.is_none() {
                // Expected shutdown: SIGINT finalizes an ongoing recording.
                // Never restart the recorder or ask the portal for a new source.
                if let Ok(mut state) = inner.lock() {
                    if state.session.as_ref().is_some_and(|s| s.id == session_id) {
                        state.startup_canceled = true;
                    }
                }
                if let Ok(mut process) = child.lock() {
                    if matches!(process.try_wait(), Ok(None)) {
                        unsafe {
                            libc::kill(process.id() as i32, libc::SIGINT);
                        }
                    }
                }
                game_closed_at = Some(Instant::now());
            } else if game_closed_at.is_some_and(|since| since.elapsed() >= Duration::from_secs(5))
            {
                if let Ok(mut process) = child.lock() {
                    if matches!(process.try_wait(), Ok(None)) {
                        let _ = process.kill();
                    }
                }
            }
        }
        if let Some(error) = microphone_failure
            .as_ref()
            .and_then(|failure| failure.lock().ok().and_then(|v| v.clone()))
        {
            if let Ok(mut state) = inner.lock() {
                if state.session.as_ref().is_some_and(|s| s.id == session_id)
                    && !state.startup_canceled
                {
                    state.status.error = Some(error);
                }
            }
            if let Ok(mut child) = child.lock() {
                if matches!(child.try_wait(), Ok(None)) {
                    match microphone_failed_at {
                        None => {
                            unsafe {
                                libc::kill(child.id() as i32, libc::SIGINT);
                            }
                            microphone_failed_at = Some(Instant::now());
                        }
                        Some(since) if since.elapsed() >= Duration::from_secs(1) => {
                            let _ = child.kill();
                        }
                        _ => {}
                    }
                }
            }
        }
        let exit = child.lock().unwrap().try_wait();
        if !matches!(exit, Ok(None)) {
            let mut state = inner.lock().unwrap();
            if state.session.as_ref().is_some_and(|s| s.id == session_id) {
                let failed = !matches!(exit,Ok(Some(status)) if status.success());
                let text = String::from_utf8_lossy(
                    &log.lock().unwrap().iter().copied().collect::<Vec<_>>(),
                )
                .into_owned();
                state.session = None;
                state.status.phase = "idle".into();
                state.status.recording = false;
                state.status.replay = false;
                if failed && !state.startup_canceled && state.status.error.is_none() {
                    state.status.error = Some(if text.trim().is_empty() {
                        "O gravador foi encerrado antes de concluir a captura.".into()
                    } else {
                        text
                    });
                }
                let status = state.status.clone();
                drop(state);
                callback(status);
            }
            let _ = fs::remove_file(directory.join("ipc"));
            let _ = fs::remove_dir(directory);
            break;
        }
        if game_closed_at.is_some() {
            thread::sleep(Duration::from_millis(100));
        } else {
            game.wait();
        }
    }
}

impl CaptureManager {
    pub fn status(&self) -> CaptureStatus {
        let mut s = self.inner.lock().unwrap().status.clone();
        if s.phase.is_empty() {
            s.phase = "idle".into();
        }
        s
    }
    pub fn rename_clip(&self,key:&str,name:&str,path:&Path,title:&str)->Result<library::Renamed,String>{
        let _operation=self.operation.try_lock().map_err(|_|"Aguarde a operação de captura antes de renomear o vídeo.".to_string())?;
        if self.status().phase!="idle"{return Err("Encerre a captura ou o replay antes de renomear vídeos.".into());}
        let result=library::rename(&output_folder(key,name)?,path,title)?;
        if let Ok(mut state)=self.inner.lock(){if state.status.last_file.as_deref()==path.to_str(){state.status.last_file=Some(result.path.clone());}}
        Ok(result)
    }
    pub fn configure_storage(&self,directory:Option<String>)->Result<storage::StorageInfo,String>{
        let _operation=self.operation.try_lock().map_err(|_|"Aguarde a operação de captura antes de alterar a pasta.".to_string())?;
        if self.status().phase!="idle" {return Err("Encerre a captura ou o replay antes de alterar a pasta de gravações.".into());}
        storage::configure(directory)
    }
    pub fn start(
        &self,
        options: CaptureOptions,
        replay: bool,
        notify: Notify,
    ) -> Result<CaptureStatus, String> {
        self.start_owned(options, replay, notify, None)
    }
    pub fn start_from_overlay(&self, options:CaptureOptions, replay:bool, notify:Notify, peer:u32) -> Result<CaptureStatus,String> {
        self.start_owned(options, replay, notify, Some(peer))
    }
    fn start_owned(&self, mut options:CaptureOptions, replay:bool, notify:Notify, peer:Option<u32>) -> Result<CaptureStatus,String> {
        let _operation = self.operation.lock().unwrap();
        let _runtime = super::capture_runtime::OPERATION
            .try_lock()
            .map_err(|_| "Aguarde a instalação do componente de captura.")?;
        validate(&options)?;
        if self.inner.lock().unwrap().session.is_some() {
            return Err(
                "Já existe uma captura do PeliGames. Encerre-a antes de trocar de fonte ou jogo."
                    .into(),
            );
        }
        let mut game = capture_lifetime::Game::new(&options.game_directory, &options.game_executable, &options.steam_app_id, peer);
        let supported = support()?;
        if !supported.installed || !supported.ipc {
            return Err(
                "Instale ou repare o componente de captura pela aba Capturas. O backend precisa oferecer controle IPC.".into(),
            );
        }
        options.selection.codec = encoder_support()?.resolve(&options.selection)?;
        if options.method == "portal" && !supported.portal {
            return Err("O portal não está disponível nesta sessão. Verifique PipeWire/xdg-desktop-portal e use uma sessão Wayland.".into());
        }
        if options.method == "direct" && !supported.monitors.contains(&options.monitor) {
            return Err("Selecione um monitor disponível para a captura direta.".into());
        }
        if options.selection.desktop_audio && options.selection.output_source == "game:auto" {
            let available = audio_sources()?.applications.into_iter().filter_map(|s|s.id.strip_prefix("app:").map(str::to_owned)).collect::<Vec<_>>();
            options.selection.output_source = capture_game_audio::resolve(&capture_game_audio::Target {
                directory: options.game_directory.clone(), executable: options.game_executable.clone(), steam_app_id: options.steam_app_id.clone(),
            }, &available)?;
        }
        let microphone=if options.selection.microphone && options.selection.microphone_noise_reduction {
            let filter=capture_microphone::MicrophoneFilter::start(&options.selection.input_source)?;
            options.selection.input_source=filter.source().into();Some(filter)
        }else{None};
        let microphone_failure=microphone.as_ref().map(|filter|filter.failure());
        let folder = output_folder(&options.game_key, &options.game_name)?;
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let id = uuid::Uuid::new_v4().to_string();
        // Short unique path, created atomically with private permissions, including on shared /tmp.
        let directory = std::env::temp_dir().join(format!("peli-capture-{id}"));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|e| e.to_string())?;
        let socket = directory.join("ipc");
        let mut command = Command::new(supported.executable.unwrap());
        super::env_utils::host_environment(&mut command);
        if game.ended() { let _=fs::remove_dir(&directory); return Err("O jogo foi encerrado antes de iniciar a captura.".into()); }
        let mut child = match command
            .args(args(&options, replay, &folder, &socket, &id))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_dir(&directory);
                return Err(e.to_string());
            }
        };
        let log = Arc::new(Mutex::new(VecDeque::new()));
        drain(child.stderr.take().unwrap(), log.clone());
        drain(
            child.stdout.take().unwrap(),
            Arc::new(Mutex::new(VecDeque::new())),
        );
        let child = Arc::new(Mutex::new(child));
        {
            let mut inner = self.inner.lock().unwrap();
            inner.startup_canceled=false;
            inner.status = CaptureStatus {
                phase: "starting".into(),
                game_key: Some(options.game_key),
                game_name: Some(options.game_name),
                replay,
                ..Default::default()
            };
            inner.session = Some(Session {
                child: child.clone(),
                socket: socket.clone(),
                directory: directory.clone(),
                id: id.clone(),
                _microphone: microphone,
            });
        }
        notify(self.status());
        let inner = self.inner.clone();
        let session_id = id.clone();
        let callback = notify.clone();
        thread::spawn(move || monitor_session(inner, child, log, session_id, directory, game, microphone_failure, callback));
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if !self
                .inner
                .lock()
                .unwrap()
                .session
                .as_ref()
                .is_some_and(|s| s.id == id)
            {
                return Err(self.status().error.unwrap_or("Captura cancelada.".into()));
            }
            if socket.exists() && recorder_ready(&socket) {
                break;
            }
            if Instant::now() > deadline {
                let reason="O gravador não confirmou a inicialização da captura no prazo. Selecione a fonte e tente novamente.";
                self.abort_start(Some(reason.into()),notify.clone());
                return Err(reason.into());
            }
            thread::sleep(Duration::from_millis(150));
        }
        let mut inner = self.inner.lock().unwrap();
        if !inner.session.as_ref().is_some_and(|s| s.id == id) {
            return Err("O gravador foi encerrado.".into());
        }
        inner.status.phase = "active".into();
        inner.status.replay = replay;
        inner.status.recording = !replay;
        let status = inner.status.clone();
        drop(inner);
        notify(status.clone());
        Ok(status)
    }
    fn abort_start(&self,error:Option<String>,notify:Notify)->CaptureStatus {
        self.abort_start_owned(error,notify,None).unwrap_or_else(|_|self.status())
    }
    fn abort_start_owned(&self,error:Option<String>,notify:Notify,game:Option<&str>)->Result<CaptureStatus,String> {
        let session={let mut inner=self.inner.lock().unwrap();
            if game.is_some_and(|game|inner.status.game_key.as_deref()!=Some(game)) {return Err("A captura pertence a outro jogo.".into());}
            if inner.status.phase!="starting" {return Ok(inner.status.clone());}
            inner.startup_canceled=true;inner.status.error=error.clone();
            inner.session.as_ref().map(|s|(s.id.clone(),s.child.clone(),s.directory.clone()))
        };
        let Some((id,child,directory))=session else{return Ok(self.status());};
        if let Ok(mut child)=child.lock(){if matches!(child.try_wait(),Ok(None)){unsafe{libc::kill(child.id() as i32,libc::SIGINT);}}}
        let deadline=Instant::now()+Duration::from_secs(1);
        loop {
            let done=child.lock().map(|mut c|!matches!(c.try_wait(),Ok(None))).unwrap_or(true);
            if done {break;}
            if Instant::now()>=deadline {
                if let Ok(mut c)=child.lock(){let _=c.kill();let _=c.wait();}break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let mut inner=self.inner.lock().unwrap();
        if inner.session.as_ref().is_some_and(|s|s.id==id) {
            inner.session=None;inner.status.phase="idle".into();inner.status.recording=false;inner.status.replay=false;inner.status.error=error;
        }
        let status=inner.status.clone();drop(inner);
        let _=fs::remove_file(directory.join("ipc"));let _=fs::remove_dir(directory);
        notify(status.clone());Ok(status)
    }
    fn interrupt(&self) {
        if let Some(session) = &self.inner.lock().unwrap().session {
            let mut child = session.child.lock().unwrap();
            if matches!(child.try_wait(), Ok(None)) {
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGINT);
                }
            }
        }
    }
    pub fn action(&self, action: &str, notify: Notify) -> Result<CaptureStatus, String> {
        self.action_owned(action,notify,None)
    }
    pub fn action_for_game(&self,action:&str,notify:Notify,game:&str)->Result<CaptureStatus,String> {
        self.action_owned(action,notify,Some(game))
    }
    fn action_owned(&self,action:&str,notify:Notify,game:Option<&str>)->Result<CaptureStatus,String> {
        // Cancellation remains available while a portal/authentication dialog is open.
        if action == "stop" && self.status().phase == "starting" {
            return self.abort_start_owned(None,notify,game);
        }
        let _operation = self.operation.lock().unwrap();
        let (socket, id) = {
            let inner = self.inner.lock().unwrap();
            if game.is_some_and(|game|inner.status.game_key.as_deref()!=Some(game)) {return Err("A captura pertence a outro jogo.".into());}
            let s = inner.session.as_ref().ok_or("Nenhuma captura ativa.")?;
            (s.socket.clone(), s.id.clone())
        };
        let status = self.status();
        let command = match action {
            "stop" => "stop",
            "save-replay" if status.replay => "save-replay",
            "start-recording" if status.replay && !status.recording => "start-replay-recording",
            "stop-recording" if status.replay && status.recording => "stop-replay-recording",
            _ => return Err("Esta ação não está disponível no estado atual da captura.".into()),
        };
        let saved = ipc(&socket, command, Value::Null)?;
        let mut inner = self.inner.lock().unwrap();
        // The exit worker may have already reaped the process, but cannot create another session.
        if inner.session.as_ref().is_none_or(|s| s.id == id) {
            if let Some(file) = saved {
                inner.status.last_file = Some(file);
            }
            match action {
                "stop" => {
                    inner.status.phase = "idle".into();
                    inner.status.recording = false;
                    inner.status.replay = false;
                }
                "start-recording" => inner.status.recording = true,
                "stop-recording" => inner.status.recording = false,
                _ => {}
            }
        }
        let status = inner.status.clone();
        drop(inner);
        notify(status.clone());
        Ok(status)
    }
    pub fn shutdown(&self) {
        // SIGINT finalizes recordings; never signal an unrelated recorder instance.
        self.interrupt();
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.inner.lock().unwrap().session.is_some() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(50));
        }
        let inner = self.inner.lock().unwrap();
        if let Some(session) = &inner.session {
            let mut child = session.child.lock().unwrap();
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&session.socket);
            let _ = fs::remove_dir(&session.directory);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> CaptureOptions {
        CaptureOptions {
            game_key: "steam:123".into(),
            game_name: "../Game / Test".into(),
            game_directory: String::new(), game_executable: String::new(), steam_app_id: String::new(),
            method: "portal".into(),
            monitor: "DP-1".into(),
            replay_seconds: 60,
            selection: selection::Selection {output_source:"default_output".into(), ..Default::default()},
        }
    }
    #[test]
    fn nvidia_nvenc_uses_reported_codecs_without_assuming_av1() {
        let report = |codecs: &str| {
            format!("section=gpu_info\nvendor|nvidia\ncard_path|/dev/dri/card1\nsection=video_codecs\n{codecs}\nsection=containers\nmp4\nmkv\nwebm\n")
        };
        let older = parse_encoder_info(&report("h264\nhevc")).unwrap();
        assert_eq!(older.vendor, "nvidia");
        assert_eq!(older.encoder, "NVENC");
        let mut settings = selection::Selection::default();
        assert_eq!(older.resolve(&settings).unwrap(), "hevc");
        settings.codec = "auto".into();
        let newer_auto = parse_encoder_info(&report("h264\nhevc\nav1")).unwrap();
        assert_eq!(newer_auto.resolve(&settings).unwrap(), "av1");
        let baseline = parse_encoder_info(&report("h264")).unwrap();
        assert_eq!(baseline.resolve(&settings).unwrap(), "h264");
        settings.codec = "av1".into();
        assert!(older.resolve(&settings).is_err());
        let newer = parse_encoder_info(&report("h264\nhevc\nav1")).unwrap();
        assert_eq!(newer.resolve(&settings).unwrap(), "av1");
        settings.format = "webm".into();
        assert_eq!(newer.resolve(&settings).unwrap(), "av1");
        assert!(older.resolve(&settings).is_err());
        assert!(parse_encoder_info(&report("h264_software"))
            .unwrap_err()
            .contains("NVENC"));
    }
    #[test]
    fn encoder_detection_and_codec_compatibility_use_confirmed_hardware_only() {
        let info=parse_encoder_info("section=gpu_info\nvendor|amd\ncard_path|/dev/dri/card0\nsection=video_codecs\nh264\nh264_software\nav1_hdr\nav1\nhevc\nsection=containers\nmp4\nmkv\nwebm\n").unwrap();
        assert_eq!(info.codecs, vec!["h264", "av1", "hevc"]);
        let mut settings = selection::Selection::default();
        settings.codec = "av1".into();
        assert_eq!(info.resolve(&settings).unwrap(), "av1");
        settings.format = "webm".into();
        assert_eq!(info.resolve(&settings).unwrap(), "av1");
        settings.codec = "h264".into();
        assert!(info.resolve(&settings).is_err());
        settings.codec = "auto".into();
        assert_eq!(info.resolve(&settings).unwrap(), "av1");
        settings.format = "mp4".into();
        settings.codec = "vp9".into();
        assert!(info.resolve(&settings).is_err());
        assert!(parse_encoder_info("section=video_codecs\nh264_software\nav1_hdr\n").is_err());
        let mut options = options();
        options.selection.codec = "av1".into();
        let arguments = args(
            &options,
            false,
            Path::new("/clips"),
            Path::new("/ipc"),
            "test",
        );
        assert!(arguments.windows(2).any(|v| v == ["-k", "av1"]));
        assert!(arguments
            .windows(2)
            .any(|v| v == ["-fallback-cpu-encoding", "no"]));
    }
    #[test]
    fn quality_resolution_and_frame_rate_reach_the_encoder() {
        let mut settings = options();
        let legacy:CaptureOptions=serde_json::from_value(serde_json::json!({"game_key":"game","game_name":"Game","method":"portal","monitor":"","replay_seconds":60,"fps":30,"desktop_audio":true,"microphone":false})).unwrap();
        assert_eq!(legacy.selection.fps, 30);
        assert_eq!(legacy.selection.resolution, "1280x720");
        assert_eq!(legacy.selection.bitrate_kbps, 5000);
        settings.selection.resolution = "1920x1080".into();
        settings.selection.fps = 144;
        settings.selection.bitrate_kbps = 45000;
        let arguments = args(
            &settings,
            true,
            Path::new("/clips"),
            Path::new("/ipc"),
            "test",
        );
        for pair in [
            ["-s", "1920x1080"],
            ["-f", "144"],
            ["-bm", "cbr"],
            ["-q", "45000"],
        ] {
            assert!(arguments.windows(2).any(|v| v == pair));
        }
        for mode in ["qp", "vbr"] {
            settings.selection.rate_control = mode.into();
            settings.selection.quality_preset = "ultra".into();
            let arguments = args(
                &settings,
                false,
                Path::new("/clips"),
                Path::new("/ipc"),
                "test",
            );
            assert!(arguments.windows(2).any(|v| v == ["-bm", mode]));
            assert!(arguments.windows(2).any(|v| v == ["-q", "ultra"]));
        }
        settings.selection.resolution = "854x480".into();
        validate(&settings).unwrap();
        let low_resolution = args(&settings, false, Path::new("/clips"), Path::new("/ipc"), "test");
        assert!(low_resolution.windows(2).any(|v|v==["-s","854x480"]));
        settings.selection.resolution = "1920x1080;bad".into();
        assert!(validate(&settings).is_err());
        settings.selection.resolution = "0x0".into();
        settings.selection.bitrate_kbps = 0;
        assert!(validate(&settings).is_err());
        settings.selection.bitrate_kbps = 20000;
        settings.selection.fps = 999;
        assert!(validate(&settings).is_err());
    }
    #[test]
    fn storage_cannot_change_while_capturing_or_starting() {
        let manager=CaptureManager::default();
        for phase in ["starting","active"] {
            manager.inner.lock().unwrap().status.phase=phase.into();
            assert!(manager.configure_storage(Some("/unused-fixture".into())).unwrap_err().contains("Encerre"));
            assert!(manager.rename_clip("unused","Unused",Path::new("/unused-fixture"),"Nome").unwrap_err().contains("Encerre"));
        }
        manager.inner.lock().unwrap().status.phase="idle".into();
        let _operation=manager.operation.lock().unwrap();
        assert!(manager.configure_storage(None).unwrap_err().contains("Aguarde"));
    }
    #[test]
    fn custom_replay_duration_is_validated_and_does_not_limit_manual_recording(){
        let mut settings=options();
        for seconds in [1,15,45,300,900] {
            settings.replay_seconds=seconds;validate(&settings).unwrap();
            let replay=args(&settings,true,Path::new("/clips"),Path::new("/ipc"),"test");
            assert!(replay.windows(2).any(|pair|pair[0]=="-r"&&pair[1]==seconds.to_string()));
            let manual=args(&settings,false,Path::new("/clips"),Path::new("/ipc"),"test");
            assert!(!manual.iter().any(|arg|arg=="-r"));
        }
        for seconds in [0,901,u16::MAX] {settings.replay_seconds=seconds;assert!(validate(&settings).is_err());}
    }
    #[test]
    fn formats_audio_choices_and_legacy_settings_are_preserved() {
        let mut automatic = selection::Selection::default();
        assert_eq!(automatic.output_source, "game:auto");
        automatic.validate().unwrap();
        automatic.input_source = "game:auto".into();
        assert!(automatic.validate().is_err());
        let mut options = options();
        let old = serde_json::json!({"game_key":"g","game_name":"Game","method":"portal","monitor":"","fps":60,"replay_seconds":60,"desktop_audio":false,"microphone":true});
        let restored: CaptureOptions = serde_json::from_value(old).unwrap();
        assert_eq!(restored.selection.format, "mkv");
        assert!(!restored.selection.desktop_audio);
        assert!(restored.selection.microphone);
        assert!(!restored.selection.microphone_noise_reduction);
        assert!(!selection::Selection::default().microphone_noise_reduction);
        for format in ["mp4", "mkv", "webm"] {
            options.selection.format = format.into();
            options.selection.output_source = "app:Game With Spaces".into();
            options.selection.microphone = true;
            options.selection.input_source = "device:USB microphone".into();
            validate(&options).unwrap();
            let arguments = args(
                &options,
                false,
                Path::new("/videos"),
                Path::new("/ipc"),
                "test",
            );
            assert!(arguments.windows(2).any(|v| v == ["-c", format]));
            assert!(arguments
                .windows(2)
                .any(|v| v == ["-a", "app:Game With Spaces|device:USB microphone"]));
            assert!(arguments.last().unwrap().ends_with(&format!(".{format}")));
            assert!(arguments
                .windows(2)
                .any(|v| v == ["-k", if format == "webm" { "vp9" } else { "h264" }]));
        }
        options.selection.output_source = "app:Game|default_output".into();
        assert!(validate(&options).is_err());
        options.selection.output_source = "default_output".into();
        options.selection.format = "webp".into();
        assert!(validate(&options).is_err());
        let sources=parse_audio_sources("default_output|Default output\ndefault_input|Default input\nspeakers.monitor|Speakers\nusb-mic|USB microphone","Game With Spaces\nGame With Spaces\nBrowser\n");
        assert_eq!(sources.outputs[1].id, "device:speakers.monitor");
        assert_eq!(sources.inputs[1].label, "USB microphone");
        assert_eq!(sources.applications.len(), 2);
        assert_eq!(sources.applications[0].id, "app:Game With Spaces");
    }
    #[test]
    fn validation_and_paths() {
        let mut o = options();
        assert!(validate(&o).is_ok());
        o.method = "root-shell".into();
        assert!(validate(&o).is_err());
        o = options();
        o.selection.fps = 999;
        assert!(validate(&o).is_err());
        let base = Path::new("/clips");
        let path = game_folder(base, "../../x", "../../x");
        assert_eq!(path.parent(), Some(base));
        assert_ne!(path, game_folder(base, "other", "../../x"));
    }
    #[test]
    fn arguments_keep_portal_unprivileged_and_replay_shared() {
        let o = options();
        let a = args(
            &o,
            true,
            Path::new("/videos with spaces"),
            Path::new("/private/ipc"),
            "id",
        );
        assert_eq!(&a[..2], ["-w", "portal"]);
        assert!(!a.iter().any(|x| x == "sudo" || x == "pkexec"));
        assert!(a.windows(2).any(|w| w == ["-r", "60"]));
        assert!(a.windows(2).any(|w| w == ["-ro", "/videos with spaces"]));
        assert!(!a.iter().any(|x| x == "default_input"));
    }
    #[test]
    fn startup_uses_the_real_gsr_unknown_request_reply_without_changing_capture(){
        use std::os::unix::net::UnixListener;
        let dir=std::env::temp_dir().join(format!("peli-ready-{}",uuid::Uuid::new_v4()));fs::create_dir(&dir).unwrap();
        let socket=dir.join("ipc");let listener=UnixListener::bind(&socket).unwrap();
        let worker=thread::spawn(move||{
            let (mut stream,_)=listener.accept().unwrap();let mut request=String::new();BufReader::new(stream.try_clone().unwrap()).read_line(&mut request).unwrap();
            let request:Value=serde_json::from_str(&request).unwrap();assert_eq!(request["name"],"peligames-readiness");
            // Exact GSR 6.1.4 behavior: it offers neither status nor ping.
            writeln!(stream,"{}",json!({"id":1,"result":"error","data":"unknown request name 'peligames-readiness'"})).unwrap();
        });
        assert!(recorder_ready(&socket));worker.join().unwrap();fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn startup_probe_has_a_short_timeout_while_portal_is_waiting(){
        use std::os::unix::net::UnixListener;
        let dir=std::env::temp_dir().join(format!("peli-not-ready-{}",uuid::Uuid::new_v4()));fs::create_dir(&dir).unwrap();
        let socket=dir.join("ipc");let listener=UnixListener::bind(&socket).unwrap();
        let worker=thread::spawn(move||{let (_stream,_)=listener.accept().unwrap();thread::sleep(Duration::from_millis(600));});
        let before=Instant::now();assert!(!recorder_ready(&socket));assert!(before.elapsed()<Duration::from_secs(1));
        worker.join().unwrap();fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cancellation_releases_starting_even_if_the_owned_recorder_ignores_sigint(){
        let dir=std::env::temp_dir().join(format!("peli-cancel-{}",uuid::Uuid::new_v4()));fs::create_dir(&dir).unwrap();
        let mut command=Command::new("/bin/sh");command.args(["-c","trap '' INT; echo ready; exec sleep 60"]).stdout(Stdio::piped());
        let mut child=command.spawn().unwrap();let mut line=String::new();BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();assert_eq!(line.trim(),"ready");
        let child=Arc::new(Mutex::new(child));let manager=CaptureManager::default();
        {let mut inner=manager.inner.lock().unwrap();inner.status=CaptureStatus{phase:"starting".into(),replay:true,..Default::default()};inner.session=Some(Session{child:child.clone(),socket:dir.join("ipc"),directory:dir.clone(),id:"cancel-test".into(),_microphone:None});}
        // start holds this lock: cancellation must not wait behind initialization.
        let _operation=manager.operation.lock().unwrap();let before=Instant::now();
        let status=manager.action("stop",Arc::new(|_|{})).unwrap();
        assert_eq!(status.phase,"idle");assert!(!status.replay);assert!(manager.inner.lock().unwrap().session.is_none());assert!(before.elapsed()<Duration::from_secs(3));
        assert!(child.lock().unwrap().try_wait().unwrap().is_some());assert!(!dir.exists());
    }
    #[test]
    fn an_overlay_cannot_stop_or_cancel_another_games_session() {
        for phase in ["starting","active","idle"] {
            let child=Command::new("/bin/sleep").arg("30").spawn().unwrap();
            let manager=CaptureManager::default();
            let owned=Arc::new(Mutex::new(child));
            {let mut inner=manager.inner.lock().unwrap();inner.status=CaptureStatus{phase:phase.into(),game_key:Some("diablo".into()),replay:true,..Default::default()};
             inner.session=Some(Session{child:owned.clone(),socket:PathBuf::from("/unused/ipc"),directory:PathBuf::from("/unused"),id:"foreign-test".into(),_microphone:None});}
            assert!(manager.action_for_game("stop",Arc::new(|_|{}),"tomb").unwrap_err().contains("outro jogo"));
            assert_eq!(manager.status().phase,phase);assert!(owned.lock().unwrap().try_wait().unwrap().is_none());
            let mut child=owned.lock().unwrap();child.kill().unwrap();child.wait().unwrap();drop(child);
            manager.inner.lock().unwrap().session=None;
        }
    }
    #[test]
    fn game_exit_finalizes_the_owned_recorder_and_clears_replay_state() {
        for (phase, recording, replay) in [
            ("active", true, false),
            ("active", false, true),
            ("active", true, true),
            ("starting", false, true),
        ] {
            let directory =
                std::env::temp_dir().join(format!("peli-game-exit-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&directory).unwrap();
            let marker = directory.join("finalized");
            let mut recorder=Command::new("/usr/bin/python3").args(["-u","-c",
                "import pathlib,signal,sys,time\ndef stop(*args):\n pathlib.Path(sys.argv[1]).write_text('finalized')\n sys.exit(0)\nsignal.signal(signal.SIGINT,stop)\nprint('ready')\nwhile True: time.sleep(1)"])
                .arg(&marker).stdout(Stdio::piped()).spawn().unwrap();
            let mut ready = String::new();
            BufReader::new(recorder.stdout.take().unwrap())
                .read_line(&mut ready)
                .unwrap();
            assert_eq!(ready.trim(), "ready");
            let child = Arc::new(Mutex::new(recorder));
            let mut unrelated = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            let mut process = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            let game = capture_lifetime::Game::new("", "", "", Some(process.id()));
            let manager = CaptureManager::default();
            {
                let mut inner = manager.inner.lock().unwrap();
                inner.status = CaptureStatus {
                    phase: phase.into(),
                    recording,
                    replay,
                    ..Default::default()
                };
                inner.session = Some(Session {
                    child: child.clone(),
                    socket: directory.join("ipc"),
                    directory: directory.clone(),
                    id: "owned-game-test".into(),
                    _microphone: None,
                });
            }
            // Startup owns this mutex while the portal dialog is pending; shutdown
            // must still work instead of waiting behind the permission request.
            let _operation = manager.operation.lock().unwrap();
            let (sender, receiver) = std::sync::mpsc::channel();
            let notify: Notify = Arc::new(move |status| {
                let _ = sender.send(status);
            });
            let inner = manager.inner.clone();
            let dir = directory.clone();
            let worker = thread::spawn(move || {
                monitor_session(
                    inner,
                    child,
                    Default::default(),
                    "owned-game-test".into(),
                    dir,
                    game,
                    None,
                    notify,
                )
            });
            assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
            process.kill().unwrap();
            process.wait().unwrap();
            let status = receiver.recv_timeout(Duration::from_secs(3)).unwrap();
            worker.join().unwrap();
            assert_eq!(status.phase, "idle");
            assert!(!status.recording);
            assert!(!status.replay);
            assert!(status.error.is_none());
            assert!(manager.inner.lock().unwrap().session.is_none());
            assert_eq!(fs::read_to_string(&marker).unwrap(), "finalized");
            assert!(unrelated.try_wait().unwrap().is_none());
            unrelated.kill().unwrap();
            unrelated.wait().unwrap();
            fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn ipc_replies_and_errors() {
        use std::os::unix::net::UnixListener;
        let folder = std::env::temp_dir().join(format!("peli-ipc-test-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new().mode(0o700).create(&folder).unwrap();
        let socket = folder.join("ipc");
        let listener = UnixListener::bind(&socket).unwrap();
        let worker = thread::spawn(move || {
            for result in [
                json!({"id":1,"result":"ok","data":"/clips/video.mp4"}),
                json!({"id":1,"result":"error","data":"disk full"}),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["name"], "save-replay");
                writeln!(stream, "{result}").unwrap();
            }
        });
        assert_eq!(
            ipc(&socket, "save-replay", Value::Null).unwrap(),
            Some("/clips/video.mp4".into())
        );
        assert_eq!(
            ipc(&socket, "save-replay", Value::Null).unwrap_err(),
            "disk full"
        );
        worker.join().unwrap();
        fs::remove_file(socket).unwrap();
        fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn replay_actions_track_state_and_reject_invalid_commands() {
        use std::os::unix::net::UnixListener;
        let directory = std::env::temp_dir().join(format!("peli-actions-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let socket = directory.join("ipc");
        let listener = UnixListener::bind(&socket).unwrap();
        let worker = thread::spawn(move || {
            for expected in [
                "start-replay-recording",
                "save-replay",
                "stop-replay-recording",
                "stop",
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut line)
                    .unwrap();
                let request: Value = serde_json::from_str(&line).unwrap();
                assert_eq!(request["name"], expected);
                writeln!(stream,"{}",json!({"id":1,"result":"ok","data":if expected=="save-replay"||expected=="stop-replay-recording"{Some("/clips/test.mp4")}else{None}})).unwrap();
            }
        });
        let child = Arc::new(Mutex::new(
            Command::new("/bin/sleep").arg("30").spawn().unwrap(),
        ));
        // Only this owned fixture child can ever receive a signal in the test.
        struct Reap(Arc<Mutex<Child>>);
        impl Drop for Reap {
            fn drop(&mut self) {
                let mut c = self.0.lock().unwrap();
                let _ = c.kill();
                let _ = c.wait();
            }
        }
        let _reap = Reap(child.clone());
        let manager = CaptureManager::default();
        {
            let mut inner = manager.inner.lock().unwrap();
            inner.status = CaptureStatus {
                phase: "active".into(),
                replay: true,
                game_key: Some("fixture".into()),
                ..Default::default()
            };
            inner.session = Some(Session {
                child,
                socket: socket.clone(),
                directory: directory.clone(),
                id: "fixture".into(),
                _microphone: None,
            });
        }
        let notify: Notify = Arc::new(|_| {});
        assert!(manager.action("delete-everything", notify.clone()).is_err());
        let status = manager.action("start-recording", notify.clone()).unwrap();
        assert!(status.recording && status.replay);
        assert!(manager.action("start-recording", notify.clone()).is_err());
        assert_eq!(
            manager
                .action("save-replay", notify.clone())
                .unwrap()
                .last_file
                .as_deref(),
            Some("/clips/test.mp4")
        );
        let status = manager.action("stop-recording", notify.clone()).unwrap();
        assert!(!status.recording && status.replay);
        let status = manager.action("stop", notify).unwrap();
        assert_eq!(status.phase, "idle");
        assert!(!status.recording && !status.replay);
        worker.join().unwrap();
        fs::remove_file(socket).unwrap();
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn ipc_rejects_wrong_reply_identity_and_incomplete_frames() {
        use std::os::unix::net::UnixListener;
        let directory = std::env::temp_dir().join(format!("peli-frame-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let socket = directory.join("ipc");
        let listener = UnixListener::bind(&socket).unwrap();
        let worker = thread::spawn(move || {
            for reply in [
                "{\"id\":2,\"result\":\"ok\"}\n",
                "{\"id\":1,\"result\":\"ok\"}",
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = String::new();
                BufReader::new(stream.try_clone().unwrap())
                    .read_line(&mut request)
                    .unwrap();
                stream.write_all(reply.as_bytes()).unwrap();
            }
        });
        assert!(ipc(&socket, "status", Value::Null).is_err());
        assert!(ipc(&socket, "status", Value::Null).is_err());
        worker.join().unwrap();
        fs::remove_file(socket).unwrap();
        fs::remove_dir(directory).unwrap();
    }
    #[test]
    fn audio_sources_mix_and_direct_source_is_preserved() {
        let mut o = options();
        o.method = "direct".into();
        o.selection.microphone = true;
        let a = args(
            &o,
            false,
            Path::new("/videos"),
            Path::new("/private/ipc"),
            "id",
        );
        assert_eq!(&a[..2], ["-w", "DP-1"]);
        assert!(a
            .windows(2)
            .any(|w| w == ["-a", "default_output|default_input"]));
        assert!(!a.iter().any(|x| x == "-r" || x == "-ro"));
        o.selection.microphone = false;
        o.selection.desktop_audio = false;
        assert!(!args(
            &o,
            false,
            Path::new("/videos"),
            Path::new("/private/ipc"),
            "id"
        )
        .iter()
        .any(|x| x == "-a"));
    }
}

/// Global bindings are read on game startup, like the overlay's own shortcut.
pub fn shortcut_preferences() -> Result<shortcuts::Preferences,String> {
    let path=super::paths::app_root()?.join("capture-shortcuts.json");
    let bytes=match fs::read(path) {Ok(bytes) if bytes.len()<=4096=>bytes,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Default::default()),_=>return Err("Não foi possível ler os atalhos de captura.".into())};
    let value:shortcuts::Preferences=serde_json::from_slice(&bytes).map_err(|_|"Atalhos de captura inválidos.")?;value.validate()?;Ok(value)
}
pub fn shortcut_action<'a>(action:&'a str,status:&CaptureStatus,game_key:&str)->Result<&'a str,String>{
    if status.phase=="starting" {return Err("Aguarde a captura terminar de iniciar.".into());}
    if status.phase=="active" && status.game_key.as_deref()!=Some(game_key) {return Err("A captura ativa pertence a outro jogo.".into());}
    match action {
        "toggle-recording"=>Ok(if status.phase!="active" {"record"}else if !status.recording {"start-recording"}else if status.replay {"stop-recording"}else {"stop"}),
        "toggle-replay" if status.recording=>Err("Encerre a gravação antes de alternar o replay.".into()),
        "toggle-replay"=>Ok(if status.phase=="active" && status.replay {"stop"}else {"replay"}),
        "save-replay" if status.phase=="active" && status.replay=>Ok("save-replay"),
        _=>Err("Inicie o replay antes de salvar um clipe.".into()),
    }
}
#[cfg(test)] mod shortcut_action_tests {
    use super::*;
    #[test] fn capture_toggles_preserve_recordings_and_reject_other_games() {
        let mut s=CaptureStatus{phase:"idle".into(),..Default::default()};
        assert_eq!(shortcut_action("toggle-recording",&s,"g").unwrap(),"record");assert!(shortcut_action("save-replay",&s,"g").is_err());
        s.phase="active".into();s.game_key=Some("g".into());s.replay=true;
        assert_eq!(shortcut_action("toggle-recording",&s,"g").unwrap(),"start-recording");assert_eq!(shortcut_action("save-replay",&s,"g").unwrap(),"save-replay");
        s.recording=true;assert_eq!(shortcut_action("toggle-recording",&s,"g").unwrap(),"stop-recording");assert!(shortcut_action("toggle-replay",&s,"g").is_err());assert!(shortcut_action("toggle-recording",&s,"other").is_err());
    }
}

#[cfg(test)] mod saved_capture_settings_tests {
    use super::*;
    #[test]
    fn per_game_settings_survive_reload_without_reverting_replay_duration() {
        use std::os::unix::fs::PermissionsExt;
        let root=std::env::temp_dir().join(format!("peli-capture-settings-{}",uuid::Uuid::new_v4()));
        let mut options:CaptureOptions=serde_json::from_value(serde_json::json!({"game_key":"game/a","game_name":"Game","method":"portal","monitor":"","replay_seconds":137})).unwrap();
        write_settings(&root,&options).unwrap();
        let restored=read_settings(&root,"game/a").unwrap().unwrap();
        assert_eq!(restored.replay_seconds,137);assert_eq!(restored.selection,options.selection);
        assert!(read_settings(&root,"other").unwrap().is_none());
        assert_eq!(fs::metadata(settings_path(&root,"game/a")).unwrap().permissions().mode()&0o777,0o600);
        options.replay_seconds=60;write_settings(&root,&options).unwrap();
        assert_eq!(read_settings(&root,"game/a").unwrap().unwrap().replay_seconds,60);
        options.replay_seconds=901;assert!(write_settings(&root,&options).is_err());
        assert_eq!(read_settings(&root,"game/a").unwrap().unwrap().replay_seconds,60);
        fs::remove_dir_all(root).unwrap();
    }
}
