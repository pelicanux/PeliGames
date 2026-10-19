//! Temporary pointer ownership on Wine's existing Xlib connection. Keep the
//! game's latest grab, substitute a native cursor free within the game while the panel
//! is open, and restore only when the game still owns keyboard focus.
use super::symbol;
use libc::{c_int, c_uint, c_ulong, c_void};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Grab {
    pub display: usize,
    pub window: c_ulong,
    pub owner_events: c_int,
    pub event_mask: c_uint,
    pub pointer_mode: c_int,
    pub keyboard_mode: c_int,
    pub confine_to: c_ulong,
    pub cursor: c_ulong,
    pub time: c_ulong,
}
type GrabFn = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    c_int,
    c_uint,
    c_int,
    c_int,
    c_ulong,
    c_ulong,
    c_ulong,
) -> c_int;
type UngrabFn = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
type ChangeFn = unsafe extern "C" fn(*mut c_void, c_uint, c_ulong, c_ulong) -> c_int;
type WarpFn = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    c_ulong,
    c_int,
    c_int,
    c_uint,
    c_uint,
    c_int,
    c_int,
) -> c_int;
type QueryFn = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    *mut c_ulong,
    *mut c_ulong,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_int,
    *mut c_uint,
) -> c_int;
type GeometryFn = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    *mut c_ulong,
    *mut c_int,
    *mut c_int,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
) -> c_int;
type TranslateFn = unsafe extern "C" fn(
    *mut c_void,
    c_ulong,
    c_ulong,
    c_int,
    c_int,
    *mut c_int,
    *mut c_int,
    *mut c_ulong,
) -> c_int;
type FocusFn = unsafe extern "C" fn(*mut c_void, *mut c_ulong, *mut c_int) -> c_int;
type CursorFn = unsafe extern "C" fn(*mut c_void, c_uint) -> c_ulong;
type FreeCursorFn = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
type FlushFn = unsafe extern "C" fn(*mut c_void) -> c_int;

