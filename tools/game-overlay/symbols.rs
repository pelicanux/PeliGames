//! Shared, allocation-free ABI between the preload library and rtld-audit.
//! Keep symbol order identical in both crates. Names here omit their trailing NUL.
pub const WAYLAND: u8 = 1;
pub const X11: u8 = 2;
pub const WINELAND: u8 = 3;
// (symbol, defining DSO, redirect calls to the wrapper)
pub const SYMBOLS: [(&[u8], u8, bool); 28] = [
    (b"wl_proxy_add_listener", WAYLAND, true),
    (b"wl_proxy_destroy", WAYLAND, true),
    (b"wl_proxy_add_dispatcher", WAYLAND, true),
    (b"wl_proxy_get_class", WAYLAND, false),
    (b"wl_proxy_get_version", WAYLAND, false),
    (b"wl_proxy_get_listener", WAYLAND, false),
    (b"XCheckIfEvent", X11, true),
    (b"XGetEventData", X11, true),
    (b"XFreeEventData", X11, false),
    (b"XQueryKeymap", X11, true),
    (b"XQueryExtension", X11, false),
    (b"XQueryTree", X11, false),
    (b"XFree", X11, false),
    (b"XGetInputFocus", X11, false),
    (b"XGrabPointer", X11, true),
    (b"XUngrabPointer", X11, true),
    (b"XChangeActivePointerGrab", X11, true),
    (b"XWarpPointer", X11, true),
    (b"XQueryPointer", X11, true),
    (b"XCreateFontCursor", X11, false),
    (b"XFreeCursor", X11, false),
    (b"XGetGeometry", X11, false),
    (b"XTranslateCoordinates", X11, false),
    (b"XFlush", X11, false),
    (b"XCloseDisplay", X11, true),
    (b"XDestroyWindow", X11, true),
    (b"__wine_wayland_vulkan_register_surface_v1", WINELAND, true),
    (b"wl_proxy_marshal_array_flags", WAYLAND, true),
];
