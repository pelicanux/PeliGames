//! Shared on-disk layout for the launcher and its in-game helpers.
use std::path::{Path, PathBuf};
pub const MOVES: &[(&str, &str)] = &[
    ("config.json", "settings/config.json"),
    ("mod-backup.json", "settings/mod-backup.json"),
    ("mod-recovery-preserved", "library/mod-recovery-preserved"),
    (
        "external-game-settings.json",
        "settings/external-game-settings.json",
    ),
    ("frame-overlay.json", "settings/frame-overlay.json"),
    ("capture-shortcuts.json", "settings/capture-shortcuts.json"),
    ("capture-storage.json", "settings/capture-storage.json"),
    ("discord-presence.json", "settings/discord-presence.json"),
    ("updates.json", "settings/updates.json"),
    ("playtime.json", "library/playtime.json"),
    (
        "installed-destinations.json",
        "library/installed-destinations.json",
    ),
    (
        "installed-destinations.lock",
        "library/installed-destinations.lock",
    ),
    ("CaptureSettings", "profiles/capture"),
    ("GameModules", "profiles/game-modules"),
    ("FrameGeneration", "components/FrameGeneration"),
    ("MangoHUD", "components/MangoHUD"),
    ("Mod", "components/Mods"),
    ("Overlay", "components/Overlay"),
    ("CaptureRuntime", "components/GPU-Screen-Recorder"),
    ("runners", "components/runners"),
    ("tools", "components/tools"),
    ("CaptureLibrary", "cache/video-library"),
    ("covers", "cache/covers"),
    ("icons", "cache/icons"),
    ("analyzer_cache.json", "cache/games/analyzer.json"),
    ("release_date_cache.json", "cache/games/release-dates.json"),
    ("official-build.json", "cache/official-build.json"),
    ("updates", "cache/updates"),
    ("launcher.log", "logs/launcher.log"),
    ("launcher.previous.log", "logs/launcher.previous.log"),
    ("game-logs", "logs/games"),
    ("overlay-logs", "logs/overlay"),
    ("Logs", "logs/components"),
    ("launch-reports", "logs/launch-reports"),
    ("executions", "runtime/executions"),
    ("capture-overlay.json", "runtime/capture-overlay.json"),
    (
        "steam-pending-settings.json",
        "runtime/steam-pending-settings.json",
    ),
    ("nexus-browser", "webview/nexus-browser"),
];
pub fn join(root: &Path, relative: impl AsRef<Path>) -> PathBuf {
    let relative = relative.as_ref();
    for &(old, new) in MOVES {
        if let Ok(rest) = relative.strip_prefix(old) {
            return if rest.as_os_str().is_empty() {
                root.join(new)
            } else {
                root.join(new).join(rest)
            };
        }
    }
    root.join(relative)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_roots_cover_both_component_layouts() {
        assert_eq!(
            from_profile(Path::new(
                "/config/PeliGames/components/FrameGeneration/profiles/game.toml"
            )),
            Some(PathBuf::from("/config/PeliGames"))
        );
        assert_eq!(
            from_profile(Path::new(
                "/config/PeliGames/FrameGeneration/profiles/game.toml"
            )),
            Some(PathBuf::from("/config/PeliGames"))
        );
    }
    #[test]
    fn routes_only_complete_owned_components_and_accepts_organized_paths() {
        let root = Path::new("/fixture/PeliGames");
        assert_eq!(
            join(root, "Mod/DLSSNR/config.json"),
            root.join("components/Mods/DLSSNR/config.json")
        );
        assert_eq!(
            join(root, "settings/config.json"),
            root.join("settings/config.json")
        );
        assert_eq!(join(root, "Models/file"), root.join("Models/file"));
        assert_eq!(join(root, "launcher.lock"), root.join("launcher.lock"));
    }
}

/// MAKO profile nesting changed when components became a dedicated group.
pub fn from_profile(profile: &Path) -> Option<PathBuf> {
    let parent = profile.parent()?.parent()?.parent()?;
    Some(
        if parent.file_name().is_some_and(|n| n == "components") {
            parent.parent()?
        } else {
            parent
        }
        .to_owned(),
    )
}
