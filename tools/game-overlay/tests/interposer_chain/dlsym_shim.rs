//! Model a preload shim that intercepts dlsym and shifts RTLD_NEXT's caller.
use std::{
    ffi::{c_char, c_void, CStr},
    sync::atomic::{AtomicUsize, Ordering},
};
#[link(name = "dl")]
extern "C" {
    fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
    fn dlvsym(handle: *mut c_void, name: *const c_char, version: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
}
static CALLS: AtomicUsize = AtomicUsize::new(0);
#[no_mangle]
pub unsafe extern "C" fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void {
    if !name.is_null() && CStr::from_ptr(name).to_bytes() == b"XCheckIfEvent" {
        CALLS.fetch_add(1, Ordering::Relaxed);
    }
    let libc = dlopen(c"libc.so.6".as_ptr(), 1 | 4);
    assert!(!libc.is_null());
    let p = dlvsym(libc, c"dlsym".as_ptr(), c"GLIBC_2.2.5".as_ptr());
    assert!(!p.is_null());
    dlclose(libc);
    let real: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void =
        std::mem::transmute(p);
    real(handle, name)
}
#[no_mangle]
pub extern "C" fn probe_shim_calls() -> usize {
    CALLS.load(Ordering::Relaxed)
}
