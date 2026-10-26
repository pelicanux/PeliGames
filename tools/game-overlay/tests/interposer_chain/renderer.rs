//! Use the production resolver with a synthetic X11 wrapper.
extern crate self as libc;
pub use std::ffi::{c_char, c_void};
pub const RTLD_LAZY: i32 = 1;
pub const RTLD_NOLOAD: i32 = 4;
pub const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;
#[link(name = "dl")]
extern "C" {
    pub fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
    pub fn dlclose(handle: *mut c_void) -> i32;
    pub fn dlvsym(handle: *mut c_void, name: *const c_char, version: *const c_char) -> *mut c_void;
    pub fn dlsym(handle: *mut c_void, name: *const std::ffi::c_char) -> *mut c_void;
}
#[path = "../../src/symbols.rs"]
mod symbols;
use std::sync::atomic::{AtomicUsize, Ordering};
static CALLS: AtomicUsize = AtomicUsize::new(0);
#[no_mangle]
pub unsafe extern "C" fn XCheckIfEvent(
    d: *mut c_void,
    e: *mut c_void,
    p: *mut c_void,
    a: *mut c_void,
) -> i32 {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let next = symbols::resolve(b"XCheckIfEvent\0");
    assert!(!next.is_null());
    let next: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32 =
        std::mem::transmute(next);
    next(d, e, p, a)
}
#[no_mangle]
pub extern "C" fn probe_calls() -> usize {
    CALLS.load(Ordering::Relaxed)
}

#[no_mangle]
pub unsafe extern "C" fn probe_resolve() -> *mut c_void {
    symbols::resolve(b"XCheckIfEvent\0")
}
