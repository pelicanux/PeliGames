//! Screen sharing in a native MatrixRTC call, all outside the webview.
//!
//! Sending: WebRTC's desktop capturer (the PipeWire portal on Wayland, X11
//! otherwise) gives BGRA frames, converted to I420 by libyuv and published as
//! one H.264 layer (VA-API or NVENC when available). Receiving: frames are
//! scaled to the stage size before conversion and drawn by a native GTK
//! layer above the webview, placed by the interface (call view or mini player).
use super::matrix_screen_audio::{self, ShareAudio};
use livekit::options::{AudioEncoding, DegradationPreference, TrackPublishOptions, VideoCodec, VideoEncoderBackend, VideoEncoding, VideoPreset};
use livekit::prelude::*;
use livekit::webrtc::{
    audio_source::{native::NativeAudioSource, AudioSourceOptions, RtcAudioSource},
    desktop_capturer::{
        CaptureError, DesktopCaptureSourceType, DesktopCapturer, DesktopCapturerOptions,
        DesktopFrame,
    },
    native::yuv_helper,
    prelude::{I420Buffer, VideoBuffer, VideoFrame, VideoRotation},
    video_source::{native::NativeVideoSource, RtcVideoSource, VideoResolution},
    video_stream::native::NativeVideoStream,
};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use serde::Deserialize;
use tauri::{Emitter, WebviewWindow};
use tokio_stream::StreamExt;

/// Resolution and frame rate chosen for a share: 720p or 1080p, 30 or 60 fps.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct ShareQuality {
    height: u32,
    fps: u32,
    /// Automatic: layers for every connection (1080p, 720p, 360p); each
    /// viewer gets the one their network carries, switched on the fly.
    #[serde(default)]
    auto: bool,
}
impl Default for ShareQuality {
    fn default() -> Self {
        Self { height: 1080, fps: 30, auto: false }
    }
}
impl ShareQuality {
    /// Only the offered choices; anything else falls back to 1080p30.
    pub(super) fn checked(self) -> Self {
        if self.auto { Self { height: 1080, fps: 30, auto: true } }
        else if matches!(self.height, 720 | 1080) && matches!(self.fps, 30 | 60) { self } else { Self::default() }
    }
    fn size(self) -> (i32, i32) {
        if self.height == 720 { (1280, 720) } else { (1920, 1080) }
    }
    fn interval(self) -> Duration {
        Duration::from_micros(1_000_000 / self.fps as u64)
    }
    /// H.264 bits per second: text stays sharp without loading the network.
    fn bitrate(self) -> u64 {
        match (self.height, self.fps) {
            (720, 30) => 2_000_000,
            (720, _) | (_, 30) => 3_500_000,
            _ => 6_000_000,
        }
    }
}
const AUDIO_BITRATE: u64 = 128_000;
/// Time to answer the portal's screen or window picker.
const CHOICE_TIMEOUT: Duration = Duration::from_secs(180);

/// Largest even size inside the limits, keeping the aspect ratio.
fn fit(width: i32, height: i32, max_width: i32, max_height: i32) -> (i32, i32) {
    let scale = (max_width as f64 / width as f64)
        .min(max_height as f64 / height as f64)
        .min(1.0);
    let even = |value: f64| ((value as i32) & !1).max(2);
    (even(width as f64 * scale), even(height as f64 * scale))
}

fn convert(frame: &DesktopFrame, (max_width, max_height): (i32, i32)) -> Option<I420Buffer> {
    let (width, height) = (frame.width() & !1, frame.height() & !1);
    if width < 2 || height < 2 || frame.stride() < width as u32 * 4 {
        return None;
    }
    let mut buffer = I420Buffer::new(width as u32, height as u32);
    let (stride_y, stride_u, stride_v) = buffer.strides();
    let (y, u, v) = buffer.data_mut();
    // DesktopFrame is BGRA in memory, which libyuv calls ARGB.
    yuv_helper::argb_to_i420(
        frame.data(),
        frame.stride(),
        y,
        stride_y,
        u,
        stride_u,
        v,
        stride_v,
        width,
        height,
    );
    let (fit_width, fit_height) = fit(width, height, max_width, max_height);
    Some(if (fit_width, fit_height) == (width, height) {
        buffer
    } else {
        buffer.scale(fit_width, fit_height)
    })
}

