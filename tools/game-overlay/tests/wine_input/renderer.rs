//! Loader fixture: production Wine filter with a synthetic capture state.
#![allow(dead_code)]
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex, OnceLock,
};
#[path = "../../symbols.rs"]
mod abi;
#[path = "../../src/wine_input.rs"]
mod wine_input;
#[no_mangle]
pub static peli_overlay_originals: [AtomicUsize; abi::SYMBOLS.len()] =
    [const { AtomicUsize::new(0) }; abi::SYMBOLS.len()];
static CAPTURE: AtomicBool = AtomicBool::new(false);
struct Input;
impl Input {
    fn capturing(&self) -> bool {
        CAPTURE.load(Ordering::Relaxed)
    }
}
fn state() -> &'static Mutex<Input> {
    static S: OnceLock<Mutex<Input>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Input))
}
fn last_present_age() -> std::time::Duration {
    std::time::Duration::ZERO
}
fn report(message: &str) {
    eprintln!("[fixture] {message}");
}
mod symbols {
    pub unsafe fn resolve(name: &'static [u8]) -> *mut std::ffi::c_void {
        crate::abi::SYMBOLS
            .iter()
            .position(|(n, _, _)| name.strip_suffix(&[0]) == Some(*n))
            .map(|i| {
                crate::peli_overlay_originals[i].load(std::sync::atomic::Ordering::Acquire)
                    as *mut _
            })
            .unwrap_or(std::ptr::null_mut())
    }
}
#[no_mangle]
pub extern "C" fn probe_capture(value: bool) {
    CAPTURE.store(value, Ordering::Relaxed)
}
