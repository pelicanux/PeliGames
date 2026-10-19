use std::sync::atomic::{AtomicUsize, Ordering};
static TABLE: AtomicUsize = AtomicUsize::new(0);
#[no_mangle]
pub unsafe extern "C" fn KeAddSystemServiceTable(
    table: *const usize,
    _: *mut usize,
    count: u32,
    _: *const u8,
    index: u32,
) -> u8 {
    assert_eq!(count, 4);
    assert_eq!(index, 1);
    TABLE.store(table as usize, Ordering::Relaxed);
    1
}
#[no_mangle]
pub extern "C" fn probe_table() -> *const usize {
    TABLE.load(Ordering::Relaxed) as *const usize
}
