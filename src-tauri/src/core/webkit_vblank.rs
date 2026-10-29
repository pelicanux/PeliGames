//! WebKitGTK paces animation frames to a monitor's vblank only after finding the
//! DRM connector of its GdkMonitor, and it compares physical sizes exactly. Wayland
//! compositors that read the EDID detailed timing (KWin) report millimetres while
//! the kernel reports whole centimetres, so WebKit finds no connector and falls back
//! to a fixed 60 Hz timer even on faster monitors.
//!
//! The executable exports the two GDK size getters (see `build.rs`), so WebKit calls
//! these. When no connected connector has the monitor's size, they answer with the
//! kernel size of the connector that does describe this monitor; otherwise GDK's own
//! answer is returned untouched. Remove this module once WebKit matches sizes loosely.

use std::ffi::{c_char, c_int, c_void, CStr};

/// What the kernel and the EDID say about one connected monitor.
#[derive(Debug, Clone, PartialEq)]
struct Connector {
    name: Option<String>,
    /// Size the kernel gives DRM clients (EDID basic block, whole centimetres).
    kernel_mm: (c_int, c_int),
    /// Size of the first detailed timing, which compositors may report instead.
    detailed_mm: Option<(c_int, c_int)>,
}

fn parse_edid(edid: &[u8]) -> Option<Connector> {
    if edid.len() < 128 || edid[..8] != [0, 255, 255, 255, 255, 255, 255, 0] {
        return None;
    }
    let kernel_mm = (c_int::from(edid[21]) * 10, c_int::from(edid[22]) * 10);
    let mut name = None;
    let mut detailed_mm = None;
    for block in edid[54..126].chunks_exact(18) {
        if block[0] != 0 || block[1] != 0 {
            detailed_mm.get_or_insert((
                c_int::from(block[12]) | c_int::from(block[14] >> 4) << 8,
                c_int::from(block[13]) | c_int::from(block[14] & 0x0f) << 8,
            ));
        } else if block[3] == 0xfc {
            let text = &block[5..18];
            let end = text.iter().position(|&byte| byte == b'\n').unwrap_or(text.len());
            name = Some(String::from_utf8_lossy(&text[..end]).trim().to_string());
        }
    }
    Some(Connector { name, kernel_mm, detailed_mm })
}

/// Connectors driving a monitor. `status` is never read: reading it makes the kernel
/// probe every output again, which slows startup and stalls the window.
fn connected() -> Vec<Connector> {
    let Ok(entries) = std::fs::read_dir("/sys/class/drm") else { return Vec::new() };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| std::fs::read_to_string(path.join("enabled")).is_ok_and(|state| state.trim() == "enabled"))
        .filter_map(|path| std::fs::read(path.join("edid")).ok().as_deref().and_then(parse_edid))
        .collect()
}

/// The kernel size WebKit needs for a monitor GDK describes with `size`, or `None`
/// when WebKit already finds a connector or none clearly describes this monitor.
fn kernel_size(connectors: &[Connector], model: Option<&str>, size: (c_int, c_int)) -> Option<(c_int, c_int)> {
    const TOLERANCE_MM: c_int = 10;
    if connectors.iter().any(|connector| connector.kernel_mm == size) {
        return None;
    }
    let close = |connector: &&Connector| {
        connector.detailed_mm == Some(size)
            || ((connector.kernel_mm.0 - size.0).abs() <= TOLERANCE_MM && (connector.kernel_mm.1 - size.1).abs() <= TOLERANCE_MM)
    };
    let candidates: Vec<&Connector> = connectors.iter().filter(close).collect();
    let named = candidates.iter().find(|connector| model.is_some() && connector.name.as_deref() == model);
    named.or(candidates.first()).map(|connector| connector.kernel_mm)
}

type SizeGetter = unsafe extern "C" fn(*mut c_void) -> c_int;
type ModelGetter = unsafe extern "C" fn(*mut c_void) -> *const c_char;

