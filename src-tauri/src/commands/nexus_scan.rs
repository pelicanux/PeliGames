//! Read-only detection of existing Unreal/UE4SS mods, including manual installs.
use std::{fs, path::Path};

use super::nexus_modules::Ue4ssLayout;

pub(super) fn ue4ss_installed(root: &Path, layout: &Ue4ssLayout) -> bool {
    let binaries = root.join(&layout.binaries);
    binaries.join("dwmapi.dll").is_file()
        && (binaries.join("ue4ss/UE4SS.dll").is_file() || binaries.join("UE4SS.dll").is_file())
}

/// UE4SS layout that will be active: nested (`<bin>/ue4ss/Mods`, current
/// releases) or flat (`<bin>/Mods`, 3.0.x).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Ue4ssKind { Nested, Flat }
/// The loader in the files being deployed wins (e.g. a UE4SS package in the
/// profile); otherwise a loader installed by hand in the game, ignoring files a
/// previous deployment owns (they are about to be replaced). None: no loader yet.
pub(super) fn ue4ss_kind(root: &Path, layout: &Ue4ssLayout, deployed: &[&str], owned: &dyn Fn(&str) -> bool) -> Result<Option<Ue4ssKind>, String> {
    let nested = format!("{}/ue4ss/UE4SS.dll", layout.binaries);
    let flat = format!("{}/UE4SS.dll", layout.binaries);
    let both = || Err::<Option<Ue4ssKind>, String>("Dois layouts UE4SS encontrados. Mantenha somente o loader desejado antes de instalar scripts; nenhum arquivo foi aplicado.".into());
    let has = |path: &str| deployed.iter().any(|target| target.eq_ignore_ascii_case(path));
    match (has(&nested), has(&flat)) {
        (true, true) => return both(),
        (true, false) => return Ok(Some(Ue4ssKind::Nested)),
        (false, true) => return Ok(Some(Ue4ssKind::Flat)),
        _ => {}
    }
    let manual = |path: &str| root.join(path).is_file() && !owned(path);
    match (manual(&nested), manual(&flat)) {
        (true, true) => both(),
        (true, false) => Ok(Some(Ue4ssKind::Nested)),
        (false, true) => Ok(Some(Ue4ssKind::Flat)),
        _ => Ok(None),
    }
}
/// Script folder of the active layout, for targets written in either layout.
/// Without a loader the canonical nested path is kept.
pub(super) fn ue4ss_layout_target(layout: &Ue4ssLayout, kind: Option<Ue4ssKind>, target: String) -> String {
    let nested = format!("{}/ue4ss/Mods/", layout.binaries);
    let flat = format!("{}/Mods/", layout.binaries);
    let tail = target.strip_prefix(&nested).or_else(|| target.strip_prefix(&flat)).map(str::to_string);
    match (tail, kind) {
        (Some(tail), Some(Ue4ssKind::Flat)) => format!("{flat}{tail}"),
        (Some(tail), _) => format!("{nested}{tail}"),
        (None, _) => target,
    }
}
// Package routes use one canonical path; deployment selects the existing loader's layout.
pub(super) fn ue4ss_mod_target(root: &Path, layout: &Ue4ssLayout, target: String) -> Result<String, String> {
    let kind = ue4ss_kind(root, layout, &[], &|_| false)?;
    Ok(if target.starts_with(&format!("{}/ue4ss/Mods/", layout.binaries)) { ue4ss_layout_target(layout, kind, target) } else { target })
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
    fn replacing_the_loader_moves_installed_scripts_to_its_layout() {
        let root = std::env::temp_dir().join(format!("palworld-switch-{}", uuid::Uuid::new_v4()));
        let none = |_: &str| false;
        let old_target = "Pal/Binaries/Win64/Mods/PalInsight/Scripts/main.lua".to_string();
        // Pal Insight was installed for the old flat UE4SS; the new nested package is in the profile.
        let deployed = ["Pal/Binaries/Win64/ue4ss/UE4SS.dll", "Pal/Binaries/Win64/dwmapi.dll", old_target.as_str()];
        let kind = ue4ss_kind(&root, &pal(), &deployed, &none).unwrap();
        assert_eq!(kind, Some(Ue4ssKind::Nested));
        assert_eq!(ue4ss_layout_target(&pal(), kind, old_target.clone()), "Pal/Binaries/Win64/ue4ss/Mods/PalInsight/Scripts/main.lua");
        // A loader installed by hand still decides when no package provides one...
        put(&root, "Pal/Binaries/Win64/UE4SS.dll", "fixture");
        let kind = ue4ss_kind(&root, &pal(), &[], &none).unwrap();
        assert_eq!(ue4ss_layout_target(&pal(), kind, "Pal/Binaries/Win64/ue4ss/Mods/X/Scripts/main.lua".into()), "Pal/Binaries/Win64/Mods/X/Scripts/main.lua");
        // ...but a loader file owned by the previous deployment is about to be replaced.
        assert_eq!(ue4ss_kind(&root, &pal(), &[], &|path: &str| path.ends_with("UE4SS.dll")).unwrap(), None);
        // The profile's loader wins over a different manual one.
        assert_eq!(ue4ss_kind(&root, &pal(), &deployed, &none).unwrap(), Some(Ue4ssKind::Nested));
        // Non-script targets are untouched.
        assert_eq!(ue4ss_layout_target(&pal(), Some(Ue4ssKind::Flat), "Pal/Content/Paks/LogicMods/X.pak".into()), "Pal/Content/Paks/LogicMods/X.pak");
        fs::remove_dir_all(root).unwrap();
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