#[derive(Clone, Copy, Debug, Default)]
struct Position {
    root: c_ulong,
    root_x: c_int,
    root_y: c_int,
    x: c_int,
    y: c_int,
    mask: c_uint,
}
trait Api {
    fn grab(&mut self, request: Grab) -> c_int;
    fn ungrab(&mut self, display: usize, time: c_ulong) -> c_int;
    fn change(&mut self, display: usize, mask: c_uint, cursor: c_ulong, time: c_ulong) -> c_int;
    fn cursor(&mut self, display: usize) -> c_ulong;
    fn free_cursor(&mut self, display: usize, cursor: c_ulong);
    fn query(&mut self, display: usize, window: c_ulong) -> Option<Position>;
    fn geometry(&mut self, display: usize, window: c_ulong) -> Option<(u32, u32)>;
    fn translate(
        &mut self,
        display: usize,
        from: c_ulong,
        to: c_ulong,
        x: i32,
        y: i32,
    ) -> Option<(i32, i32)>;
    fn focus(&mut self, display: usize) -> c_ulong;
    fn warp(&mut self, display: usize, root: c_ulong, x: i32, y: i32);
    fn flush(&mut self, display: usize);
}
#[derive(Default)]
struct Native;
impl Api for Native {
    fn grab(&mut self, r: Grab) -> c_int {
        unsafe {
            symbol::<GrabFn>(b"XGrabPointer\0")
                .map(|f| {
                    f(
                        r.display as *mut _,
                        r.window,
                        r.owner_events,
                        r.event_mask,
                        r.pointer_mode,
                        r.keyboard_mode,
                        r.confine_to,
                        r.cursor,
                        r.time,
                    )
                })
                .unwrap_or(1)
        }
    }
    fn ungrab(&mut self, d: usize, time: c_ulong) -> c_int {
        unsafe {
            symbol::<UngrabFn>(b"XUngrabPointer\0")
                .map(|f| f(d as *mut _, time))
                .unwrap_or(0)
        }
    }
    fn change(&mut self, d: usize, mask: c_uint, cursor: c_ulong, time: c_ulong) -> c_int {
        unsafe {
            symbol::<ChangeFn>(b"XChangeActivePointerGrab\0")
                .map(|f| f(d as *mut _, mask, cursor, time))
                .unwrap_or(0)
        }
    }
    fn cursor(&mut self, d: usize) -> c_ulong {
        unsafe {
            // XC_left_ptr: a server cursor, never another triangle in the game image.
            symbol::<CursorFn>(b"XCreateFontCursor\0")
                .map(|f| f(d as *mut _, 68))
                .unwrap_or(0)
        }
    }
    fn free_cursor(&mut self, d: usize, cursor: c_ulong) {
        unsafe {
            if let Some(f) = symbol::<FreeCursorFn>(b"XFreeCursor\0") {
                f(d as *mut _, cursor);
            }
        }
    }
    fn query(&mut self, d: usize, w: c_ulong) -> Option<Position> {
        unsafe {
            let f = symbol::<QueryFn>(b"XQueryPointer\0")?;
            let mut p = Position::default();
            let mut child = 0;
            (f(
                d as *mut _,
                w,
                &mut p.root,
                &mut child,
                &mut p.root_x,
                &mut p.root_y,
                &mut p.x,
                &mut p.y,
                &mut p.mask,
            ) != 0)
                .then_some(p)
        }
    }
    fn geometry(&mut self, d: usize, w: c_ulong) -> Option<(u32, u32)> {
        unsafe {
            let f = symbol::<GeometryFn>(b"XGetGeometry\0")?;
            let (mut root, mut x, mut y, mut width, mut height, mut border, mut depth) =
                (0, 0, 0, 0, 0, 0, 0);
            (f(
                d as *mut _,
                w,
                &mut root,
                &mut x,
                &mut y,
                &mut width,
                &mut height,
                &mut border,
                &mut depth,
            ) != 0
                && width != 0
                && height != 0)
                .then_some((width, height))
        }
    }
    fn translate(
        &mut self,
        d: usize,
        from: c_ulong,
        to: c_ulong,
        x: i32,
        y: i32,
    ) -> Option<(i32, i32)> {
        unsafe {
            let f = symbol::<TranslateFn>(b"XTranslateCoordinates\0")?;
            let (mut dx, mut dy, mut child) = (0, 0, 0);
            (f(d as *mut _, from, to, x, y, &mut dx, &mut dy, &mut child) != 0).then_some((dx, dy))
        }
    }
    fn focus(&mut self, d: usize) -> c_ulong {
        unsafe {
            let (mut window, mut revert) = (0, 0);
            if let Some(f) = symbol::<FocusFn>(b"XGetInputFocus\0") {
                f(d as *mut _, &mut window, &mut revert);
            }
            window
        }
    }
    fn warp(&mut self, d: usize, root: c_ulong, x: i32, y: i32) {
        unsafe {
            if let Some(f) = symbol::<WarpFn>(b"XWarpPointer\0") {
                f(d as *mut _, 0, root, 0, 0, 0, 0, x, y);
            }
        }
    }
    fn flush(&mut self, d: usize) {
        unsafe {
            if let Some(f) = symbol::<FlushFn>(b"XFlush\0") {
                f(d as *mut _);
            }
        }
    }
}

