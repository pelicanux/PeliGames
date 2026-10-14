//! Native LiveKit audio for MatrixRTC rooms and the isolated loopback probe.
use super::matrix_voice::{CallInput, MicSettings, NoiseMode, SPEAKING_HOLD, SPEAKING_LEVEL};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use livekit::e2ee::{
    key_provider::{KeyDerivationAlgorithm, KeyProvider, KeyProviderOptions},
    E2eeOptions, EncryptionType,
};
use livekit::options::TrackPublishOptions;
use livekit::{
    prelude::*,
    webrtc::{
        audio_source::{native::NativeAudioSource, AudioSourceOptions, RtcAudioSource},
        audio_stream::native::NativeAudioStream,
    },
};
use serde::Deserialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, WebviewWindow};
use tokio_stream::StreamExt;

#[derive(Deserialize)]
struct Credentials {
    url: String,
    a: String,
    b: String,
}
fn credentials() -> Result<(String, String), String> {
    let path = std::env::var("PELIGAMES_RTC_TEST_CONFIG")
        .map_err(|_| "Abra o teste pelo script de duas sessões.")?;
    let text = std::fs::read(path).map_err(|_| "Configuração local indisponível.")?;
    if text.len() > 16384 {
        return Err("Configuração inválida.".into());
    }
    let config: Credentials =
        serde_json::from_slice(&text).map_err(|_| "Configuração inválida.")?;
    if config.url != "ws://127.0.0.1:17880" {
        return Err("Este teste aceita somente o servidor local.".into());
    }
    let token = match std::env::var("PELIGAMES_RTC_TEST_ID").as_deref() {
        Ok("a") => config.a,
        Ok("b") => config.b,
        _ => return Err("Identidade de teste ausente.".into()),
    };
    Ok((config.url, token))
}
#[derive(Clone, Deserialize)]
pub struct MediaKey {
    identity: String,
    index: i32,
    key: Vec<u8>,
}
struct SecureMedia {
    provider: KeyProvider,
    keys: std::sync::mpsc::Receiver<MediaKey>,
    local_key: MediaKey,
}
/// One remote speaker: its samples and whether playback has started.
#[derive(Default)]
struct Speaker {
    samples: VecDeque<i16>,
    playing: bool,
}
type Mixer = HashMap<String, Speaker>;
static PREPARATION: AtomicU64 = AtomicU64::new(0);
static JOIN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct Call {
    owner: String,
    id: Option<String>,
    keys: Option<std::sync::mpsc::SyncSender<MediaKey>>,
    stop: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    mic: Arc<MicSettings>,
    thread: std::thread::JoinHandle<()>,
}
static CALL: OnceLock<Mutex<Option<Call>>> = OnceLock::new();
fn state() -> &'static Mutex<Option<Call>> {
    CALL.get_or_init(|| Mutex::new(None))
}
pub(super) fn active() -> bool {
    state()
        .lock()
        .map(|c| c.as_ref().is_some_and(|c| !c.thread.is_finished()))
        .unwrap_or(true)
}
fn guard(w: &WebviewWindow) -> Result<(), String> {
    if matches!(w.label(), "matrix-rtc-probe" | "main") {
        Ok(())
    } else {
        Err("Janela de teste inválida.".into())
    }
}
fn stop_owned(owner: Option<&str>) {
    if owner.is_none() || owner == Some("main") {
        PREPARATION.fetch_add(1, Ordering::AcqRel);
    }
    if let Ok(mut current) = state().lock() {
        if current
            .as_ref()
            .is_some_and(|call| owner.is_none() || owner == Some(call.owner.as_str()))
        {
            if let Some(call) = current.take() {
                call.stop.store(true, Ordering::Release);
                let _ = call.thread.join();
            }
        }
    }
}
pub fn stop() {
    stop_owned(None);
}
pub(super) fn stop_probe() {
    stop_owned(Some("matrix-rtc-probe"));
}