fn capturer() -> Option<DesktopCapturer> {
    // Generic is the portal picker (screens and windows) under Wayland; X11
    // has no generic capturer and shares the whole desktop.
    [DesktopCaptureSourceType::Generic, DesktopCaptureSourceType::Screen]
        .into_iter()
        .find_map(|kind| {
            let mut options = DesktopCapturerOptions::new(kind);
            options.set_include_cursor(true);
            DesktopCapturer::new(options)
        })
}

/// Captures until `stop`; `ready` is told once the first frame went out.
fn capture(
    source: NativeVideoSource,
    stop: &AtomicBool,
    ready: mpsc::SyncSender<()>,
    quality: ShareQuality,
) -> Result<(), String> {
    let mut capturer = capturer().ok_or("Captura de tela indisponível neste sistema.")?;
    let failed = Arc::new(AtomicBool::new(false));
    let lost = failed.clone();
    let mut ready = Some(ready);
    let mut count = 0u32;
    capturer.start_capture(None, move |result| match result {
        Ok(frame) => {
            if let Some(buffer) = convert(&frame, quality.size()) {
                let frame = VideoFrame {
                    rotation: VideoRotation::VideoRotation0,
                    timestamp_us: 0,
                    frame_metadata: None,
                    buffer,
                };
                source.capture_frame(&frame);
                if let Some(ready) = ready.take() {
                    let _ = ready.send(());
                }
                // The own preview, when shown, at about 15 fps: it is only a check.
                count = count.wrapping_add(1);
                if PREVIEW.load(Ordering::Acquire) && count % (quality.fps / 15).max(1) == 0 {
                    stage().show(&frame.buffer);
                }
            }
        }
        Err(CaptureError::Permanent) => lost.store(true, Ordering::Release),
        Err(CaptureError::Temporary) => {}
    });
    let mut next = Instant::now();
    while !stop.load(Ordering::Acquire) {
        capturer.capture_frame();
        if failed.load(Ordering::Acquire) {
            return Err("O compartilhamento foi cancelado ou a tela deixou de existir.".into());
        }
        next += quality.interval();
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now; // Fell behind: skip instead of catching up in a burst.
        }
    }
    Ok(())
}

