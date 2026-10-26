//! Additional Wine Unix boundary for mouse polling and Raw Input. Native event
//! routing owns the UI cursor; these services must not move the game camera.
//! Resolve service addresses by name, never by Proton-specific syscall numbers.
//! Wine's ELF services use the Unix C ABI, not the Windows PE calling convention.
use std::{ffi::c_void, sync::Mutex};

const HEADER: usize = 24; // RAWINPUTHEADER64, including new WoW64's Unix host.
const MOUSE: usize = 24;
const RID_INPUT: u32 = 0x10000003;
const MAX_BUFFER: usize = 16 * 1024 * 1024;
static CURSOR: Mutex<Option<[i32; 2]>> = Mutex::new(None);

type Register = unsafe extern "C" fn(*const usize, *mut usize, u32, *const u8, u32) -> u8;
type Buffer = unsafe extern "C" fn(*mut c_void, *mut u32, u32) -> u32;
type Data = unsafe extern "C" fn(usize, u32, *mut c_void, *mut u32, u32) -> u32;
type Cursor = unsafe extern "C" fn(*mut i32) -> i32;
type CursorDpi = unsafe extern "C" fn(*mut i32, i32) -> i32;

unsafe fn original<T: Copy>(name: &'static [u8]) -> Option<T> {
    let address = crate::symbols::resolve(name);
    (!address.is_null()).then(|| std::mem::transmute_copy(&address))
}
fn capturing() -> bool {
    // A stalled renderer must never leave the camera blocked indefinitely.
    crate::process_scope::enabled()
        && crate::last_present_age() < std::time::Duration::from_secs(1)
        && crate::state().lock().is_ok_and(|s| s.capturing())
}

/// Only replace a complete, recognized win32u table. Unknown versions retain
/// their original table unchanged. Arguments are Wine's x64 syscall byte counts.
fn redirected(table: &[usize], args: &[u8], originals: [usize; 3]) -> Option<Box<[usize]>> {
    if originals.contains(&0) || table.len() != args.len() {
        return None;
    }
    let mut replacements = [
        raw_buffer as *const () as usize,
        raw_data as *const () as usize,
        cursor_pos as *const () as usize,
    ];
    let sizes = [24, 40, 8];
    let mut indexes = [0; 3];
    for n in 0..3 {
        let mut found = table
            .iter()
            .enumerate()
            .filter(|(_, value)| **value == originals[n]);
        let (index, _) = found.next()?;
        if found.next().is_some() {
            return None;
        }
        // GE-Proton 11-7 has POINT* only (8 argument bytes). Other Wine
        // versions add a DPI argument (16). Forward each with its exact ABI.
        if n == 2 && args[index] == 16 {
            replacements[n] = cursor_pos_dpi as *const () as usize;
        } else if args[index] != sizes[n] {
            return None;
        }
        indexes[n] = index;
    }
    let mut copy = table.to_vec().into_boxed_slice();
    for n in 0..3 {
        copy[indexes[n]] = replacements[n];
    }
    Some(copy)
}

