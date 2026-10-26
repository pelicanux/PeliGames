//! Rust adaptation of the event-routing approach documented by vkDaedalus.
//! No extra Wayland connection, pointer constraint, GTK object, or seat grab.
//! The borrowed game's listeners keep their original data and callbacks.
use crate::{input::Route, state};
use libc::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    collections::HashMap,
    ffi::CStr,
    sync::{Mutex, OnceLock},
};
static INCOMPLETE: AtomicBool = AtomicBool::new(false);
fn unavailable() {
    INCOMPLETE.store(true, Ordering::Release);
    if let Ok(mut s) = state().lock() {
        s.ready = false;
        s.release();
    }
    crate::report("Rota de entrada Wayland não suportada; captura desativada.");
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Pointer,
    Keyboard,
    Relative,
}
#[derive(Clone, Copy)]
struct Listener {
    kind: Kind,
    functions: [usize; 11],
}
static LISTENERS: OnceLock<Mutex<HashMap<usize, Listener>>> = OnceLock::new();
fn listeners() -> &'static Mutex<HashMap<usize, Listener>> {
    LISTENERS.get_or_init(Default::default)
}
fn original(proxy: *mut c_void, index: usize) -> Option<usize> {
    let map = listeners().lock().ok()?;
    let l = map.get(&(proxy as usize))?;
    let f = *l.functions.get(index)?;
    (f != 0).then_some(f)
}
pub unsafe fn symbol(name: &'static [u8]) -> *mut c_void {
    crate::symbols::resolve(name)
}
fn route(f: impl FnOnce(&mut crate::input::Input) -> Route) -> bool {
    if !crate::process_scope::enabled() || !matches!(crate::target(), crate::Target::Wayland(_)) {
        return true;
    }
    state()
        .lock()
        .map(|mut s| f(&mut s) == Route::Game)
        .unwrap_or(true)
}
fn focus(surface: *mut c_void) {
    if crate::process_scope::enabled() { crate::set_focus(surface as usize); }
}
macro_rules! forward {
    ($data:expr,$proxy:expr,$index:expr,($($ty:ty),*),($($arg:expr),*)) => {
        if let Some(function)=original($proxy,$index) {
            let function:unsafe extern "C" fn(*mut c_void,*mut c_void,$($ty),*)=std::mem::transmute(function);
            function($data,$proxy,$($arg),*);
        }
    };
}
unsafe extern "C" fn p_enter(
    data: *mut c_void,
    p: *mut c_void,
    serial: u32,
    surface: *mut c_void,
    x: i32,
    y: i32,
) {
    forward!(
        data,
        p,
        0,
        (u32, *mut c_void, i32, i32),
        (serial, surface, x, y)
    );
}
unsafe extern "C" fn p_leave(data: *mut c_void, p: *mut c_void, serial: u32, surface: *mut c_void) {
    forward!(data, p, 1, (u32, *mut c_void), (serial, surface));
}
unsafe extern "C" fn p_motion(data: *mut c_void, p: *mut c_void, time: u32, x: i32, y: i32) {
    if route(|s| s.absolute(x as f32 / 256., y as f32 / 256.)) {
        forward!(data, p, 2, (u32, i32, i32), (time, x, y));
    }
}
unsafe extern "C" fn p_button(
    data: *mut c_void,
    p: *mut c_void,
    serial: u32,
    time: u32,
    button: u32,
    value: u32,
) {
    if route(|s| s.button(button, value != 0)) {
        forward!(
            data,
            p,
            3,
            (u32, u32, u32, u32),
            (serial, time, button, value)
        );
    }
}
unsafe extern "C" fn p_axis(data: *mut c_void, p: *mut c_void, time: u32, axis: u32, value: i32) {
    if route(|s| {
        s.scroll(
            if axis == 1 { value as f32 / 256. } else { 0. },
            if axis == 0 { value as f32 / 256. } else { 0. },
        )
    }) {
        forward!(data, p, 4, (u32, u32, i32), (time, axis, value));
    }
}
unsafe extern "C" fn p_frame(data: *mut c_void, p: *mut c_void) {
    forward!(data, p, 5, (), ());
}
unsafe extern "C" fn p_source(data: *mut c_void, p: *mut c_void, source: u32) {
    if route(|s| {
        if s.capturing() {
            Route::Overlay
        } else {
            Route::Game
        }
    }) {
        forward!(data, p, 6, (u32), (source));
    }
}
unsafe extern "C" fn p_stop(data: *mut c_void, p: *mut c_void, time: u32, axis: u32) {
    if route(|s| {
        if s.capturing() {
            Route::Overlay
        } else {
            Route::Game
        }
    }) {
        forward!(data, p, 7, (u32, u32), (time, axis));
    }
}
unsafe extern "C" fn p_discrete(data: *mut c_void, p: *mut c_void, axis: u32, value: i32) {
    // Continuous axis is the single source of scrolling; do not count a
    // discrete/value120 companion twice in egui.
    if route(|s| {
        if s.capturing() {
            Route::Overlay
        } else {
            Route::Game
        }
    }) {
        forward!(data, p, 8, (u32, i32), (axis, value));
    }
}
unsafe extern "C" fn p_value120(data: *mut c_void, p: *mut c_void, axis: u32, value: i32) {
    if route(|s| {
        if s.capturing() {
            Route::Overlay
        } else {
            Route::Game
        }
    }) {
        forward!(data, p, 9, (u32, i32), (axis, value));
    }
}
unsafe extern "C" fn p_direction(data: *mut c_void, p: *mut c_void, axis: u32, value: u32) {
    if route(|s| {
        if s.capturing() {
            Route::Overlay
        } else {
            Route::Game
        }
    }) {
        forward!(data, p, 10, (u32, u32), (axis, value));
    }
}
unsafe extern "C" fn k_keymap(data: *mut c_void, p: *mut c_void, format: u32, fd: i32, size: u32) {
    // Pass ownership of the fd exactly as received. Never inspect typed text.
    forward!(data, p, 0, (u32, i32, u32), (format, fd, size));
}
unsafe extern "C" fn k_enter(
    data: *mut c_void,
    p: *mut c_void,
    serial: u32,
    surface: *mut c_void,
    keys: *mut c_void,
) {
    focus(surface);
    #[repr(C)]
    struct Array {
        size: usize,
        allocation: usize,
        data: *const u32,
    }
    if !keys.is_null() && !matches!(crate::target(), crate::Target::Xlib(_)) {
        let keys = &*(keys as *const Array);
        if keys.size <= 4096 && keys.size % 4 == 0 && (keys.size == 0 || !keys.data.is_null()) {
            let values = if keys.size == 0 {
                &[][..]
            } else {
                std::slice::from_raw_parts(keys.data, keys.size / 4)
            };
            if let Ok(mut s) = state().lock() {
                s.seed_keys(values);
            }
        }
    }
    forward!(
        data,
        p,
        1,
        (u32, *mut c_void, *mut c_void),
        (serial, surface, keys)
    );
}
unsafe extern "C" fn k_leave(data: *mut c_void, p: *mut c_void, serial: u32, surface: *mut c_void) {
    crate::clear_focus(surface as usize);
    forward!(data, p, 2, (u32, *mut c_void), (serial, surface));
}
unsafe extern "C" fn k_key(
    data: *mut c_void,
    p: *mut c_void,
    serial: u32,
    time: u32,
    key: u32,
    value: u32,
) {
    if route(|s| s.key(key, value != 0)) {
        forward!(data, p, 3, (u32, u32, u32, u32), (serial, time, key, value));
    }
}
unsafe extern "C" fn k_modifiers(
    data: *mut c_void,
    p: *mut c_void,
    serial: u32,
    depressed: u32,
    latched: u32,
    locked: u32,
    group: u32,
) {
    forward!(
        data,
        p,
        4,
        (u32, u32, u32, u32, u32),
        (serial, depressed, latched, locked, group)
    );
}
unsafe extern "C" fn k_repeat(data: *mut c_void, p: *mut c_void, rate: i32, delay: i32) {
    forward!(data, p, 5, (i32, i32), (rate, delay));
}
unsafe extern "C" fn relative(
    data: *mut c_void,
    p: *mut c_void,
    hi: u32,
    lo: u32,
    dx: i32,
    dy: i32,
    ux: i32,
    uy: i32,
) {
    if route(|s| s.relative(dx as f32 / 256., dy as f32 / 256.)) {
        forward!(
            data,
            p,
            0,
            (u32, u32, i32, i32, i32, i32),
            (hi, lo, dx, dy, ux, uy)
        );
    }
}

