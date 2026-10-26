//! Keep Steam's Wayland focus bridge visible without changing X11 input order.
use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::Path,
};

pub fn wayland_requested(enable: Option<&OsStr>, alias: Option<&OsStr>) -> bool {
    // Proton processes PROTON_USE_WAYLAND after PROTON_ENABLE_WAYLAND.
    alias
        .or(enable)
        .is_some_and(|value| !value.is_empty() && value != "0")
}

pub fn input(
    path: &Path,
    existing: Option<OsString>,
    enable_wayland: Option<&OsStr>,
    use_wayland: Option<&OsStr>,
) -> Result<OsString, String> {
    compose(
        path,
        existing,
        wayland_requested(enable_wayland, use_wayland),
    )
}

pub fn prepend(path: &Path, existing: Option<OsString>) -> Result<OsString, String> {
    compose(path, existing, false)
}

fn compose(
    path: &Path,
    existing: Option<OsString>,
    after_existing: bool,
) -> Result<OsString, String> {
    if path
        .as_os_str()
        .as_bytes()
        .iter()
        .any(|b| b.is_ascii_whitespace() || *b == b':')
    {
        return Err("O diretório do overlay não pode conter espaços ou dois-pontos.".into());
    }
    let mut entries = Vec::new();
    if let Some(old) = existing {
        for entry in old
            .as_bytes()
            .split(|b| *b == b':' || b.is_ascii_whitespace())
        {
            if entry.is_empty() {
                continue;
            }
            let entry = OsString::from_vec(entry.to_vec());
            if Path::new(&entry).file_name() != path.file_name() {
                entries.push(entry);
            }
        }
    }
    // GE reads &XCheckIfEvent from a GOT relocation, outside la_symbind64.
    // In Wayland Steam must own that address; our actual input still arrives
    // through Wine/Wayland hooks. X11 instead needs our XCheckIfEvent first.
    if after_existing {
        entries.push(path.as_os_str().to_owned());
    } else {
        entries.insert(0, path.as_os_str().to_owned());
    }
    let mut value = OsString::new();
    for entry in entries {
        if !value.is_empty() {
            value.push(":");
        }
        value.push(entry);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wayland_preserves_steam_and_mangohud_priority_for_direct_symbol_bindings() {
        let path = Path::new("/peli/libpeligames_game_overlay.so");
        let old = "/mango/libMangoHud_shim.so:/steam/${LIB}/gameoverlayrenderer.so";
        assert_eq!(
            input(path, Some(old.into()), Some(OsStr::new("1")), None).unwrap(),
            OsString::from(format!("{old}:{}", path.display()))
        );
        assert_eq!(
            input(path, None, Some(OsStr::new("1")), None).unwrap(),
            path.as_os_str()
        );
    }
    #[test]
    fn x11_keeps_our_event_hook_first_even_in_a_wayland_desktop_session() {
        let path = Path::new("/peli/libpeligames_game_overlay.so");
        for flag in [None, Some(OsStr::new("0")), Some(OsStr::new(""))] {
            assert_eq!(
                input(
                    path,
                    Some("/steam/gameoverlayrenderer.so".into()),
                    flag,
                    None
                )
                .unwrap(),
                "/peli/libpeligames_game_overlay.so:/steam/gameoverlayrenderer.so"
            );
        }
    }
    #[test]
    fn proton_alias_has_the_same_precedence_and_nonzero_semantics_as_proton() {
        assert!(wayland_requested(None, Some(OsStr::new("1"))));
        assert!(wayland_requested(
            Some(OsStr::new("0")),
            Some(OsStr::new("1"))
        ));
        assert!(!wayland_requested(
            Some(OsStr::new("1")),
            Some(OsStr::new("0"))
        ));
        assert!(!wayland_requested(
            Some(OsStr::new("1")),
            Some(OsStr::new(""))
        ));
        assert!(wayland_requested(Some(OsStr::new("true")), None));
    }
    #[test]
    fn nested_wayland_runners_replace_old_payloads_and_keep_order_idempotent() {
        let path = Path::new("/new/libpeligames_game_overlay.so");
        let old = "/old/libpeligames_game_overlay.so:/steam/gameoverlayrenderer.so /new/libpeligames_game_overlay.so:/mango/libMangoHud_shim.so";
        let result = input(path, Some(old.into()), Some(OsStr::new("1")), None).unwrap();
        assert_eq!(result, "/steam/gameoverlayrenderer.so:/mango/libMangoHud_shim.so:/new/libpeligames_game_overlay.so");
        assert_eq!(
            input(path, Some(result.clone()), Some(OsStr::new("1")), None).unwrap(),
            result
        );
    }
}
