use std::ffi::{c_void, CString};
#[link(name = "dl")]
extern "C" {
    fn dlopen(path: *const i8, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const i8) -> *mut c_void;
}
unsafe fn open(path: &str) -> *mut c_void {
    let p = CString::new(path).unwrap();
    let h = dlopen(p.as_ptr(), 2);
    assert!(!h.is_null());
    h
}
unsafe fn count(h: *mut c_void) -> usize {
    let f = dlsym(h, b"probe_calls\0".as_ptr().cast());
    assert!(!f.is_null());
    let f: extern "C" fn() -> usize = std::mem::transmute(f);
    f()
}
unsafe fn steam_discovery(dir: &str) {
    let steam = open(&format!("{dir}/gameoverlayrenderer.so"));
    let expected = dlsym(steam, c"XCheckIfEvent".as_ptr());
    assert!(!expected.is_null());
    for name in [
        "liblsteamclient_overlay_bridge.so.1",
        "libVkLayer_GE_Wayland_SteamOverlay.so",
    ] {
        let bridge = open(&format!("{dir}/{name}"));
        let lookup = dlsym(bridge, c"probe_bridge_default".as_ptr());
        let lookup: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(lookup);
        let bound = dlsym(bridge, c"probe_bridge_bound".as_ptr());
        let bound: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(bound);
        assert_eq!(
            bound(),
            expected,
            "GE's direct GOT binding must belong to Steam ({name})"
        );
        assert_eq!(
            lookup(),
            expected,
            "GE must discover Steam's own hook, not PeliGames or raw X11 ({name})"
        );
    }
}
fn main() {
    unsafe {
        let args: Vec<_> = std::env::args().collect();
        let dir = &args[1];
        let layers: usize = args[2].parse().unwrap();
        let x11 = open(&format!("{dir}/libX11.so.6"));
        if std::env::var_os("PROBE_STEAM_DISCOVERY_ONLY").is_some() {
            steam_discovery(dir);
            println!("OK: GE Steam hook discovery with interposed dlsym");
            return;
        }

        if std::env::var_os("PROBE_X11_PRIORITY").is_some() {
            let bridge = open(&format!("{dir}/liblsteamclient_overlay_bridge.so.1"));
            let bound = dlsym(bridge, c"probe_bridge_bound".as_ptr());
            let bound: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(bound);
            let renderer = open(&format!("{dir}/libpeligames_game_overlay.so"));
            assert_eq!(
                bound(),
                dlsym(renderer, c"XCheckIfEvent".as_ptr()),
                "X11 must retain PeliGames' event hook before Steam"
            );
        }
        if std::env::var_os("PROBE_DLSYM_SHIM").is_some() {
            let renderer = open(&format!("{dir}/libpeligames_game_overlay.so"));
            let shim = open(&format!("{dir}/dlsym_shim.so"));
            let resolve = dlsym(renderer, c"probe_resolve".as_ptr());
            let resolve: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(resolve);
            let calls = dlsym(shim, c"probe_shim_calls".as_ptr());
            let calls: extern "C" fn() -> usize = std::mem::transmute(calls);
            let before = calls();
            assert!(!resolve().is_null());
            assert_eq!(
                calls(),
                before,
                "production resolver must bypass interposed dlsym to preserve RTLD_NEXT caller"
            );
        }

        // Explicit RTLD_LOCAL lookup must reach PeliGames, then each foreign
        // interposer exactly once, then X11. Foreign forwarding must not loop back.
        let f = dlsym(x11, b"XCheckIfEvent\0".as_ptr().cast());
        assert!(!f.is_null());
        let f: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32 =
            std::mem::transmute(f);
        assert_eq!(f(11 as _, 22 as _, 33 as _, 44 as _), 73);
        assert_eq!(
            count(open(&format!("{dir}/libpeligames_game_overlay.so"))),
            1
        );
        assert_eq!(count(x11), 1);
        for n in 0..layers {
            assert_eq!(count(open(&format!("{dir}/interposer{n}.so"))), 1);
        }
        if std::env::var_os("PROBE_DLSYM_SHIM").is_none() {
            let bridge = open(&format!("{dir}/liblsteamclient_overlay_bridge.so.1"));
            let input = dlsym(bridge, c"probe_bridge_input".as_ptr());
            let input: unsafe extern "C" fn(*mut c_void) -> i32 = std::mem::transmute(input);
            assert_eq!(input(x11), 73);
            assert_eq!(
                count(open(&format!("{dir}/libpeligames_game_overlay.so"))),
                1,
                "foreign focus bridge lookup must not be redirected through PeliGames"
            );
            assert_eq!(count(x11), 2);
            for n in 0..layers {
                assert_eq!(count(open(&format!("{dir}/interposer{n}.so"))), 1);
            }
        }
        if std::env::var_os("PROBE_X11_PRIORITY").is_some() {
            assert_eq!(
                count(open(&format!("{dir}/gameoverlayrenderer.so"))),
                1,
                "X11 input must still pass through Steam's hook exactly once"
            );
        }
        println!("OK: local X11 fallback + {layers} foreign interposer(s), each called once");
    }
}