// Independent jitter buffers prevent one participant from delaying another.
// Packets arrive in bursts: playback starts (and resumes after running dry)
// only with 40 ms buffered, so gaps become one short pause instead of a click
// every few milliseconds. Above 200 ms the oldest audio is dropped back to 60 ms.
const PREBUFFER: usize = 1920;
const MAX_BACKLOG: usize = 9600;
const TRIMMED_BACKLOG: usize = 2880;
fn enqueue(speaker: &mut Speaker, samples: &[i16]) {
    speaker.samples.extend(samples.iter().copied());
    if speaker.samples.len() > MAX_BACKLOG {
        let excess = speaker.samples.len() - TRIMMED_BACKLOG;
        speaker.samples.drain(..excess);
    }
    if speaker.samples.len() >= PREBUFFER {
        speaker.playing = true;
    }
}
fn frame_level(samples: &[i16]) -> f32 {
    let sum: f32 = samples.iter().map(|&x| (x as f32 / 32768.).powi(2)).sum();
    (sum / samples.len().max(1) as f32).sqrt()
}
fn mix_sample(queues: &mut Mixer) -> f32 {
    let mut sum = 0i64;
    for speaker in queues.values_mut().filter(|s| s.playing) {
        match speaker.samples.pop_front() {
            Some(sample) => sum += i64::from(sample),
            None => speaker.playing = false,
        }
    }
    (sum as f32 / 32768.).clamp(-1., 1.)
}
fn key_provider() -> KeyProvider {
    KeyProvider::new(KeyProviderOptions {
        key_derivation_algorithm: KeyDerivationAlgorithm::HKDF,
        ratchet_window_size: 10,
        key_ring_size: 256,
        ..Default::default()
    })
}
fn valid_key(key: &MediaKey) -> bool {
    !key.identity.is_empty()
        && key.identity.len() <= 512
        && (0..256).contains(&key.index)
        && matches!(key.key.len(), 16 | 32)
}
fn output<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queue: Arc<Mutex<Mixer>>,
    volume: Arc<AtomicU32>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _| {
                // Blocking briefly is fine: writers only append one 10 ms frame.
                let mut locked = queue.lock().ok();
                let gain = volume.load(Ordering::Relaxed) as f32 / 100.;
                for frame in data.chunks_mut(channels) {
                    let value = locked
                        .as_mut()
                        .map(|queues| mix_sample(queues))
                        .unwrap_or(0.)
                        * gain;
                    // Above 100% the boosted mix saturates instead of wrapping.
                    let value = value.clamp(-1., 1.);
                    for sample in frame {
                        *sample = T::from_sample_(value);
                    }
                }
            },
            move |_| {
                failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|_| "Não foi possível abrir a saída de áudio.".into())
}
fn playback(
    queue: Arc<Mutex<Mixer>>,
    volume: Arc<AtomicU32>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("Nenhuma saída de áudio padrão.")?;
    let cfg = device
        .supported_output_configs()
        .map_err(|_| "Saída de áudio indisponível.")?
        .find(|c| {
            c.channels() > 0
                && c.min_sample_rate().0 <= 48000
                && c.max_sample_rate().0 >= 48000
                && matches!(
                    c.sample_format(),
                    cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
                )
        })
        .ok_or("A saída de áudio precisa oferecer 48 kHz neste teste.")?
        .with_sample_rate(cpal::SampleRate(48000));
    let config = cfg.config();
    let stream = match cfg.sample_format() {
        cpal::SampleFormat::F32 => output::<f32>(&device, &config, queue, volume, failed),
        cpal::SampleFormat::I16 => output::<i16>(&device, &config, queue, volume, failed),
        cpal::SampleFormat::U16 => output::<u16>(&device, &config, queue, volume, failed),
        _ => unreachable!(),
    }?;
    stream
        .play()
        .map_err(|_| "Falha ao iniciar saída de áudio.")?;
    Ok(stream)
}
async fn cancelled(stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
fn run(
    w: &WebviewWindow,
    mic_settings: Arc<MicSettings>,
    stop: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    volume: Arc<AtomicU32>,
    url: String,
    token: String,
    mut secure: Option<SecureMedia>,
    call_id: Option<String>,
) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|_| "Falha ao preparar chamada.")?;
    let _guard = runtime.enter();
    let queue = Arc::new(Mutex::new(Mixer::new()));
    let failed = Arc::new(AtomicBool::new(false));
    let _speaker = playback(queue.clone(), volume, failed.clone())?;
    runtime.block_on(async {
        let mut options = RoomOptions::default();
        if let Some(secure) = &secure {
            options.encryption = Some(E2eeOptions { encryption_type: EncryptionType::Gcm, key_provider: secure.provider.clone() });
        }
        let connection=tokio::select! { _=cancelled(&stop)=>return Ok(()), result=tokio::time::timeout(Duration::from_secs(20),Room::connect(&url,&token,options))=>result.map_err(|_|"O servidor de voz não respondeu.")? };
        let (room,mut events)=connection.map_err(|_|"Não foi possível conectar ao servidor de voz.")?;
        if stop.load(Ordering::Acquire) { let _=room.close().await; return Ok(()); }
        let source=NativeAudioSource::new(AudioSourceOptions::default(),48000,1,0);
        let track=LocalAudioTrack::create_audio_track("microphone",RtcAudioSource::Native(source.clone()));
        let local_participant = room.local_participant();
        let publication=tokio::select! { _=cancelled(&stop)=>{let _=tokio::time::timeout(Duration::from_secs(3),room.close()).await;return Ok(());}, result=tokio::time::timeout(Duration::from_secs(15),local_participant.publish_track(LocalTrack::Audio(track),TrackPublishOptions{source:TrackSource::Microphone,..Default::default()}))=>result };
        if !matches!(publication,Ok(Ok(_))) {let _=room.close().await;return Err("Falha ao publicar o microfone.".into());}
        let (ready_tx,ready_rx)=std::sync::mpsc::sync_channel(1);
        let capture_stop=stop.clone();let capture_flag=capture_stop.clone();let window=w.clone();
        let (input_mic, local_identity) = (mic_settings.clone(), local_participant.identity().to_string());
        let mic=std::thread::spawn(move ||super::matrix_voice::capture(&window,NoiseMode::from_code(input_mic.noise.load(Ordering::Acquire)),&capture_flag,ready_tx,Some(CallInput{source,muted,mic:input_mic})));
        // Chosen microphone and output: moved to them once PipeWire lists our streams.
        super::matrix_audio::route_new_streams();
        // Who spoke last and when, per remote identity ("user:device").
        let loud: Arc<Mutex<HashMap<String, Instant>>> = Arc::default();
        let mut speaking_now: Vec<String> = Vec::new();
        let mut identities: HashMap<String, String> = HashMap::new();
        let ready=tokio::task::spawn_blocking(move ||ready_rx.recv_timeout(Duration::from_secs(15))).await;
        let mut outcome=Ok(());
        let mut remote_tasks:HashMap<String,tokio::task::JoinHandle<()>>=HashMap::new();
        if !matches!(ready,Ok(Ok(Ok(())))) {outcome=Err("Não foi possível iniciar o microfone da chamada.".into());}
        else {
            let _=w.emit("matrix-call-status",serde_json::json!({"state":"connected","participants":room.remote_participants().len()+1,"call_id":call_id}));
            let mut tick=tokio::time::interval(Duration::from_millis(100));
            loop {
                tokio::select! {
                    _=tick.tick()=>{
                        if stop.load(Ordering::Acquire){break;}
                        if let Some(secure) = &mut secure {
                            while let Ok(key) = secure.keys.try_recv() {
                                if key.identity == secure.local_key.identity { secure.local_key = key.clone(); }
                                secure.provider.set_key(&key.identity.clone().into(), key.index, key.key);
                                let own = &secure.local_key;
                                secure.provider.set_key(&own.identity.clone().into(), own.index, own.key.clone());
                                if key.identity == own.identity {
                                    for ((identity,_), cryptor) in room.e2ee_manager().frame_cryptors() {
                                        if identity.to_string() == key.identity { cryptor.set_key_index(key.index); }
                                    }
                                }
                            }
                        }
                        if failed.load(Ordering::Acquire)||mic.is_finished(){outcome=Err("Dispositivo de áudio interrompido. A chamada foi encerrada.".into());break;}
                        let mut speaking: Vec<String> = loud.lock().map(|loud| loud.iter().filter(|(_, at)| at.elapsed() < SPEAKING_HOLD).map(|(identity, _)| identity.clone()).collect()).unwrap_or_default();
                        if mic_settings.speaking.load(Ordering::Acquire) { speaking.push(local_identity.clone()); }
                        speaking.sort();
                        if speaking != speaking_now {
                            let _=w.emit("matrix-call-speaking",serde_json::json!({"call_id":call_id,"identities":speaking}));
                            speaking_now = speaking;
                        }
                    }
                    event=events.recv()=>match event {
                        Some(RoomEvent::TrackSubscribed{track:RemoteTrack::Audio(audio),publication,participant})=>{
                            let sid=publication.sid().to_string();
                            if remote_tasks.contains_key(&sid){continue;}
                            let identity=participant.identity().to_string();
                            identities.insert(sid.clone(),identity.clone());
                            let loud=loud.clone();
                            let mut stream=NativeAudioStream::new(audio.rtc_track(),48000,1);
                            let queue=queue.clone();let track_id=sid.clone();
                            if let Ok(mut q)=queue.lock(){q.insert(sid.clone(),Speaker::default());}
                            remote_tasks.insert(sid,tokio::spawn(async move {
                                while let Some(frame)=stream.next().await {
                                    if frame_level(&frame.data) > SPEAKING_LEVEL { if let Ok(mut loud)=loud.lock(){loud.insert(identity.clone(),Instant::now());} }
                                    if let Ok(mut queues)=queue.lock(){if let Some(q)=queues.get_mut(&track_id){enqueue(q,&frame.data);}}
                                }
                                if let Ok(mut q)=queue.lock(){q.remove(&track_id);}
                            }));
                        }
                        Some(RoomEvent::TrackUnsubscribed{track:RemoteTrack::Audio(_),publication,..})=>{
                            let sid=publication.sid().to_string();
                            if let Some(task)=remote_tasks.remove(&sid){task.abort();}
                            if let Some(identity)=identities.remove(&sid){ if let Ok(mut loud)=loud.lock(){loud.remove(&identity);} }
                            if let Ok(mut q)=queue.lock(){q.remove(&sid);}
                        }
                        Some(RoomEvent::Reconnecting)=>{let _=w.emit("matrix-call-status",serde_json::json!({"state":"reconnecting","call_id":call_id}));}
                        Some(RoomEvent::Reconnected)=>{let _=w.emit("matrix-call-status",serde_json::json!({"state":"connected","participants":room.remote_participants().len()+1,"call_id":call_id}));}
                        Some(RoomEvent::ParticipantConnected(_))|Some(RoomEvent::ParticipantDisconnected(_))=>{let _=w.emit("matrix-call-status",serde_json::json!({"state":"connected","participants":room.remote_participants().len()+1,"call_id":call_id}));}
                        Some(RoomEvent::Disconnected{..})|None=>{outcome=Err("A conexão com o servidor foi encerrada.".into());break;}
                        _=>{}
                    }
                }
            }
        }
        capture_stop.store(true,Ordering::Release);
        let _=tokio::task::spawn_blocking(move||mic.join()).await;
        for (_,task) in remote_tasks{task.abort();}
        let _=tokio::time::timeout(Duration::from_secs(3),room.close()).await;
        outcome
    })
}
#[tauri::command]
pub fn matrix_call_start(window: WebviewWindow, mode: NoiseMode) -> Result<(), String> {
    if window.label() != "matrix-rtc-probe" {
        return Err("Use o botão Entrar na voz da sala Matrix.".into());
    }
    let (url, token) = credentials()?;
    let mut current = state().lock().map_err(|_| "Estado indisponível.")?;
    if current.as_ref().is_some_and(|c| !c.thread.is_finished()) {
        return Err("Uma chamada já está ativa.".into());
    }
    if let Some(old) = current.take() {
        let _ = old.thread.join();
    }
    super::matrix_voice::stop();
    let stop = Arc::new(AtomicBool::new(false));
    let muted = Arc::new(AtomicBool::new(true));
    let volume = Arc::new(AtomicU32::new(50));
    let mic = Arc::new(MicSettings::new(mode));
    let (s, m, v, n) = (stop.clone(), muted.clone(), volume.clone(), mic.clone());
    let owner = window.label().to_string();
    let thread = std::thread::spawn(move || {
        let result = run(&window, n, s, m, v, url, token, None, None);
        let _ = window.emit("matrix-call-ended", result.err());
    });
    *current = Some(Call {
        owner,
        id: None,
        keys: None,
        stop,
        muted,
        volume,
        mic,
        thread,
    });
    Ok(())
}
#[tauri::command]
pub async fn matrix_call_stop(window: WebviewWindow) -> Result<(), String> {
    guard(&window)?;
    let owner = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || stop_owned(Some(&owner)))
        .await
        .map_err(|_| "Falha ao encerrar chamada.".to_string())?;
    Ok(())
}
#[tauri::command]
pub fn matrix_call_controls(
    window: WebviewWindow,
    muted: bool,
    volume: u32,
    noise: Option<NoiseMode>,
    mic_volume: Option<u32>,
    agc: Option<bool>,
) -> Result<(), String> {
    guard(&window)?;
    if volume > 200 {
        return Err("Volume inválido.".into());
    }
    let current = state().lock().map_err(|_| "Estado indisponível.")?;
    let call = current.as_ref().ok_or("Nenhuma chamada ativa.")?;
    if call.owner != window.label() {
        return Err("Esta chamada pertence a outra janela.".into());
    }
    call.muted.store(muted, Ordering::Release);
    call.volume.store(volume, Ordering::Release);
    if mic_volume.is_some_and(|value| value > 200) {
        return Err("Volume do microfone inválido.".into());
    }
    if let Some(mode) = noise {
        call.mic.noise.store(mode.code(), Ordering::Release);
    }
    if let Some(percent) = mic_volume {
        call.mic.gain.store(percent, Ordering::Release);
    }
    if let Some(enabled) = agc {
        call.mic.agc.store(enabled, Ordering::Release);
    }
    Ok(())
}
#[tauri::command]
pub async fn matrix_room_call_start(
    window: WebviewWindow,
    room: String,
    mode: NoiseMode,
    keys: Vec<MediaKey>,
    call_id: String,
    encrypted: bool,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Janela Matrix inválida.".into());
    }
    if call_id.len() > 64
        || call_id.is_empty()
        || keys.len() > 512
        || keys.iter().any(|k| !valid_key(k))
        || (!encrypted && !keys.is_empty())
    {
        return Err("Chaves de chamada inválidas.".into());
    }
    let _join = JOIN
        .try_lock()
        .map_err(|_| "Uma chamada já está sendo preparada.")?;
    let attempt = PREPARATION.load(Ordering::Acquire);
    let authorization = async {
        let identity = super::matrix_rtc::matrix_rtc_identity(window.clone()).await?;
        let local_identity = format!(
            "{}:{}",
            identity["user_id"].as_str().ok_or("Conta inválida.")?,
            identity["device_id"]
                .as_str()
                .ok_or("Dispositivo inválido.")?
        );
        // Like Element Call: media is end-to-end encrypted only in encrypted
        // rooms. Elsewhere the other clients expect plain audio and would
        // play our encrypted frames as noise.
        let own = if encrypted {
            Some(
                keys.iter()
                    .rev()
                    .find(|k| k.identity == local_identity)
                    .ok_or("A chave de voz ainda não foi criada.")?
                    .clone(),
            )
        } else {
            None
        };
        let (url, token) = super::matrix_rtc::credentials(&room).await?;
        Ok::<_, String>((local_identity, own, url, token))
    };
    let cancellation = async {
        while attempt == PREPARATION.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };
    let (_local_identity, own, url, token) = tokio::select! {
        _=cancellation => return Err("Conexão cancelada.".into()),
        result=authorization => result?,
    };
    if attempt != PREPARATION.load(Ordering::Acquire) {
        return Err("Conexão cancelada.".into());
    }
    let mut current = state().lock().map_err(|_| "Estado indisponível.")?;
    if current.as_ref().is_some_and(|c| !c.thread.is_finished()) {
        return Err("Saia da chamada atual antes de entrar em outra sala.".into());
    }
    if let Some(old) = current.take() {
        let _ = old.thread.join();
    }
    let (key_tx, secure) = match own {
        Some(own) => {
            let provider = key_provider();
            for key in keys {
                provider.set_key(&key.identity.into(), key.index, key.key);
            }
            // The provider's latest index must refer to our sender, not the last remote key.
            provider.set_key(&own.identity.clone().into(), own.index, own.key.clone());
            let (key_tx, key_rx) = std::sync::mpsc::sync_channel(512);
            (Some(key_tx), Some(SecureMedia { provider, keys: key_rx, local_key: own }))
        }
        None => (None, None),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let muted = Arc::new(AtomicBool::new(true));
    let volume = Arc::new(AtomicU32::new(50));
    let mic = Arc::new(MicSettings::new(mode));
    let (s, m, v, n) = (stop.clone(), muted.clone(), volume.clone(), mic.clone());
    let owner = window.label().to_string();
    let event_id = call_id.clone();
    let thread = std::thread::spawn(move || {
        let result = run(
            &window,
            n,
            s,
            m,
            v,
            url,
            token,
            secure,
            Some(event_id.clone()),
        );
        let _ = window.emit(
            "matrix-call-ended",
            serde_json::json!({"call_id":event_id,"error":result.err()}),
        );
    });
    *current = Some(Call {
        owner,
        id: Some(call_id),
        keys: key_tx,
        stop,
        muted,
        volume,
        mic,
        thread,
    });
    Ok(())
}
#[tauri::command]
pub fn matrix_room_call_key(
    window: WebviewWindow,
    call_id: String,
    key: MediaKey,
) -> Result<(), String> {
    if window.label() != "main" || !valid_key(&key) {
        return Err("Chave de voz inválida.".into());
    }
    let current = state().lock().map_err(|_| "Estado indisponível.")?;
    let call = current
        .as_ref()
        .filter(|c| c.id.as_deref() == Some(&call_id))
        .ok_or("Chamada encerrada.")?;
    call.keys
        .as_ref()
        .ok_or("Chamada sem criptografia.")?
        .try_send(key)
        .map_err(|_| "Não foi possível atualizar a chave de voz.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_backlog_is_bounded() {
        let mut speaker = Speaker::default();
        enqueue(&mut speaker, &vec![1; MAX_BACKLOG]);
        enqueue(&mut speaker, &[2; 480]);
        assert_eq!(speaker.samples.len(), TRIMMED_BACKLOG);
        assert_eq!(speaker.samples.back(), Some(&2));
    }
    #[test]
    fn playback_waits_for_prebuffer_after_running_dry() {
        let mut queues = Mixer::new();
        queues.insert("a".into(), Speaker::default());
        enqueue(queues.get_mut("a").unwrap(), &[1000; 480]);
        assert_eq!(mix_sample(&mut queues), 0., "10 ms is not enough to start");
        enqueue(queues.get_mut("a").unwrap(), &[1000; PREBUFFER]);
        for _ in 0..PREBUFFER + 480 {
            assert!(mix_sample(&mut queues) > 0.);
        }
        assert_eq!(mix_sample(&mut queues), 0.);
        enqueue(queues.get_mut("a").unwrap(), &[1000; 480]);
        assert_eq!(mix_sample(&mut queues), 0., "after running dry it buffers again");
    }
    #[test]
    fn mixer_combines_speakers_without_overflow_or_stale_audio() {
        let mut queues = Mixer::new();
        let speaker = |samples: [i16; 2]| Speaker { samples: VecDeque::from(samples), playing: true };
        queues.insert("a".into(), speaker([30000, 100]));
        queues.insert("b".into(), speaker([30000, -100]));
        assert_eq!(mix_sample(&mut queues), 1.);
        assert_eq!(mix_sample(&mut queues), 0.);
        queues.remove("a");
        queues.remove("b");
        assert_eq!(mix_sample(&mut queues), 0.);
    }
    #[test]
    fn media_key_validation_rejects_invalid_indices_and_material() {
        assert!(valid_key(&MediaKey {
            identity: "a".into(),
            index: 255,
            key: vec![1; 16]
        }));
        assert!(!valid_key(&MediaKey {
            identity: "a".into(),
            index: 256,
            key: vec![1; 16]
        }));
        assert!(!valid_key(&MediaKey {
            identity: "a".into(),
            index: 0,
            key: vec![1; 15]
        }));
    }
    #[test]
    #[ignore = "requires scripts/test-native-call.py --check"]
    fn two_sessions_exchange_audio() {
        let path = std::env::var("PELIGAMES_RTC_TEST_CONFIG").unwrap();
        let c: Credentials = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            tokio::time::timeout(Duration::from_secs(35), async {
                use base64::Engine;
                let identity = |token: &str| {
                    let value: serde_json::Value = serde_json::from_slice(
                        &base64::engine::general_purpose::URL_SAFE_NO_PAD
                            .decode(token.split('.').nth(1).unwrap())
                            .unwrap(),
                    )
                    .unwrap();
                    value["sub"].as_str().unwrap().to_string()
                };
                let ai = identity(&c.a);
                let bi = identity(&c.b);
                for encrypted in [false, true] {
                    let pa = key_provider();
                    let pb = key_provider();
                    for provider in [&pa, &pb] {
                        provider.set_key(&ai.clone().into(), 0, vec![1; 16]);
                        provider.set_key(&bi.clone().into(), 0, vec![2; 16]);
                    }
                    let options = |provider: KeyProvider| {
                        let mut options = RoomOptions::default();
                        if encrypted {
                            options.encryption = Some(E2eeOptions {
                                encryption_type: EncryptionType::Gcm,
                                key_provider: provider,
                            });
                        }
                        options
                    };
                    let (a, mut ae) = Room::connect(&c.url, &c.a, options(pa.clone()))
                        .await
                        .unwrap();
                    let (b, mut be) = Room::connect(&c.url, &c.b, options(pb.clone()))
                        .await
                        .unwrap();
                    for (sender, events, receiver_keys, sender_id, sender_key) in
                        [(&a, &mut be, &pb, &ai, 1u8), (&b, &mut ae, &pa, &bi, 2u8)]
                    {
                        if encrypted {
                            receiver_keys.set_key(&sender_id.clone().into(), 0, vec![99; 16]);
                        }
                        let source =
                            NativeAudioSource::new(AudioSourceOptions::default(), 48000, 1, 0);
                        let track = LocalAudioTrack::create_audio_track(
                            "synthetic",
                            RtcAudioSource::Native(source.clone()),
                        );
                        sender
                            .local_participant()
                            .publish_track(
                                LocalTrack::Audio(track),
                                TrackPublishOptions {
                                    source: TrackSource::Microphone,
                                    ..Default::default()
                                },
                            )
                            .await
                            .unwrap();
                        let remote = loop {
                            if let Some(RoomEvent::TrackSubscribed {
                                track: RemoteTrack::Audio(audio),
                                ..
                            }) = events.recv().await
                            {
                                break audio;
                            }
                        };
                        let mut stream = NativeAudioStream::new(remote.rtc_track(), 48000, 1);
                        let generator = tokio::spawn(async move {
                            for n in 0..200 {
                                let samples: Vec<i16> = (0..480)
                                    .map(|i| {
                                        (((n * 480 + i) as f32 * 440. * std::f32::consts::TAU
                                            / 48000.)
                                            .sin()
                                            * 8000.) as i16
                                    })
                                    .collect();
                                source
                                    .capture_frame(&livekit::webrtc::audio_frame::AudioFrame {
                                        data: std::borrow::Cow::Owned(samples),
                                        sample_rate: 48000,
                                        num_channels: 1,
                                        samples_per_channel: 480,
                                    })
                                    .await
                                    .unwrap();
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        });
                        if encrypted {
                            // A wrong key must never fall back to plaintext audio.
                            let leaked = tokio::time::timeout(Duration::from_millis(300), async {
                                while let Some(frame) = stream.next().await {
                                    if frame.data.iter().any(|x| x.abs() > 100) {
                                        return;
                                    }
                                }
                            })
                            .await;
                            assert!(
                                leaked.is_err(),
                                "audio was audible with an incorrect media key"
                            );
                            receiver_keys.set_key(
                                &sender_id.clone().into(),
                                0,
                                vec![sender_key; 16],
                            );
                        }
                        let mut audible = 0;
                        while audible < 10 {
                            let frame = stream.next().await.unwrap();
                            if frame.data.iter().any(|x| x.abs() > 100) {
                                audible += 1;
                            }
                        }
                        generator.abort();
                        let _ = generator.await;
                        stream.close();
                    }
                    a.close().await.unwrap();
                    b.close().await.unwrap();
                }
            })
            .await
            .expect("two-way audio exchange timed out");
        });
    }
}
