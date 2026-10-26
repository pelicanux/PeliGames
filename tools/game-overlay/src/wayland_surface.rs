//! Track only observed wl_subcompositor parentage. Vulkan may present a child
//! while wl_keyboard.enter names its toplevel parent. Never infer focus from
//! another window merely because it belongs to the same process or display.
use std::{
    collections::HashMap,
    ffi::{c_void, CStr},
    sync::{Mutex, OnceLock},
};

#[derive(Default)]
struct Tree {
    // wl_subsurface role -> (child wl_surface, parent wl_surface)
    roles: HashMap<usize, (usize, usize)>,
}
impl Tree {
    fn insert(&mut self, role: usize, child: usize, parent: usize) {
        if role == 0 || child == 0 || parent == 0 || child == parent {
            return;
        }
        if self.roles.len() >= 4096 {
            return;
        }
        self.roles.retain(|_, (c, _)| *c != child);
        self.roles.insert(role, (child, parent));
    }
    fn remove(&mut self, proxy: usize) {
        self.roles.remove(&proxy);
        self.roles
            .retain(|_, (child, parent)| *child != proxy && *parent != proxy);
    }
    fn focused(&self, mut target: usize, focus: usize) -> bool {
        if target == 0 || focus == 0 {
            return false;
        }
        for _ in 0..64 {
            if target == focus {
                return true;
            }
            let Some((_, parent)) = self.roles.values().find(|(child, _)| *child == target) else {
                return false;
            };
            target = *parent;
        }
        false // malformed/cyclic or unbounded trees must never capture input
    }
}
static TREE: OnceLock<Mutex<Tree>> = OnceLock::new();
fn tree() -> &'static Mutex<Tree> {
    TREE.get_or_init(Default::default)
}
pub(crate) fn focused(target: usize, focus: usize) -> bool {
    tree().lock().is_ok_and(|t| t.focused(target, focus))
}
pub(crate) fn forget(proxy: usize) {
    if let Ok(mut t) = tree().lock() {
        t.remove(proxy);
    }
    crate::clear_focus(proxy);
}

/// Public wl_argument ABI. Only object slots of a known core request are read.
#[repr(C)]
pub union Argument {
    object: *mut c_void,
    integer: i32,
    unsigned: u32,
}
#[derive(Clone, Copy)]
enum Request {
    None,
    Child(usize, usize),
    Destroy(usize),
}
unsafe fn request(class: &[u8], proxy: usize, opcode: u32, args: *mut Argument) -> Request {
    match (class, opcode) {
        (b"wl_subcompositor", 1) if !args.is_null() => Request::Child(
            (*args.add(1)).object as usize,
            (*args.add(2)).object as usize,
        ),
        (b"wl_surface" | b"wl_subsurface", 0) => Request::Destroy(proxy),
        _ => Request::None,
    }
}
fn complete(request: Request, result: usize) {
    if let Ok(mut t) = tree().lock() {
        match request {
            Request::Child(child, parent) if result != 0 => t.insert(result, child, parent),
            Request::Destroy(proxy) => t.remove(proxy),
            _ => return,
        }
    }
    if let Request::Destroy(proxy) = request {
        crate::clear_focus(proxy);
    } else {
        crate::refresh_wayland_focus();
    }
}

