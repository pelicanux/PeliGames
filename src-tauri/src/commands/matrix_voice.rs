//! Native microphone pipeline for Matrix calls.
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use livekit::{
    prelude::LocalAudioTrack,
    webrtc::{
        audio_frame::AudioFrame,
        audio_source::{native::NativeAudioSource, RtcAudioSource},
        native::apm::AudioProcessingModule,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, WebviewWindow};

const FRAME: usize = 480; // Both processors consume 10 ms, mono, 48 kHz.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NoiseMode {
    Off,
    Standard,
    Rnnoise,
}

impl NoiseMode {
    pub(super) fn code(self) -> u8 {
        match self {
            NoiseMode::Off => 0,
            NoiseMode::Standard => 1,
            NoiseMode::Rnnoise => 2,
        }
    }
    pub(super) fn from_code(code: u8) -> Self {
        match code {
            1 => NoiseMode::Standard,
            2 => NoiseMode::Rnnoise,
            _ => NoiseMode::Off,
        }
    }
}
/// Microphone settings of the live call, changeable while it runs.
pub(super) struct MicSettings {
    pub noise: AtomicU8,
    /// WebRTC automatic gain control: evens out quiet and loud voices.
    pub agc: AtomicBool,
    /// Manual input volume in percent (0 to 200), applied before the filters.
    pub gain: AtomicU32,
    /// Whether the user is speaking now (unmuted and above the voice threshold).
    pub speaking: AtomicBool,
}
impl MicSettings {
    pub(super) fn new(mode: NoiseMode) -> Self {
        Self { noise: AtomicU8::new(mode.code()), agc: AtomicBool::new(true), gain: AtomicU32::new(100), speaking: AtomicBool::new(false) }
    }
}
/// What the call's microphone feeds: the LiveKit source, mute and settings.
pub(super) struct CallInput {
    pub source: NativeAudioSource,
    pub muted: Arc<AtomicBool>,
    pub mic: Arc<MicSettings>,
}
/// Above this level (RMS of a 10 ms frame) someone is considered to be speaking.
pub(super) const SPEAKING_LEVEL: f32 = 0.015;
/// Speaking stays lit this long after the last loud frame, so words do not flicker.
pub(super) const SPEAKING_HOLD: Duration = Duration::from_millis(300);
/// Multiplies the samples by `percent` / 100, saturating instead of wrapping.
fn apply_gain(frame: &mut [i16], percent: u32) {
    if percent == 100 {
        return;
    }
    let factor = percent.min(200) as f32 / 100.;
    for sample in frame {
        *sample = (*sample as f32 * factor).clamp(-32768., 32767.) as i16;
    }
}

struct Processor {
    mode: NoiseMode,
    agc: bool,
    apm: AudioProcessingModule,
    rnnoise: crate::core::audio_denoise::RnNoise,
}
impl Processor {
    fn new(mode: NoiseMode, agc: bool) -> Self {
        // Echo cancellation needs a playback reference; do not claim it is active yet.
        Self {
            mode,
            agc,
            apm: AudioProcessingModule::new(false, agc, false, mode == NoiseMode::Standard),
            rnnoise: crate::core::audio_denoise::RnNoise::new(),
        }
    }
    fn process(&mut self, frame: &mut [i16; FRAME]) -> Result<(), String> {
        if self.mode == NoiseMode::Rnnoise {
            self.rnnoise.process(frame);
        }
        // WebRTC: its noise suppression (Standard) and/or gain control, after
        // RNNoise so the gain does not lift the noise RNNoise removes.
        if self.mode == NoiseMode::Standard || self.agc {
            self.apm
                .process_stream(frame, 48000, 1)
                .map_err(|_| "Falha no filtro WebRTC.".to_string())?;
        }
        Ok(())
    }
}
fn level(frame: &[i16]) -> f32 {
    (frame
        .iter()
        .map(|&x| (x as f32 / 32768.).powi(2))
        .sum::<f32>()
        / frame.len().max(1) as f32)
        .sqrt()
}
#[derive(Clone, Serialize)]
struct Meter {
    input: f32,
    output: f32,
    mode: NoiseMode,
}