struct Lease {
    display: usize,
    window: c_ulong,
    ancestors: Vec<c_ulong>,
    cursor: c_ulong,
    saved: Position,
    size: (u32, u32),
    warp_requested: bool,
}
struct Pointer<A> {
    api: A,
    // Each Wine thread can have a different Display and clipping window.
    requested: HashMap<usize, Option<Grab>>,
    game_grab: Option<Grab>,
    lease: Option<Lease>,
}
impl<A: Api> Pointer<A> {
    fn new(api: A) -> Self {
        Self {
            api,
            requested: HashMap::new(),
            game_grab: None,
            lease: None,
        }
    }
    fn remember(&mut self, display: usize, request: Option<Grab>) {
        if self.requested.len() < 64 || self.requested.contains_key(&display) {
            self.requested.insert(display, request);
        }
    }
    fn game_grab(&mut self, request: Grab) -> c_int {
        if self.lease.is_some() {
            self.remember(request.display, Some(request));
            self.game_grab = Some(request);
            return 0;
        }
        let result = self.api.grab(request);
        if result == 0 {
            self.remember(request.display, Some(request));
            self.game_grab = Some(request);
        }
        result
    }
    fn game_ungrab(&mut self, display: usize, time: c_ulong) -> c_int {
        self.remember(display, None);
        if self.game_grab.is_some_and(|r| r.display == display) {
            self.game_grab = None;
        }
        if self.lease.is_some() {
            1
        } else {
            self.api.ungrab(display, time)
        }
    }
    fn game_change(&mut self, d: usize, mask: c_uint, cursor: c_ulong, time: c_ulong) -> c_int {
        if let Some(Some(request)) = self.requested.get_mut(&d) {
            request.event_mask = mask;
            request.cursor = cursor;
            request.time = time;
        }
        if let Some(request) = self.game_grab.as_mut().filter(|r| r.display == d) {
            request.event_mask = mask;
            request.cursor = cursor;
            request.time = time;
        }
        if self.lease.is_some() {
            1
        } else {
            self.api.change(d, mask, cursor, time)
        }
    }
    fn open(
        &mut self,
        display: usize,
        window: c_ulong,
        ancestors: &[c_ulong],
    ) -> Result<Option<Position>, &'static str> {
        if self.lease.as_ref().is_some_and(|s| s.window == window) {
            return Ok(None);
        }
        self.close(false);
        let display = self.game_grab.map(|r| r.display).unwrap_or(display);
        if self.requested.len() >= 64 && !self.requested.contains_key(&display) {
            return Err("Limite de conexões de entrada; captura cancelada.");
        }
        let size = self
            .api
            .geometry(display, window)
            .ok_or("Não foi possível medir a janela do jogo.")?;
        if !ancestors.contains(&self.api.focus(display)) {
            return Err("O jogo perdeu o foco; captura cancelada.");
        }
        let saved = self
            .api
            .query(display, window)
            .ok_or("Não foi possível consultar o cursor do jogo.")?;
        let cursor = self.api.cursor(display);
        if cursor == 0 {
            return Err("Não foi possível preparar o cursor nativo.");
        }
        // Replace Wine's tiny clipping window with the whole game surface.
        // Do not confine/capture the pointer across the rest of the desktop.
        let request = Grab {
            display,
            window,
            owner_events: 0,
            event_mask: (1 << 2) | (1 << 3) | (1 << 6),
            pointer_mode: 1,
            keyboard_mode: 1,
            confine_to: window,
            cursor,
            time: 0,
        };
        // XGrabPointer replaces this client's old grab. An unrelated client's
        // grab is never removed; failure leaves its ownership unchanged.
        if self.api.grab(request) != 0 {
            self.api.free_cursor(display, cursor);
            return Err("Não foi possível obter o mouse exclusivo; captura cancelada.");
        }
        self.requested.entry(display).or_insert(None);
        self.lease = Some(Lease {
            display,
            window,
            ancestors: ancestors.to_vec(),
            cursor,
            saved,
            size,
            warp_requested: false,
        });
        self.api.flush(display);
        Ok(Some(saved))
    }
    fn close(&mut self, restore: bool) {
        let Some(s) = self.lease.take() else {
            return;
        };
        self.api.ungrab(s.display, 0);
        // Alt+Tab/watchdog/window destruction must never steal the pointer back
        // from the desktop. Restore a game grab only on an ordinary close.
        if restore && s.ancestors.contains(&self.api.focus(s.display)) {
            let request = self.game_grab;
            if s.warp_requested || request.is_some_and(|r| r.confine_to != 0) {
                self.api
                    .warp(s.display, s.saved.root, s.saved.root_x, s.saved.root_y);
            }
            if let Some(mut request) = request {
                request.time = 0; // old timestamps predate our grab and are invalid
                self.api.grab(request);
            }
        }
        self.api.free_cursor(s.display, s.cursor);
        self.api.flush(s.display);
    }
    fn accepts(&self, display: usize, window: c_ulong) -> bool {
        self.lease
            .as_ref()
            .is_some_and(|s| s.display == display && s.window == window)
            || self
                .requested
                .get(&display)
                .copied()
                .flatten()
                .is_some_and(|r| r.window == window)
    }
    fn coordinates(
        &mut self,
        display: usize,
        window: c_ulong,
        x: i32,
        y: i32,
    ) -> Option<(f32, f32)> {
        let s = self.lease.as_ref()?;
        if s.display != display {
            return None;
        }
        let (x, y) = if window == s.window {
            (x, y)
        } else {
            self.api.translate(display, window, s.window, x, y)?
        };
        Some((x as f32 / s.size.0 as f32, y as f32 / s.size.1 as f32))
    }
    fn sample(&mut self) -> Option<(f32, f32)> {
        let s = self.lease.as_ref()?;
        let p = self.api.query(s.display, s.window)?;
        Some((p.x as f32 / s.size.0 as f32, p.y as f32 / s.size.1 as f32))
    }
    fn resize(&mut self, window: c_ulong) {
        if let Some(s) = self
            .lease
            .as_mut()
            .filter(|s| s.ancestors.contains(&window))
        {
            if let Some(size) = self.api.geometry(s.display, s.window) {
                s.size = size;
            }
        }
    }
    fn frozen(&mut self, display: usize, window: c_ulong) -> Option<Position> {
        let s = self.lease.as_ref()?;
        let mut p = s.saved;
        (p.x, p.y) = if window == s.window {
            (p.x, p.y)
        } else if window == p.root {
            (p.root_x, p.root_y)
        } else {
            self.api
                .translate(display, p.root, window, p.root_x, p.root_y)?
        };
        Some(p)
    }
    fn suppress_warp(&mut self) -> bool {
        if let Some(s) = self.lease.as_mut() {
            s.warp_requested = true;
            true
        } else {
            false
        }
    }
    fn forget_display(&mut self, display: usize) {
        if self.lease.as_ref().is_some_and(|s| s.display == display) {
            self.close(false);
        }
        if self.game_grab.is_some_and(|r| r.display == display) {
            self.game_grab = None;
        }
        self.requested.remove(&display);
    }
    fn forget_window(&mut self, window: c_ulong) {
        if self
            .lease
            .as_ref()
            .is_some_and(|s| s.ancestors.contains(&window))
        {
            self.close(false);
        }
        if self
            .game_grab
            .is_some_and(|r| r.window == window || r.confine_to == window)
        {
            self.game_grab = None;
        }
        for request in self.requested.values_mut() {
            if request.is_some_and(|r| r.window == window || r.confine_to == window) {
                *request = None;
            }
        }
    }
}
static POINTER: OnceLock<Mutex<Pointer<Native>>> = OnceLock::new();
fn pointer() -> &'static Mutex<Pointer<Native>> {
    POINTER.get_or_init(|| Mutex::new(Pointer::new(Native)))
}

