//! Read-only detection of existing Unreal/UE4SS mods, including manual installs.
use std::{fs, path::Path};

use super::nexus_modules::Ue4ssLayout;

pub(super) fn ue4ss_installed(root: &Path, layout: &Ue4ssLayout) -> bool {
    let binaries = root.join(&layout.binaries);
    binaries.join("dwmapi.dll").is_file()
        && (binaries.join("ue4ss/UE4SS.dll").is_file() || binaries.join("UE4SS.dll").is_file())
}

// Package routes use one canonical path; deployment selects the existing loader's layout.
pub(super) fn ue4ss_mod_target(root: &Path, layout: &Ue4ssLayout, target: String) -> Result<String, String> {
    let prefix = format!("{}/ue4ss/Mods/", layout.binaries);
    let Some(tail) = target.strip_prefix(&prefix) else { return Ok(target); };
    let binaries = root.join(&layout.binaries);
    let flat = binaries.join("UE4SS.dll").is_file();
    let nested = binaries.join("ue4ss/UE4SS.dll").is_file();
    match (flat, nested) {
        (true, true) => Err("Dois layouts UE4SS encontrados. Mantenha somente o loader desejado antes de instalar scripts; nenhum arquivo foi aplicado.".into()),
        (true, false) => Ok(format!("{}/Mods/{tail}", layout.binaries)),
        _ => Ok(target),
    }
}

pub(super) fn unreal(root: &Path, layout: &Ue4ssLayout) -> Vec<(String, bool)> {
    let mut mods = Vec::new();
    let loader = ue4ss_installed(root, layout);
    if loader {
        mods.push(("UE4SS".into(), true));
    }
    for relative in layout.mod_folders() {
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
    for relative in [layout.paks.as_str(), layout.logic_mods.as_str()] {
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
    fn pal() -> Ue4ssLayout {
        Ue4ssLayout { binaries: "Pal/Binaries/Win64".into(), logic_mods: "Pal/Content/Paks/LogicMods".into(), paks: "Pal/Content/Paks/~mods".into() }
    }
    fn put(root: &Path, relative: &str, value: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, value).unwrap();
    }
    #[test]
    fn palworld_scripts_follow_the_existing_loader_and_reject_ambiguity() {
        let root = std::env::temp_dir().join(format!("palworld-layout-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let canonical = "Pal/Binaries/Win64/ue4ss/Mods/Menu/Scripts/main.lua";
        assert_eq!(ue4ss_mod_target(&root, &pal(), canonical.into()).unwrap(), canonical);
        put(&root, "Pal/Binaries/Win64/UE4SS.dll", "fixture");
        assert_eq!(ue4ss_mod_target(&root, &pal(), canonical.into()).unwrap(), "Pal/Binaries/Win64/Mods/Menu/Scripts/main.lua");
        let pak = "Pal/Content/Paks/LogicMods/Menu.pak";
        assert_eq!(ue4ss_mod_target(&root, &pal(), pak.into()).unwrap(), pak);
        put(&root, "Pal/Binaries/Win64/ue4ss/UE4SS.dll", "fixture");
        assert!(ue4ss_mod_target(&root, &pal(), canonical.into()).is_err());
        fs::remove_file(root.join("Pal/Binaries/Win64/UE4SS.dll")).unwrap();
        assert_eq!(ue4ss_mod_target(&root, &pal(), canonical.into()).unwrap(), canonical);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn detects_manual_loader_and_mods_without_treating_partial_loader_as_installed() {
        let root = std::env::temp_dir().join(format!("peligames-scan-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        put(&root, "Pal/Binaries/Win64/ue4ss/UE4SS.dll", "fixture");
        assert!(!ue4ss_installed(&root, &pal()));
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
        let found = unreal(&root, &pal());
        assert!(found.contains(&("UE4SS".into(), true)));
        assert!(found.contains(&("ModConfigMenu".into(), true)));
        assert!(found.contains(&("Disabled".into(), false)));
        assert!(found.contains(&("Enabled".into(), true)));
        assert!(found.contains(&("Outfit".into(), true)));
        fs::remove_dir_all(&root).unwrap();
    }
}
