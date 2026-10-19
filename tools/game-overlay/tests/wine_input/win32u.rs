use std::{
    ffi::c_void,
    sync::atomic::{AtomicI32, Ordering},
};
static POSITION: AtomicI32 = AtomicI32::new(100);
#[no_mangle]
pub extern "C" fn probe_move_cursor(value: i32) {
    POSITION.store(value, Ordering::Relaxed);
}
extern "C" {
    fn KeAddSystemServiceTable(
        table: *const usize,
        counters: *mut usize,
        count: u32,
        args: *const u8,
        index: u32,
    ) -> u8;
}
#[no_mangle]
pub unsafe extern "C" fn NtUserGetRawInputBuffer(
    data: *mut c_void,
    size: *mut u32,
    header: u32,
) -> u32 {
    if size.is_null() || header != 24 {
        return u32::MAX;
    }
    if data.is_null() {
        *size = 48;
        return 0;
    }
    if *size < 48 {
        return u32::MAX;
    }
    std::ptr::write_bytes(data, 0x55, 48);
    let bytes = std::slice::from_raw_parts_mut(data.cast::<u8>(), 48);
    bytes[..4].copy_from_slice(&0u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&48u32.to_le_bytes());
    *size = 0;
    1
}
#[no_mangle]
pub unsafe extern "C" fn NtUserGetRawInputData(
    _: usize,
    command: u32,
    data: *mut c_void,
    size: *mut u32,
    header: u32,
) -> u32 {
    if command != 0x10000003 {
        return u32::MAX;
    }
    let value = NtUserGetRawInputBuffer(data, size, header);
    if value == u32::MAX || data.is_null() {
        value
    } else {
        *size = 48;
        48
    }
}
#[cfg(cursor_one)]
#[no_mangle]
pub unsafe extern "C" fn NtUserGetCursorPos(point: *mut i32) -> i32 {
    write_cursor(point)
}
#[cfg(not(cursor_one))]
#[no_mangle]
pub unsafe extern "C" fn NtUserGetCursorPos(point: *mut i32, dpi: i32) -> i32 {
    assert_eq!(dpi, 96);
    write_cursor(point)
}
unsafe fn write_cursor(point: *mut i32) -> i32 {
    if point.is_null() {
        return 0;
    }
    *point = POSITION.load(Ordering::Relaxed);
    *point.add(1) = *point + 1;
    1
}
extern "C" fn untouched() -> u32 {
    0x12345678
}
#[no_mangle]
pub unsafe extern "C" fn probe_register() -> u8 {
    let table = Box::leak(Box::new([
        NtUserGetRawInputData as *const () as usize,
        untouched as *const () as usize,
        NtUserGetCursorPos as *const () as usize,
        NtUserGetRawInputBuffer as *const () as usize,
    ]));
    #[cfg(cursor_one)]
    static ARGS: [u8; 4] = [40, 0, 8, 24];
    #[cfg(not(cursor_one))]
    static ARGS: [u8; 4] = [40, 0, 16, 24];
    KeAddSystemServiceTable(table.as_ptr(), std::ptr::null_mut(), 4, ARGS.as_ptr(), 1)
}