pub(super) fn available() -> bool {
    unsafe {
        symbol::<GrabFn>(b"XGrabPointer\0").is_some()
            && symbol::<UngrabFn>(b"XUngrabPointer\0").is_some()
            && symbol::<CursorFn>(b"XCreateFontCursor\0").is_some()
            && symbol::<FreeCursorFn>(b"XFreeCursor\0").is_some()
            && symbol::<QueryFn>(b"XQueryPointer\0").is_some()
            && symbol::<GeometryFn>(b"XGetGeometry\0").is_some()
            && symbol::<TranslateFn>(b"XTranslateCoordinates\0").is_some()
            && symbol::<FlushFn>(b"XFlush\0").is_some()
    }
}
pub(super) fn reconcile(display: *mut c_void) {
    let (capture, restore) = crate::state()
        .lock()
        .map(|s| (s.capturing(), s.focused && s.ready))
        .unwrap_or_default();
    let target = if capture { super::current() } else { None };
    let result = if let Ok(mut p) = pointer().lock() {
        if capture {
            if let Some((window, target)) = target {
                p.open(display as usize, window, &target.ancestors)
            } else {
                p.close(false);
                Ok(None)
            }
        } else {
            p.close(restore);
            Ok(None)
        }
    } else {
        return;
    };
    match result {
        Ok(Some(position)) => {
            if let Ok(mut s) = crate::state().lock() {
                s.seed_x11_buttons(position.mask);
            }
            if let Some((x, y)) = sample() {
                move_pointer(x, y);
            }
            crate::report("Mouse exclusivo do painel ativado; cursor nativo liberado.");
        }
        Err(e) => {
            if let Ok(mut s) = crate::state().lock() {
                s.release();
            }
            crate::report(e);
        }
        _ => {}
    }
}
pub(crate) fn suspend() {
    if let Ok(mut p) = pointer().lock() {
        p.close(false);
    }
}
pub(super) fn active() -> bool {
    pointer().lock().is_ok_and(|p| p.lease.is_some())
}
pub(super) fn accepts(display: *mut c_void, window: c_ulong) -> bool {
    pointer()
        .lock()
        .is_ok_and(|p| p.accepts(display as usize, window))
}
fn sample() -> Option<(f32, f32)> {
    pointer().lock().ok()?.sample()
}
fn move_pointer(x: f32, y: f32) {
    if let Ok(mut s) = crate::state().lock() {
        let extent = s.extent;
        s.native_absolute(x * extent.x, y * extent.y);
    }
}
pub(super) fn motion(display: *mut c_void, window: c_ulong, x: i32, y: i32) {
    let point = pointer()
        .lock()
        .ok()
        .and_then(|mut p| p.coordinates(display as usize, window, x, y));
    if let Some((x, y)) = point {
        move_pointer(x, y);
    }
}
pub(super) fn resized(window: c_ulong) {
    if let Ok(mut p) = pointer().lock() {
        p.resize(window);
    }
}

