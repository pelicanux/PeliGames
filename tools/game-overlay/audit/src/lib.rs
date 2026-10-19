//! glibc rtld-audit adapter. No dlopen/dlsym, allocations, locks, or I/O in loader
//! callbacks. Resolve the base-namespace wrapper from its public ELF tables.
//! Only Linux x86_64 is supported by this prototype.
#![no_std]
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("O audit shim inicial suporta Linux/glibc x86_64.");
use core::{
    ffi::{c_char, c_void},
    sync::atomic::{AtomicUsize, Ordering},
};
#[path = "../../symbols.rs"]
mod abi;
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    extern "C" {
        fn abort() -> !;
    }
    unsafe { abort() }
}
#[repr(C)]
pub struct LinkMap {
    base: usize,
    name: *const c_char,
    dynamic: *const Dyn,
    next: *mut c_void,
    previous: *mut c_void,
}
#[repr(C)]
pub struct Dyn {
    tag: i64,
    value: usize,
}
#[repr(C)]
pub struct Sym {
    name: u32,
    info: u8,
    other: u8,
    section: u16,
    value: usize,
    size: usize,
}
static WRAPPERS: [AtomicUsize; abi::SYMBOLS.len()] =
    [const { AtomicUsize::new(0) }; abi::SYMBOLS.len()];
static ORIGINALS: [AtomicUsize; abi::SYMBOLS.len()] =
    [const { AtomicUsize::new(0) }; abi::SYMBOLS.len()];
static OWNERS: [AtomicUsize; abi::SYMBOLS.len()] =
    [const { AtomicUsize::new(0) }; abi::SYMBOLS.len()];
