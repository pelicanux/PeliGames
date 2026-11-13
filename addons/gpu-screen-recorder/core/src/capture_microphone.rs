//! Session-owned microphone processing. No default-device changes or persisted routing.
use super::super::audio_denoise::RnNoise;
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone)]
struct Tool {
    program: PathBuf,
    prefix: Vec<String>,
}
impl Tool {
    fn new(name: &str) -> Self {
        Self {
            program: name.into(),
            prefix: Vec::new(),
        }
    }
    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        super::super::env_utils::host_environment(&mut command);
        command.args(&self.prefix);
        command
    }
}
fn output(tool: &Tool, args: &[String]) -> Result<String, String> {
    let mut child=tool.command().args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
        .map_err(|_|"A redução de ruído precisa de pactl, parec e pacat (pulseaudio-utils). Instale as ferramentas ou desligue a redução de ruído.".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Não foi possível consultar o serviço de áudio.")?;
    let reader = thread::spawn(move || {
        let mut data = Vec::new();
        let result = stdout.take(16385).read_to_end(&mut data);
        (result, data)
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("O serviço de áudio não respondeu à preparação do microfone.".into());
            }
        }
    };
    let (read, bytes) = reader.join().map_err(|_| "Falha ao consultar o áudio.")?;
    if !status.success() || read.is_err() || bytes.len() > 16384 {
        return Err("Não foi possível preparar a fonte temporária do microfone. Desligue a redução de ruído ou verifique o serviço de áudio.".into());
    }
    String::from_utf8(bytes)
        .map(|v| v.trim().to_owned())
        .map_err(|_| "Resposta de áudio inválida.".into())
}
pub struct MicrophoneFilter {
    control: Tool,
    module: Option<String>,
    recorder: Option<Child>,
    player: Option<Child>,
    worker: Option<thread::JoinHandle<()>>,
    stopping: Arc<AtomicBool>,
    failure: Arc<Mutex<Option<String>>>,
    source: String,
}
impl MicrophoneFilter {
    pub fn start(source: &str) -> Result<Self, String> {
        Self::start_with(
            source,
            Tool::new("pactl"),
            Tool::new("parec"),
            Tool::new("pacat"),
        )
    }
    fn start_with(source: &str, control: Tool, record: Tool, play: Tool) -> Result<Self, String> {
        let source = if source == "default_input" {
            output(&control, &["get-default-source".into()])?
        } else {
            source
                .strip_prefix("device:")
                .ok_or("Selecione um microfone para aplicar a redução de ruído.")?
                .to_owned()
        };
        if source.is_empty() || source.len() > 512 || source.chars().any(char::is_control) {
            return Err("Microfone inválido para a redução de ruído.".into());
        }
        let sink = format!("peligames_mic_{}", uuid::Uuid::new_v4().simple());
        let mut filter = Self {
            control,
            module: None,
            recorder: None,
            player: None,
            worker: None,
            stopping: Arc::new(AtomicBool::new(false)),
            failure: Arc::new(Mutex::new(None)),
            source: format!("device:{sink}.monitor"),
        };
        let module = output(
            &filter.control,
            &[
                "load-module".into(),
                "module-null-sink".into(),
                format!("sink_name={sink}"),
                "channels=1".into(),
                "rate=48000".into(),
                "sink_properties=device.description=PeliGames_RNNoise".into(),
            ],
        )?;
        module
            .parse::<u32>()
            .map_err(|_| "O serviço de áudio não identificou a fonte temporária.")?;
        filter.module = Some(module);
        filter.recorder=Some(record.command().args([format!("--device={source}"),"--format=s16le".into(),"--rate=48000".into(),"--latency-msec=30".into(),"--channels=1".into(),"--raw".into(),"--client-name=PeliGamesMicrophone".into()]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|_|"Não foi possível abrir parec. Instale pulseaudio-utils ou desligue a redução de ruído.".to_string())?);
        filter.player=Some(play.command().args(["--playback".into(),format!("--device={sink}"),"--format=s16le".into(),"--rate=48000".into(),"--latency-msec=30".into(),"--channels=1".into(),"--raw".into(),"--client-name=PeliGamesFilteredMicrophone".into()]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|"Não foi possível abrir pacat. Instale pulseaudio-utils ou desligue a redução de ruído.".to_string())?);
        let mut input = filter
            .recorder
            .as_mut()
            .unwrap()
            .stdout
            .take()
            .ok_or("Entrada do filtro de microfone indisponível.")?;
        let mut destination = filter
            .player
            .as_mut()
            .unwrap()
            .stdin
            .take()
            .ok_or("Saída do filtro de microfone indisponível.")?;
        let stopping = filter.stopping.clone();
        let failure = filter.failure.clone();
        filter.worker=Some(thread::Builder::new().name("peli-capture-rnnoise".into()).spawn(move||{
            let mut processor=RnNoise::new();let mut bytes=[0;960];
            let result=(||->std::io::Result<()>{while !stopping.load(Ordering::Acquire){input.read_exact(&mut bytes)?;let mut frame=[0i16;480];for (dst,pair) in frame.iter_mut().zip(bytes.chunks_exact(2)){*dst=i16::from_le_bytes([pair[0],pair[1]]);}processor.process(&mut frame);for (value,pair) in frame.iter().zip(bytes.chunks_exact_mut(2)){pair.copy_from_slice(&value.to_le_bytes());}destination.write_all(&bytes)?;}Ok(())})();
            if result.is_err()&&!stopping.load(Ordering::Acquire){if let Ok(mut error)=failure.lock(){*error=Some("O filtro do microfone foi interrompido. A captura foi encerrada; confira a entrada de áudio e tente novamente.".into());}}
        }).map_err(|e|format!("Não foi possível iniciar o RNNoise: {e}"))?);
        Ok(filter)
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn failure(&self) -> Arc<Mutex<Option<String>>> {
        self.failure.clone()
    }
}
impl Drop for MicrophoneFilter {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        for child in [&mut self.recorder, &mut self.player].into_iter().flatten() {
            let _ = child.kill();
        }
        for child in [&mut self.recorder, &mut self.player].into_iter().flatten() {
            let _ = child.wait();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(module) = self.module.take() {
            let _ = output(&self.control, &["unload-module".into(), module]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    struct Fixture {
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("peli-mic-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            Self { root }
        }
        fn script(&self, name: &str, body: &str) -> Tool {
            let path = self.root.join(name);
            fs::write(&path, body).unwrap();
            Tool {
                program: "/bin/sh".into(),
                prefix: vec![path.to_string_lossy().into_owned()],
            }
        }
        fn control(&self) -> Tool {
            self.script("control", &format!(
                "printf '%s\\n' \"$*\" >> '{}'\ncase \"$1\" in\nget-default-source) echo fixture_mic;;\nload-module) echo 4321;;\nesac\n",
                self.root.join("calls").display()
            ))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn processes_pcm_and_releases_only_owned_children_and_module() {
        let fixture = Fixture::new();
        let record = fixture.script(
            "record",
            "dd if=/dev/zero bs=960 count=5 2>/dev/null\nexec sleep 60\n",
        );
        let play = fixture.script(
            "play",
            &format!("exec cat > '{}'\n", fixture.root.join("pcm").display()),
        );
        let filter =
            MicrophoneFilter::start_with("default_input", fixture.control(), record, play).unwrap();
        assert!(filter.source().starts_with("device:peligames_mic_"));
        assert!(filter.source().ends_with(".monitor"));
        let recorder = filter.recorder.as_ref().unwrap().id();
        let player = filter.player.as_ref().unwrap().id();
        let deadline = Instant::now() + Duration::from_secs(3);
        while fs::metadata(fixture.root.join("pcm")).map_or(0, |m| m.len()) < 4800 {
            assert!(Instant::now() < deadline, "Filtered PCM did not arrive");
            thread::sleep(Duration::from_millis(10));
        }
        assert!(filter.failure().lock().unwrap().is_none());
        let failure = filter.failure();
        drop(filter);
        assert!(
            failure.lock().unwrap().is_none(),
            "Intentional stop must not report a filter failure"
        );
        assert_eq!(fs::read(fixture.root.join("pcm")).unwrap(), vec![0; 4800]);
        assert!(!std::path::Path::new(&format!("/proc/{recorder}")).exists());
        assert!(!std::path::Path::new(&format!("/proc/{player}")).exists());
        let calls = fs::read_to_string(fixture.root.join("calls")).unwrap();
        assert!(calls.contains("get-default-source"));
        assert_eq!(
            calls
                .lines()
                .filter(|line| line.starts_with("unload-module"))
                .collect::<Vec<_>>(),
            vec!["unload-module 4321"]
        );
    }
    #[test]
    fn partial_start_failure_unloads_temporary_source() {
        let fixture = Fixture::new();
        let missing = Tool::new("/nonexistent/peligames-parec-test");
        let result = MicrophoneFilter::start_with(
            "device:fixture_mic",
            fixture.control(),
            missing,
            Tool::new("/bin/false"),
        );
        assert!(result.is_err());
        assert!(fs::read_to_string(fixture.root.join("calls"))
            .unwrap()
            .contains("unload-module 4321"));
    }
    #[test]
    fn unexpected_microphone_exit_is_reported() {
        let fixture = Fixture::new();
        let record = fixture.script("record", "exit 1\n");
        let filter = MicrophoneFilter::start_with(
            "device:fixture_mic",
            fixture.control(),
            record,
            Tool::new("/bin/cat"),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while filter.failure().lock().unwrap().is_none() {
            assert!(Instant::now() < deadline, "Missing microphone failure");
            thread::sleep(Duration::from_millis(10));
        }
    }
}
