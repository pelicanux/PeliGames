//! Interoperability with Wineland's versioned surface-registration ABI.
//! Its Xlib window is only a Steam input proxy, never the game's X11 window.
//! Observe the successful registration and preserve its original return value;
//! do not change the translator, layer order or the game's Wayland objects.
use crate::Target;
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Copy)]
struct Registration {
    proxy_display: usize,
    proxy_window: u64,
    surface: usize,
}
#[derive(Default)]
struct Registrations(HashMap<usize, Registration>);
impl Registrations {
    fn update(
        &mut self,
        instance: usize,
        display: usize,
        window: u64,
        wl_display: usize,
        surface: usize,
        result: i32,
    ) {
        // A rejected registration/clear must not invalidate the translator's
        // previous accepted mapping. Never dereference any borrowed pointer.
        if result != 0 {
            return;
        }
        if wl_display == 0 && surface == 0 {
            self.0.remove(&instance);
        } else if instance != 0 && display != 0 && window != 0 && wl_display != 0 && surface != 0 {
            if self.0.len() < 256 || self.0.contains_key(&instance) {
                self.0.insert(
                    instance,
                    Registration {
                        proxy_display: display,
                        proxy_window: window,
                        surface,
                    },
                );
            }
        }
    }
    fn target(&self, instance: usize, display: usize, window: u64) -> Option<Target> {
        let entry = self.0.get(&instance)?;
        (entry.proxy_display == display && entry.proxy_window == window)
            .then_some(Target::Wayland(entry.surface))
    }
}
static REGISTRATIONS: OnceLock<Mutex<Registrations>> = OnceLock::new();
fn registrations() -> &'static Mutex<Registrations> {
    REGISTRATIONS.get_or_init(Default::default)
}
pub(crate) fn target(instance: usize, display: usize, window: u64) -> Option<Target> {
    registrations()
        .lock()
        .ok()?
        .target(instance, display, window)
}
pub(crate) fn forget(instance: usize) {
    if let Ok(mut state) = registrations().lock() {
        state.0.remove(&instance);
    }
}

/// Public v1 ABI from Wineland 11.0-20260913 (not a Vulkan entry point).
/// rtld-audit resolves its defining DSO even when Wine loads it RTLD_LOCAL.
#[no_mangle]
pub unsafe extern "C" fn __wine_wayland_vulkan_register_surface_v1(
    instance: *mut c_void,
    display: *mut c_void,
    window: u64,
    wl_display: *mut c_void,
    surface: *mut c_void,
) -> i32 {
    type Register =
        unsafe extern "C" fn(*mut c_void, *mut c_void, u64, *mut c_void, *mut c_void) -> i32;
    let real = crate::symbols::resolve(b"__wine_wayland_vulkan_register_surface_v1\0");
    if real.is_null() {
        return -3;
    } // VK_ERROR_INITIALIZATION_FAILED
    let real: Register = std::mem::transmute(real);
    let result = real(instance, display, window, wl_display, surface);
    if let Ok(mut state) = registrations().lock() {
        state.update(
            instance as usize,
            display as usize,
            window,
            wl_display as usize,
            surface as usize,
            result,
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapper_forwards_v1_arguments_and_tracks_only_successful_calls() {
        unsafe extern "C" fn original(
            i: *mut c_void,
            d: *mut c_void,
            w: u64,
            wd: *mut c_void,
            s: *mut c_void,
        ) -> i32 {
            assert_eq!((i as usize, d as usize, w), (91, 92, 93));
            if wd as usize == 99 {
                -3
            } else {
                assert!(matches!((wd as usize, s as usize), (94, 95) | (0, 0)));
                0
            }
        }
        let index = crate::symbols::abi::SYMBOLS
            .iter()
            .position(|(_, owner, _)| *owner == crate::symbols::abi::WINELAND)
            .unwrap();
        let slot = &crate::symbols::peli_overlay_originals[index];
        slot.store(
            original as *const () as usize,
            std::sync::atomic::Ordering::Release,
        );
        unsafe {
            assert_eq!(
                __wine_wayland_vulkan_register_surface_v1(91 as _, 92 as _, 93, 94 as _, 95 as _),
                0
            );
            assert_eq!(target(91, 92, 93), Some(Target::Wayland(95)));
            assert_eq!(
                __wine_wayland_vulkan_register_surface_v1(91 as _, 92 as _, 93, 99 as _, 95 as _),
                -3
            );
            assert_eq!(target(91, 92, 93), Some(Target::Wayland(95)));
            assert_eq!(
                __wine_wayland_vulkan_register_surface_v1(
                    91 as _,
                    92 as _,
                    93,
                    std::ptr::null_mut(),
                    std::ptr::null_mut()
                ),
                0
            );
            assert_eq!(target(91, 92, 93), None);
        }
        forget(91);
        slot.store(0, std::sync::atomic::Ordering::Release);
    }
    #[test]
    fn translated_proxy_resolves_only_the_exact_instance_display_and_window() {
        let mut map = Registrations::default();
        map.update(1, 2, 3, 4, 5, 0);
        assert_eq!(map.target(1, 2, 3), Some(Target::Wayland(5)));
        for (i, d, w) in [(9, 2, 3), (1, 9, 3), (1, 2, 9)] {
            assert_eq!(map.target(i, d, w), None);
        }
    }
    #[test]
    fn failed_registration_preserves_mapping_and_successful_clear_removes_it() {
        let mut map = Registrations::default();
        map.update(1, 2, 3, 4, 5, 0);
        map.update(1, 2, 3, 4, 6, -3);
        map.update(1, 2, 3, 0, 0, -3);
        assert_eq!(map.target(1, 2, 3), Some(Target::Wayland(5)));
        map.update(1, 2, 3, 0, 0, 0);
        assert_eq!(map.target(1, 2, 3), None);
        map.update(1, 2, 3, 4, 7, 0);
        assert_eq!(map.target(1, 2, 3), Some(Target::Wayland(7)));
    }
    #[test]
    fn independent_instances_and_invalid_registrations_do_not_alias() {
        let mut map = Registrations::default();
        map.update(1, 2, 3, 4, 5, 0);
        map.update(6, 2, 3, 4, 7, 0);
        map.update(8, 2, 3, 0, 9, 0);
        assert_eq!(map.target(8, 2, 3), None);
        map.update(1, 2, 3, 0, 0, 0);
        assert_eq!(map.target(6, 2, 3), Some(Target::Wayland(7)));
    }
}
