//! Initial Wine/Proton Xlib event path, including games on XWayland.
//! Deliberately uses Wine's nonblocking XCheckIfEvent + XGetEventData contract.
//! Native Xlib/XCB clients are NOT enabled by guessing that their loops match.
//! The native pointer lease lives on Wine's existing connection, is scoped to
//! its game window, and is released on close/focus loss/watchdog/destruction.
#[path = "x11_pointer.rs"]
mod pointer;
use crate::{input::Route, state, Target};
use libc::{c_char, c_int, c_ulong, c_void};
use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Any {
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut c_void,
    window: c_ulong,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Key {
    any: Any,
    root: c_ulong,
    child: c_ulong,
    time: c_ulong,
    x: c_int,
    y: c_int,
    root_x: c_int,
    root_y: c_int,
    modifiers: u32,
    detail: u32, // keycode / button; XMotionEvent's is_hint is not read here
    same_screen: c_int,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Focus {
    any: Any,
    mode: c_int,
    detail: c_int,
}
#[repr(C)]
pub union Event {
    any: Any,
    key: Key,
    focus: Focus,
    storage: [c_ulong; 24],
}
#[repr(C)]
pub struct Cookie {
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut c_void,
    extension: c_int,
    event_type: c_int,
    cookie: u32,
    data: *mut c_void,
}
// Prefixes from libXi's public XInput2.h (not XCB's wire structs).
#[repr(C)]
struct XiHeader {
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut c_void,
    extension: c_int,
    event_type: c_int,
    time: c_ulong,
    device: c_int,
    source: c_int,
    detail: c_int,
}
#[repr(C)]
struct Valuators {
    length: c_int,
    mask: *const u8,
    values: *const f64,
}
#[repr(C)]
struct Raw {
    // Do not embed XiHeader: its trailing alignment padding would move flags
    // from byte 60 to 64, corrupting the valuator pointers below it.
    kind: c_int,
    serial: c_ulong,
    send_event: c_int,
    display: *mut c_void,
    extension: c_int,
    event_type: c_int,
    time: c_ulong,
    device: c_int,
    source: c_int,
    detail: c_int,
    flags: c_int,
    valuators: Valuators,
    raw_values: *const f64,
}
#[repr(C)]
struct Device {
    header: XiHeader,
    root: c_ulong,
    window: c_ulong,
    child: c_ulong,
    root_x: f64,
    root_y: f64,
    x: f64,
    y: f64,
    // Rest of XIDeviceEvent is intentionally never accessed.
}
type Predicate = Option<unsafe extern "C" fn(*mut c_void, *mut Event, *mut c_char) -> c_int>;
type Check = unsafe extern "C" fn(*mut c_void, *mut Event, Predicate, *mut c_char) -> c_int;
type GetData = unsafe extern "C" fn(*mut c_void, *mut Cookie) -> c_int;
type FreeData = unsafe extern "C" fn(*mut c_void, *mut Cookie);
type QueryKeys = unsafe extern "C" fn(*mut c_void, *mut c_char) -> c_int;

#[derive(Clone)]
struct Window {
    display: usize,
    ancestors: Arc<[u64]>,
    xi_opcode: c_int,
    references: usize,
}
static WINDOWS: OnceLock<Mutex<HashMap<u64, Window>>> = OnceLock::new();
static FOCUS: AtomicU64 = AtomicU64::new(0);
static CHECK_SEEN: AtomicBool = AtomicBool::new(false);
fn windows() -> &'static Mutex<HashMap<u64, Window>> {
    WINDOWS.get_or_init(Default::default)
}
fn wine() -> bool {
    if !crate::process_scope::enabled() { return false; }
    static WINE: OnceLock<bool> = OnceLock::new();
    *WINE.get_or_init(|| {
        let mut maps = String::new();
        std::fs::File::open("/proc/self/maps")
            .and_then(|file| file.take(2 * 1024 * 1024).read_to_string(&mut maps))
            .is_ok()
            && maps.lines().any(|line| line.contains("/winex11.drv"))
    })
}
unsafe fn symbol<T: Copy>(name: &'static [u8]) -> Option<T> {
    let address = crate::symbols::resolve(name);
    (!address.is_null()).then(|| std::mem::transmute_copy(&address))
}
fn current() -> Option<(u64, Window)> {
    let Target::Xlib(window) = crate::target() else {
        return None;
    };
    windows()
        .lock()
        .ok()?
        .get(&window)
        .cloned()
        .map(|target| (window, target))
}
pub fn available(window: u64) -> bool {
    CHECK_SEEN.load(Ordering::Acquire)
        && pointer::available()
        && windows().lock().is_ok_and(|w| w.contains_key(&window))
}
pub(crate) fn cursor_hidden(window: u64) -> bool {
    pointer::hidden_for(window)
}
pub(crate) fn suspend_pointer() {
    pointer::suspend();
}
pub fn focused(window: u64) -> bool {
    let focused = FOCUS.load(Ordering::Acquire);
    focused != 0
        && windows().lock().is_ok_and(|w| {
            w.get(&window)
                .is_some_and(|target| target.ancestors.contains(&focused))
        })
}
fn focus(window: u64) {
    FOCUS.store(window, Ordering::Release);
    if let Target::Xlib(active) = crate::target() {
        let focused = focused(active);
        if let Ok(mut s) = state().lock() {
            s.focus(focused);
        }
        crate::tools::focus(focused);
    }
}
pub unsafe fn register(display: *mut c_void, window: u64) -> bool {
    if display.is_null() || window == 0 || !wine() {
        return false;
    }
    // Fail open if the complete event/cookie forwarding ABI cannot be found.
    if symbol::<Check>(b"XCheckIfEvent\0").is_none()
        || symbol::<GetData>(b"XGetEventData\0").is_none()
        || symbol::<FreeData>(b"XFreeEventData\0").is_none()
        || symbol::<QueryKeys>(b"XQueryKeymap\0").is_none()
    {
        return false;
    }
    type QueryTree = unsafe extern "C" fn(
        *mut c_void,
        c_ulong,
        *mut c_ulong,
        *mut c_ulong,
        *mut *mut c_ulong,
        *mut u32,
    ) -> c_int;
    type Free = unsafe extern "C" fn(*mut c_void) -> c_int;
    type Extension = unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        *mut c_int,
        *mut c_int,
        *mut c_int,
    ) -> c_int;
    type FocusQuery = unsafe extern "C" fn(*mut c_void, *mut c_ulong, *mut c_int) -> c_int;
    let (Some(query), Some(free), Some(extension), Some(get_focus)) = (
        symbol::<QueryTree>(b"XQueryTree\0"),
        symbol::<Free>(b"XFree\0"),
        symbol::<Extension>(b"XQueryExtension\0"),
        symbol::<FocusQuery>(b"XGetInputFocus\0"),
    ) else {
        return false;
    };
    let mut ancestors = vec![window];
    let mut child = window;
    // Wine's Vulkan child and the top-level window receiving keys can differ.
    // Never include the root: that would consume other windows' input.
    for _ in 0..16 {
        let (mut root, mut parent, mut children, mut count) = (0, 0, std::ptr::null_mut(), 0);
        let result = query(
            display,
            child,
            &mut root,
            &mut parent,
            &mut children,
            &mut count,
        );
        if !children.is_null() {
            free(children.cast());
        }
        if result == 0 {
            return false;
        }
        if parent == 0 || parent == root || ancestors.contains(&parent) {
            break;
        }
        ancestors.push(parent);
        child = parent;
    }
    let (mut opcode, mut first_event, mut error) = (0, 0, 0);
    if extension(
        display,
        c"XInputExtension".as_ptr(),
        &mut opcode,
        &mut first_event,
        &mut error,
    ) == 0
    {
        opcode = 0;
    }
    let (mut focused, mut revert) = (0, 0);
    get_focus(display, &mut focused, &mut revert);
    if let Ok(mut w) = windows().lock() {
        if w.len() >= 64 && !w.contains_key(&window) {
            return false;
        }
        w.entry(window)
            .and_modify(|w| w.references += 1)
            .or_insert(Window {
                display: display as usize,
                ancestors: ancestors.into(),
                xi_opcode: opcode,
                references: 1,
            });
    } else {
        return false;
    }
    focus(focused);
    true
}
pub fn unregister(window: u64) {
    if let Ok(mut w) = windows().lock() {
        if let Some(target) = w.get_mut(&window) {
            target.references -= 1;
            if target.references == 0 {
                w.remove(&window);
            }
        }
    }
    if crate::target() == Target::Xlib(window) {
        if let Ok(mut s) = state().lock() {
            s.ready = false;
            s.release();
        }
        pointer::suspend();
    }
}
fn destroyed_window(window: u64) {
    let removed = windows()
        .lock()
        .map(|mut map| {
            let active = crate_window();
            let removed = map
                .get(&active)
                .is_some_and(|w| w.ancestors.contains(&window));
            map.retain(|_, w| !w.ancestors.contains(&window));
            removed
        })
        .unwrap_or(false);
    if removed {
        release_target();
    }
}
fn closed_display(display: usize) {
    let removed = windows()
        .lock()
        .map(|mut map| {
            let removed = map
                .get(&crate_window())
                .is_some_and(|w| w.display == display);
            map.retain(|_, w| w.display != display);
            removed
        })
        .unwrap_or(false);
    if removed {
        release_target();
    }
}
fn release_target() {
    if let Ok(mut s) = state().lock() {
        s.ready = false;
        s.release();
    }
    pointer::suspend();
}
unsafe fn snapshot_keys(display: *mut c_void) {
    if let Some(query) = symbol::<QueryKeys>(b"XQueryKeymap\0") {
        let mut map = [0u8; 32];
        query(display, map.as_mut_ptr().cast());
        let keys: Vec<u32> = (8..256u32)
            .filter(|key| map[*key as usize / 8] & (1 << (*key % 8)) != 0)
            .map(|key| key - 8)
            .collect();
        if let Ok(mut s) = state().lock() {
            s.seed_keys(&keys);
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
struct Stamp {
    kind: u8,
    detail: u32,
    down: bool,
    time: u64,
}
// Raw and cooked delivery of the same physical press must not click twice or
// suppress a release that a different input path has already given the game.
fn once(
    stamp: Stamp,
    action: impl FnOnce(&mut crate::input::Input, Option<Route>) -> Route,
) -> Route {
    static RECENT: OnceLock<Mutex<VecDeque<(Stamp, Route)>>> = OnceLock::new();
    let Ok(mut recent) = RECENT.get_or_init(Default::default).lock() else {
        return Route::Game;
    };
    // In the core X11 protocol autorepeat can be Release+Press with the same
    // server timestamp. Recognize the pair, rather than querying the CURRENT
    // physical state (which can be newer than events waiting in the queue).
    let repeat = (stamp.kind == 0 && stamp.down)
        .then(|| {
            recent
                .iter()
                .rev()
                .find(|(s, _)| {
                    s.kind == 0 && !s.down && s.detail == stamp.detail && s.time == stamp.time
                })
                .map(|(_, route)| *route)
        })
        .flatten();
    if let Some((_, route)) = recent.iter().rev().find(|(s, _)| *s == stamp) {
        if let Some(repeated) = repeat {
            if let Some(key) = stamp.detail.checked_sub(8) {
                if let Ok(mut s) = state().lock() {
                    s.x11_repeat(key, repeated);
                }
            }
        }
        return *route;
    }
    let route = state()
        .lock()
        .map(|mut s| action(&mut s, repeat))
        .unwrap_or(Route::Game);
    if recent.len() == 64 {
        recent.pop_front();
    }
    recent.push_back((stamp, route));
    route
}
fn key(detail: u32, down: bool, time: u64) -> Route {
    let Some(key) = detail.checked_sub(8).filter(|key| *key < 248) else {
        return Route::Game;
    };
    once(
        Stamp {
            kind: 0,
            detail,
            down,
            time,
        },
        |s, repeat| match repeat {
            Some(route) => s.x11_repeat(key, route),
            None => s.key(key, down),
        },
    )
}
fn button(detail: u32, down: bool, time: u64) -> Route {
    once(
        Stamp {
            kind: 1,
            detail,
            down,
            time,
        },
        |s, _| {
            if (4..=7).contains(&detail) {
                if down {
                    s.scroll(
                        match detail {
                            6 => 28.,
                            7 => -28.,
                            _ => 0.,
                        },
                        match detail {
                            4 => -28.,
                            5 => 28.,
                            _ => 0.,
                        },
                    )
                } else if s.capturing() {
                    Route::Overlay
                } else {
                    Route::Game
                }
            } else {
                let button = match detail {
                    1 => 272,
                    2 => 274,
                    3 => 273,
                    n if n >= 8 => 275 + n - 8,
                    _ => return Route::Game,
                };
                s.button(button, down)
            }
        },
    )
}
fn raw_button(detail: u32, down: bool, time: u64) -> Route {
    once(
        Stamp {
            kind: 2,
            detail,
            down,
            time,
        },
        |s, _| s.raw_button(detail, down),
    )
}
unsafe fn core(event: &Event, display: *mut c_void) -> Route {
    let Some((_, target)) = current() else {
        return Route::Game;
    };
    let any = event.any;
    let pointer_event = matches!(any.kind, 4..=6);
    if !target.ancestors.contains(&any.window)
        && !(pointer_event && focused(crate_window()) && pointer::accepts(display, any.window))
    {
        return Route::Game;
    }
    match any.kind {
        9 | 10 if event.focus.mode == 0 || event.focus.mode == 3 => {
            type GetFocus = unsafe extern "C" fn(*mut c_void, *mut c_ulong, *mut c_int) -> c_int;
            if let Some(get) = symbol::<GetFocus>(b"XGetInputFocus\0") {
                let (mut window, mut revert) = (0, 0);
                get(display, &mut window, &mut revert);
                focus(window);
                if target.ancestors.contains(&window) && any.kind == 9 {
                    snapshot_keys(display);
                }
            }
            Route::Game
        }
        2 | 3 => {
            let e = event.key;
            key(e.detail, any.kind == 2, e.time)
        }
        4 | 5 => {
            let e = event.key;
            if pointer::active() {
                pointer::motion(display, any.window, e.x, e.y);
            }
            button(e.detail, any.kind == 4, e.time)
        }
        6 => {
            let e = event.key;
            if pointer::active() {
                pointer::motion(display, any.window, e.x, e.y);
                return Route::Overlay;
            }
            state()
                .lock()
                .map(|mut s| s.absolute(e.x as f32, e.y as f32))
                .unwrap_or(Route::Game)
        }
        22 => {
            pointer::resized(any.window);
            Route::Game
        }
        _ => Route::Game,
    }
}
fn crate_window() -> u64 {
    match crate::target() {
        Target::Xlib(window) => window,
        _ => 0,
    }
}
#[no_mangle]
pub unsafe extern "C" fn XCheckIfEvent(
    display: *mut c_void,
    event: *mut Event,
    predicate: Predicate,
    argument: *mut c_char,
) -> c_int {
    let Some(real) = symbol::<Check>(b"XCheckIfEvent\0") else {
        return 0;
    };
    if event.is_null() || !wine() {
        return real(display, event, predicate, argument);
    }
    CHECK_SEEN.store(true, Ordering::Release);
    pointer::reconcile(display);
    // Call the application's predicate unchanged, outside all overlay locks.
    // Draining captured events is valid for this NONBLOCKING API. Do not apply
    // the same loop blindly to XNextEvent following an XPending count.
    for _ in 0..512 {
        let result = real(display, event, predicate, argument);
        let route = if result == 0 {
            Route::Game
        } else {
            core(&*event, display)
        };
        pointer::reconcile(display);
        if result == 0 || route == Route::Game {
            return result;
        }
    }
    // Bound work during an input flood; leave later events queued for next pump.
    0
}
unsafe fn xi(cookie: &Cookie) -> Route {
    let Some((_, target)) = current() else {
        return Route::Game;
    };
    if cookie.extension != target.xi_opcode || target.xi_opcode == 0 || cookie.data.is_null() {
        return Route::Game;
    }
    let h = &*cookie.data.cast::<XiHeader>();
    if h.extension != cookie.extension || h.event_type != cookie.event_type {
        return Route::Game;
    }
    match cookie.event_type {
        2..=6 => {
            let e = &*cookie.data.cast::<Device>();
            if !target.ancestors.contains(&e.window) && !pointer::accepts(cookie.display, e.window)
            {
                return Route::Game;
            }
            match cookie.event_type {
                2 | 3 => key(h.detail as u32, cookie.event_type == 2, h.time),
                4 | 5 => {
                    if pointer::active() {
                        pointer::motion(cookie.display, e.window, e.x as i32, e.y as i32);
                    }
                    button(h.detail as u32, cookie.event_type == 4, h.time)
                }
                _ if pointer::active() => {
                    pointer::motion(cookie.display, e.window, e.x as i32, e.y as i32);
                    Route::Overlay
                }
                _ => state()
                    .lock()
                    .map(|mut s| s.absolute(e.x as f32, e.y as f32))
                    .unwrap_or(Route::Game),
            }
        }
        13..=17
            if focused(match crate::target() {
                Target::Xlib(w) => w,
                _ => 0,
            }) =>
        {
            match cookie.event_type {
                13 | 14 => key(h.detail as u32, cookie.event_type == 13, h.time),
                15 | 16 => raw_button(h.detail as u32, cookie.event_type == 15, h.time),
                _ if pointer::active() => Route::Overlay,
                _ => {
                    let e = &*cookie.data.cast::<Raw>();
                    let Some((dx, dy)) = motion(&e.valuators) else {
                        if let Ok(mut s) = state().lock() {
                            s.ready = false;
                            s.release();
                        }
                        return Route::Game;
                    };
                    state()
                        .lock()
                        .map(|mut s| s.relative(dx, dy))
                        .unwrap_or(Route::Game)
                }
            }
        }
        _ => Route::Game,
    }
}
unsafe fn motion(values: &Valuators) -> Option<(f32, f32)> {
    if values.length <= 0 || values.length > 32 || values.mask.is_null() || values.values.is_null()
    {
        return None;
    }
    let mask = *values.mask;
    let dx = if mask & 1 != 0 { *values.values } else { 0. };
    let dy = if mask & 2 != 0 {
        *values.values.add(usize::from(mask & 1 != 0))
    } else {
        0.
    };
    if !dx.is_finite() || !dy.is_finite() || dx.abs() > 100_000. || dy.abs() > 100_000. {
        return None;
    }
    Some((dx as f32, dy as f32))
}
#[no_mangle]
pub unsafe extern "C" fn XGetEventData(display: *mut c_void, cookie: *mut Cookie) -> c_int {
    let Some(real) = symbol::<GetData>(b"XGetEventData\0") else {
        return 0;
    };
    let result = real(display, cookie);
    if result == 0 || cookie.is_null() || !wine() {
        return result;
    }
    let route = xi(&*cookie);
    pointer::reconcile(display);
    if route == Route::Overlay {
        // Wine explicitly accepts a failed lookup/null data in GenericEvent.
        // Free exactly once here; its existing free_event_data skips null.
        if let Some(free) = symbol::<FreeData>(b"XFreeEventData\0") {
            free(display, cookie);
            (*cookie).data = std::ptr::null_mut();
            return 0;
        }
    }
    result
}
#[no_mangle]
pub unsafe extern "C" fn XQueryKeymap(display: *mut c_void, keys: *mut c_char) -> c_int {
    let Some(real) = symbol::<QueryKeys>(b"XQueryKeymap\0") else {
        return 0;
    };
    let result = real(display, keys);
    if crate::process_scope::enabled() && !keys.is_null() && current().is_some() {
        if let Ok(s) = state().lock() {
            s.filter_x11_keymap(&mut *keys.cast::<[u8; 32]>());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xlib_and_xinput_are_not_xcb_wire_structures() {
        assert_eq!(std::mem::size_of::<Event>(), 192);
        assert_eq!(std::mem::size_of::<Any>(), 40);
        assert_eq!(std::mem::size_of::<Key>(), 96);
        assert_eq!(std::mem::size_of::<Cookie>(), 56);
        assert_eq!(std::mem::offset_of!(XiHeader, detail), 56);
        assert_eq!(std::mem::offset_of!(Device, window), 72);
        assert_eq!(std::mem::size_of::<Raw>(), 96);
        assert_eq!(std::mem::offset_of!(Raw, flags), 60);
        assert_eq!(std::mem::offset_of!(Raw, valuators), 64);
    }
    #[test]
    fn sparse_raw_axes_have_packed_values() {
        let mask = 2u8;
        let values = [4.5f64];
        let data = Valuators {
            length: 1,
            mask: &mask,
            values: values.as_ptr(),
        };
        assert_eq!(unsafe { motion(&data) }, Some((0., 4.5)));
    }
    #[test]
    fn invalid_raw_coordinates_fail_open() {
        let mask = 3u8;
        let values = [f64::NAN, 1.];
        let data = Valuators {
            length: 1,
            mask: &mask,
            values: values.as_ptr(),
        };
        assert_eq!(unsafe { motion(&data) }, None);
    }
}
