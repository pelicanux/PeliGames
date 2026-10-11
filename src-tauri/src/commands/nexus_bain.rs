//! BAIN (Wrye Bash) complex packages: top-level `NN Name` folders whose contents
//! are relative to the game's Data folder. Presented as one FOMOD-style group so
//! selection, confirmation and redeployment reuse the FOMOD flow. `wizard.txt`
//! scripts are not executed; later packages replace files of earlier ones.
use super::{nexus_fomod::{Group, Installer, OptionInfo, Resolution, Step}, nexus_modules::{ActivationKind, Definition}};
use std::collections::{BTreeMap, BTreeSet};

fn package(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 3 && bytes[0].is_ascii_digit() && bytes[1].is_ascii_digit() && bytes[2] == b' ' && !name[3..].trim().is_empty()
}

/// Sub-packages of a BAIN archive, or none when the layout is not BAIN: every
/// top-level folder must be a sub-package; loose top-level files are documentation.
pub(super) fn packages(entries: &[String]) -> Vec<String> {
    let mut found = BTreeSet::new();
    for entry in entries.iter().filter(|entry| !entry.ends_with('/')) {
        let Some((top, _)) = entry.split_once('/') else { continue; };
        if !package(top) { return Vec::new(); }
        found.insert(top.to_string());
    }
    found.into_iter().collect()
}

pub(super) fn resolve(entries: &[String], module: &Definition, mod_id: &str, choices: Option<&[String]>) -> Result<Option<Resolution>, String> {
    if !module.activation.as_ref().is_some_and(|rule| matches!(rule.kind, ActivationKind::Bethesda | ActivationKind::Morrowind)) {
        return Ok(None);
    }
    let packages = packages(entries);
    if packages.is_empty() { return Ok(None); }
    // As in the Vortex BAIN installer, only the first package is preselected.
    let selection: Vec<String> = match choices {
        Some(choices) => packages.iter().filter(|name| choices.contains(name)).cloned().collect(),
        None => packages.iter().take(1).cloned().collect(),
    };
    let mut issues = Vec::new();
    if selection.is_empty() { issues.push("Selecione ao menos um pacote BAIN.".to_string()); }
    for entry in entries.iter().filter(|entry| !entry.ends_with('/') && !entry.contains('/')) {
        super::nexus_deployment::unmapped_file(entry)?;
    }
    let mut output: BTreeMap<String, (String, String)> = BTreeMap::new();
    for name in &selection {
        let prefix = format!("{name}/");
        let inside: Vec<(&String, String)> = entries.iter().filter(|entry| !entry.ends_with('/'))
            .filter_map(|entry| entry.strip_prefix(&prefix).map(|rest| (entry, rest.to_string()))).collect();
        let relative: Vec<String> = inside.iter().map(|(_, rest)| rest.clone()).collect();
        for (source, rest) in &inside {
            // Package contents are Data-relative; an explicit Data/ folder is accepted too.
            let data = if rest.to_ascii_lowercase().starts_with("data/") { rest.clone() } else { format!("Data/{rest}") };
            let target = if module.allowed(&data) { Some(data) } else { module.route_package(rest, mod_id, &relative)? };
            match target {
                Some(target) => { output.insert(target.to_ascii_lowercase(), ((*source).clone(), target)); }
                None => super::nexus_deployment::unmapped_file(source)?,
            }
        }
    }
    let wizard = entries.iter().any(|entry| entry.eq_ignore_ascii_case("wizard.txt"));
    let options = packages.iter().map(|name| OptionInfo {
        id: name.clone(), name: name.clone(), description: String::new(), kind: "Optional".into(), selected: selection.contains(name),
    }).collect();
    Ok(Some(Resolution {
        installer: Some(Installer {
            name: if wizard { "BAIN (wizard.txt não é executado)".into() } else { "BAIN".into() },
            steps: vec![Step { name: "Pacotes".into(), groups: vec![Group { name: "Pacotes BAIN".into(), kind: "SelectAtLeastOne".into(), options }] }],
        }),
        selection,
        issues,
        files: Some(output.into_values().collect()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn module(name: &str) -> Definition {
        super::super::nexus_modules::registry().definitions.into_iter().find(|m| m.name == name).unwrap()
    }
    fn entries(items: &[&str]) -> Vec<String> { items.iter().map(|item| item.to_string()).collect() }
    #[test]
    fn detects_only_archives_made_of_numbered_packages() {
        assert_eq!(packages(&entries(&["00 Core/Mod.esp", "10 Option/textures/a.dds", "readme.txt"])), ["00 Core", "10 Option"]);
        assert!(packages(&entries(&["00 Core/Mod.esp", "Docs/readme.txt"])).is_empty());
        assert!(packages(&entries(&["Data/Mod.esp"])).is_empty());
        assert!(packages(&entries(&["001/Mod.esp", "0A Core/Mod.esp"])).is_empty());
    }
    #[test]
    fn selected_packages_install_data_relative_and_later_ones_win() {
        let skyrim = module("Skyrim Special Edition");
        let files = entries(&["00 Core/Mod.esp", "00 Core/textures/a.dds", "10 Option/textures/a.dds", "20 Patch/Data/Patch.esp", "wizard.txt"]);
        let first = resolve(&files, &skyrim, "id", None).unwrap().unwrap();
        assert_eq!(first.selection, ["00 Core"]);
        assert_eq!(first.files.unwrap(), [("00 Core/Mod.esp".to_string(), "Data/Mod.esp".to_string()), ("00 Core/textures/a.dds".into(), "Data/textures/a.dds".into())]);
        let all: Vec<String> = ["00 Core", "10 Option", "20 Patch"].map(String::from).to_vec();
        let chosen = resolve(&files, &skyrim, "id", Some(&all)).unwrap().unwrap();
        let mapped: BTreeMap<_, _> = chosen.files.unwrap().into_iter().map(|(source, target)| (target, source)).collect();
        assert_eq!(mapped["Data/textures/a.dds"], "10 Option/textures/a.dds");
        assert_eq!(mapped["Data/Patch.esp"], "20 Patch/Data/Patch.esp");
        assert!(chosen.installer.unwrap().name.contains("wizard.txt"));
        assert!(!resolve(&files, &skyrim, "id", Some(&[])).unwrap().unwrap().issues.is_empty());
        // Games without plugin activation keep their own package handling.
        assert!(resolve(&files, &module("Palworld"), "id", None).unwrap().is_none());
        // Functional content outside the packages is never dropped silently.
        assert!(resolve(&entries(&["00 Core/Mod.esp", "Loose.esp"]), &skyrim, "id", None).is_err());
    }
}
