use std::ffi::{c_char, c_void, CString};
#[link(name = "dl")]
extern "C" {
    fn dlopen(path: *const c_char, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
}
unsafe fn open(path: &str, flags: i32) -> *mut c_void {
    let path = CString::new(path).unwrap();
    let h = dlopen(path.as_ptr(), flags);
    assert!(!h.is_null(), "{:?}", std::ffi::CStr::from_ptr(dlerror()));
    h
}
unsafe fn symbol<T: Copy>(h: *mut c_void, name: &str) -> T {
    let name = CString::new(name).unwrap();
    let p = dlsym(h, name.as_ptr());
    assert!(!p.is_null());
    std::mem::transmute_copy(&p)
}
fn main() {
    unsafe {
        let dir = std::env::args().nth(1).unwrap();
        let renderer = open(&format!("{dir}/libpeligames_game_overlay.so"), 2 | 0x100);
        let ntdll = open(&format!("{dir}/ntdll.so"), 2 | 0x100);
        let win32u = open(&format!("{dir}/win32u.so"), 2);
        let register: unsafe extern "C" fn() -> u8 = symbol(win32u, "probe_register");
        assert_eq!(register(), 1);
        let get: unsafe extern "C" fn() -> *const usize = symbol(ntdll, "probe_table");
        let table = get();
        let data: unsafe extern "C" fn(usize, u32, *mut c_void, *mut u32, u32) -> u32 =
            std::mem::transmute(*table);
        let untouched: unsafe extern "C" fn() -> u32 = std::mem::transmute(*table.add(1));
        assert_eq!(untouched(), 0x12345678);
        #[cfg(cursor_one)]
        let cursor_fn: unsafe extern "C" fn(*mut i32) -> i32 = std::mem::transmute(*table.add(2));
        #[cfg(not(cursor_one))]
        let cursor_fn: unsafe extern "C" fn(*mut i32, i32) -> i32 =
            std::mem::transmute(*table.add(2));
        let move_cursor: unsafe extern "C" fn(i32) = symbol(win32u, "probe_move_cursor");
        let cursor = |point: *mut i32, position: i32| {
            move_cursor(position);
            #[cfg(cursor_one)]
            {
                cursor_fn(point)
            }
            #[cfg(not(cursor_one))]
            {
                cursor_fn(point, 96)
            }
        };
        let buffer: unsafe extern "C" fn(*mut c_void, *mut u32, u32) -> u32 =
            std::mem::transmute(*table.add(3));
        let capture: unsafe extern "C" fn(bool) = symbol(renderer, "probe_capture");
        let mut point = [0i32; 2];
        assert_eq!(cursor(point.as_mut_ptr(), 100), 1);
        assert_eq!(point, [100, 101]);
        for active in [false, true, false, true, false] {
            capture(active);
            let mut bytes = [0u8; 48];
            let mut size = 48;
            assert_eq!(buffer(bytes.as_mut_ptr().cast(), &mut size, 24), 1);
            assert_eq!(&bytes[36..44], if active { &[0; 8] } else { &[0x55; 8] });
            assert_eq!(&bytes[26..36], &[0x55; 10]);
            size = 48;
            assert_eq!(
                data(1, 0x10000003, bytes.as_mut_ptr().cast(), &mut size, 24),
                48
            );
            assert_eq!(&bytes[36..44], if active { &[0; 8] } else { &[0x55; 8] });
            if active {
                assert_eq!(cursor(point.as_mut_ptr(), 200), 1);
                assert_eq!(point, [100, 101]);
            }
            size = 0;
            assert_eq!(buffer(std::ptr::null_mut(), &mut size, 24), 0);
            assert_eq!(size, 48);
            size = 48;
            assert_eq!(
                data(1, 0, bytes.as_mut_ptr().cast(), &mut size, 24),
                u32::MAX
            );
        }
        capture(false);
        assert_eq!(cursor(point.as_mut_ptr(), 300), 1);
        assert_eq!(point, [300, 301]);
        println!("Wine Unix registration/RTLD_LOCAL, raw batch/single reads, errors, cursor freeze and restoration: OK");
    }
}