fn input_stream<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    tx: std::sync::mpsc::SyncSender<[i16; FRAME]>,
    failed: Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let channels = config.channels as usize;
    let mut frame = [0i16; FRAME];
    let mut offset = 0;
    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                for samples in data.chunks_exact(channels) {
                    let sample = samples
                        .iter()
                        .map(|&x| <f32 as cpal::FromSample<T>>::from_sample_(x))
                        .sum::<f32>()
                        / channels as f32;
                    frame[offset] = (sample.clamp(-1., 1.) * 32767.) as i16;
                    offset += 1;
                    if offset == FRAME {
                        // Bounded queue: never block the audio callback or build a growing backlog.
                        let _ = tx.try_send(frame);
                        offset = 0;
                    }
                }
            },
            move |_| {
                failed.store(true, Ordering::Release);
            },
            None,
        )
        .map_err(|_| "Não foi possível abrir o microfone.".into())
}

pub(super) fn capture(
    window: &WebviewWindow,
    mode: NoiseMode,
    stop: &AtomicBool,
    ready: std::sync::mpsc::SyncSender<Result<(), String>>,
    target: CallInput,
) -> Result<(), String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("Nenhum microfone padrão disponível.")?;
    let supported = device.supported_input_configs().map_err(|_|"Não foi possível consultar o microfone.")?
        .find(|c| c.channels() > 0 && c.min_sample_rate().0 <= 48000 && c.max_sample_rate().0 >= 48000 && matches!(c.sample_format(),cpal::SampleFormat::F32|cpal::SampleFormat::I16|cpal::SampleFormat::U16))
        .ok_or("O microfone não oferece 48 kHz neste backend. Selecione um dispositivo padrão compatível no sistema.")?.with_sample_rate(cpal::SampleRate(48000));
    let config = supported.config();
    let (tx, rx) = std::sync::mpsc::sync_channel(5);
    let failed = Arc::new(AtomicBool::new(false));
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => input_stream::<f32>(&device, &config, tx, failed.clone()),
        cpal::SampleFormat::I16 => input_stream::<i16>(&device, &config, tx, failed.clone()),
        cpal::SampleFormat::U16 => input_stream::<u16>(&device, &config, tx, failed.clone()),
        _ => unreachable!(),
    }?;
    let mut processor = Processor::new(mode, false);
    // Explicit APM above; disable additional processing in the source to avoid double filtering.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Falha ao preparar o áudio.")?;
    let _runtime_guard = runtime.enter();
    let CallInput { source, muted, mic } = target;
    let mut last_loud: Option<Instant> = None;
    let _track = LocalAudioTrack::create_audio_track(
        "peligames-microphone",
        RtcAudioSource::Native(source.clone()),
    );
    if stop.load(Ordering::Acquire) {
        return Err("Captura cancelada.".into());
    }
    stream
        .play()
        .map_err(|_| "Não foi possível iniciar o microfone.")?;
    let _ = ready.send(Ok(()));
    let mut frames = 0;
    while !stop.load(Ordering::Acquire) {
        if failed.load(Ordering::Acquire) {
            return Err("O dispositivo de áudio foi desconectado ou interrompido.".into());
        }
        let mut frame = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(f) => f,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        let wanted = NoiseMode::from_code(mic.noise.load(Ordering::Acquire));
        let agc = mic.agc.load(Ordering::Acquire);
        if wanted != processor.mode || agc != processor.agc {
            // A fresh filter: no state carried over from the previous settings.
            processor = Processor::new(wanted, agc);
        }
        apply_gain(&mut frame, mic.gain.load(Ordering::Acquire));
        let input = level(&frame);
        processor.process(&mut frame)?;
        let output = level(&frame);
        let silenced = muted.load(Ordering::Acquire);
        if silenced {
            frame.fill(0);
        }
        if !silenced && output > SPEAKING_LEVEL {
            last_loud = Some(Instant::now());
        }
        mic.speaking.store(
            !silenced && last_loud.is_some_and(|at| at.elapsed() < SPEAKING_HOLD),
            Ordering::Release,
        );
        runtime
            .block_on(source.capture_frame(&AudioFrame {
                data: Cow::Borrowed(&frame),
                sample_rate: 48000,
                num_channels: 1,
                samples_per_channel: FRAME as u32,
            }))
            .map_err(|_| "Falha ao alimentar a faixa LiveKit.".to_string())?;
        frames += 1;
        if frames % 10 == 0 {
            let _ = window.emit(
                "matrix-voice-meter",
                Meter {
                    input,
                    output,
                    mode: processor.mode,
                },
            );
        }
    }
    drop(stream);
    mic.speaking.store(false, Ordering::Release);
    source.clear_buffer();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use livekit::webrtc::audio_source::AudioSourceOptions;
    #[test]
    fn off_preserves_pcm_exactly() {
        let mut frame = std::array::from_fn(|i| (i as i16 - 240) * 100);
        let expected = frame;
        Processor::new(NoiseMode::Off, false).process(&mut frame).unwrap();
        assert_eq!(frame, expected);
    }
    #[test]
    fn filters_process_silence_and_signal() {
        for mode in [NoiseMode::Standard, NoiseMode::Rnnoise] {
            let mut p = Processor::new(mode, false);
            for _ in 0..20 {
                let mut silence = [0; FRAME];
                p.process(&mut silence).unwrap();
                assert!(level(&silence) < 0.01);
            }
            let mut changed = false;
            for _ in 0..40 {
                let input = std::array::from_fn(|i| if i % 2 == 0 { 1000 } else { -1000 });
                let mut output = input;
                p.process(&mut output).unwrap();
                changed |= input != output;
            }
            assert!(changed, "filter must actually modify the signal: {mode:?}");
        }
    }
    #[test]
    fn native_livekit_track_accepts_processed_frames() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = runtime.enter();
        let source = NativeAudioSource::new(AudioSourceOptions::default(), 48000, 1, 0);
        let _track =
            LocalAudioTrack::create_audio_track("test", RtcAudioSource::Native(source.clone()));
        for mode in [NoiseMode::Off, NoiseMode::Standard, NoiseMode::Rnnoise] {
            let mut frame = [0; FRAME];
            Processor::new(mode, true).process(&mut frame).unwrap();
            runtime
                .block_on(source.capture_frame(&AudioFrame {
                    data: Cow::Borrowed(&frame),
                    sample_rate: 48000,
                    num_channels: 1,
                    samples_per_channel: FRAME as u32,
                }))
                .unwrap();
        }
        source.clear_buffer();
    }
    #[test]
    fn gain_scales_and_saturates() {
        let mut frame = [1000, -1000, 30000, -30000];
        apply_gain(&mut frame, 100);
        assert_eq!(frame, [1000, -1000, 30000, -30000]);
        apply_gain(&mut frame, 200);
        assert_eq!(frame, [2000, -2000, 32767, -32768]);
        apply_gain(&mut frame, 0);
        assert_eq!(frame, [0; 4]);
    }
    #[test]
    fn automatic_gain_runs_with_every_noise_mode() {
        // WebRTC's AGC2 only raises the gain on frames its voice detector
        // classifies as speech, so synthetic tones cannot measure it: this
        // checks the filter chain runs and keeps the signal.
        for mode in [NoiseMode::Off, NoiseMode::Standard, NoiseMode::Rnnoise] {
            let mut p = Processor::new(mode, true);
            let mut last = 0.;
            for n in 0..100 {
                let mut frame: [i16; FRAME] = std::array::from_fn(|i| {
                    let t = (n * FRAME + i) as f32 / 48000.;
                    ((t * 220. * std::f32::consts::TAU).sin() * 3000.) as i16
                });
                p.process(&mut frame).unwrap();
                last = level(&frame);
            }
            assert!(last > 0., "{mode:?} with AGC silenced the signal");
        }
    }
    #[test]
    fn noise_mode_codes_round_trip() {
        for mode in [NoiseMode::Off, NoiseMode::Standard, NoiseMode::Rnnoise] {
            assert_eq!(NoiseMode::from_code(mode.code()), mode);
        }
        assert_eq!(NoiseMode::from_code(99), NoiseMode::Off);
    }
    #[test]
    fn invalid_modes_are_rejected() {
        assert!(serde_json::from_str::<NoiseMode>("\"other\"").is_err());
    }
}