#[no_mangle]
pub unsafe extern "C" fn KeAddSystemServiceTable(
    table: *const usize,
    counters: *mut usize,
    count: u32,
    args: *const u8,
    index: u32,
) -> u8 {
    let Some(real) = original::<Register>(b"KeAddSystemServiceTable\0") else {
        return 0;
    };
    if !crate::process_scope::enabled() || index != 1 || table.is_null() || args.is_null() || count == 0 || count > 4096 {
        return real(table, counters, count, args, index);
    }
    let originals = [
        crate::symbols::resolve(b"NtUserGetRawInputBuffer\0") as usize,
        crate::symbols::resolve(b"NtUserGetRawInputData\0") as usize,
        crate::symbols::resolve(b"NtUserGetCursorPos\0") as usize,
    ];
    let services = std::slice::from_raw_parts(table, count as usize);
    let argument_sizes = std::slice::from_raw_parts(args, count as usize);
    let Some(copy) = redirected(services, argument_sizes, originals) else {
        let sizes = originals.map(|address| {
            if address == 0 {
                return None;
            }
            services
                .iter()
                .position(|entry| *entry == address)
                .map(|index| argument_sizes[index])
        });
        crate::report(&format!("Wine: tabela de entrada não reconhecida; filtro de câmera não instalado. Argumentos Buffer/Data/Cursor: {sizes:?}."));
        return real(table, counters, count, args, index);
    };
    // Wine retains this table. Keep the copied table alive for the process,
    // without changing its read-only original or touching other services.
    let result = real(copy.as_ptr(), counters, count, args, index);
    if result != 0 {
        let _ = Box::leak(copy);
        crate::report("Wine: filtro de movimento Raw Input e consulta do cursor instalado.");
    }
    result
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
/// Keep held-button transitions, keyboard and HID intact. Suppress motion and
/// wheel while the overlay owns input; native events already scroll its UI.
fn filter_packet(packet: &mut [u8]) {
    if read_u32(packet, 0) != Some(0) || packet.len() < HEADER + MOUSE {
        return;
    }
    let flags = u16::from_le_bytes([packet[HEADER], packet[HEADER + 1]]) & !3;
    packet[HEADER..HEADER + 2].copy_from_slice(&flags.to_le_bytes());
    packet[HEADER + 12..HEADER + 20].fill(0);
    // RAWMOUSE.usButtonFlags includes vertical (0x0400) and horizontal (0x0800)
    // wheel events. Leave click/release bits intact so held buttons cannot stick.
    let buttons=u16::from_le_bytes([packet[HEADER+4],packet[HEADER+5]]);
    packet[HEADER+4..HEADER+6].copy_from_slice(&(buttons & !0x0c00).to_le_bytes());
    if buttons & 0x0c00 != 0 {packet[HEADER+6..HEADER+8].fill(0);}

}
fn filter_batch(bytes: &mut [u8], count: u32) {
    // First validate every block, including alignment, before changing any data.
    let mut offset = 0usize;
    for index in 0..count {
        let Some(size) = read_u32(bytes, offset + 4).map(|s| s as usize) else {
            return;
        };
        if size < HEADER || size > bytes.len().saturating_sub(offset) {
            return;
        }
        offset += (size + 7) & !7;
        // Only the final packet may omit its unused trailing padding.
        if index + 1 < count && offset > bytes.len() {
            return;
        }
    }
    offset = 0;
    for _ in 0..count {
        let size = read_u32(bytes, offset + 4).unwrap() as usize;
        filter_packet(&mut bytes[offset..offset + size]);
        offset += (size + 7) & !7;
    }
}
unsafe extern "C" fn raw_buffer(data: *mut c_void, size: *mut u32, header: u32) -> u32 {
    let Some(real) = original::<Buffer>(b"NtUserGetRawInputBuffer\0") else {
        return u32::MAX;
    };
    let capacity = if size.is_null() { 0 } else { *size as usize };
    let result = real(data, size, header);
    if result != u32::MAX
        && result != 0
        && !data.is_null()
        && header as usize == HEADER
        && capacity <= MAX_BUFFER
        && capturing()
    {
        filter_batch(
            std::slice::from_raw_parts_mut(data.cast(), capacity),
            result,
        );
    }
    result
}
unsafe extern "C" fn raw_data(
    handle: usize,
    command: u32,
    data: *mut c_void,
    size: *mut u32,
    header: u32,
) -> u32 {
    let Some(real) = original::<Data>(b"NtUserGetRawInputData\0") else {
        return u32::MAX;
    };
    let capacity = if size.is_null() { 0 } else { *size as usize };
    let result = real(handle, command, data, size, header);
    if command == RID_INPUT
        && result != u32::MAX
        && !data.is_null()
        && header as usize == HEADER
        && result as usize <= capacity
        && result as usize <= MAX_BUFFER
        && capturing()
    {
        let bytes = std::slice::from_raw_parts_mut(data.cast(), result as usize);
        if read_u32(bytes, 4) == Some(result) {
            filter_packet(bytes);
        }
    }
    result
}
fn freeze(position: &mut [i32; 2], saved: &mut Option<[i32; 2]>, capture: bool) {
    if capture {
        *position = *saved.get_or_insert(*position);
    } else {
        *saved = Some(*position);
    }
}
unsafe fn filter_cursor(point: *mut i32, result: i32) {
    if result == 0 || point.is_null() {
        return;
    }
    let capture = capturing();
    if let Ok(mut saved) = CURSOR.lock() {
        let mut position = [*point, *point.add(1)];
        freeze(&mut position, &mut saved, capture);
        *point = position[0];
        *point.add(1) = position[1];
    }
}
unsafe extern "C" fn cursor_pos(point: *mut i32) -> i32 {
    let Some(real) = original::<Cursor>(b"NtUserGetCursorPos\0") else {
        return 0;
    };
    let result = real(point);
    filter_cursor(point, result);
    result
}
unsafe extern "C" fn cursor_pos_dpi(point: *mut i32, dpi: i32) -> i32 {
    let Some(real) = original::<CursorDpi>(b"NtUserGetCursorPos\0") else {
        return 0;
    };
    let result = real(point, dpi);
    filter_cursor(point, result);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(kind: u32, size: usize) -> Vec<u8> {
        let mut p = vec![0x55; size];
        p[..4].copy_from_slice(&kind.to_le_bytes());
        p[4..8].copy_from_slice(&(size as u32).to_le_bytes());
        p
    }
    #[test]
    fn raw_mouse_filters_motion_and_wheel_preserving_button_transitions() {
        let mut p = packet(0, 48);
        let before = p.clone();
        filter_packet(&mut p);
        assert_eq!(&p[36..44], &[0; 8]);
        assert_eq!(p[24] & 3, 0);
        assert_eq!(&p[..24], &before[..24]);
        assert_eq!(&p[26..28], &before[26..28]);
        assert_eq!(u16::from_le_bytes([p[28],p[29]]),0x5555 & !0x0c00);
        assert_eq!(&p[30..32], &[0;2]);
        assert_eq!(&p[32..36], &before[32..36]);
        assert_eq!(&p[44..], &before[44..]);
    }
    #[test]
    fn both_wheel_axes_are_removed_without_losing_held_button_releases(){
        for wheel in [0x0400u16,0x0800,0x0c00] {
            let mut p=packet(0,48);
            p[28..30].copy_from_slice(&(wheel|0x0002|0x0008).to_le_bytes());
            p[30..32].copy_from_slice(&120i16.to_le_bytes());
            filter_packet(&mut p);
            assert_eq!(u16::from_le_bytes([p[28],p[29]]),0x000a);
            assert_eq!(&p[30..32],&[0;2]);
        }
    }
    #[test]
    fn buffered_mouse_keyboard_and_hid_keep_sizes_and_alignment() {
        let mut bytes = packet(0, 48);
        let keyboard = packet(1, 40);
        let hid = packet(2, 33);
        bytes.extend(&keyboard);
        bytes.extend(&hid);
        filter_batch(&mut bytes, 3);
        assert_eq!(&bytes[36..44], &[0; 8]);
        assert_eq!(&bytes[48..88], &keyboard);
        assert_eq!(&bytes[88..], &hid);
    }
    #[test]
    fn malformed_or_truncated_batch_is_unchanged() {
        for count in [2, u32::MAX] {
            let mut bytes = packet(0, 48);
            let before = bytes.clone();
            filter_batch(&mut bytes, count);
            assert_eq!(bytes, before);
        }
        let mut bytes = packet(0, 48);
        bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let before = bytes.clone();
        filter_batch(&mut bytes, 1);
        assert_eq!(bytes, before);
    }
    #[test]
    fn table_is_matched_by_addresses_and_abi_not_service_numbers() {
        let table = [77, 11, 33, 22, 88];
        let args = [0, 24, 16, 40, 0];
        let copy = redirected(&table, &args, [11, 22, 33]).unwrap();
        assert_eq!(copy[0], 77);
        assert_eq!(copy[4], 88);
        assert_ne!(copy[1], 11);
        assert_ne!(copy[2], 33);
        assert_ne!(copy[3], 22);
        assert_eq!(copy[2], cursor_pos_dpi as *const () as usize);
        let ge_proton_args = [0, 24, 8, 40, 0];
        let ge_copy = redirected(&table, &ge_proton_args, [11, 22, 33]).unwrap();
        assert_eq!(ge_copy[2], cursor_pos as *const () as usize);
        assert_eq!(ge_copy[0], 77);
        assert_eq!(ge_copy[4], 88);
        assert!(redirected(&table, &[0, 24, 24, 40, 0], [11, 22, 33]).is_none());
        assert_eq!(table, [77, 11, 33, 22, 88]);
        assert!(redirected(&table, &[0; 5], [11, 22, 33]).is_none());
        assert!(redirected(&table, &args, [11, 22, 0]).is_none());
        assert!(redirected(&[11, 11, 22, 33], &[24, 24, 40, 16], [11, 22, 33]).is_none());
    }
    #[test]
    fn cursor_freezes_only_during_capture_and_resumes_after_close_or_focus_loss() {
        let mut saved = None;
        let mut point = [10, 20];
        freeze(&mut point, &mut saved, false);
        point = [70, 80];
        freeze(&mut point, &mut saved, true);
        assert_eq!(point, [10, 20]);
        point = [90, 100];
        freeze(&mut point, &mut saved, false);
        assert_eq!(point, [90, 100]);
        point = [1, 2];
        freeze(&mut point, &mut saved, true);
        assert_eq!(point, [90, 100]);
    }
}
