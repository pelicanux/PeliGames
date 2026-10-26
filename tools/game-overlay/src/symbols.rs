//! RTLD_NEXT alone cannot forward Wine's RTLD_LOCAL imports. The audit adapter
//! publishes local originals as a fallback; globally visible interposers must
//! remain in the forwarding chain (for example Steam's X11 input hooks).
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock,
    },
};
#[path = "../symbols.rs"]
pub(crate) mod abi;

#[no_mangle]
pub static peli_overlay_originals: [AtomicUsize; abi::SYMBOLS.len()] =
    [const { AtomicUsize::new(0) }; abi::SYMBOLS.len()];

// Calling an interposed dlsym changes the caller used for RTLD_NEXT. In
// particular, the MangoHUD shim can then skip Steam's hooks or resolve back
// into this library. Obtain glibc's function from its already-loaded DSO;
// RTLD_NOLOAD never loads a second libc or initializes another dependency.
unsafe fn next(name: &'static [u8]) -> *mut c_void {
    type Lookup = unsafe extern "C" fn(*mut c_void, *const libc::c_char) -> *mut c_void;
    static LOOKUP: OnceLock<Option<Lookup>> = OnceLock::new();
    let lookup = LOOKUP.get_or_init(|| {
        let handle = libc::dlopen(c"libc.so.6".as_ptr(), libc::RTLD_LAZY | libc::RTLD_NOLOAD);
        if handle.is_null() {
            return None;
        }
        // The overlay targets Linux/glibc x86_64, where this public version
        // is provided alongside GLIBC_2.34 on newer distributions.
        let address = libc::dlvsym(handle, c"dlsym".as_ptr(), c"GLIBC_2.2.5".as_ptr());
        libc::dlclose(handle);
        if address.is_null() {
            None
        } else {
            Some(std::mem::transmute::<*mut c_void, Lookup>(address))
        }
    });
    match lookup {
        Some(lookup) => lookup(libc::RTLD_NEXT, name.as_ptr().cast()),
        None => std::ptr::null_mut(),
    }
}

pub unsafe fn resolve(name: &'static [u8]) -> *mut c_void {
    let Some(name_without_nul) = name.strip_suffix(&[0]) else {
        return std::ptr::null_mut();
    };
    // Resolve on every call: a cached interposer can be unloaded. The audit
    // adapter leaves other interposers' own forwarding lookups untouched.
    let next = next(name);
    if !next.is_null() {
        return next;
    }
    if let Some(index) = abi::SYMBOLS
        .iter()
        .position(|(n, _, _)| *n == name_without_nul)
    {
        let address = peli_overlay_originals[index].load(Ordering::Acquire);
        if address != 0 {
            return address as *mut c_void;
        }
    }
    std::ptr::null_mut()
}
