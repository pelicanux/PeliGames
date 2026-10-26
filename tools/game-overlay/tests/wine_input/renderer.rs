//! Loader fixture: production Wine filter with a synthetic capture state.
#![allow(dead_code)]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};
extern crate self as libc;
pub use std::ffi::{c_char, c_void};
pub const RTLD_NEXT: *mut c_void = -1isize as *mut c_void;
pub const RTLD_LAZY: i32 = 1;
pub const RTLD_NOLOAD: i32 = 4;
#[link(name = "dl")]
extern "C" {
    pub fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
    pub fn dlclose(handle: *mut c_void) -> i32;
    pub fn dlvsym(handle: *mut c_void, name: *const c_char, version: *const c_char) -> *mut c_void;
    pub fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
}
#[path = "../../src/process_scope.rs"]
mod process_scope;
#[path = "../../src/symbols.rs"]
mod symbols;
#[path = "../../src/wine_input.rs"]
mod wine_input;
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
#[no_mangle]
pub extern "C" fn probe_capture(value: bool) {
    CAPTURE.store(value, Ordering::Relaxed)
}
