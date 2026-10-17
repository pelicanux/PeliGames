//! Sound of a screen share: the whole PC except the call, or one application.
//!
//! The screen portal gives no audio, so each playing stream is recorded on its
//! own with `parec --monitor-stream` (PipeWire's PulseAudio server), which taps
//! the stream without moving it: the user keeps hearing everything as before.
//! The call's own playback is never recorded, so nobody hears themselves back.
use livekit::webrtc::{audio_frame::AudioFrame, audio_source::native::NativeAudioSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    io::Read,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const RATE: u32 = 48000;
const CHANNELS: usize = 2;
/// 10 ms per frame, as WebRTC expects.
const FRAME: usize = 480;
/// Playback of a stream starts (or resumes) with 40 ms buffered; above 200 ms
/// the oldest audio is dropped back to 60 ms.
const PREBUFFER: usize = 4 * FRAME * CHANNELS;
const MAX_BACKLOG: usize = 20 * FRAME * CHANNELS;
const TRIMMED_BACKLOG: usize = 6 * FRAME * CHANNELS;
const RESCAN: Duration = Duration::from_millis(1500);

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ShareAudio {
    #[default]
    None,
    /// Everything playing on the PC, except the call itself.
    All,
    /// The streams of one application, by the key of `matrix_screen_audio_apps`.
    App { key: String },
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct AudioApp {
    key: String,
    name: String,
}

/// One playing stream of another application.
#[derive(Debug, PartialEq)]
struct Stream {
    index: u64,
    key: String,
    name: String,
}

fn text<'a>(properties: &'a Value, key: &str) -> Option<&'a str> {
    properties[key].as_str().map(str::trim).filter(|value| !value.is_empty())
}
/// Playing streams of `pactl list sink-inputs`, without this launcher's own.
fn streams_of(list: &[Value], pid: &str, binary: &str) -> Vec<Stream> {
    let own_node = format!("alsa_playback.{binary}");
    list.iter()
        .filter_map(|item| {
            let properties = &item["properties"];
            let node = text(properties, "node.name").unwrap_or_default();
            if text(properties, "application.process.id") == Some(pid)
                || node == own_node
                || node.starts_with(&format!("{own_node}-"))
                || text(properties, "application.process.binary") == Some(binary)
            {
                return None;
            }
            let name = text(properties, "application.name")
                .or_else(|| text(properties, "media.name"))
                .unwrap_or(node);
            // Group by the program when known: a browser has one stream per tab.
            let key = text(properties, "application.process.binary")
                .or_else(|| text(properties, "application.name"))
                .unwrap_or(node);
            Some(Stream {
                index: item["index"].as_u64()?,
                key: key.to_string(),
                name: name.to_string(),
            })
        })
        .filter(|stream| !stream.key.is_empty())
        .collect()
}
fn streams() -> Vec<Stream> {
    let Ok(output) = Command::new("pactl")
        .args(["-f", "json", "list", "sink-inputs"])
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    let list: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap_or_default();
    let binary = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "peligames".into());
    streams_of(&list, &std::process::id().to_string(), &binary)
}

/// Applications playing sound now, for the share's audio choice.
#[tauri::command]
pub async fn matrix_screen_audio_apps() -> Vec<AudioApp> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut apps: Vec<AudioApp> = Vec::new();
        for stream in streams() {
            if !apps.iter().any(|app| app.key == stream.key) {
                apps.push(AudioApp { key: stream.key, name: stream.name });
            }
        }
        apps.sort_by_key(|app| app.name.to_lowercase());
        apps
    })
    .await
    .unwrap_or_default()
}