/// libwayland's variadic wl_proxy_marshal_flags delegates to this fixed-ABI
/// public function. All six original arguments and the return value survive.
/// No variadic shim, assembly, replacement connection or protocol request.
#[no_mangle]
pub unsafe extern "C" fn wl_proxy_marshal_array_flags(
    proxy: *mut c_void,
    opcode: u32,
    interface: *const c_void,
    version: u32,
    flags: u32,
    arguments: *mut Argument,
) -> *mut c_void {
    type Marshal = unsafe extern "C" fn(
        *mut c_void,
        u32,
        *const c_void,
        u32,
        u32,
        *mut Argument,
    ) -> *mut c_void;
    let real = crate::symbols::resolve(b"wl_proxy_marshal_array_flags\0");
    if real.is_null() {
        return std::ptr::null_mut();
    }
    let real: Marshal = std::mem::transmute(real);
    if !crate::process_scope::enabled() { return real(proxy, opcode, interface, version, flags, arguments); }
    let get_class = crate::symbols::resolve(b"wl_proxy_get_class\0");
    let observed = if !proxy.is_null() && !get_class.is_null() {
        let get_class: unsafe extern "C" fn(*mut c_void) -> *const libc::c_char =
            std::mem::transmute(get_class);
        let name = get_class(proxy);
        if name.is_null() {
            Request::None
        } else {
            request(
                CStr::from_ptr(name).to_bytes(),
                proxy as usize,
                opcode,
                arguments,
            )
        }
    } else {
        Request::None
    };
    // The destroy flag may free proxy: never inspect it after forwarding.
    let result = real(proxy, opcode, interface, version, flags, arguments);
    complete(observed, result as usize);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forwarded_core_request_enables_f8_for_parent_and_leave_suspends_capture() {
        use std::sync::atomic::{AtomicBool, Ordering};
        static FORWARDED: AtomicBool = AtomicBool::new(false);
        unsafe extern "C" fn class(proxy: *mut c_void) -> *const libc::c_char {
            if proxy as usize == 9 {
                c"wl_subcompositor".as_ptr()
            } else {
                c"wl_subsurface".as_ptr()
            }
        }
        unsafe extern "C" fn marshal(
            proxy: *mut c_void,
            opcode: u32,
            interface: *const c_void,
            version: u32,
            flags: u32,
            args: *mut Argument,
        ) -> *mut c_void {
            FORWARDED.store(
                tree().try_lock().is_ok()
                    && version == 1
                    && ((proxy as usize == 9
                        && opcode == 1
                        && interface as usize == 8
                        && flags == 0
                        && !args.is_null()
                        && (*args.add(1)).object as usize == 2
                        && (*args.add(2)).object as usize == 1)
                        || (proxy as usize == 10
                            && opcode == 0
                            && interface.is_null()
                            && flags == 1
                            && args.is_null())),
                Ordering::Release,
            );
            if proxy as usize == 9 {
                10 as _
            } else {
                std::ptr::null_mut()
            }
        }
        let slot = |name: &[u8]| {
            crate::symbols::abi::SYMBOLS
                .iter()
                .position(|(n, _, _)| *n == name)
                .unwrap()
        };
        let m = &crate::symbols::peli_overlay_originals[slot(b"wl_proxy_marshal_array_flags")];
        let c = &crate::symbols::peli_overlay_originals[slot(b"wl_proxy_get_class")];
        let old_m = m.swap(marshal as *const () as usize, Ordering::AcqRel);
        let old_c = c.swap(class as *const () as usize, Ordering::AcqRel);
        *crate::state().lock().unwrap() = Default::default();
        crate::set_target(crate::Target::Wayland(2));
        crate::set_focus(1);
        assert!(!crate::state().lock().unwrap().focused);
        let mut args = [
            Argument {
                object: std::ptr::null_mut(),
            },
            Argument { object: 2 as _ },
            Argument { object: 1 as _ },
        ];
        unsafe {
            assert_eq!(
                wl_proxy_marshal_array_flags(9 as _, 1, 8 as _, 1, 0, args.as_mut_ptr()) as usize,
                10
            );
        }
        assert!(FORWARDED.load(Ordering::Acquire));
        {
            let mut input = crate::state().lock().unwrap();
            input.ready = true;
            assert!(input.focused);
            assert_eq!(input.key(66, true), crate::input::Route::Overlay);
            assert!(input.visible && input.capturing());
        }
        crate::clear_focus(1); // wl_keyboard.leave names the parent, not child
        assert!(!crate::state().lock().unwrap().capturing());
        crate::set_focus(99);
        crate::clear_focus(1); // a delayed leave must not erase newer focus
        assert_eq!(crate::FOCUS.load(Ordering::Acquire), 99);
        assert!(!crate::state().lock().unwrap().capturing());
        crate::set_focus(1);
        assert!(crate::state().lock().unwrap().capturing());
        unsafe {
            assert!(wl_proxy_marshal_array_flags(
                10 as _,
                0,
                std::ptr::null(),
                1,
                1,
                std::ptr::null_mut()
            )
            .is_null());
        }
        assert!(FORWARDED.load(Ordering::Acquire));
        assert!(!crate::state().lock().unwrap().capturing());
        crate::set_target(crate::Target::Unavailable);
        crate::set_focus(0);
        *crate::state().lock().unwrap() = Default::default();
        m.store(old_m, Ordering::Release);
        c.store(old_c, Ordering::Release);
    }
    #[test]
    fn child_and_nested_child_accept_the_focused_parent_but_not_other_windows() {
        let mut tree = Tree::default();
        tree.insert(10, 2, 1);
        tree.insert(11, 3, 2);
        for target in [1, 2, 3] {
            assert!(tree.focused(target, 1));
        }
        assert!(tree.focused(3, 2));
        assert!(!tree.focused(1, 2));
        assert!(!tree.focused(3, 99));
        assert!(!tree.focused(0, 0));
        assert!(!tree.focused(3, 0));
        tree.insert(12, 4, 1);
        assert!(!tree.focused(3, 4)); // sibling does not own keyboard focus
    }
    #[test]
    fn destroyed_roles_surfaces_and_reused_addresses_do_not_keep_old_focus() {
        let mut tree = Tree::default();
        tree.insert(10, 2, 1);
        tree.remove(10);
        assert!(!tree.focused(2, 1));
        tree.insert(11, 2, 1);
        tree.remove(1);
        assert!(!tree.focused(2, 1));
        tree.insert(12, 2, 3);
        assert!(tree.focused(2, 3));
        assert!(!tree.focused(2, 1));
        tree.remove(2);
        assert!(!tree.focused(2, 3));
    }
    #[test]
    fn malformed_cycles_are_bounded_and_do_not_capture_unrelated_focus() {
        let mut tree = Tree::default();
        tree.insert(10, 2, 1);
        tree.insert(11, 1, 2);
        assert!(!tree.focused(2, 99));
        tree.insert(0, 4, 1);
        tree.insert(12, 5, 0);
        assert!(!tree.focused(4, 1));
        assert!(!tree.focused(5, 1));
    }
    #[test]
    fn protocol_decoder_reads_only_known_object_slots() {
        let mut args = [
            Argument {
                object: std::ptr::null_mut(),
            },
            Argument { object: 2 as _ },
            Argument { object: 1 as _ },
        ];
        unsafe {
            assert!(matches!(
                request(b"wl_subcompositor", 9, 1, args.as_mut_ptr()),
                Request::Child(2, 1)
            ));
            assert!(matches!(
                request(b"wl_surface", 2, 0, std::ptr::null_mut()),
                Request::Destroy(2)
            ));
            assert!(matches!(
                request(b"wl_subsurface", 10, 0, std::ptr::null_mut()),
                Request::Destroy(10)
            ));
            assert!(matches!(
                request(b"wl_subcompositor", 9, 1, std::ptr::null_mut()),
                Request::None
            ));
            assert!(matches!(request(b"unknown", 9, 1, 1 as _), Request::None));
        }
    }
}
