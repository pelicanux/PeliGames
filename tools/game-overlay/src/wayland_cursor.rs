//! Cursor ownership on observed game pointers only. No new seat or input grab.
//! Keep the game's latest cursor request and replay it when the panel releases it.
use crate::wayland_surface::Argument;
use std::{
    collections::BTreeMap,
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Image {
    Surface { surface: usize, x: i32, y: i32 },
    Shape { device: usize, shape: u32 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Request {
    pointer: usize,
    serial: u32,
    image: Image,
}
#[derive(Default)]
struct Pointer {
    surface: usize,
    serial: u32,
    desired: Option<Image>,
    hidden: bool,
    restore_on_enter: bool,
    pending_restore: bool,
}
#[derive(Default)]
struct Cursors {
    pointers: BTreeMap<usize, Pointer>,
    shapes: BTreeMap<usize, usize>,
}
trait Api {
    fn send(&mut self, request: Request) -> bool;
}
impl Cursors {
    fn enter(&mut self, pointer: usize, surface: usize, serial: u32) {
        if pointer == 0 || (self.pointers.len() >= 64 && !self.pointers.contains_key(&pointer)) {
            return;
        }
        let p = self.pointers.entry(pointer).or_default();
        p.surface = surface;
        p.serial = serial;
        p.hidden = false;
        p.pending_restore = p.restore_on_enter;
    }
    fn leave(&mut self, pointer: usize, surface: usize) {
        if self
            .pointers
            .get(&pointer)
            .is_some_and(|p| p.surface == surface)
        {
            let p = self.pointers.get_mut(&pointer).unwrap();
            p.surface = 0;
            p.hidden = false;
            p.pending_restore = false;
        }
    }
    fn remember(&mut self, pointer: usize, serial: u32, image: Image) -> bool {
        let Some(p) = self.pointers.get_mut(&pointer) else {
            return false;
        };
        // Rejected/outdated requests must never replace the cursor to restore.
        if p.surface == 0 || serial != p.serial {
            return false;
        }
        p.desired = Some(image);
        if !p.hidden {
            p.pending_restore = false;
            p.restore_on_enter = false;
        }
        p.hidden
    }
    fn shape_device(&mut self, device: usize, pointer: usize) {
        if device != 0 && pointer != 0 && self.shapes.len() < 128 {
            self.shapes.insert(device, pointer);
        }
    }
    fn reconcile(&mut self, capture: bool, owns: impl Fn(usize) -> bool, api: &mut impl Api) {
        for (&pointer, p) in &mut self.pointers {
            if p.surface == 0 {
                continue;
            }
            let hide = capture && owns(p.surface) && p.desired.is_some();
            if hide == p.hidden && !p.pending_restore {
                continue;
            }
            let Some(image) = (if hide {
                Some(Image::Surface {
                    surface: 0,
                    x: 0,
                    y: 0,
                })
            } else {
                p.desired
            }) else {
                continue;
            };
            if api.send(Request {
                pointer,
                serial: p.serial,
                image,
            }) {
                p.hidden = hide;
                p.pending_restore = false;
                // The compositor may already have moved focus before its leave
                // event is dispatched. Reapply once with the next enter serial.
                p.restore_on_enter = true;
            }
        }
    }
    fn forget(&mut self, object: usize, api: &mut impl Api) {
        // Restore before destroying a pointer or shape device. Never replay an
        // object after it has been freed, including reused proxy addresses.
        if let Some(p) = self.pointers.remove(&object) {
            if p.hidden {
                if let Some(image) = p.desired {
                    api.send(Request {
                        pointer: object,
                        serial: p.serial,
                        image,
                    });
                }
            }
        }
        if let Some(pointer) = self.shapes.remove(&object) {
            if let Some(p) = self.pointers.get_mut(&pointer) {
                if matches!(p.desired, Some(Image::Shape { device, .. }) if device == object) {
                    if p.hidden {
                        if let Some(image) = p.desired {
                            api.send(Request {
                                pointer,
                                serial: p.serial,
                                image,
                            });
                        }
                    }
                    p.hidden = false;
                    p.desired = None;
                    p.restore_on_enter = false;
                    p.pending_restore = false;
                }
            }
        }
        self.shapes.retain(|_, pointer| *pointer != object);
        for p in self.pointers.values_mut() {
            if p.surface == object {
                p.surface = 0;
                p.hidden = false;
                p.pending_restore = false;
            }
            if matches!(p.desired, Some(Image::Surface { surface, .. }) if surface == object) {
                p.desired = Some(Image::Surface {
                    surface: 0,
                    x: 0,
                    y: 0,
                });
            }
        }
    }
}

// Use the same fixed, public protocol entry point as the surface observer.
type Marshal =
    unsafe extern "C" fn(*mut c_void, u32, *const c_void, u32, u32, *mut Argument) -> *mut c_void;
type Version = unsafe extern "C" fn(*mut c_void) -> u32;
struct Native {
    marshal: Marshal,
    version: Version,
}
thread_local! { static INTERNAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
struct Forwarding(bool);
impl Forwarding {
    fn new() -> Self {
        Self(INTERNAL.with(|v| v.replace(true)))
    }
}
impl Drop for Forwarding {
    fn drop(&mut self) {
        INTERNAL.with(|v| v.set(self.0));
    }
}
impl Native {
    unsafe fn new() -> Option<Self> {
        let marshal = crate::symbols::resolve(b"wl_proxy_marshal_array_flags\0");
        let version = crate::symbols::resolve(b"wl_proxy_get_version\0");
        if marshal.is_null() || version.is_null() {
            return None;
        }
        Some(Self {
            marshal: std::mem::transmute::<*mut c_void, Marshal>(marshal),
            version: std::mem::transmute::<*mut c_void, Version>(version),
        })
    }
}
impl Api for Native {
    fn send(&mut self, request: Request) -> bool {
        let _forwarding = Forwarding::new();
        let (proxy, opcode, mut args) = match request.image {
            Image::Surface { surface, x, y } => (
                request.pointer,
                0,
                vec![
                    Argument {
                        unsigned: request.serial,
                    },
                    Argument {
                        object: surface as _,
                    },
                    Argument { integer: x },
                    Argument { integer: y },
                ],
            ),
            Image::Shape { device, shape } => (
                device,
                1,
                vec![
                    Argument {
                        unsigned: request.serial,
                    },
                    Argument { unsigned: shape },
                ],
            ),
        };
        if proxy == 0 {
            return false;
        }
        unsafe {
            (self.marshal)(
                proxy as _,
                opcode,
                std::ptr::null(),
                (self.version)(proxy as _),
                0,
                args.as_mut_ptr(),
            );
        }
        true
    }
}
static CURSORS: OnceLock<Mutex<Cursors>> = OnceLock::new();
fn cursors() -> &'static Mutex<Cursors> {
    CURSORS.get_or_init(Default::default)
}

pub(crate) fn enter(pointer: usize, surface: usize, serial: u32) {
    if let Ok(mut cursors) = cursors().lock() {
        cursors.enter(pointer, surface, serial);
    }
}
pub(crate) fn leave(pointer: usize, surface: usize) {
    if let Ok(mut cursors) = cursors().lock() {
        cursors.leave(pointer, surface);
    }
}
pub(crate) fn forget(object: usize) {
    // A marshalled destructor can call wl_proxy_destroy internally. The outer
    // protocol wrapper already retired this proxy under the lifetime lock.
    if INTERNAL.with(|v| v.get()) {
        return;
    }
    let Some(mut api) = (unsafe { Native::new() }) else {
        return;
    };
    if let Ok(mut cursors) = cursors().lock() {
        cursors.forget(object, &mut api);
    }
}
pub(crate) fn reconcile() {
    if CURSORS.get().is_none() || !crate::process_scope::enabled() {
        return;
    }
    let capture = crate::state().lock().is_ok_and(|s| s.capturing());
    let target = crate::target();
    let Some(mut api) = (unsafe { Native::new() }) else {
        return;
    };
    if let Ok(mut cursors) = cursors().lock() {
        cursors.reconcile(
            capture,
            |surface| match target {
                crate::Target::Wayland(target) => crate::wayland_surface::focused(target, surface),
                _ => false,
            },
            &mut api,
        );
    }
}
pub(crate) fn hidden() -> bool {
    let crate::Target::Wayland(target) = crate::target() else {
        return false;
    };
    cursors().lock().is_ok_and(|c| {
        c.pointers
            .values()
            .any(|p| p.hidden && crate::wayland_surface::focused(target, p.surface))
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Observed {
    None,
    Surface {
        pointer: usize,
        serial: u32,
        surface: usize,
        x: i32,
        y: i32,
    },
    Shape {
        device: usize,
        serial: u32,
        shape: u32,
    },
    Device {
        pointer: usize,
    },
    Destroy(usize),
}
pub(crate) unsafe fn observe(
    class: &[u8],
    proxy: usize,
    opcode: u32,
    flags: u32,
    args: *mut Argument,
) -> Observed {
    if INTERNAL.with(|v| v.get()) {
        return Observed::None;
    }
    match (class, opcode) {
        (b"wl_pointer", 0) if !args.is_null() => Observed::Surface {
            pointer: proxy,
            serial: (*args).unsigned,
            surface: (*args.add(1)).object as usize,
            x: (*args.add(2)).integer,
            y: (*args.add(3)).integer,
        },
        (b"wp_cursor_shape_device_v1", 1) if !args.is_null() => Observed::Shape {
            device: proxy,
            serial: (*args).unsigned,
            shape: (*args.add(1)).unsigned,
        },
        (b"wp_cursor_shape_manager_v1", 1) if !args.is_null() => Observed::Device {
            pointer: (*args.add(1)).object as usize,
        },
        (b"wl_surface", 0) | (b"wp_cursor_shape_device_v1", 0) | (b"wl_pointer", 1) => {
            Observed::Destroy(proxy)
        }
        _ if flags & 1 != 0 => Observed::Destroy(proxy),
        _ => Observed::None,
    }
}
/// Hold the proxy-lifetime lock through protocol forwarding, so another thread
/// cannot restore a saved cursor through an object being destroyed here.
pub(crate) unsafe fn marshal(
    observed: Observed,
    forward: impl FnOnce() -> *mut c_void,
) -> *mut c_void {
    if observed == Observed::None {
        return forward();
    }
    let Some(mut api) = Native::new() else {
        return forward();
    };
    let Ok(mut cursors) = cursors().lock() else {
        return forward();
    };
    let suppress = match observed {
        Observed::Surface {
            pointer,
            serial,
            surface,
            x,
            y,
        } => cursors.remember(pointer, serial, Image::Surface { surface, x, y }),
        Observed::Shape {
            device,
            serial,
            shape,
        } => {
            if let Some(&pointer) = cursors.shapes.get(&device) {
                cursors.remember(pointer, serial, Image::Shape { device, shape })
            } else {
                false
            }
        }
        Observed::Destroy(object) => {
            cursors.forget(object, &mut api);
            false
        }
        _ => false,
    };
    if suppress {
        return std::ptr::null_mut();
    }
    let _forwarding = Forwarding::new();
    let result = forward();
    if let Observed::Device { pointer } = observed {
        cursors.shape_device(result as usize, pointer);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        requests: Vec<Request>,
        fail: bool,
    }
    impl Api for Fake {
        fn send(&mut self, request: Request) -> bool {
            if self.fail {
                false
            } else {
                self.requests.push(request);
                true
            }
        }
    }
    fn surface(id: usize) -> Image {
        Image::Surface {
            surface: id,
            x: 4,
            y: 5,
        }
    }
    #[test]
    fn only_overlay_cursor_is_visible_and_latest_game_request_returns_on_close() {
        let mut cursors = Cursors::default();
        let mut api = Fake::default();
        cursors.enter(1, 10, 7);
        assert!(!cursors.remember(1, 7, surface(20)));
        cursors.reconcile(true, |surface| surface == 10, &mut api);
        assert_eq!(
            api.requests,
            [Request {
                pointer: 1,
                serial: 7,
                image: Image::Surface {
                    surface: 0,
                    x: 0,
                    y: 0
                }
            }]
        );
        assert!(cursors.remember(1, 7, surface(21)));
        assert!(!cursors.remember(1, 6, surface(99))); // stale request was never accepted by the compositor
        cursors.reconcile(true, |_| true, &mut api);
        assert_eq!(api.requests.len(), 1);
        cursors.reconcile(false, |_| true, &mut api);
        assert_eq!(api.requests.last().unwrap().image, surface(21));
        assert!(!cursors.pointers[&1].hidden);
        assert_eq!(api.requests.len(), 2);
    }
    #[test]
    fn another_window_and_alt_tab_never_keep_the_game_cursor_hidden() {
        let mut cursors = Cursors::default();
        let mut api = Fake::default();
        cursors.enter(1, 10, 7);
        cursors.remember(1, 7, surface(20));
        cursors.enter(2, 30, 8);
        cursors.remember(2, 8, surface(40));
        cursors.reconcile(true, |surface| surface == 10, &mut api);
        assert!(api.requests.iter().all(|r| r.pointer == 1));
        cursors.reconcile(false, |_| false, &mut api);
        assert_eq!(api.requests.last().unwrap().image, surface(20));
        cursors.leave(1, 10);
        cursors.enter(1, 30, 9);
        cursors.reconcile(true, |surface| surface == 10, &mut api);
        assert_eq!(api.requests.len(), 3);
        assert_eq!(
            api.requests.last().unwrap(),
            &Request {
                pointer: 1,
                serial: 9,
                image: surface(20)
            }
        );
        assert!(!cursors.remember(1, 7, surface(99)));
        assert_eq!(cursors.pointers[&1].desired, Some(surface(20)));
    }
    #[test]
    fn cursor_shape_requests_are_saved_and_restored_before_destroy() {
        let mut cursors = Cursors::default();
        let mut api = Fake::default();
        cursors.enter(1, 10, 7);
        cursors.shape_device(2, 1);
        let image = Image::Shape {
            device: 2,
            shape: 9,
        };
        cursors.remember(1, 7, image);
        cursors.reconcile(true, |_| true, &mut api);
        cursors.forget(2, &mut api);
        assert_eq!(api.requests.last().unwrap().image, image);
        cursors.reconcile(false, |_| true, &mut api);
        cursors.reconcile(true, |_| true, &mut api);
        assert_eq!(api.requests.len(), 2); // cannot replay a freed shape device
        assert!(!cursors.pointers[&1].hidden);
    }
    #[test]
    fn freed_cursor_surface_and_pointer_are_never_replayed() {
        let mut cursors = Cursors::default();
        let mut api = Fake::default();
        cursors.enter(1, 10, 7);
        cursors.remember(1, 7, surface(20));
        cursors.reconcile(true, |_| true, &mut api);
        cursors.forget(20, &mut api);
        cursors.reconcile(false, |_| true, &mut api);
        assert_eq!(
            api.requests.last().unwrap().image,
            Image::Surface {
                surface: 0,
                x: 0,
                y: 0
            }
        );
        cursors.remember(1, 7, surface(21));
        cursors.reconcile(true, |_| true, &mut api);
        cursors.forget(1, &mut api);
        assert_eq!(api.requests.last().unwrap().image, surface(21));
        assert!(cursors.pointers.is_empty());
        let count = api.requests.len();
        cursors.reconcile(false, |_| true, &mut api);
        assert_eq!(api.requests.len(), count);
    }
    #[test]
    fn unsupported_and_unknown_cursor_states_do_not_hide_the_cursor() {
        let mut cursors = Cursors::default();
        let mut api = Fake::default();
        cursors.enter(1, 10, 7);
        cursors.reconcile(true, |_| true, &mut api);
        assert!(api.requests.is_empty());
        cursors.remember(1, 7, surface(20));
        api.fail = true;
        cursors.reconcile(true, |_| true, &mut api);
        assert!(!cursors.pointers[&1].hidden);
        assert!(!cursors.remember(1, 7, surface(21)));
    }
    #[test]
    fn decoder_reads_only_known_protocol_slots_and_skips_internal_requests() {
        let mut args = [
            Argument { unsigned: 7 },
            Argument { object: 20 as _ },
            Argument { integer: 4 },
            Argument { integer: 5 },
        ];
        unsafe {
            assert_eq!(
                observe(b"wl_pointer", 1, 0, 0, args.as_mut_ptr()),
                Observed::Surface {
                    pointer: 1,
                    serial: 7,
                    surface: 20,
                    x: 4,
                    y: 5
                }
            );
            assert_eq!(observe(b"unknown", 1, 0, 0, 1 as _), Observed::None);
            assert_eq!(
                observe(b"wl_pointer", 1, 0, 0, std::ptr::null_mut()),
                Observed::None
            );
            let _forwarding = Forwarding::new();
            assert_eq!(
                observe(b"wl_pointer", 1, 0, 0, args.as_mut_ptr()),
                Observed::None
            );
        }
    }
    #[test]
    fn native_protocol_packets_hide_restore_image_and_restore_shape() {
        static CALLS: Mutex<Vec<(usize, u32, u32, usize, i32, i32)>> = Mutex::new(Vec::new());
        unsafe extern "C" fn version(_: *mut c_void) -> u32 {
            2
        }
        unsafe extern "C" fn marshal(
            proxy: *mut c_void,
            opcode: u32,
            interface: *const c_void,
            version: u32,
            flags: u32,
            args: *mut Argument,
        ) -> *mut c_void {
            assert!(interface.is_null());
            assert_eq!(version, 2);
            assert_eq!(flags, 0);
            let (object, x, y) = if opcode == 0 {
                (
                    (*args.add(1)).object as usize,
                    (*args.add(2)).integer,
                    (*args.add(3)).integer,
                )
            } else {
                ((*args.add(1)).unsigned as usize, 0, 0)
            };
            CALLS
                .lock()
                .unwrap()
                .push((proxy as usize, opcode, (*args).unsigned, object, x, y));
            std::ptr::null_mut()
        }
        let mut api = Native { marshal, version };
        assert!(api.send(Request {
            pointer: 1,
            serial: 7,
            image: Image::Surface {
                surface: 0,
                x: 0,
                y: 0
            }
        }));
        assert!(api.send(Request {
            pointer: 1,
            serial: 7,
            image: surface(20)
        }));
        assert!(api.send(Request {
            pointer: 1,
            serial: 7,
            image: Image::Shape {
                device: 2,
                shape: 9
            }
        }));
        assert_eq!(
            *CALLS.lock().unwrap(),
            [(1, 0, 7, 0, 0, 0), (1, 0, 7, 20, 4, 5), (2, 1, 7, 9, 0, 0)]
        );
    }
    #[test]
    fn actual_protocol_wrapper_preserves_requests_and_handles_nested_destruction() {
        static CALLS: Mutex<Vec<(usize, u32, u32, usize)>> = Mutex::new(Vec::new());
        unsafe extern "C" fn class(proxy: *mut c_void) -> *const libc::c_char {
            match proxy as usize {
                1001 => c"wl_pointer".as_ptr(),
                1002 => c"wp_cursor_shape_manager_v1".as_ptr(),
                _ => c"wp_cursor_shape_device_v1".as_ptr(),
            }
        }
        unsafe extern "C" fn version(_: *mut c_void) -> u32 {
            2
        }
        unsafe extern "C" fn marshal(
            proxy: *mut c_void,
            opcode: u32,
            _: *const c_void,
            _: u32,
            flags: u32,
            args: *mut Argument,
        ) -> *mut c_void {
            let (serial, object) = if args.is_null() {
                (0, 0)
            } else {
                let object = if proxy as usize == 1003 && opcode == 1 {
                    (*args.add(1)).unsigned as usize
                } else {
                    (*args.add(1)).object as usize
                };
                ((*args).unsigned, object)
            };
            CALLS
                .lock()
                .unwrap()
                .push((proxy as usize, opcode, serial, object));
            if flags & 1 != 0 {
                assert!(INTERNAL.with(|v| v.get()));
                forget(proxy as usize); // same nested path used by libwayland
            }
            if proxy as usize == 1002 {
                1003 as _
            } else {
                std::ptr::null_mut()
            }
        }
        let slot = |name: &[u8]| {
            crate::symbols::abi::SYMBOLS
                .iter()
                .position(|(n, _, _)| *n == name)
                .unwrap()
        };
        let replacements = [
            (slot(b"wl_proxy_get_class"), class as *const () as usize),
            (slot(b"wl_proxy_get_version"), version as *const () as usize),
            (
                slot(b"wl_proxy_marshal_array_flags"),
                marshal as *const () as usize,
            ),
        ];
        let previous: Vec<_> = replacements
            .iter()
            .map(|(slot, value)| {
                (
                    *slot,
                    crate::symbols::peli_overlay_originals[*slot]
                        .swap(*value, std::sync::atomic::Ordering::AcqRel),
                )
            })
            .collect();
        *cursors().lock().unwrap() = Cursors::default();
        enter(1001, 10, 7);
        let mut args = [
            Argument { unsigned: 7 },
            Argument { object: 20 as _ },
            Argument { integer: 4 },
            Argument { integer: 5 },
        ];
        unsafe {
            crate::wayland_surface::wl_proxy_marshal_array_flags(
                1001 as _,
                0,
                std::ptr::null(),
                2,
                0,
                args.as_mut_ptr(),
            );
        }
        let mut api = Native { marshal, version };
        cursors()
            .lock()
            .unwrap()
            .reconcile(true, |s| s == 10, &mut api);
        args[1] = Argument { object: 21 as _ };
        unsafe {
            crate::wayland_surface::wl_proxy_marshal_array_flags(
                1001 as _,
                0,
                std::ptr::null(),
                2,
                0,
                args.as_mut_ptr(),
            );
        }
        assert_eq!(CALLS.lock().unwrap().len(), 2); // replacement is deferred while hidden
        cursors()
            .lock()
            .unwrap()
            .reconcile(false, |_| false, &mut api);
        assert_eq!(*CALLS.lock().unwrap().last().unwrap(), (1001, 0, 7, 21));
        let mut device_args = [
            Argument {
                object: std::ptr::null_mut(),
            },
            Argument { object: 1001 as _ },
        ];
        unsafe {
            assert_eq!(
                crate::wayland_surface::wl_proxy_marshal_array_flags(
                    1002 as _,
                    1,
                    99 as _,
                    2,
                    0,
                    device_args.as_mut_ptr()
                ) as usize,
                1003
            );
        }
        let mut shape_args = [Argument { unsigned: 7 }, Argument { unsigned: 9 }];
        unsafe {
            crate::wayland_surface::wl_proxy_marshal_array_flags(
                1003 as _,
                1,
                std::ptr::null(),
                2,
                0,
                shape_args.as_mut_ptr(),
            );
        }
        cursors()
            .lock()
            .unwrap()
            .reconcile(true, |_| true, &mut api);
        unsafe {
            crate::wayland_surface::wl_proxy_marshal_array_flags(
                1003 as _,
                0,
                std::ptr::null(),
                2,
                1,
                std::ptr::null_mut(),
            );
        }
        let calls = CALLS.lock().unwrap();
        assert_eq!(calls[calls.len() - 2], (1003, 1, 7, 9)); // restore before proxy is freed
        assert_eq!(calls[calls.len() - 1], (1003, 0, 0, 0));
        drop(calls);
        assert!(!cursors().lock().unwrap().pointers[&1001].hidden);
        *cursors().lock().unwrap() = Cursors::default();
        for (slot, value) in previous {
            crate::symbols::peli_overlay_originals[slot]
                .store(value, std::sync::atomic::Ordering::Release);
        }
    }
    #[test]
    fn cursor_recovers_after_leaving_while_hidden_without_a_new_game_cursor_request() {
        let mut cursors = Cursors::default();
        let mut api = Fake::default();
        cursors.enter(1, 10, 7);
        cursors.remember(1, 7, surface(20));
        cursors.reconcile(true, |_| true, &mut api);
        cursors.leave(1, 10); // compositor has changed focus; old serial cannot restore it
        cursors.reconcile(false, |_| false, &mut api);
        assert_eq!(api.requests.len(), 1);
        cursors.enter(1, 10, 9); // the game caches its cursor and sends no new set_cursor
        cursors.reconcile(false, |_| true, &mut api);
        assert_eq!(
            api.requests.last().unwrap(),
            &Request {
                pointer: 1,
                serial: 9,
                image: surface(20)
            }
        );
        assert!(!cursors.pointers[&1].hidden);
    }
}
