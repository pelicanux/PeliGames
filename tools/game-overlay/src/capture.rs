//! All launcher IPC runs outside the game's rendering thread.
use serde::Deserialize;
#[path = "../capture_selection.rs"]
mod selection;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::MetadataExt, net::UnixStream},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Status {
    pub phase: String,
    pub game_name: Option<String>,
    pub recording: bool,
    pub replay: bool,
    pub last_file: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Reply {
    pub status: Status,
    pub prepared: Option<String>,
    pub selection: Option<selection::Selection>,
    pub audio: Option<selection::AudioSources>,
    pub encoder: Option<selection::EncoderSupport>,
    pub error: Option<String>,
}
pub(crate) struct Controller {
    state: Arc<Mutex<Reply>>,
    sender: mpsc::SyncSender<String>,
    pending: Arc<std::sync::atomic::AtomicUsize>,
    actions: Arc<std::sync::atomic::AtomicUsize>,
    last_request: Mutex<Instant>,
    sources_requested: std::sync::atomic::AtomicBool,
    cancel_pending: Arc<std::sync::atomic::AtomicBool>,
}
fn root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("PELI_OVERLAY_CONFIG_ROOT") {
        return Some(root.into());
    }
    Some(
        PathBuf::from(std::env::var_os("MAKO_CONFIG")?)
            .parent()?
            .parent()?
            .parent()?
            .to_owned(),
    )
}
pub(crate) fn load_shortcuts()->crate::capture_shortcuts::Preferences {
    let Some(path)=root().map(|r|r.join("capture-shortcuts.json")) else {return Default::default();};
    let Ok(meta)=fs::symlink_metadata(&path) else{return Default::default();};
    if !meta.is_file()||meta.uid()!=unsafe{libc::geteuid()}||meta.mode()&0o077!=0||meta.len()>4096{return Default::default();}
    fs::read(path).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn request(command: &str) -> Result<Reply, String> {
    let descriptor = root()
        .ok_or("Pasta do launcher indisponível.")?
        .join("capture-overlay.json");
    let meta = fs::symlink_metadata(&descriptor)
        .map_err(|_| "Abra a aba Capturas do jogo e mantenha o PeliGames aberto.")?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o077 != 0
        || meta.len() > 4096
    {
        return Err("Ponte de captura sem permissões privadas.".into());
    }
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(descriptor).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let socket = PathBuf::from(
        value["socket"]
            .as_str()
            .ok_or("Ponte de captura inválida.")?,
    );
    let parent = socket.parent().ok_or("Ponte de captura inválida.")?;
    let meta = fs::symlink_metadata(parent).map_err(|_| "A ponte de captura não está acessível ao jogo. Abra Capturas no launcher e reabra o jogo após atualizar o PeliGames.")?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err("Ponte de captura insegura.".into());
    }
    let mut stream = UnixStream::connect(socket)
        .map_err(|_| "PeliGames desconectado. Abra a aba Capturas do jogo no launcher.")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(if command == "status" {
            2
        } else {
            130
        })))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;
    writeln!(stream, "{}", serde_json::json!({"command":command})).map_err(|e| e.to_string())?;
    let mut line = String::new();
    BufReader::new(stream.take(65537))
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if line.len() > 65536 {
        return Err("Resposta de captura grande demais.".into());
    }
    serde_json::from_str(&line).map_err(|e| e.to_string())
}
impl Controller {
    pub fn new() -> Self { Self::with_request(request) }
    #[cfg(test)]
    pub(crate) fn fixture(reply:Reply)->Self {
        let saved=reply.clone();
        let controller=Self::with_request(move |_|Ok(saved.clone()));
        *controller.state.lock().unwrap()=reply;
        controller
    }