/// GDK's own definition of `name`, found past this executable.
unsafe fn next<T: Copy>(name: &CStr) -> Option<T> {
    let symbol = libc::dlsym(libc::RTLD_NEXT, name.as_ptr());
    (!symbol.is_null()).then(|| std::mem::transmute_copy(&symbol))
}

unsafe fn physical_size(monitor: *mut c_void) -> (c_int, c_int) {
    static GETTERS: std::sync::OnceLock<Option<(SizeGetter, SizeGetter, ModelGetter)>> = std::sync::OnceLock::new();
    let Some((width, height, model)) = *GETTERS.get_or_init(|| unsafe {
        Some((
            next(c"gdk_monitor_get_width_mm")?,
            next(c"gdk_monitor_get_height_mm")?,
            next(c"gdk_monitor_get_model")?,
        ))
    }) else {
        return (0, 0);
    };
    let size = (width(monitor), height(monitor));
    let model = model(monitor);
    let model = (!model.is_null()).then(|| CStr::from_ptr(model).to_string_lossy());
    kernel_size(&connected(), model.as_deref(), size).unwrap_or(size)
}

#[no_mangle]
pub unsafe extern "C" fn gdk_monitor_get_width_mm(monitor: *mut c_void) -> c_int {
    physical_size(monitor).0
}

#[no_mangle]
pub unsafe extern "C" fn gdk_monitor_get_height_mm(monitor: *mut c_void) -> c_int {
    physical_size(monitor).1
}

/// Keeps the exported getters linked into the executable.
pub fn install() {
    std::hint::black_box((gdk_monitor_get_width_mm as *const (), gdk_monitor_get_height_mm as *const ()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edid(cm: (u8, u8), detailed_mm: (u16, u16), name: &str) -> Vec<u8> {
        let mut edid = vec![0u8; 128];
        edid[..8].copy_from_slice(&[0, 255, 255, 255, 255, 255, 255, 0]);
        edid[21] = cm.0;
        edid[22] = cm.1;
        edid[54] = 1;
        edid[66] = detailed_mm.0 as u8;
        edid[67] = detailed_mm.1 as u8;
        edid[68] = ((detailed_mm.0 >> 8) as u8) << 4 | (detailed_mm.1 >> 8) as u8;
        edid[75] = 0xfc;
        let text = format!("{name}\n");
        edid[77..77 + text.len()].copy_from_slice(text.as_bytes());
        edid
    }

    fn monitors() -> Vec<Connector> {
        [edid((60, 34), (597, 336), "Q27G4"), edid((60, 33), (597, 336), "MGN-004-27S"), edid((80, 45), (576, 324), "HichipTV")]
            .iter()
            .map(|edid| parse_edid(edid).unwrap())
            .collect()
    }

    #[test]
    fn reads_kernel_size_detailed_size_and_name() {
        assert_eq!(
            monitors()[0],
            Connector { name: Some("Q27G4".into()), kernel_mm: (600, 340), detailed_mm: Some((597, 336)) }
        );
        assert_eq!(parse_edid(&[0; 128]), None);
    }

    #[test]
    fn answers_with_the_kernel_size_of_the_same_monitor() {
        let connectors = monitors();
        assert_eq!(kernel_size(&connectors, Some("Q27G4"), (597, 336)), Some((600, 340)));
        assert_eq!(kernel_size(&connectors, Some("MGN-004-27S"), (597, 336)), Some((600, 330)));
        assert_eq!(kernel_size(&connectors, Some("HichipTV"), (576, 324)), Some((800, 450)));
        assert_eq!(kernel_size(&connectors, None, (597, 336)), Some((600, 340)));
    }

    #[test]
    fn leaves_sizes_webkit_already_finds_or_cannot_place() {
        let connectors = monitors();
        assert_eq!(kernel_size(&connectors, Some("Q27G4"), (600, 340)), None);
        assert_eq!(kernel_size(&connectors, None, (300, 200)), None);
        assert_eq!(kernel_size(&[], None, (597, 336)), None);
    }
}
