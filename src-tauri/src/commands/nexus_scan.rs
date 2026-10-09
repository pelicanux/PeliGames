//! Read-only detection of existing Palworld mods, including manual installs.
use std::{fs, path::Path};

pub(super) fn ue4ss_installed(root: &Path) -> bool {
    let binaries = root.join("Pal/Binaries/Win64");
    binaries.join("dwmapi.dll").is_file()
        && (binaries.join("ue4ss/UE4SS.dll").is_file() || binaries.join("UE4SS.dll").is_file())
}

pub(super) fn palworld(root: &Path) -> Vec<(String, bool)> {
    let mut mods = Vec::new();
    let loader = ue4ss_installed(root);
    if loader {
        mods.push(("UE4SS".into(), true));
    }
    for relative in ["Pal/Binaries/Win64/ue4ss/Mods", "Pal/Binaries/Win64/Mods"] {
        let directory = root.join(relative);
        let settings = fs::read_to_string(directory.join("mods.txt")).unwrap_or_default();
        if let Ok(entries) = fs::read_dir(&directory) {
            for entry in entries.flatten().take(2048) {
                let path = entry.path();
                if !path.join("Scripts/main.lua").is_file() && !path.join("dlls/main.dll").is_file()
                {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let configured = settings
                    .lines()
                    .filter_map(|line| {
                        let line = line.split(';').next()?.trim();
                        let (key, value) = line.split_once(':')?;
                        key.trim()
                            .eq_ignore_ascii_case(&name)
                            .then(|| value.trim() == "1")
                    })
                    .last();
                let enabled =
                    loader && (path.join("enabled.txt").is_file() || configured == Some(true));
                mods.push((name, enabled));
            }
        }
    }
    for relative in ["Pal/Content/Paks/~mods", "Pal/Content/Paks/LogicMods"] {
        if let Ok(entries) = fs::read_dir(root.join(relative)) {
            for entry in entries.flatten().take(2048) {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("pak"))
                {
                    if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                        mods.push((
                            name.trim_end_matches("_P").into(),
                            !relative.ends_with("LogicMods") || loader,
                        ));
                    }
                }
            }
        }
    }
    mods.sort();
    mods.dedup();
    mods
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put(root: &Path, relative: &str, value: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    #[test]
    fn detects_manual_loader_and_mods_without_treating_partial_loader_as_installed() {
        let root = std::env::temp_dir().join(format!("peligames-scan-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        put(&root, "Pal/Binaries/Win64/ue4ss/UE4SS.dll", "fixture");
        assert!(!ue4ss_installed(&root));
        put(&root, "Pal/Binaries/Win64/dwmapi.dll", "fixture");
        put(
            &root,
            "Pal/Binaries/Win64/ue4ss/Mods/ModConfigMenu/Scripts/main.lua",
            "fixture",
        );
        put(
            &root,
            "Pal/Binaries/Win64/ue4ss/Mods/Disabled/Scripts/main.lua",
            "fixture",
        );
        put(
            &root,
            "Pal/Binaries/Win64/ue4ss/Mods/Enabled/Scripts/main.lua",
            "fixture",
        );
        put(
            &root,
            "Pal/Binaries/Win64/ue4ss/Mods/Enabled/enabled.txt",
            "",
        );
        put(
            &root,
            "Pal/Binaries/Win64/ue4ss/Mods/mods.txt",
            "ModConfigMenu : 1\r\nDisabled : 0\r\n",
        );
        put(&root, "Pal/Content/Paks/~mods/Outfit_P.pak", "fixture");
        let found = palworld(&root);
        assert!(found.contains(&("UE4SS".into(), true)));
        assert!(found.contains(&("ModConfigMenu".into(), true)));
        assert!(found.contains(&("Disabled".into(), false)));
        assert!(found.contains(&("Enabled".into(), true)));
        assert!(found.contains(&("Outfit".into(), true)));
        fs::remove_dir_all(&root).unwrap();
    }
}
