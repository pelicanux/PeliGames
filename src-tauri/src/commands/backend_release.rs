//! Platform selection for backend downloads; ZIP versus tar is not a platform test.
pub fn windows_release(tag: &str, title: &str, body: &str) -> bool {
    let metadata = format!("{tag} {title}").to_ascii_lowercase();
    let body = body.to_ascii_lowercase();
    metadata.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| matches!(word, "windows" | "win" | "win32" | "win64"))
        || body.contains("windows only") || body.contains("windows-only")
}

/// Legacy Linux packages had no OS label. Accept those only as tar.gz and
/// only when the release metadata does not identify a Windows release.
pub fn linux_bitness(name: &str, windows_release: bool) -> Option<&'static str> {
    let name = name.to_ascii_lowercase();
    let words: Vec<_> = name.split(|c: char| !c.is_ascii_alphanumeric() && c != '_').collect();
    if words.iter().any(|word| matches!(*word, "windows" | "win" | "win32" | "win64" | "darwin" | "macos" | "symbols" | "debug")) {
        return None;
    }
    let linux = words.contains(&"linux");
    if !(name.ends_with(".tar.gz") || (linux && name.ends_with(".zip"))) || (!linux && windows_release) {
        return None;
    }
    match (words.contains(&"i686"), words.contains(&"x86_64")) {
        (true, false) => Some("32"),
        (false, true) => Some("64"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_latest_is_not_a_linux_update() {
        let windows = windows_release("v0.0.4-win-preview", "DLSSNR-AMD 0.0.4-win-preview", "This release is Windows only");
        assert!(windows);
        assert_eq!(linux_bitness("DLSSNR-AMD-Vulkan-Windows-0.0.4-win-preview-x86_64.zip", windows), None);
        assert_eq!(linux_bitness("DLSSNR-AMD-0.0.4-x86_64.tar.gz", windows), None);
    }
    #[test]
    fn mixed_release_selects_only_linux_architectures() {
        assert_eq!(linux_bitness("DLSSNR-AMD-Vulkan-Linux-0.0.3-x86_64.tar.gz", false), Some("64"));
        assert_eq!(linux_bitness("DLSSNR-AMD-Vulkan-Linux-0.0.3-i686.tar.gz", false), Some("32"));
        assert_eq!(linux_bitness("DLSSNR-AMD-Vulkan-Windows-0.0.3-preview-x86_64.zip", false), None);
        // A Windows preview may carry explicitly labelled Linux assets too.
        assert_eq!(linux_bitness("DLSSNR-AMD-Linux-0.0.4-x86_64.zip", true), Some("64"));
    }
    #[test]
    fn legacy_and_ambiguous_assets() {
        assert_eq!(linux_bitness("DLSSNR-AMD-Vulkan-0.0.2.5-i686.tar.gz", false), Some("32"));
        assert_eq!(linux_bitness("backend-x86_64.zip", false), None);
        assert_eq!(linux_bitness("backend-Windows-x86_64.tar.gz", false), None);
        assert_eq!(linux_bitness("backend-Linux-arm64.tar.gz", false), None);
        assert_eq!(linux_bitness("backend-Linux-x86_64-symbols.tar.gz", false), None);
        assert_eq!(linux_bitness("backend-Linux-i686-x86_64.tar.gz", false), None);
    }
    #[test]
    fn changelog_mentioning_windows_does_not_hide_linux() {
        assert!(!windows_release("v0.0.4", "DLSSNR-AMD 0.0.4", "This release is Linux only; the Windows preview stays at 0.0.3."));
        assert!(windows_release("v1", "Full Preview", "This release is Windows-only."));
    }
}