fn wrappers(kind: Kind) -> Vec<usize> {
    match kind {
        Kind::Pointer => vec![
            p_enter as *const () as usize,
            p_leave as *const () as usize,
            p_motion as *const () as usize,
            p_button as *const () as usize,
            p_axis as *const () as usize,
            p_frame as *const () as usize,
            p_source as *const () as usize,
            p_stop as *const () as usize,
            p_discrete as *const () as usize,
            p_value120 as *const () as usize,
            p_direction as *const () as usize,
        ],
        Kind::Keyboard => vec![
            k_keymap as *const () as usize,
            k_enter as *const () as usize,
            k_leave as *const () as usize,
            k_key as *const () as usize,
            k_modifiers as *const () as usize,
            k_repeat as *const () as usize,
        ],
        Kind::Relative => vec![relative as *const () as usize],
    }
}
// The tables passed to libwayland must outlive every proxy; do not pass Vec's
// temporary storage. Addresses are initialized once in the base namespace.
static TABLES: OnceLock<[Vec<usize>; 3]> = OnceLock::new();

#[no_mangle]
pub unsafe extern "C" fn wl_proxy_add_listener(
    proxy: *mut c_void,
    implementation: *mut *const c_void,
    data: *mut c_void,
) -> c_int {
    type Add = unsafe extern "C" fn(*mut c_void, *mut *const c_void, *mut c_void) -> c_int;
    let real = symbol(b"wl_proxy_add_listener\0");
    if real.is_null() {
        return -1;
    }
    let real: Add = std::mem::transmute(real);
    if !crate::process_scope::enabled() || proxy.is_null() || implementation.is_null() {
        return real(proxy, implementation, data);
    }
    let class = symbol(b"wl_proxy_get_class\0");
    let version = symbol(b"wl_proxy_get_version\0");
    if class.is_null() || version.is_null() {
        return real(proxy, implementation, data);
    }
    let class: unsafe extern "C" fn(*mut c_void) -> *const c_char = std::mem::transmute(class);
    let version: unsafe extern "C" fn(*mut c_void) -> u32 = std::mem::transmute(version);
    let name = class(proxy);
    if name.is_null() {
        return real(proxy, implementation, data);
    }
    let v = version(proxy);
    // Reject newer protocol versions rather than letting libwayland index past
    // a table compiled against an older ABI.
    let (kind, index, count) = match CStr::from_ptr(name).to_bytes() {
        b"wl_pointer" if v <= 9 => (
            Kind::Pointer,
            0,
            match v {
                0..=4 => 5,
                5..=7 => 9,
                8 => 10,
                _ => 11,
            },
        ),
        b"wl_keyboard" if v <= 9 => (Kind::Keyboard, 1, if v < 4 { 5 } else { 6 }),
        b"zwp_relative_pointer_v1" if v == 1 => (Kind::Relative, 2, 1),
        other => {
            if matches!(
                other,
                b"wl_pointer" | b"wl_keyboard" | b"zwp_relative_pointer_v1"
            ) {
                unavailable();
            }
            return real(proxy, implementation, data);
        }
    };
    if INCOMPLETE.load(Ordering::Acquire) {
        return real(proxy, implementation, data);
    }
    let tables = TABLES.get_or_init(|| {
        [
            wrappers(Kind::Pointer),
            wrappers(Kind::Keyboard),
            wrappers(Kind::Relative),
        ]
    });
    let get_listener = symbol(b"wl_proxy_get_listener\0");
    if get_listener.is_null() {
        unavailable();
        return real(proxy, implementation, data);
    }
    let get_listener: unsafe extern "C" fn(*mut c_void) -> *const c_void =
        std::mem::transmute(get_listener);
    let installed = get_listener(proxy);
    if !installed.is_null() {
        if !tables
            .iter()
            .any(|t| t.as_ptr().cast::<c_void>() == installed)
        {
            unavailable();
        }
        // Preserve a previous registration. libwayland decides whether the
        // caller's attempted second listener is legal; don't overwrite ours.
        return real(proxy, implementation, data);
    }
    let mut functions = [0; 11];
    for (i, f) in functions.iter_mut().enumerate().take(count) {
        *f = *implementation.add(i) as usize;
    }
    {
        let Ok(mut map) = listeners().lock() else {
            return real(proxy, implementation, data);
        };
        if map.len() >= 4096 {
            drop(map);
            unavailable();
            return real(proxy, implementation, data);
        }
        map.insert(proxy as usize, Listener { kind, functions });
    }
    let result = real(proxy, tables[index].as_ptr() as *mut *const c_void, data);
    if result != 0 {
        if let Ok(mut m) = listeners().lock() {
            m.remove(&(proxy as usize));
        }
    }
    result
}

