//! RTLD_NEXT alone cannot forward Wine's RTLD_LOCAL imports. The audit adapter
//! publishes the original DSO's functions before redirecting any caller.
use std::{
    ffi::c_void,
    sync::atomic::{AtomicUsize, Ordering},
};
#[path = "../symbols.rs"]
pub(crate) mod abi;

#[no_mangle]
pub static peli_overlay_originals: [AtomicUsize; abi::SYMBOLS.len()] =
    [const { AtomicUsize::new(0) }; abi::SYMBOLS.len()];

pub unsafe fn resolve(name: &'static [u8]) -> *mut c_void {
    let Some(name_without_nul) = name.strip_suffix(&[0]) else {
        return std::ptr::null_mut();
    };
    if let Some(index) = abi::SYMBOLS
        .iter()
        .position(|(n, _, _)| *n == name_without_nul)
    {
        let address = peli_overlay_originals[index].load(Ordering::Acquire);
        if address != 0 {
            return address as *mut c_void;
        }
    }
    // Normal, globally visible clients can still forward without the audit
    // adapter. Do not retain this fallback across dlclose/dlopen boundaries.
    libc::dlsym(libc::RTLD_NEXT, name.as_ptr().cast())
}