static SHARED: AtomicUsize = AtomicUsize::new(0);
static COOKIE: AtomicUsize = AtomicUsize::new(0);
unsafe fn equals(p: *const c_char, name: &[u8]) -> bool {
    if p.is_null() {
        return false;
    }
    for (i, b) in name.iter().enumerate() {
        if *(p.add(i) as *const u8) != *b {
            return false;
        }
    }
    *p.add(name.len()) == 0
}
unsafe fn basename(mut p: *const c_char) -> *const c_char {
    if p.is_null() {
        return p;
    }
    let mut basename = p;
    for _ in 0..4096 {
        let c = *p;
        if c == 0 {
            return basename;
        }
        if c == b'/' as c_char {
            basename = p.add(1);
        }
        p = p.add(1);
    }
    core::ptr::null()
}
unsafe fn publish(index: usize, address: usize) {
    ORIGINALS[index].store(address, Ordering::Release);
    let shared = SHARED.load(Ordering::Acquire) as *const AtomicUsize;
    if !shared.is_null() {
        (*shared.add(index)).store(address, Ordering::Release);
    }
}
fn hash(name: &[u8]) -> u32 {
    name.iter()
        .fold(5381u32, |h, c| h.wrapping_mul(33).wrapping_add(*c as u32))
}
unsafe fn find(map: &LinkMap, wanted: &[u8]) -> usize {
    let (mut symbols, mut strings, mut strings_len, mut gnu) = (0usize, 0usize, 0usize, 0usize);
    let mut dynamic = map.dynamic;
    if dynamic.is_null() {
        return 0;
    }
    for _ in 0..4096 {
        let d = &*dynamic;
        if d.tag == 0 {
            break;
        }
        // glibc normally relocates pointer tags in-place. Accept relative
        // values as well, but do not apply relocation to integer sizes.
        let pointer = if d.value < map.base {
            map.base.wrapping_add(d.value)
        } else {
            d.value
        };
        match d.tag {
            6 => symbols = pointer,
            5 => strings = pointer,
            10 => strings_len = d.value,
            0x6ffffef5 => gnu = pointer,
            _ => {}
        }
        dynamic = dynamic.add(1);
    }
    if symbols == 0 || strings == 0 || strings_len == 0 || gnu == 0 {
        return 0;
    }
    let table = gnu as *const u32;
    let buckets = *table as usize;
    let offset = *table.add(1) as usize;
    let bloom = *table.add(2) as usize;
    if buckets == 0 || buckets > 1_000_000 || bloom > 1_000_000 {
        return 0;
    }
    let bucket = table.add(4).cast::<usize>().add(bloom).cast::<u32>();
    let chains = bucket.add(buckets);
    let h = hash(wanted);
    let mut index = *bucket.add(h as usize % buckets) as usize;
    if index < offset {
        return 0;
    }
    for _ in 0..65536 {
        let chain = *chains.add(index - offset);
        if chain | 1 == h | 1 {
            let sym = &*(symbols as *const Sym).add(index);
            let name = sym.name as usize;
            if name
                .checked_add(wanted.len())
                .is_some_and(|end| end < strings_len)
                && equals((strings + name) as *const c_char, wanted)
                && sym.section != 0
            {
                return map.base.wrapping_add(sym.value);
            }
        }
        if chain & 1 != 0 {
            break;
        }
        index += 1;
    }
    0
}
#[no_mangle]
pub extern "C" fn la_version(v: u32) -> u32 {
    v.min(2)
}
#[no_mangle]
pub unsafe extern "C" fn la_objopen(
    map: *mut LinkMap,
    namespace: isize,
    cookie: *mut usize,
) -> u32 {
    if map.is_null() || namespace != 0 || cookie.is_null() {
        return 0;
    }
    let name = basename((*map).name);
    if equals(name, b"libpeligames_game_overlay.so") {
        let shared = find(&*map, b"peli_overlay_originals");
        if shared != 0 {
            COOKIE.store(*cookie, Ordering::Release);
            SHARED.store(shared, Ordering::Release);
            for (i, (symbol, _, redirect)) in abi::SYMBOLS.iter().enumerate() {
                if *redirect {
                    WRAPPERS[i].store(find(&*map, symbol), Ordering::Release);
                }
                publish(i, ORIGINALS[i].load(Ordering::Acquire));
            }
        }
    } else {
        let library = if equals(name, b"libwayland-client.so.0") {
            abi::WAYLAND
        } else if equals(name, b"libX11.so.6") {
            abi::X11
        } else if equals(name, b"libVkLayer_WINELAND_translate_x86_64.so") {
            abi::WINELAND
        } else {
            0
        };
        if library != 0 {
            for (i, (symbol, owner, _)) in abi::SYMBOLS.iter().enumerate() {
                if *owner == library {
                    let address = find(&*map, symbol);
                    if address != 0 {
                        OWNERS[i].store(*cookie, Ordering::Release);
                        publish(i, address);
                    }
                }
            }
        }
    }
    3 // LA_FLG_BINDTO | LA_FLG_BINDFROM; actual redirection is symbol-specific.
}
#[no_mangle]
pub unsafe extern "C" fn la_symbind64(
    sym: *mut Sym,
    _index: u32,
    from: *mut usize,
    to: *mut usize,
    _flags: *mut u32,
    name: *const c_char,
) -> usize {
    if sym.is_null() {
        return 0;
    }
    let owner = COOKIE.load(Ordering::Acquire);
    if owner != 0 && !from.is_null() && !to.is_null() && *from != owner && *to != owner {
        for (i, (symbol, _, redirect)) in abi::SYMBOLS.iter().enumerate() {
            let wrapper = WRAPPERS[i].load(Ordering::Acquire);
            // A matching name in another interposer is not the defining DSO.
            // Only intercept when forwarding to the original is already known.
            if *redirect
                && wrapper != 0
                && *to == OWNERS[i].load(Ordering::Acquire)
                && ORIGINALS[i].load(Ordering::Acquire) != 0
                && equals(name, symbol)
            {
                return wrapper;
            }
        }
    }
    (*sym).value
}
#[no_mangle]
pub unsafe extern "C" fn la_objclose(cookie: *mut usize) -> u32 {
    if !cookie.is_null() && *cookie == COOKIE.load(Ordering::Acquire) {
        SHARED.store(0, Ordering::Release);
        for wrapper in &WRAPPERS {
            wrapper.store(0, Ordering::Release);
        }
        COOKIE.store(0, Ordering::Release);
    }
    if !cookie.is_null() {
        for (i, owner) in OWNERS.iter().enumerate() {
            if owner.load(Ordering::Acquire) == *cookie {
                owner.store(0, Ordering::Release);
                publish(i, 0);
            }
        }
    }
    0
}