#[no_mangle]
pub unsafe extern "C" fn wl_proxy_destroy(proxy: *mut c_void) {
    crate::wayland_surface::forget(proxy as usize);
    if let Ok(mut map) = listeners().lock() {
        map.remove(&(proxy as usize));
    }
    let real = symbol(b"wl_proxy_destroy\0");
    if !real.is_null() {
        let real: unsafe extern "C" fn(*mut c_void) = std::mem::transmute(real);
        real(proxy);
    }
}

pub fn available() -> bool {
    !INCOMPLETE.load(Ordering::Acquire)
        && listeners().lock().is_ok_and(|m| {
            m.values().any(|p| p.kind == Kind::Keyboard)
                && m.values().any(|p| p.kind == Kind::Pointer)
        })
}

type Dispatcher =
    unsafe extern "C" fn(*const c_void, *mut c_void, u32, *const c_void, *mut c_void) -> c_int;
#[no_mangle]
pub unsafe extern "C" fn wl_proxy_add_dispatcher(
    proxy: *mut c_void,
    dispatcher: Option<Dispatcher>,
    implementation: *const c_void,
    data: *mut c_void,
) -> c_int {
    let real = symbol(b"wl_proxy_add_dispatcher\0");
    if real.is_null() {
        return -1;
    }
    let real: unsafe extern "C" fn(
        *mut c_void,
        Option<Dispatcher>,
        *const c_void,
        *mut c_void,
    ) -> c_int = std::mem::transmute(real);
    if !crate::process_scope::enabled() { return real(proxy, dispatcher, implementation, data); }
    let class = symbol(b"wl_proxy_get_class\0");
    if !class.is_null() && !proxy.is_null() {
        let class: unsafe extern "C" fn(*mut c_void) -> *const c_char = std::mem::transmute(class);
        let name = class(proxy);
        if !name.is_null()
            && matches!(
                CStr::from_ptr(name).to_bytes(),
                b"wl_keyboard" | b"wl_pointer" | b"zwp_relative_pointer_v1"
            )
        {
            // Rust Wayland clients may use dispatchers instead of listener
            // arrays. Do not claim event isolation for an unwrapped path.
            unavailable();
        }
    }
    real(proxy, dispatcher, implementation, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    static DATA: AtomicUsize = AtomicUsize::new(0);
    static UNLOCKED: AtomicBool = AtomicBool::new(false);
    unsafe extern "C" fn callback(data: *mut c_void, _p: *mut c_void, _rate: i32, _delay: i32) {
        DATA.store(data as usize, Ordering::Release);
        UNLOCKED.store(listeners().try_lock().is_ok(), Ordering::Release);
    }
    #[test]
    fn forwards_current_user_data_and_releases_lock_before_game_callback() {
        unsafe {
            let proxy = 0x555usize as *mut c_void;
            let mut functions = [0; 11];
            functions[5] = callback as *const () as usize;
            listeners().lock().unwrap().insert(
                proxy as usize,
                Listener {
                    kind: Kind::Keyboard,
                    functions,
                },
            );
            k_repeat(0x123usize as *mut c_void, proxy, 60, 30);
            assert_eq!(DATA.load(Ordering::Acquire), 0x123);
            assert!(UNLOCKED.load(Ordering::Acquire));
            listeners().lock().unwrap().remove(&(proxy as usize));
        }
    }
}