/// The recording of one stream, read by its own thread.
struct Tap {
    child: Child,
    samples: Arc<Mutex<VecDeque<i16>>>,
    playing: bool,
}
impl Drop for Tap {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn tap(index: u64) -> Option<Tap> {
    let mut child = Command::new("parec")
        .args([
            &format!("--monitor-stream={index}"),
            "--format=s16le",
            "--rate=48000",
            "--channels=2",
            "--raw",
            "--latency-msec=20",
            "--client-name=PeliGames",
            "--stream-name=Transmissão de tela",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let samples: Arc<Mutex<VecDeque<i16>>> = Arc::default();
    let queue = samples.clone();
    std::thread::spawn(move || {
        let mut bytes = [0u8; FRAME * CHANNELS * 2];
        let mut carry: Option<u8> = None;
        while let Ok(read) = stdout.read(&mut bytes) {
            if read == 0 {
                break;
            }
            let Ok(mut queue) = queue.lock() else { break };
            let mut chunk = &bytes[..read];
            if let Some(low) = carry.take() {
                queue.push_back(i16::from_le_bytes([low, chunk[0]]));
                chunk = &chunk[1..];
            }
            let mut pairs = chunk.chunks_exact(2);
            queue.extend(pairs.by_ref().map(|pair| i16::from_le_bytes([pair[0], pair[1]])));
            carry = pairs.remainder().first().copied();
            if queue.len() > MAX_BACKLOG {
                let excess = queue.len() - TRIMMED_BACKLOG;
                queue.drain(..excess - excess % CHANNELS);
            }
        }
    });
    Some(Tap { child, samples, playing: false })
}

fn wanted(choice: &ShareAudio, stream: &Stream) -> bool {
    match choice {
        ShareAudio::None => false,
        ShareAudio::All => true,
        ShareAudio::App { key } => stream.key == *key,
    }
}

/// Adds one 10 ms frame of every playing tap into `mix`.
fn mix_frame(taps: &mut HashMap<u64, Tap>, mix: &mut [i16]) {
    let mut sum = [0i32; FRAME * CHANNELS];
    for tap in taps.values_mut() {
        let Ok(mut queue) = tap.samples.lock() else { continue };
        if !tap.playing && queue.len() >= PREBUFFER {
            tap.playing = true;
        }
        if !tap.playing {
            continue;
        }
        if queue.len() < FRAME * CHANNELS {
            tap.playing = false; // Ran dry: wait for a cushion again.
            continue;
        }
        for (total, sample) in sum.iter_mut().zip(queue.drain(..FRAME * CHANNELS)) {
            *total += sample as i32;
        }
    }
    for (out, total) in mix.iter_mut().zip(sum) {
        *out = total.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }
}

/// Feeds `source` with the chosen sound until `stop`. Streams that start
/// later (a game launched mid-share, a new tab) join on the next scan.
/// `parec` is installed (pulseaudio-utils, next to `pactl`).
pub(super) fn available() -> bool {
    Command::new("parec").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|status| status.success())
}
pub(super) const MISSING: &str = "A transmissão segue sem som: falta o parec (pacote pulseaudio-utils).";

pub(super) fn capture(choice: ShareAudio, source: NativeAudioSource, stop: Arc<AtomicBool>) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Falha ao preparar o som da transmissão.")?;
    let mut taps: HashMap<u64, Tap> = HashMap::new();
    let mut frame = vec![0i16; FRAME * CHANNELS];
    let mut scanned: Option<Instant> = None;
    let mut next = Instant::now();
    while !stop.load(Ordering::Acquire) {
        if scanned.is_none_or(|at| at.elapsed() >= RESCAN) {
            scanned = Some(Instant::now());
            let current: Vec<u64> = streams()
                .into_iter()
                .filter(|stream| wanted(&choice, stream))
                .map(|stream| stream.index)
                .collect();
            taps.retain(|index, tap| current.contains(index) && tap.child.try_wait().is_ok_and(|done| done.is_none()));
            for index in current {
                if let std::collections::hash_map::Entry::Vacant(entry) = taps.entry(index) {
                    if let Some(tap) = tap(index) {
                        entry.insert(tap);
                    }
                }
            }
        }
        mix_frame(&mut taps, &mut frame);
        let _ = runtime.block_on(source.capture_frame(&AudioFrame {
            data: Cow::Borrowed(&frame),
            sample_rate: RATE,
            num_channels: CHANNELS as u32,
            samples_per_channel: FRAME as u32,
        }));
        next += Duration::from_millis(10);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_playback_is_never_recorded_and_apps_are_grouped() {
        let list: Vec<Value> = serde_json::from_str(
            r#"[
            {"index":1,"properties":{"application.name":"PipeWire ALSA [peligames]","node.name":"alsa_playback.peligames"}},
            {"index":2,"properties":{"application.name":"Firefox","application.process.binary":"firefox","application.process.id":"50"}},
            {"index":3,"properties":{"application.name":"Firefox","application.process.binary":"firefox","application.process.id":"50"}},
            {"index":4,"properties":{"application.name":"Spotify","node.name":"spotify"}},
            {"index":5,"properties":{"application.name":"Other","application.process.id":"42"}}
        ]"#,
        )
        .unwrap();
        let streams = streams_of(&list, "42", "peligames");
        assert_eq!(streams.iter().map(|s| s.index).collect::<Vec<_>>(), [2, 3, 4]);
        assert_eq!(streams[0].key, "firefox");
        assert_eq!(streams[2].key, "Spotify");
        let app = ShareAudio::App { key: "firefox".into() };
        assert!(wanted(&app, &streams[1]) && !wanted(&app, &streams[2]));
        assert!(wanted(&ShareAudio::All, &streams[2]) && !wanted(&ShareAudio::None, &streams[0]));
    }

    #[test]
    fn choice_arrives_from_the_interface() {
        assert_eq!(serde_json::from_str::<ShareAudio>(r#"{"kind":"all"}"#).unwrap(), ShareAudio::All);
        assert_eq!(
            serde_json::from_str::<ShareAudio>(r#"{"kind":"app","key":"firefox"}"#).unwrap(),
            ShareAudio::App { key: "firefox".into() }
        );
    }
}
