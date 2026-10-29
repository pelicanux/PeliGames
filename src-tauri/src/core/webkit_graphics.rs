//! Narrow workarounds for the WebKitGTK version actually loaded at runtime.
fn compositor_compatibility(major: u32, minor: u32) -> bool {
    // 2.54's Skia compositor can omit backdrop blur and leave stale/empty layers.
    // TextureMapper remains available in this series, with GPU acceleration.
    // Recheck newer series rather than carrying this override forward blindly.
    major == 2 && minor == 54
}
fn needs_nvidia_dmabuf_workaround(major: u32, minor: u32, nvidia: bool) -> bool {
    // Disabling DMABUF itself regresses transparent windows on 2.54.
    nvidia && major == 2 && minor < 54
}

fn opaque_series(major: u32, minor: u32) -> bool {
    // 2.54 repaints a transparent window only under the pointer, whatever the
    // compositor, DMABUF or X11 setting; an opaque one is stable. Recheck newer series.
    major == 2 && minor == 54
}
/// Opaque windows (square corners) on WebKit series that lose parts of a
/// transparent one. `PELIGAMES_OPAQUE_WINDOW=1` forces it, `=0` keeps transparency.
pub fn opaque_window() -> bool {
    static OPAQUE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OPAQUE.get_or_init(|| match std::env::var("PELIGAMES_OPAQUE_WINDOW").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => unsafe { opaque_series(webkit2gtk::ffi::webkit_get_major_version(), webkit2gtk::ffi::webkit_get_minor_version()) },
    })
}
/// Background of an opaque window before the page draws (the dark theme base).
pub const OPAQUE_BACKGROUND: (u8, u8, u8) = (16, 17, 25);

/// Lets the page square its frame when the window cannot be transparent.
#[tauri::command]
pub fn window_opaque() -> bool {
    opaque_window()
}

/// Called before GTK or any webview starts. Explicit user overrides win.
pub fn configure() {
    let (major, minor) = unsafe {
        (
            webkit2gtk::ffi::webkit_get_major_version(),
            webkit2gtk::ffi::webkit_get_minor_version(),
        )
    };
    if compositor_compatibility(major, minor)
        && std::env::var_os("WEBKIT_USE_SKIA_FOR_COMPOSITION").is_none()
    {
        std::env::set_var("WEBKIT_USE_SKIA_FOR_COMPOSITION", "0");
    }
    let nvidia = std::path::Path::new("/proc/driver/nvidia/version").exists()
        || std::path::Path::new("/sys/module/nvidia").exists();
    if needs_nvidia_dmabuf_workaround(major, minor, nvidia)
        && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

/// Let page updates follow the display, retaining the affected compositor's
/// compatibility setting through WebKit's own feature API.
pub fn configure_webview(view: &webkit2gtk::WebView) -> Result<(), String> {
    use webkit2gtk::{glib::translate::ToGlibPtr, WebViewExt};
    let (major, minor) = unsafe {
        (
            webkit2gtk::ffi::webkit_get_major_version(),
            webkit2gtk::ffi::webkit_get_minor_version(),
        )
    };
    let damage_compatibility = compositor_compatibility(major, minor)
        && std::env::var("WEBKIT_USE_SKIA_FOR_COMPOSITION").as_deref() == Ok("0");
    let settings = view.settings().ok_or("Configuração WebKit indisponível")?;
    // Feature-list API arrived after our minimum link-time WebKit version.
    // Resolve it dynamically, keeping older distributions loadable.
    unsafe {
        use std::ffi::{c_char, c_void, CStr};
        let names = [
            c"webkit_settings_get_all_features",
            c"webkit_feature_list_get_length",
            c"webkit_feature_list_get",
            c"webkit_feature_get_identifier",
            c"webkit_settings_set_feature_enabled",
            c"webkit_feature_list_unref",
        ];
        let symbols = names.map(|name| libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()));
        if symbols.iter().any(|symbol| symbol.is_null()) {
            return if damage_compatibility {
                Err("API de compatibilidade gráfica do WebKit indisponível".into())
            } else {
                Ok(())
            };
        }
        let all: unsafe extern "C" fn() -> *mut c_void = std::mem::transmute(symbols[0]);
        let length: unsafe extern "C" fn(*mut c_void) -> u32 = std::mem::transmute(symbols[1]);
        let get: unsafe extern "C" fn(*mut c_void, u32) -> *mut c_void =
            std::mem::transmute(symbols[2]);
        let identifier: unsafe extern "C" fn(*mut c_void) -> *const c_char =
            std::mem::transmute(symbols[3]);
        let set: unsafe extern "C" fn(*mut webkit2gtk::ffi::WebKitSettings, *mut c_void, i32) =
            std::mem::transmute(symbols[4]);
        let unref: unsafe extern "C" fn(*mut c_void) = std::mem::transmute(symbols[5]);
        let list = all();
        if list.is_null() {
            return Err("Lista de recursos WebKit indisponível".into());
        }
        let mut applied = false;
        for index in 0..length(list) {
            let feature = get(list, index);
            if feature.is_null() {
                continue;
            }
            let id = identifier(feature);
            if id.is_null() {
                continue;
            }
            let id = CStr::from_ptr(id);
            if id == c"PreferPageRenderingUpdatesNear60FPS" {
                set(settings.to_glib_none().0, feature, 0);
            } else if damage_compatibility && id == c"UseDamagingInformationForCompositing" {
                set(settings.to_glib_none().0, feature, 0);
                applied = true;
            }
        }
        unref(list);
        if damage_compatibility && !applied {
            return Err("Recurso de composição parcial WebKit não encontrado".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_affected_series_gets_an_opaque_window() {
        assert!(!opaque_series(2, 52));
        assert!(opaque_series(2, 54));
        assert!(!opaque_series(2, 56));
    }
    #[test]
    fn only_the_affected_compositor_series_is_changed() {
        assert!(!compositor_compatibility(2, 52));
        assert!(compositor_compatibility(2, 54));
        assert!(!compositor_compatibility(2, 56));
        assert!(!compositor_compatibility(3, 54));
    }
    #[test]
    fn dmabuf_is_not_disabled_on_newer_webkit_or_other_gpus() {
        assert!(needs_nvidia_dmabuf_workaround(2, 52, true));
        assert!(!needs_nvidia_dmabuf_workaround(2, 54, true));
        assert!(!needs_nvidia_dmabuf_workaround(2, 56, true));
        assert!(!needs_nvidia_dmabuf_workaround(2, 52, false));
    }
}