    fn with_request(request: impl Fn(&str) -> Result<Reply,String> + Send + 'static) -> Self {
        let state = Arc::new(Mutex::new(Reply::default()));
        let result = state.clone();
        let pending = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let actions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let working = pending.clone();
        let action_count = actions.clone();
        let (sender, receiver) = mpsc::sync_channel::<String>(1);
        let _ = std::thread::Builder::new()
            .name("peli-overlay-capture".into())
            .spawn(move || {
                while let Ok(command) = receiver.recv() {
                    if matches!(command.as_str(),"record"|"replay"|"toggle-recording"|"toggle-replay") {
                        if let Ok(mut state)=result.lock(){if state.status.phase!="active"{state.status.phase="starting".into();state.status.replay=matches!(command.as_str(),"replay"|"toggle-replay");state.status.game_name=state.prepared.clone();state.error=None;}}
                    }
                    let reply = match request(&command) {
                        Ok(reply) => reply,
                        Err(error) => Reply {
                            error: Some(error),
                            ..Default::default()
                        },
                    };
                    if command != "status" && command != "sources" {if let Some(error)=&reply.error {crate::report(&format!("Captura: {error}"));}}
                    if let Ok(mut current) = result.lock() {
                        let previous = current.error.clone();
                        let previous_phase=current.status.phase.clone();
                        let previous_game=current.prepared.clone();
                        let audio = current.audio.clone();
                        let encoder = current.encoder.clone();
                        *current = reply;
                        if current.encoder.is_none() {
                            current.encoder = encoder;
                        }
                        if current.audio.is_none() {
                            current.audio = audio;
                        }
                        if command == "status" && current.error.is_none() && current.status.phase==previous_phase && current.prepared==previous_game {
                            current.error = previous;
                        }
                    }
                    if command != "status" && command != "sources" {
                        action_count.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                    }
                    working.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                }
            });
        Self {
            state,
            sender,
            pending,
            actions,
            last_request: Mutex::new(Instant::now() - Duration::from_secs(5)),
            sources_requested: std::sync::atomic::AtomicBool::new(false),
            cancel_pending: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    pub fn send_action(&self, action:&str){
        use std::sync::atomic::Ordering;
        if self.cancel_pending.load(Ordering::Acquire)||self.actions.compare_exchange(0,1,Ordering::AcqRel,Ordering::Acquire).is_err(){return;}
        self.pending.fetch_add(1,Ordering::AcqRel);
        if self.sender.try_send(action.into()).is_err(){self.actions.fetch_sub(1,Ordering::AcqRel);self.pending.fetch_sub(1,Ordering::AcqRel);}
    }
    pub fn draw(&self, ui: &mut egui::Ui) {
        let mut current = self.state.lock().map(|v| v.clone()).unwrap_or_default();
        let busy = self.actions.load(std::sync::atomic::Ordering::Acquire) > 0 || self.cancel_pending.load(std::sync::atomic::Ordering::Acquire);
        let mut command = None;
        draw(ui, &mut current, busy, &mut command);
        // Refresh only while the panel is being drawn, without touching log files.
        if command.is_none()
            && self.pending.load(std::sync::atomic::Ordering::Acquire) == 0
            && self
                .last_request
                .lock()
                .is_ok_and(|t| t.elapsed() >= Duration::from_secs(1))
        {
            command = Some(
                if self
                    .sources_requested
                    .swap(true, std::sync::atomic::Ordering::AcqRel)
                {
                    "status"
                } else {
                    "sources"
                }
                .into(),
            );
        }
        if let Some(command) = command {
            if command=="stop" && current.status.phase=="starting" {
                if !self.cancel_pending.swap(true,std::sync::atomic::Ordering::AcqRel){
                    let pending=self.cancel_pending.clone();let state=self.state.clone();
                    let result=std::thread::Builder::new().name("peli-capture-cancel".into()).spawn(move||{
                        let reply=request("stop");
                        if let Ok(mut state)=state.lock(){match reply{Ok(reply)=>{state.status=reply.status;state.error=reply.error;},Err(error)=>state.error=Some(error)}}
                        pending.store(false,std::sync::atomic::Ordering::Release);
                    });
                    if result.is_err(){self.cancel_pending.store(false,std::sync::atomic::Ordering::Release);}
                }
                return;
            }
            let action = command != "status" && command != "sources";
            if action || self.pending.load(std::sync::atomic::Ordering::Acquire) == 0 {
                self.pending
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                if action {
                    self.actions
                        .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                }
                if self.sender.try_send(command).is_err() {
                    self.pending
                        .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                    if action {
                        self.actions
                            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                    }
                }
                if let Ok(mut time) = self.last_request.lock() {
                    *time = Instant::now();
                }
            }
        }
    }
}
pub(crate) fn draw(
    ui: &mut egui::Ui,
    current: &mut Reply,
    busy: bool,
    command: &mut Option<String>,
) {
    ui.strong("Capturas");
    let status = &current.status;
    let other_game=status.phase=="other-game";
    let busy=busy||other_game;
    let active = status.phase == "active";
    let starting = status.phase == "starting";
    let name = if active || starting {
        status.game_name.as_deref()
    } else {
        current.prepared.as_deref()
    };
    if let Some(name) = name {
        ui.small(name);
    }
    ui.colored_label(
        if active {
            egui::Color32::from_rgb(90, 225, 170)
        } else {
            egui::Color32::LIGHT_GRAY
        },
        if other_game {
            "Captura ativa em outro jogo"
        } else if busy {
            "Aguarde…"
        } else if starting {
            "Aguardando autorização…"
        } else if status.recording && status.replay {
            "Gravando · replay ativo"
        } else if status.recording {
            "Gravando"
        } else if status.replay {
            "Replay ativo"
        } else {
            "Captura desligada"
        },
    );
    if let Some(mut chosen) = current.selection.clone() {
        let previous = chosen.clone();
        let fields_width = ui.available_width();
        let column_width = ((fields_width-ui.spacing().item_spacing.x)/2.).max(72.);
        let encoder = current.encoder.as_ref();
        ui.add_enabled_ui(!busy && !active && !starting, |ui| {
            ui.separator();
            ui.small("Vídeo e áudio · próxima gravação");
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.small("Formato");
                    egui::ComboBox::from_id_salt("capture-format")
                        .selected_text(chosen.format.to_uppercase())
                        .width(column_width)
                        .show_ui(ui, |ui| {
                            for format in ["mp4", "mkv", "webm"] {
                                ui.selectable_value(
                                    &mut chosen.format,
                                    format.into(),
                                    format.to_uppercase(),
                                );
                            }
                        });
                });
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.small("Codec");
                    if chosen.format == "webm" && matches!(chosen.codec.as_str(), "h264" | "hevc") {
                        chosen.codec = "auto".into();
                    }
                    egui::ComboBox::from_id_salt("capture-codec")
                        .width(column_width)
                        .selected_text(if chosen.codec == "auto" {
                            "Automático".into()
                        } else {
                            chosen.codec.to_uppercase()
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut chosen.codec, "auto".into(), "Automático");
                            for (codec, label) in [
                                ("h264", "H.264"),
                                ("hevc", "HEVC (H.265)"),
                                ("av1", "AV1"),
                                ("vp9", "VP9"),
                            ] {
                                let supported =
                                    encoder.is_some_and(|e| e.codecs.iter().any(|v| v == codec));
                                let compatible =
                                    chosen.format != "webm" || matches!(codec, "av1" | "vp9");
                                ui.add_enabled_ui(supported && compatible, |ui| {
                                    ui.selectable_value(
                                        &mut chosen.codec,
                                        codec.into(),
                                        if !supported {
                                            format!("{label} · indisponível")
                                        } else if !compatible {
                                            format!("{label} · MP4/MKV")
                                        } else {
                                            label.into()
                                        },
                                    );
                                });
                            }
                        });
                });
            });
            if let Some(encoder) = encoder {
                ui.small(format!(
                    "GPU: {}{} · {}",
                    encoder.gpu,
                    if encoder.encoder == "NVENC" {
                        " · NVENC"
                    } else {
                        ""
                    },
                    encoder
                        .codecs
                        .iter()
                        .map(|v| v.to_uppercase())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            } else {
                ui.small("Abra uma fonte de áudio para detectar os codecs.");
            }
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.small("Resolução");
                    egui::ComboBox::from_id_salt("capture-resolution")
                        .width(column_width)
                        .selected_text(match chosen.resolution.as_str() {
                            "854x480" => "480p",
                            "1280x720" => "720p",
                            "1920x1080" => "1080p",
                            "2560x1440" => "1440p",
                            "3840x2160" => "2160p (4K)",
                            _ => "Original",
                        })
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                ("0x0", "Original"),
                                ("854x480", "480p"),
                                ("1280x720", "720p"),
                                ("1920x1080", "1080p"),
                                ("2560x1440", "1440p"),
                                ("3840x2160", "2160p (4K)"),
                            ] {
                                ui.selectable_value(&mut chosen.resolution, value.into(), label);
                            }
                        });
                });
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.small("FPS da gravação");
                    egui::ComboBox::from_id_salt("capture-fps")
                        .width(column_width)
                        .selected_text(format!("{} FPS", chosen.fps))
                        .show_ui(ui, |ui| {
                            for fps in [24, 30, 60, 90, 120, 144, 165, 240] {
                                ui.selectable_value(&mut chosen.fps, fps, format!("{fps} FPS"));
                            }
                        });
                });
            });
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.small("Qualidade");
                    egui::ComboBox::from_id_salt("capture-rate-control")
                        .width(column_width)
                        .selected_text(chosen.rate_control.to_uppercase())
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                ("cbr", "Bitrate constante (CBR)"),
                                ("qp", "Qualidade constante (QP)"),
                                ("vbr", "Bitrate variável (VBR)"),
                            ] {
                                ui.selectable_value(&mut chosen.rate_control, value.into(), label);
                            }
                        });
                });
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    if chosen.rate_control == "cbr" {
                        ui.small("Bitrate");
                        let mut mbps = chosen.bitrate_kbps as f64 / 1000.;
                        if ui
                            .add(
                                egui::DragValue::new(&mut mbps)
                                    .range(1.0..=200.0)
                                    .speed(1.)
                                    .suffix(" Mb/s"),
                            )
                            .changed()
                        {
                            chosen.bitrate_kbps = (mbps * 1000.).round() as u32;
                        }
                    } else {
                        ui.small("Preset");
                        egui::ComboBox::from_id_salt("capture-quality-preset")
                            .width(column_width)
                            .selected_text(match chosen.quality_preset.as_str() {
                                "medium" => "Média",
                                "high" => "Alta",
                                "ultra" => "Ultra",
                                _ => "Muito alta",
                            })
                            .show_ui(ui, |ui| {
                                for (value, label) in [
                                    ("medium", "Média"),
                                    ("high", "Alta"),
                                    ("very_high", "Muito alta"),
                                    ("ultra", "Ultra"),
                                ] {
                                    ui.selectable_value(
                                        &mut chosen.quality_preset,
                                        value.into(),
                                        label,
                                    );
                                }
                            });
                    }
                });
            });
            ui.checkbox(&mut chosen.desktop_audio, "Capturar áudio");
            if chosen.desktop_audio {
                let outputs = current
                    .audio
                    .as_ref()
                    .map(|v| {
                        v.outputs
                            .iter()
                            .chain(v.applications.iter())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let label = outputs
                    .iter()
                    .find(|s| s.id == chosen.output_source)
                    .map(|s| s.label.as_str())
                    .unwrap_or(if chosen.output_source == "game:auto" {"Áudio deste jogo · automático"} else {&chosen.output_source});
                let menu = egui::ComboBox::from_id_salt("capture-output")
                    .selected_text(label)
                    .width(fields_width)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut chosen.output_source, "game:auto".into(), "Áudio deste jogo · automático");
                        for source in outputs {
                            ui.selectable_value(
                                &mut chosen.output_source,
                                source.id.clone(),
                                &source.label,
                            );
                        }
                    });
                if menu.response.clicked() && menu.inner.is_some() {
                    *command = Some("sources".into());
                }
            }
            if chosen.desktop_audio && chosen.output_source == "game:auto" {ui.small("Detecta ao iniciar. Abra o jogo e aguarde ele emitir som.");}
            ui.checkbox(&mut chosen.microphone, "Microfone");
            if chosen.microphone {
                ui.checkbox(&mut chosen.microphone_noise_reduction,"Reduzir ruído · RNNoise");
                let inputs = current
                    .audio
                    .as_ref()
                    .map(|v| v.inputs.as_slice())
                    .unwrap_or(&[]);
                let label = inputs
                    .iter()
                    .find(|s| s.id == chosen.input_source)
                    .map(|s| s.label.as_str())
                    .unwrap_or(&chosen.input_source);
                let menu = egui::ComboBox::from_id_salt("capture-input")
                    .selected_text(label)
                    .width(fields_width)
                    .show_ui(ui, |ui| {
                        for source in inputs {
                            ui.selectable_value(
                                &mut chosen.input_source,
                                source.id.clone(),
                                &source.label,
                            );
                        }
                    });
                if menu.response.clicked() && menu.inner.is_some() {
                    *command = Some("sources".into());
                }
            }
        });
        if chosen.format == "webm" {
            ui.small("WebM: AV1 ou VP9, conforme suporte da GPU.");
        }
        if chosen != previous {
            *command = Some(format!(
                "configure:{}",
                serde_json::to_string(&chosen).unwrap_or_default()
            ));
        }
        if active || starting {
            ui.small("Encerre a captura para alterar áudio ou formato.");
        }
    }
    ui.add_enabled_ui(!busy && !starting, |ui| {
        if ui
            .add_enabled(
                active || current.prepared.is_some(),
                egui::Button::new(if status.recording {
                    "Parar e salvar gravação"
                } else {
                    "Gravar sessão"
                }),
            )
            .clicked()
        {
            *command = Some(
                (if !active {
                    "record"
                } else if status.recording {
                    if status.replay {
                        "stop-recording"
                    } else {
                        "stop"
                    }
                } else {
                    "start-recording"
                })
                .into(),
            );
        }
        ui.horizontal(|ui| {
            if !active {
                if ui
                    .add_enabled(
                        current.prepared.is_some(),
                        egui::Button::new("Iniciar replay"),
                    )
                    .clicked()
                {
                    *command = Some("replay".into());
                }
            } else if ui
                .add_enabled(status.replay, egui::Button::new("Salvar replay"))
                .clicked()
            {
                *command = Some("save-replay".into());
            }
            if active && ui.button("Encerrar captura").clicked() {
                *command = Some("stop".into());
            }
        });
    });
    if starting && ui.button("Cancelar captura").clicked() {
        *command = Some("stop".into());
    }
    if let Some(file) = &status.last_file {
        let name = std::path::Path::new(file)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        ui.small(format!("Salvo: {name}"));
    }
    if let Some(error) = current.error.as_ref().or(status.error.as_ref()) {
        ui.colored_label(egui::Color32::from_rgb(255, 150, 140), error);
    }
    if !active {
        ui.small("Fonte, áudio e qualidade: ajustes na aba Capturas.");
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buttons_route_recording_and_replay_without_stopping_the_buffer() {
        fn click(status: Status, text: &str, busy: bool) -> Option<String> {
            let context = egui::Context::default();
            let mut current = Reply {
                status,
                prepared: Some("Test game".into()),
                selection: Some(selection::Selection { output_source: "default_output".into(), ..Default::default() }),
                ..Default::default()
            };
            let mut command = None;
            let frame = |events| egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600., 600.),
                )),
                events,
                ..Default::default()
            };
            let paint = |ctx: &egui::Context, current: &mut Reply, command: &mut Option<String>| {
                egui::CentralPanel::default().show(ctx, |ui| draw(ui, current, busy, command));
            };
            let _ = context.run(frame(vec![]), |ctx| paint(ctx, &mut current, &mut command));
            let output = context.run(frame(vec![]), |ctx| paint(ctx, &mut current, &mut command));
            fn position(s: &egui::epaint::Shape, text: &str) -> Option<egui::Pos2> {
                match s {
                    egui::epaint::Shape::Text(t) if t.galley.text() == text => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    egui::epaint::Shape::Vec(shapes) => {
                        shapes.iter().find_map(|s| position(s, text))
                    }
                    _ => None,
                }
            }
            let point = output
                .shapes
                .iter()
                .find_map(|s| position(&s.shape, text))
                .unwrap();
            for pressed in [true, false] {
                let _ = context.run(
                    frame(vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ]),
                    |ctx| paint(ctx, &mut current, &mut command),
                );
            }
            command
        }
        assert_eq!(
            click(Status::default(), "default_output", false),
            Some("sources".into())
        );
        assert_eq!(click(Status::default(), "default_output", true), None);
        for text in ["default_output","Microfone","Gravar sessão","Iniciar replay"] {
            assert_eq!(click(Status{phase:"other-game".into(),..Default::default()},text,false),None);
        }
        assert_eq!(click(Status{phase:"starting".into(),..Default::default()},"Cancelar captura",true),Some("stop".into()));
        let changed = click(Status::default(), "Microfone", false).unwrap();
        let selection: selection::Selection =
            serde_json::from_str(changed.strip_prefix("configure:").unwrap()).unwrap();
        assert!(selection.microphone);
        assert_eq!(
            click(Status::default(), "Gravar sessão", false),
            Some("record".into())
        );
        assert_eq!(
            click(Status::default(), "Iniciar replay", false),
            Some("replay".into())
        );
        let recording = || Status {
            phase: "active".into(),
            recording: true,
            ..Default::default()
        };
        assert_eq!(
            click(recording(), "Parar e salvar gravação", false),
            Some("stop".into())
        );
        let both = || Status {
            replay: true,
            ..recording()
        };
        assert_eq!(
            click(both(), "Parar e salvar gravação", false),
            Some("stop-recording".into())
        );
        assert_eq!(
            click(both(), "Salvar replay", false),
            Some("save-replay".into())
        );
        assert_eq!(
            click(both(), "Encerrar captura", false),
            Some("stop".into())
        );
        assert_eq!(click(both(), "Salvar replay", true), None);
    }
    #[test]
    fn capture_panel_shows_saved_result_and_combined_session() {
        let context = egui::Context::default();
        let mut current = Reply {
            status: Status {
                phase: "active".into(),
                recording: true,
                replay: true,
                last_file: Some("/videos/replay.mp4".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw(ui, &mut current, false, &mut None));
        });
        fn texts(shape: &egui::epaint::Shape, output: &mut Vec<String>) {
            match shape {
                egui::epaint::Shape::Text(text) => output.push(text.galley.text().into()),
                egui::epaint::Shape::Vec(shapes) => {
                    for s in shapes {
                        texts(s, output)
                    }
                }
                _ => (),
            }
        }
        let mut labels = Vec::new();
        for shape in result.shapes {
            texts(&shape.shape, &mut labels);
        }
        for text in [
            "Capturas",
            "Gravando · replay ativo",
            "Parar e salvar gravação",
            "Salvar replay",
            "Encerrar captura",
            "Salvo: replay.mp4",
        ] {
            assert!(labels.iter().any(|v| v == text), "{text}");
        }
    }
}

