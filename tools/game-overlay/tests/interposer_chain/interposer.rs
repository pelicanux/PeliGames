//! Model an overlay forwarding via RTLD_NEXT, falling back to a local X11 DSO.
use std::{
    ffi::{c_void, CString},
    sync::atomic::{AtomicUsize, Ordering},
};
#[link(name = "dl")]
extern "C" {
    fn dlopen(path: *const i8, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const i8) -> *mut c_void;
}
static CALLS: AtomicUsize = AtomicUsize::new(0);
#[no_mangle]
pub unsafe extern "C" fn XCheckIfEvent(
    d: *mut c_void,
    e: *mut c_void,
    p: *mut c_void,
    a: *mut c_void,
) -> i32 {
    // Turn an audit-induced recursion into an assertion failure in the probe.
    if CALLS.fetch_add(1, Ordering::Relaxed) > 0 {
        return -700;
    }
    let name = b"XCheckIfEvent\0";
    let mut next = dlsym(-1isize as *mut c_void, name.as_ptr().cast());
    if next.is_null() {
        let path = CString::new(std::env::var("PROBE_X11").unwrap()).unwrap();
        let handle = dlopen(path.as_ptr(), 2);
        assert!(!handle.is_null());
        next = dlsym(handle, name.as_ptr().cast());
    }
    assert!(!next.is_null());
    let next: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32 =
        std::mem::transmute(next);
    next(d, e, p, a)
}
#[no_mangle]
pub extern "C" fn probe_calls() -> usize {
    CALLS.load(Ordering::Relaxed)
}
