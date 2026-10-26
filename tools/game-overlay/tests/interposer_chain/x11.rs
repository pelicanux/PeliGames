use std::{
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};
static CALLS: AtomicUsize = AtomicUsize::new(0);
#[no_mangle]
pub extern "C" fn XCheckIfEvent(
    d: *mut c_void,
    e: *mut c_void,
    p: *mut c_void,
    a: *mut c_void,
) -> i32 {
    assert_eq!(
        [d as usize, e as usize, p as usize, a as usize],
        [11, 22, 33, 44]
    );
    CALLS.fetch_add(1, Ordering::Relaxed);
    73
}
#[no_mangle]
pub extern "C" fn probe_calls() -> usize {
    CALLS.load(Ordering::Relaxed)
}