#[cfg(test)] mod shortcut_dispatch_tests {
    use super::*;
    #[test]
    fn f8_and_capture_bindings_share_input_but_dispatch_distinct_actions() {
        let (sender,receiver)=std::sync::mpsc::channel();
        let controller=Controller::with_request(move |command| {
            sender.send(command.to_owned()).unwrap();
            Ok(Reply::default())
        });
        let mut input=crate::input::Input::default();
        input.shortcut("F8").unwrap();
        input.capture_shortcuts(&crate::capture_shortcuts::Preferences::default()).unwrap();
        input.ready=true;input.focus(true);
        assert_eq!(input.key(66,true),crate::input::Route::Overlay);
        assert!(input.visible);assert!(input.take_capture_shortcut().is_none());
        input.key(66,false);input.release();
        input.key(56,true);
        for (key,expected) in [(64,"toggle-recording"),(65,"toggle-replay"),(66,"save-replay")] {
            assert_eq!(input.key(key,true),crate::input::Route::Overlay);
            controller.send_action(input.take_capture_shortcut().unwrap());
            assert_eq!(receiver.recv_timeout(Duration::from_secs(2)).unwrap(),expected);
            let deadline=Instant::now()+Duration::from_secs(2);
            while controller.actions.load(std::sync::atomic::Ordering::Acquire)>0 {
                assert!(Instant::now()<deadline);std::thread::yield_now();
            }
            input.key(key,false);assert!(!input.visible);
        }
    }
}
