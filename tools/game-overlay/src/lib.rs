//! Opt-in in-process Vulkan renderer. No desktop window or global input grab.
#![cfg(target_os = "linux")]
#[cfg(not(target_arch = "x86_64"))]
compile_error!("O renderer inicial suporta Linux x86_64.");
mod controls;
mod focus_fps;
mod tools;
mod input;
mod layer;
mod panel;
mod render;
#[path = "../shortcut.rs"]
mod shortcut;
mod symbols;
mod wayland;
mod wayland_surface;
mod wineland;
mod wine_input;
mod x11;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Mutex, OnceLock,
};
static INPUT: OnceLock<Mutex<input::Input>> = OnceLock::new();
static CONTROLLER: OnceLock<Option<tools::Tools>> = OnceLock::new();
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Target {
    #[default]
    Unavailable,
    Wayland(usize),
    Xlib(u64),
}
static TARGET: OnceLock<Mutex<Target>> = OnceLock::new();
static FOCUS: AtomicUsize = AtomicUsize::new(0);
static LAST_PRESENT: OnceLock<Mutex<std::time::Instant>> = OnceLock::new();
static LAST_DRAW: OnceLock<Mutex<std::time::Instant>> = OnceLock::new();
pub(crate) fn drawn() {
    if let Ok(mut time) = LAST_DRAW
        .get_or_init(|| Mutex::new(std::time::Instant::now()))
        .lock()
    {
        *time = std::time::Instant::now();
    }
}
pub(crate) fn last_draw_age() -> std::time::Duration {
    LAST_DRAW
        .get()
        .and_then(|t| t.lock().ok().map(|t| t.elapsed()))
        .unwrap_or(std::time::Duration::MAX)
}
pub(crate) fn presented() {
    if let Ok(mut t) = LAST_PRESENT
        .get_or_init(|| Mutex::new(std::time::Instant::now()))
        .lock()
    {
        *t = std::time::Instant::now();
    }
}
pub(crate) fn last_present_age() -> std::time::Duration {
    LAST_PRESENT
        .get()
        .and_then(|t| t.lock().ok().map(|t| t.elapsed()))
        .unwrap_or(std::time::Duration::MAX)
}
/// Never keep input captured if presentation or menu submission stops.
pub(crate) fn watchdog() {
    let grace = std::time::Duration::from_secs(1);
    let idle = last_present_age() > grace;
    let drawing_idle = last_draw_age() > grace;
    let mut release_pointer = false;
    if let Ok(mut state) = state().lock() {
        if idle || (state.visible && state.visibility_age() > grace && drawing_idle) {
            state.ready = false;
            state.release();
            release_pointer = true;
        }
    }
    if release_pointer {
        x11::suspend_pointer();
    }
}
pub(crate) fn state() -> &'static Mutex<input::Input> {
    INPUT.get_or_init(Default::default)
}
pub(crate) fn target() -> Target {
    TARGET
        .get_or_init(Default::default)
        .lock()
        .map(|t| *t)
        .unwrap_or_default()
}
pub(crate) fn set_target(target: Target) {
    let changed = TARGET
        .get_or_init(Default::default)
        .lock()
        .map(|mut t| {
            let changed = *t != target;
            *t = target;
            changed
        })
        .unwrap_or(false);
    if changed {
        x11::suspend_pointer();
        let focused = match target {
            Target::Wayland(surface) => {
                wayland_surface::focused(surface, FOCUS.load(Ordering::Acquire))
            }
            Target::Xlib(window) => x11::focused(window),
            Target::Unavailable => false,
        };
        if let Ok(mut s) = state().lock() {
            s.focus(focused);
        }
        tools::focus(focused);
    }
}
pub(crate) fn input_available() -> bool {
    match target() {
        Target::Wayland(_) => wayland::available(),
        Target::Xlib(window) => x11::available(window),
        Target::Unavailable => false,
    }
}
pub(crate) fn set_focus(surface: usize) {
    FOCUS.store(surface, Ordering::Release);
    refresh_wayland_focus();
}
pub(crate) fn clear_focus(surface: usize) {
    let _ = FOCUS.compare_exchange(surface, 0, Ordering::AcqRel, Ordering::Acquire);
    refresh_wayland_focus();
}
pub(crate) fn refresh_wayland_focus() {
    if let Target::Wayland(surface) = target() {
        let focused = wayland_surface::focused(surface, FOCUS.load(Ordering::Acquire));
        let mut changed = false;
        if let Ok(mut s) = state().lock() {
            changed = s.focused != focused;
            s.focus(focused);
        }
        if changed {
            tools::focus(focused);
            report(if focused {
                "Foco Wayland reconhecido na superfície do jogo ou em sua janela pai."
            } else {
                "Foco Wayland fora da superfície do jogo; captura suspensa."
            });
        }
    }
}
pub(crate) fn controller() -> Option<&'static tools::Tools> {
    CONTROLLER
        .get_or_init(|| {
            let shortcut = std::env::var("PELI_OVERLAY_SHORTCUT").unwrap_or_else(|_| "F8".into());
            if let Ok(mut s) = state().lock() {
                if let Err(e) = s.shortcut(&shortcut) {
                    report(&e);
                    return None;
                }
            }
            match tools::Tools::new() {
                Ok(tools) => Some(tools),
                Err(error) => { report(&error); None }
            }
        })
        .as_ref()
}
pub(crate) fn uptime() -> f64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
}
pub(crate) fn report(message: &str) {
    // Log transitions, not every present. Never log raw input or credentials.
    static LAST: OnceLock<Mutex<String>> = OnceLock::new();
    if let Ok(mut last) = LAST.get_or_init(Default::default).lock() {
        if *last != message {
            eprintln!("[PeliGames overlay] {message}");
            *last = message.into();
        }
    }
}
pub(crate) fn nonce() -> String {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let mut bytes = [0u8; 16];
    let result =
        unsafe { libc::getrandom(bytes.as_mut_ptr().cast(), bytes.len(), libc::GRND_NONBLOCK) };
    if result != bytes.len() as isize {
        let clock = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        bytes = (clock
            ^ ((std::process::id() as u128) << 64)
            ^ COUNT.fetch_add(1, Ordering::Relaxed) as u128)
            .to_le_bytes();
    }
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