#[no_mangle]
pub unsafe extern "C" fn XGrabPointer(
    display: *mut c_void,
    window: c_ulong,
    owner_events: c_int,
    event_mask: c_uint,
    pointer_mode: c_int,
    keyboard_mode: c_int,
    confine_to: c_ulong,
    cursor: c_ulong,
    time: c_ulong,
) -> c_int {
    let request = Grab {
        display: display as usize,
        window,
        owner_events,
        event_mask,
        pointer_mode,
        keyboard_mode,
        confine_to,
        cursor,
        time,
    };
    if let Ok(mut p) = pointer().lock() {
        p.game_grab(request)
    } else {
        Native.grab(request)
    }
}
#[no_mangle]
pub unsafe extern "C" fn XUngrabPointer(display: *mut c_void, time: c_ulong) -> c_int {
    if let Ok(mut p) = pointer().lock() {
        p.game_ungrab(display as usize, time)
    } else {
        Native.ungrab(display as usize, time)
    }
}
#[no_mangle]
pub unsafe extern "C" fn XChangeActivePointerGrab(
    display: *mut c_void,
    mask: c_uint,
    cursor: c_ulong,
    time: c_ulong,
) -> c_int {
    if let Ok(mut p) = pointer().lock() {
        p.game_change(display as usize, mask, cursor, time)
    } else {
        Native.change(display as usize, mask, cursor, time)
    }
}
#[no_mangle]
pub unsafe extern "C" fn XWarpPointer(
    display: *mut c_void,
    source: c_ulong,
    destination: c_ulong,
    x: c_int,
    y: c_int,
    width: c_uint,
    height: c_uint,
    dx: c_int,
    dy: c_int,
) -> c_int {
    if pointer().lock().is_ok_and(|mut p| p.suppress_warp()) {
        return 1;
    }
    symbol::<WarpFn>(b"XWarpPointer\0")
        .map(|f| f(display, source, destination, x, y, width, height, dx, dy))
        .unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "C" fn XQueryPointer(
    display: *mut c_void,
    window: c_ulong,
    root: *mut c_ulong,
    child: *mut c_ulong,
    root_x: *mut c_int,
    root_y: *mut c_int,
    x: *mut c_int,
    y: *mut c_int,
    mask: *mut c_uint,
) -> c_int {
    let Some(real) = symbol::<QueryFn>(b"XQueryPointer\0") else {
        return 0;
    };
    let result = real(display, window, root, child, root_x, root_y, x, y, mask);
    if result != 0 {
        let saved = pointer()
            .lock()
            .ok()
            .and_then(|mut p| p.frozen(display as usize, window));
        if let Some(p) = saved {
            for (out, value) in [(root_x, p.root_x), (root_y, p.root_y), (x, p.x), (y, p.y)] {
                if !out.is_null() {
                    *out = value;
                }
            }
        }
        if !mask.is_null() {
            if let Ok(s) = crate::state().lock() {
                s.filter_x11_buttons(&mut *mask);
            }
        }
    }
    result
}
#[no_mangle]
pub unsafe extern "C" fn XCloseDisplay(display: *mut c_void) -> c_int {
    if let Ok(mut p) = pointer().lock() {
        p.forget_display(display as usize);
    }
    super::closed_display(display as usize);
    type Close = unsafe extern "C" fn(*mut c_void) -> c_int;
    symbol::<Close>(b"XCloseDisplay\0")
        .map(|f| f(display))
        .unwrap_or(0)
}
#[no_mangle]
pub unsafe extern "C" fn XDestroyWindow(display: *mut c_void, window: c_ulong) -> c_int {
    if let Ok(mut p) = pointer().lock() {
        p.forget_window(window);
    }
    super::destroyed_window(window);
    type Destroy = unsafe extern "C" fn(*mut c_void, c_ulong) -> c_int;
    symbol::<Destroy>(b"XDestroyWindow\0")
        .map(|f| f(display, window))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Fake {
        grabs: Vec<Grab>,
        ungrabs: Vec<usize>,
        freed: Vec<c_ulong>,
        warps: Vec<(i32, i32)>,
        changes: usize,
        focus: c_ulong,
        fail_grab: bool,
    }
    impl Api for Fake {
        fn grab(&mut self, r: Grab) -> c_int {
            self.grabs.push(r);
            i32::from(self.fail_grab)
        }
        fn ungrab(&mut self, d: usize, _time: c_ulong) -> c_int {
            self.ungrabs.push(d);
            1
        }
        fn change(&mut self, _d: usize, _mask: c_uint, _cursor: c_ulong, _time: c_ulong) -> c_int {
            self.changes += 1;
            1
        }
        fn cursor(&mut self, _d: usize) -> c_ulong {
            900
        }
        fn free_cursor(&mut self, _d: usize, c: c_ulong) {
            self.freed.push(c);
        }
        fn query(&mut self, _d: usize, _w: c_ulong) -> Option<Position> {
            Some(Position {
                root: 1,
                root_x: 250,
                root_y: 200,
                x: 150,
                y: 100,
                mask: 0,
            })
        }
        fn geometry(&mut self, _d: usize, _w: c_ulong) -> Option<(u32, u32)> {
            Some((1280, 720))
        }
        fn translate(
            &mut self,
            _d: usize,
            _from: c_ulong,
            _to: c_ulong,
            x: i32,
            y: i32,
        ) -> Option<(i32, i32)> {
            Some((x - 100, y - 100))
        }
        fn focus(&mut self, _d: usize) -> c_ulong {
            self.focus
        }
        fn warp(&mut self, _d: usize, _root: c_ulong, x: i32, y: i32) {
            self.warps.push((x, y));
        }
        fn flush(&mut self, _d: usize) {}
    }
    fn request(display: usize) -> Grab {
        Grab {
            display,
            window: 30,
            owner_events: 0,
            event_mask: 76,
            pointer_mode: 1,
            keyboard_mode: 1,
            confine_to: 30,
            cursor: 500,
            time: 12,
        }
    }
    fn ready() -> Pointer<Fake> {
        Pointer::new(Fake {
            focus: 20,
            ..Fake::default()
        })
    }

    #[test]
    fn native_cursor_replaces_tiny_game_clip_and_restores_latest_request() {
        let mut p = ready();
        assert_eq!(p.game_grab(request(7)), 0);
        p.open(7, 10, &[10, 20]).unwrap();
        let native = *p.api.grabs.last().unwrap();
        assert_eq!(
            (native.window, native.confine_to, native.cursor),
            (10, 10, 900)
        );
        assert_eq!((native.pointer_mode, native.keyboard_mode), (1, 1)); // async, never freeze the seat
        assert_eq!(p.coordinates(7, 10, 640, 360), Some((0.5, 0.5)));
        assert!(p.accepts(7, 30)); // Wine's clipping window is not an ancestor
        p.game_change(7, 80, 501, 13);
        assert_eq!(p.api.changes, 0); // the game's blank cursor cannot replace ours
        assert!(p.suppress_warp());
        assert!(p.api.warps.is_empty());
        p.close(true);
        let restored = *p.api.grabs.last().unwrap();
        assert_eq!(
            (
                restored.window,
                restored.confine_to,
                restored.cursor,
                restored.event_mask,
                restored.time
            ),
            (30, 30, 501, 80, 0)
        );
        assert_eq!(p.api.warps, vec![(250, 200)]);
        assert_eq!(p.api.freed, vec![900]);
        assert!(p.lease.is_none());
        assert!(p.accepts(7, 30)); // a menu-held button's release can arrive here
    }
    #[test]
    fn grab_uses_owning_wine_connection_and_defers_regrabs_from_other_threads() {
        let mut p = ready();
        p.game_grab(request(7));
        p.open(8, 10, &[10, 20]).unwrap(); // F8 and mouse can use different Displays
        assert_eq!(p.api.grabs.last().unwrap().display, 7);
        let mut changed = request(9);
        changed.window = 31;
        changed.confine_to = 31;
        p.game_grab(changed);
        assert_eq!(p.api.grabs.len(), 2);
        p.game_ungrab(7, 0); // cannot cancel the other Wine client's desired grab
        p.close(true);
        assert_eq!(p.api.grabs.last().unwrap().display, 9);
        assert_eq!(p.api.grabs.last().unwrap().window, 31);
    }
    #[test]
    fn alt_tab_and_watchdog_release_without_stealing_desktop_focus_or_warping() {
        for watchdog in [false, true] {
            let mut p = ready();
            p.game_grab(request(7));
            p.open(7, 10, &[10, 20]).unwrap();
            if !watchdog {
                p.api.focus = 99;
            }
            p.close(!watchdog);
            assert_eq!(p.api.grabs.len(), 2);
            assert_eq!(p.api.ungrabs, vec![7]);
            assert!(p.api.warps.is_empty());
            assert!(p.lease.is_none());
        }
    }
    #[test]
    fn failed_capture_does_not_ungrab_another_client_or_leave_a_native_cursor() {
        let mut p = ready();
        p.api.fail_grab = true;
        assert!(p.open(7, 10, &[10, 20]).is_err());
        assert!(p.api.ungrabs.is_empty());
        assert!(p.api.warps.is_empty());
        assert_eq!(p.api.freed, vec![900]);
        assert!(p.lease.is_none());
    }
    #[test]
    fn closing_honors_game_ungrab_and_polling_sees_frozen_game_position() {
        let mut p = ready();
        p.game_grab(request(7));
        p.open(7, 10, &[10, 20]).unwrap();
        let frozen = p.frozen(7, 1).unwrap();
        assert_eq!(
            (frozen.root_x, frozen.root_y, frozen.x, frozen.y),
            (250, 200, 250, 200)
        );
        p.game_ungrab(7, 0);
        assert!(p.api.ungrabs.is_empty());
        p.close(true);
        assert_eq!(p.api.grabs.len(), 2);
        assert_eq!(p.api.ungrabs, vec![7]);
        assert!(p.api.warps.is_empty());
    }
    #[test]
    fn destroy_and_disconnect_drop_pointer_resources_before_the_game_handles() {
        let mut p = ready();
        p.game_grab(request(7));
        p.open(7, 10, &[10, 20]).unwrap();
        p.forget_window(30);
        p.close(true);
        assert_eq!(p.api.grabs.len(), 2); // never restore the destroyed clip window
        p.open(7, 10, &[10, 20]).unwrap();
        p.forget_display(7);
        assert!(p.lease.is_none());
        assert!(!p.requested.contains_key(&7));
        assert_eq!(p.api.freed.len(), 2);
    }
}