async fn share(
    w: &WebviewWindow,
    participant: LocalParticipant,
    stop: &Arc<AtomicBool>,
    call_id: &Option<String>,
    audio: ShareAudio,
    quality: ShareQuality,
) -> Result<(), String> {
    let (width, height) = quality.size();
    let source = NativeVideoSource::new(
        VideoResolution { width: width as u32, height: height as u32 },
        true,
    );
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let thread = {
        let (source, stop) = (source.clone(), stop.clone());
        std::thread::spawn(move || capture(source, &stop, ready_tx, quality))
    };
    let finish = |thread: std::thread::JoinHandle<Result<(), String>>| async move {
        tokio::task::spawn_blocking(move || thread.join())
            .await
            .ok()
            .and_then(|joined| joined.ok())
            .unwrap_or(Ok(()))
    };
    let _ = w.emit("matrix-screen-status", serde_json::json!({"state":"choosing","call_id":call_id}));
    let started = Instant::now();
    loop {
        if ready_rx.try_recv().is_ok() {
            break;
        }
        if stop.load(Ordering::Acquire) || thread.is_finished() {
            stop.store(true, Ordering::Release);
            return finish(thread).await;
        }
        if started.elapsed() > CHOICE_TIMEOUT {
            stop.store(true, Ordering::Release);
            finish(thread).await?;
            return Err("Nenhuma tela foi escolhida.".into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let track = LocalVideoTrack::create_video_track("screen", RtcVideoSource::Native(source));
    let options = TrackPublishOptions {
        source: TrackSource::Screenshare,
        video_codec: VideoCodec::H264,
        video_encoder: VideoEncoderBackend::Hardware,
        video_encoding: Some(VideoEncoding { max_bitrate: quality.bitrate(), max_framerate: quality.fps as f64 }),
        simulcast: quality.auto,
        simulcast_layers: quality.auto.then(|| vec![VideoPreset::new(640, 360, 600_000, 15.0), VideoPreset::new(1280, 720, 1_500_000, 30.0)]),
        degradation_preference: quality.auto.then_some(DegradationPreference::Balanced),
        ..Default::default()
    };
    let publication = tokio::time::timeout(
        Duration::from_secs(15),
        participant.publish_track(LocalTrack::Video(track), options),
    )
    .await;
    let Ok(Ok(publication)) = publication else {
        stop.store(true, Ordering::Release);
        finish(thread).await?;
        return Err("Falha ao publicar a transmissão de tela.".into());
    };
    // The sound goes as its own track, marked as the share's: viewers hear it
    // only while watching, apart from the microphone.
    let mut warning = None;
    let mut sound = None;
    if audio != ShareAudio::None && !matrix_screen_audio::available() {
        warning = Some(matrix_screen_audio::MISSING);
    } else if audio != ShareAudio::None {
        let source = NativeAudioSource::new(AudioSourceOptions::default(), 48000, 2, 0);
        let track = LocalAudioTrack::create_audio_track("screen_audio", RtcAudioSource::Native(source.clone()));
        let options = TrackPublishOptions {
            source: TrackSource::ScreenshareAudio,
            audio_encoding: Some(AudioEncoding { max_bitrate: AUDIO_BITRATE }),
            dtx: false,
            red: false,
            ..Default::default()
        };
        match tokio::time::timeout(Duration::from_secs(15), participant.publish_track(LocalTrack::Audio(track), options)).await {
            Ok(Ok(publication)) => {
                let stop = stop.clone();
                let thread = std::thread::spawn(move || matrix_screen_audio::capture(audio, source, stop));
                sound = Some((publication, thread));
            }
            _ => warning = Some("A transmissão segue sem som: falha ao publicar o áudio."),
        }
    }
    let _ = w.emit("matrix-screen-status", serde_json::json!({"state":"live","call_id":call_id,"error":warning}));
    while !stop.load(Ordering::Acquire) && !thread.is_finished() {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    stop.store(true, Ordering::Release);
    let mut sids = vec![publication.sid()];
    if let Some((publication, thread)) = sound {
        sids.push(publication.sid());
        let _ = tokio::task::spawn_blocking(move || thread.join()).await;
    }
    for sid in sids {
        let _ = tokio::time::timeout(Duration::from_secs(5), participant.unpublish_track(&sid)).await;
    }
    finish(thread).await
}

/// The local screen share of a call, started from the call's event loop.
pub(super) struct Share {
    stop: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}
impl Share {
    pub(super) fn start(
        w: WebviewWindow,
        participant: LocalParticipant,
        call_id: Option<String>,
        audio: ShareAudio,
        quality: ShareQuality,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let task = tokio::spawn(async move {
            let result = share(&w, participant, &flag, &call_id, audio, quality.checked()).await;
            let _ = w.emit(
                "matrix-screen-status",
                serde_json::json!({"state":"off","call_id":call_id,"error":result.err()}),
            );
        });
        Self { stop, task }
    }
    pub(super) fn finished(&self) -> bool {
        self.task.is_finished()
    }
    pub(super) async fn stop(self) {
        self.stop.store(true, Ordering::Release);
        let _ = tokio::time::timeout(Duration::from_secs(8), self.task).await;
    }
}

/// Requests from the interface, handled by the call's event loop.
pub(super) enum ScreenCommand {
    /// Starts sharing this account's screen with the chosen sound, or stops (`None`).
    Share(Option<(ShareAudio, ShareQuality)>),
    /// Shows the share of this call identity on the stage (the own one is a
    /// local preview), or nothing. Only the watched share is received.
    Watch(Option<String>),
}

/// One decoded picture, already at the stage's size, in cairo's RGB24 layout.
struct Picture {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}
/// The video stage: one native layer over the launcher, fed by the watched
/// stream. Nothing is converted while the layer is hidden.
#[derive(Default)]
struct Slot {
    picture: Mutex<Option<Picture>>,
    /// Layer size in device pixels, `width << 32 | height`; zero when hidden.
    target: AtomicU64,
    /// The watched stream changed or ended: the old picture goes away.
    clear: AtomicBool,
}
static STAGE: OnceLock<Slot> = OnceLock::new();
fn stage() -> &'static Slot {
    STAGE.get_or_init(Slot::default)
}
/// The own share is being watched: the capture also feeds the stage.
pub(super) static PREVIEW: AtomicBool = AtomicBool::new(false);
impl Slot {
    fn show(&self, buffer: &dyn VideoBuffer) {
        let target = self.target.load(Ordering::Acquire);
        let (width, height) = ((target >> 32) as i32, (target & 0xffff_ffff) as i32);
        if width < 2 || height < 2 {
            return;
        }
        if let Some(picture) = picture(buffer, width, height) {
            if let Ok(mut slot) = self.picture.lock() {
                *slot = Some(picture);
            }
        }
    }
}
pub(super) fn reset_stage() {
    if let Ok(mut picture) = stage().picture.lock() {
        *picture = None;
    }
    stage().clear.store(true, Ordering::Release);
}

fn picture(buffer: &dyn VideoBuffer, max_width: i32, max_height: i32) -> Option<Picture> {
    let (width, height) = (buffer.width() as i32, buffer.height() as i32);
    if width < 2 || height < 2 {
        return None;
    }
    let (fit_width, fit_height) = fit(width, height, max_width, max_height);
    let mut i420 = buffer.to_i420();
    if (fit_width, fit_height) != (width, height) {
        i420 = i420.scale(fit_width, fit_height);
    }
    let (stride_y, stride_u, stride_v) = i420.strides();
    let (y, u, v) = i420.data();
    let stride = fit_width as u32 * 4;
    let mut pixels = vec![0; stride as usize * fit_height as usize];
    // libyuv ARGB is B, G, R, A in memory: cairo's RGB24 on little-endian.
    yuv_helper::i420_to_argb(
        y,
        stride_y,
        u,
        stride_u,
        v,
        stride_v,
        &mut pixels,
        stride,
        fit_width,
        fit_height,
    );
    Some(Picture { width: fit_width, height: fit_height, pixels })
}

/// Sends a remote share to the stage; the task is aborted when it stops being watched.
pub(super) fn feed(track: RemoteVideoTrack) -> tokio::task::JoinHandle<()> {
    reset_stage();
    tokio::spawn(async move {
        // The default queue holds one frame: a slow stage gets the newest picture.
        let mut stream = NativeVideoStream::new(track.rtc_track());
        while let Some(frame) = stream.next().await {
            stage().show(frame.buffer.as_ref());
        }
    })
}

/// Where the interface wants the video, in CSS pixels of the main window.
#[derive(Clone, Copy, Deserialize)]
struct ControlRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
#[derive(Clone, Copy, Deserialize)]
pub struct ViewRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    #[serde(default)]
    crt: bool,
    #[serde(default)]
    fill: bool,
    #[serde(default)]
    controls: Option<ControlRect>,
}

#[tauri::command]
pub fn matrix_screen_view(window: WebviewWindow, rect: Option<ViewRect>) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Janela inválida.".into());
    }
    let valid = |value: f64| value.is_finite() && (-16384.0..=16384.0).contains(&value);
    if rect.is_some_and(|r| {
        !(valid(r.x) && valid(r.y) && valid(r.width) && valid(r.height))
            || r.controls
                .is_some_and(|c| !(valid(c.x) && valid(c.y) && valid(c.width) && valid(c.height)))
    }) {
        return Err("Área de vídeo inválida.".into());
    }
    if rect.is_none() {
        stage().target.store(0, Ordering::Release);
    }
    #[cfg(target_os = "linux")]
    window
        .with_webview(move |view| {
            use gtk::prelude::*;
            layer::place(&view.inner().upcast::<gtk::Widget>(), rect);
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "linux")]
mod layer {
    use super::{stage, ViewRect};
    use gtk::{cairo, glib, prelude::*};
    use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering, time::Duration};

    struct Layer {
        area: gtk::DrawingArea,
        surface: Rc<RefCell<Option<cairo::ImageSurface>>>,
        timer: Option<glib::SourceId>,
        view: Rc<RefCell<Option<ViewRect>>>,
    }
    thread_local! {
        static LAYER: RefCell<Option<Layer>> = const { RefCell::new(None) };
    }

    fn clip_video(context: &cairo::Context, width: f64, height: f64, view: Option<ViewRect>) {
        if view.is_some_and(|v| v.crt) {
            // Same normalized Bezier contour as MatrixChatPanel's SVG.
            // GTK is above WebKit, so CSS clip-path cannot clip this layer.
            context.move_to(0.10 * width, 0.04 * height);
            for [x1, y1, x2, y2, x3, y3] in [
                [0.30, 0.013333, 0.70, 0.013333, 0.90, 0.04],
                [0.953333, 0.047111, 0.965333, 0.08, 0.972, 0.14],
                [0.992, 0.32, 0.992, 0.68, 0.972, 0.86],
                [0.965333, 0.92, 0.953333, 0.952889, 0.90, 0.96],
                [0.70, 0.986667, 0.30, 0.986667, 0.10, 0.96],
                [0.046667, 0.952889, 0.034667, 0.92, 0.028, 0.86],
                [0.008, 0.68, 0.008, 0.32, 0.028, 0.14],
                [0.034667, 0.08, 0.046667, 0.047111, 0.10, 0.04],
            ] {
                context.curve_to(
                    x1 * width,
                    y1 * height,
                    x2 * width,
                    y2 * height,
                    x3 * width,
                    y3 * height,
                );
            }
            context.close_path();
            context.clip();
        }
        if let Some(controls) = view.and_then(|v| v.controls) {
            // A transparent opening lets the original WebKit buttons float
            // above the picture while GTK input still passes through.
            context.rectangle(0.0, 0.0, width, height);
            context.rectangle(controls.x, controls.y, controls.width, controls.height);
            context.set_fill_rule(cairo::FillRule::EvenOdd);
            context.clip();
        }
    }

    /// The launcher webview inside a GTK Overlay (shared with the Nexus
    /// browser), with the video area above it. Input passes through the area:
    /// clicks reach the page underneath.
    fn create(webview: &gtk::Widget) -> Option<Layer> {
        let parent = webview.parent()?;
        let overlay = match parent.clone().downcast::<gtk::Overlay>() {
            Ok(overlay) => overlay,
            Err(_) => {
                let container = parent.downcast::<gtk::Box>().ok()?;
                container
                    .children()
                    .into_iter()
                    .find_map(|widget| widget.downcast::<gtk::Overlay>().ok())
                    .unwrap_or_else(|| {
                        let overlay = gtk::Overlay::new();
                        container.remove(webview);
                        overlay.add(webview);
                        container.pack_start(&overlay, true, true, 0);
                        overlay.show();
                        overlay
                    })
            }
        };
        let area = gtk::DrawingArea::new();
        area.set_halign(gtk::Align::Start);
        area.set_valign(gtk::Align::Start);
        area.connect_realize(|area| {
            // Overlay pass-through alone does not remove DrawingArea's own
            // GDK window from pointer hit-testing. The video must never own
            // mouse input, including inside the transparent control opening.
            if let Some(window) = area.window() {
                window.set_pass_through(true);
                window.input_shape_combine_region(&cairo::Region::create(), 0, 0);
            }
        });
        overlay.add_overlay(&area);
        overlay.set_overlay_pass_through(&area, true);
        area.connect_size_allocate(|area, allocation| {
            if !area.is_visible() {
                return;
            }
            let scale = area.scale_factor().max(1) as u64;
            let width = allocation.width().max(0) as u64 * scale;
            let height = allocation.height().max(0) as u64 * scale;
            stage()
                .target
                .store(width << 32 | height, Ordering::Release);
        });
        let surface: Rc<RefCell<Option<cairo::ImageSurface>>> = Rc::default();
        let image = surface.clone();
        let view: Rc<RefCell<Option<ViewRect>>> = Rc::default();
        let draw_view = view.clone();
        area.connect_draw(move |area, context| {
            let (width, height) = (
                area.allocated_width() as f64,
                area.allocated_height() as f64,
            );
            let view = *draw_view.borrow();
            clip_video(context, width, height, view);
            context.set_source_rgb(0.0, 0.0, 0.0);
            let _ = context.paint();
            if let Some(image) = image.borrow().as_ref() {
                let scale = area.scale_factor().max(1) as f64;
                let (image_width, image_height) =
                    (image.width() as f64 / scale, image.height() as f64 / scale);
                // Pictures arrive at the area's size; only a resize in
                // progress needs cairo to scale the last one.
                let fill = view.is_some_and(|v| v.fill);
                // Contain leaves a safe rectangle inside the curved CRT, so
                // the source's corners remain visible too.
                let margin = if view.is_some_and(|v| v.crt) && !fill {
                    0.86
                } else {
                    1.0
                };
                let fit = if fill {
                    (width / image_width).max(height / image_height)
                } else {
                    (width * margin / image_width).min(height * margin / image_height)
                };
                context.translate(
                    ((width - image_width * fit) / 2.0).round(),
                    ((height - image_height * fit) / 2.0).round(),
                );
                context.scale(fit / scale, fit / scale);
                let _ = context.set_source_surface(image, 0.0, 0.0);
                let _ = context.paint();
            }
            glib::Propagation::Stop
        });
        Some(Layer {
            area,
            surface,
            timer: None,
            view,
        })
    }

    /// Copies the newest picture into the area, about 60 times a second while shown.
    fn ticker(
        area: gtk::DrawingArea,
        surface: Rc<RefCell<Option<cairo::ImageSurface>>>,
    ) -> glib::SourceId {
        glib::timeout_add_local(Duration::from_millis(16), move || {
            let slot = stage();
            if slot.clear.swap(false, Ordering::AcqRel) {
                *surface.borrow_mut() = None;
                area.queue_draw();
            }
            let Some(picture) = slot.picture.lock().ok().and_then(|mut p| p.take()) else {
                return glib::ControlFlow::Continue;
            };
            let mut current = surface.borrow_mut();
            let reuse = current
                .as_ref()
                .is_some_and(|s| s.width() == picture.width && s.height() == picture.height);
            if !reuse {
                *current = cairo::ImageSurface::create(
                    cairo::Format::Rgb24,
                    picture.width,
                    picture.height,
                )
                .ok();
            }
            if let Some(image) = current.as_mut() {
                let stride = image.stride() as usize;
                let row = picture.width as usize * 4;
                if let Ok(mut data) = image.data() {
                    for (line, source) in picture.pixels.chunks_exact(row).enumerate() {
                        data[line * stride..line * stride + row].copy_from_slice(source);
                    }
                }
            }
            drop(current);
            area.queue_draw();
            glib::ControlFlow::Continue
        })
    }

    #[cfg(test)]
    mod clip_tests {
        use super::*;
        #[test]
        fn crt_corners_are_transparent_and_controls_have_an_opening() {
            let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1000, 600).unwrap();
            let context = cairo::Context::new(&surface).unwrap();
            clip_video(
                &context,
                1000.0,
                600.0,
                Some(ViewRect {
                    x: 0.0,
                    y: 0.0,
                    width: 1000.0,
                    height: 600.0,
                    crt: true,
                    fill: true,
                    controls: Some(super::super::ControlRect {
                        x: 70.0,
                        y: 500.0,
                        width: 860.0,
                        height: 50.0,
                    }),
                }),
            );
            assert!(!context.in_clip(5.0, 5.0).unwrap());
            assert!(context.in_clip(500.0, 25.0).unwrap());
            assert!(context.in_clip(500.0, 300.0).unwrap());
            assert!(!context.in_clip(500.0, 525.0).unwrap());
            assert!(context.in_clip(500.0, 480.0).unwrap());
        }
    }

    pub(super) fn place(webview: &gtk::Widget, rect: Option<ViewRect>) {
        LAYER.with(|cell| {
            let mut cell = cell.borrow_mut();
            if cell.is_none() {
                if rect.is_none() {
                    return;
                }
                *cell = create(webview);
            }
            let Some(layer) = cell.as_mut() else {
                return;
            };
            match rect {
                Some(rect) => {
                    *layer.view.borrow_mut() = Some(rect);
                    // Margins cannot be negative: a rectangle past the top or
                    // left edge loses that part instead.
                    let (x, y) = (rect.x.round() as i32, rect.y.round() as i32);
                    let width = (rect.width.round() as i32 + x.min(0)).max(1);
                    let height = (rect.height.round() as i32 + y.min(0)).max(1);
                    layer.area.set_margin_start(x.max(0));
                    layer.area.set_margin_top(y.max(0));
                    layer.area.set_size_request(width, height);
                    layer.area.queue_draw();
                    if !layer.area.is_visible() {
                        layer.area.show();
                    }
                    if layer.timer.is_none() {
                        layer.timer = Some(ticker(layer.area.clone(), layer.surface.clone()));
                    }
                }
                None => {
                    *layer.view.borrow_mut() = None;
                    layer.area.hide();
                    if let Some(timer) = layer.timer.take() {
                        timer.remove();
                    }
                    *layer.surface.borrow_mut() = None;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{fit, ShareQuality};

    #[test]
    fn quality_choices_and_fallback() {
        let q = |height, fps| ShareQuality { height, fps, auto: false };
        assert_eq!((q(720, 30).size(), q(720, 30).bitrate()), ((1280, 720), 2_000_000));
        assert_eq!(q(720, 60).bitrate(), 3_500_000);
        assert_eq!(q(1080, 30).bitrate(), 3_500_000);
        assert_eq!((q(1080, 60).size(), q(1080, 60).bitrate()), ((1920, 1080), 6_000_000));
        assert_eq!(q(1080, 60).interval().as_micros(), 16_666);
        // Values the interface never offers become the default.
        assert_eq!(q(2160, 144).checked(), ShareQuality::default());
        assert_eq!(q(720, 60).checked(), q(720, 60));
        let auto = ShareQuality { height: 720, fps: 60, auto: true }.checked();
        assert_eq!((auto.size(), auto.fps, auto.bitrate()), ((1920, 1080), 30, 3_500_000));
        assert!(serde_json::from_str::<ShareQuality>(r#"{"height":1080,"fps":30,"auto":true}"#).unwrap().auto);
        assert_eq!(serde_json::from_str::<ShareQuality>(r#"{"height":720,"fps":60}"#).unwrap(), q(720, 60));
    }

    #[test]
    fn fit_keeps_aspect_and_even_sizes() {
        assert_eq!(fit(1920, 1080, 1920, 1080), (1920, 1080));
        assert_eq!(fit(3840, 2160, 1920, 1080), (1920, 1080));
        assert_eq!(fit(2560, 1440, 1920, 1080), (1920, 1080));
        assert_eq!(fit(3440, 1440, 1920, 1080), (1920, 802));
        assert_eq!(fit(1280, 720, 1920, 1080), (1280, 720));
        assert_eq!(fit(1001, 3, 1920, 1080), (1000, 2));
    }
}
