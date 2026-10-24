//! Crimson Desert packages, classified in the order of the Vortex extension's
//! installers: game-root packages (a `bin64` folder), mod managers and tools,
//! patch folders (`0036`) and Crimson Sharp `files/` folders for the external
//! managers in `mods/`, loose JSON/CDMOD patches, ASI/DLL plugins for `bin64`,
//! and finally the game root as it is. The managers apply the patches; the
//! deployment only places the files where they look for them.
use super::{nexus_fomod::{Group, Installer, OptionInfo, Resolution, Step}, nexus_modules::Definition};
use std::collections::BTreeMap;

const PLUGINS: [&str; 3] = [".dll", ".asi", ".addon64"];
const TOOLS: [&str; 7] = ["CD JSON Mod Manager.exe", "CrimsonSharp.exe", "CrimsonSaveEditorStandalone.exe", "PazGui.exe", "CDModManager.exe", "QT_ModManager.exe", "CrimsonDesertModManager.exe"];

#[derive(Debug, PartialEq)]
enum Layout {
    /// Files below the prefix go to the game root (`bin64`, tools, DMM).
    Root(String),
    /// Plugins to `bin64/<name>`, everything else to `mods/` as it is.
    SpecialPatch,
    /// Files below `base` go to `mods/`; a patch folder at the archive root gets the mod's own folder.
    Mods { base: String, own_folder: bool },
    /// `.json`/`.cdmod` patches, flattened into `mods/`.
    Json,
    /// Files below the plugin folder go to `bin64/`.
    Binaries(String),
    Fallback,
}

fn files(entries: &[String]) -> impl Iterator<Item = &str> {
    entries.iter().filter(|entry| !entry.ends_with('/')).map(String::as_str)
}
fn name(path: &str) -> &str { path.rsplit('/').next().unwrap_or(path) }
fn plugin(path: &str) -> bool { PLUGINS.iter().any(|ext| path.to_ascii_lowercase().ends_with(ext)) }
fn executable(path: &str) -> bool { path.to_ascii_lowercase().ends_with(".exe") }
/// Patch folders are numbered like the game's own (`0036`).
fn patch_folder(part: &str) -> bool { part.len() == 4 && part.starts_with("00") }
fn data_folder(part: &str) -> bool { part == "meta" || patch_folder(part) && part[2..].parse::<u8>().is_ok_and(|n| n <= 35) }

/// Parent prefix (`""` or `Wrapper/`) of the shallowest folder matching `matches`.
fn folder(entries: &[String], matches: impl Fn(&str) -> bool) -> Option<String> {
    files(entries).filter_map(|file| {
        let parts: Vec<&str> = file.split('/').collect();
        let index = parts[..parts.len() - 1].iter().position(|part| matches(part))?;
        Some(parts[..index].iter().map(|part| format!("{part}/")).collect::<String>())
    }).min_by(|a, b| a.matches('/').count().cmp(&b.matches('/').count()).then(a.cmp(b)))
}
fn parent(path: &str) -> String { path.rsplit_once('/').map(|(parent, _)| format!("{parent}/")).unwrap_or_default() }
/// Folder of the shallowest file matching `matches`.
fn file_folder(entries: &[String], matches: impl Fn(&str) -> bool) -> Option<String> {
    files(entries).filter(|file| matches(file)).map(parent)
        .min_by(|a, b| a.matches('/').count().cmp(&b.matches('/').count()).then(a.cmp(b)))
}
/// The folder holding the patch folder becomes the mod folder in `mods/`.
fn mods_layout(prefix: String) -> Layout {
    if prefix.is_empty() { return Layout::Mods { base: String::new(), own_folder: true }; }
    Layout::Mods { base: parent(prefix.trim_end_matches('/')), own_folder: false }
}

fn classify(entries: &[String]) -> Layout {
    let any = |test: &dyn Fn(&str) -> bool| files(entries).any(test);
    let has_exe = any(&executable);
    if let Some(prefix) = folder(entries, |part| part.eq_ignore_ascii_case("bin64")) { return Layout::Root(prefix); }
    if let Some(prefix) = file_folder(entries, |file| name(file).starts_with("DMM") && executable(file)) { return Layout::Root(prefix); }
    if let Some(prefix) = file_folder(entries, |file| TOOLS.contains(&name(file))) { return Layout::Root(prefix); }
    let patch = folder(entries, patch_folder);
    if patch.is_some() && any(&plugin) && !has_exe { return Layout::SpecialPatch; }
    if let Some(prefix) = folder(entries, |part| part.eq_ignore_ascii_case("files")) { return mods_layout(prefix); }
    if let Some(prefix) = patch { return mods_layout(prefix); }
    if any(&|file| file.to_ascii_lowercase().ends_with(".dds")) {
        if let Some(prefix) = folder(entries, data_folder) { return mods_layout(prefix); }
    }
    if !any(&|file| name(file).eq_ignore_ascii_case("modinfo.json")) && any(&|file| file.to_ascii_lowercase().ends_with(".json")) { return Layout::Json; }
    if any(&plugin) && !has_exe {
        if let Some(prefix) = file_folder(entries, plugin) { return Layout::Binaries(prefix); }
    }
    Layout::Fallback
}

fn json_patch(path: &str) -> bool { [".json", ".cdmod"].iter().any(|ext| path.to_ascii_lowercase().ends_with(ext)) }

fn place(layout: &Layout, source: &str, mod_id: &str) -> Option<String> {
    match layout {
        Layout::Root(prefix) => source.strip_prefix(prefix.as_str()).map(str::to_string),
        Layout::SpecialPatch => Some(if plugin(source) { format!("bin64/{}", name(source)) } else { format!("mods/{source}") }),
        Layout::Mods { base, own_folder } => source.strip_prefix(base.as_str())
            .map(|rest| if *own_folder { format!("mods/{mod_id}/{rest}") } else { format!("mods/{rest}") }),
        Layout::Json => json_patch(source).then(|| format!("mods/{}", name(source))),
        Layout::Binaries(prefix) => source.strip_prefix(prefix.as_str()).map(|rest| format!("bin64/{rest}")),
        Layout::Fallback => Some(source.to_string()),
    }
}

/// Destination of one file of a Crimson Desert package, or none when the
/// installer leaves it out (outside the package root, or not a JSON patch).
pub(super) fn target(module: &Definition, source: &str, mod_id: &str, entries: &[String]) -> Result<Option<String>, String> {
    if !super::nexus_modules::relative(source) { return Err(format!("Caminho inseguro no pacote: {source}")); }
    let Some(target) = place(&classify(entries), source, mod_id) else { return Ok(None); };
    if !module.allowed(&target) { return Err(format!("Destino fora das regras do módulo: {target}")); }
    Ok(Some(target))
}

/// Alternatives the player chooses from, as the Vortex dialog for several JSON
/// patches; several ASI variants in sibling folders are offered the same way.
fn alternatives(entries: &[String]) -> Option<(&'static str, Vec<String>)> {
    match classify(entries) {
        Layout::Json => {
            let patches: Vec<String> = files(entries).filter(|file| json_patch(file)).map(str::to_string).collect();
            (patches.len() > 1).then_some(("Patches JSON", patches))
        }
        Layout::Binaries(prefix) => {
            let mut folders: Vec<String> = files(entries).filter(|file| plugin(file)).map(parent).collect();
            folders.sort();
            folders.dedup();
            (folders.iter().any(|folder| !folder.starts_with(prefix.as_str()))).then_some(("Variantes do plugin", folders))
        }
        _ => None,
    }
}

pub(super) fn resolve(entries: &[String], module: &Definition, _mod_id: &str, choices: Option<&[String]>) -> Result<Option<Resolution>, String> {
    if module.domain != "crimsondesert" { return Ok(None); }
    let Some((group, options)) = alternatives(entries) else { return Ok(None); };
    let selection: Vec<String> = match choices {
        Some(choices) => options.iter().filter(|option| choices.contains(option)).cloned().collect(),
        None => options.iter().take(1).cloned().collect(),
    };
    let mut issues = Vec::new();
    if selection.is_empty() { issues.push("Selecione ao menos uma opção.".to_string()); }
    let mut output: BTreeMap<String, (String, String)> = BTreeMap::new();
    for option in &selection {
        let chosen: Vec<(String, String)> = if json_patch(option) {
            vec![(option.clone(), format!("mods/{}", name(option)))]
        } else {
            files(entries).filter_map(|file| file.strip_prefix(option.as_str()).map(|rest| (file.to_string(), format!("bin64/{rest}")))).collect()
        };
        for (source, target) in chosen {
            if !module.allowed(&target) { return Err(format!("Destino fora das regras do módulo: {target}")); }
            if let Some((previous, _)) = output.get(&target.to_ascii_lowercase()) {
                if previous != &source { issues.push(format!("As opções escolhidas instalam o mesmo arquivo: {target}")); }
            }
            output.insert(target.to_ascii_lowercase(), (source, target));
        }
    }
    let options = options.iter().map(|option| OptionInfo {
        id: option.clone(), name: option.trim_end_matches('/').to_string(), description: String::new(), kind: "Optional".into(), selected: selection.contains(option),
    }).collect();
    Ok(Some(Resolution {
        installer: Some(Installer { name: "Crimson Desert".into(), steps: vec![Step { name: group.into(), groups: vec![Group { name: group.into(), kind: "SelectAtLeastOne".into(), options }] }] }),
        selection,
        issues,
        files: Some(output.into_values().collect()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "00000000-0000-0000-0000-000000000001";
    fn module() -> Definition {
        super::super::nexus_modules::registry().definitions.into_iter().find(|m| m.domain == "crimsondesert").unwrap()
    }
    fn entries(items: &[&str]) -> Vec<String> { items.iter().map(|item| item.to_string()).collect() }
    fn routed(items: &[&str]) -> Vec<Option<String>> {
        let (game, list) = (module(), entries(items));
        list.iter().map(|source| target(&game, source, ID, &list).unwrap()).collect()
    }
    fn some(items: &[&str]) -> Vec<Option<String>> { items.iter().map(|item| Some(item.to_string())).collect() }

    #[test]
    fn root_packages_keep_their_paths_from_the_bin64_parent() {
        assert_eq!(routed(&["Mod/bin64/plugins/a.asi", "Mod/bin64/a.ini", "Mod/0036/0.paz", "readme.txt"]),
            vec![Some("bin64/plugins/a.asi".to_string()), Some("bin64/a.ini".to_string()), Some("0036/0.paz".to_string()), None]);
    }
    #[test]
    fn managers_and_tools_go_to_the_game_root() {
        assert_eq!(routed(&["DMM v2/DMM_2.1.exe", "DMM v2/lib/core.dll"]), some(&["DMM_2.1.exe", "lib/core.dll"]));
        assert_eq!(routed(&["CrimsonSharp.exe", "config.txt"]), some(&["CrimsonSharp.exe", "config.txt"]));
    }
    #[test]
    fn patch_folders_go_to_mods_inside_their_own_or_the_mod_folder() {
        assert_eq!(routed(&["0036/0.paz", "0036/0.pamt", "modinfo.json"]),
            ["0036/0.paz", "0036/0.pamt", "modinfo.json"].iter().map(|file| Some(format!("mods/{ID}/{file}"))).collect::<Vec<_>>());
        assert_eq!(routed(&["Better Lanterns/0036/0.paz", "Better Lanterns/modinfo.json"]),
            some(&["mods/Better Lanterns/0036/0.paz", "mods/Better Lanterns/modinfo.json"]));
        assert_eq!(routed(&["Pack/Armor/files/gamedata/item.pabgb", "Pack/Armor/manifest.json"]),
            some(&["mods/Armor/files/gamedata/item.pabgb", "mods/Armor/manifest.json"]));
        assert_eq!(routed(&["Retexture/meta/ui.dds"]), some(&["mods/Retexture/meta/ui.dds"]));
    }
    #[test]
    fn plugins_with_patch_folders_split_between_bin64_and_mods() {
        assert_eq!(routed(&["CharacterCreatorAsi/creator.asi", "0036/0.paz"]), some(&["bin64/creator.asi", "mods/0036/0.paz"]));
    }
    #[test]
    fn json_patches_are_flattened_and_plugins_go_to_bin64() {
        assert_eq!(routed(&["Stamina/infinite.json", "Stamina/readme.txt"]), vec![Some("mods/infinite.json".to_string()), None]);
        assert_eq!(routed(&["AutoLoot/autoloot.asi", "AutoLoot/autoloot.ini"]), some(&["bin64/autoloot.asi", "bin64/autoloot.ini"]));
        assert_eq!(routed(&["winmm.dll", "winmm-x64.SHA512"])[0].as_deref(), Some("bin64/winmm.dll"));
        assert_eq!(routed(&["preset.ini"]), some(&["preset.ini"]));
    }
    #[test]
    fn the_game_executable_is_never_a_destination() {
        let (game, list) = (module(), entries(&["Mod/bin64/CrimsonDesert.exe"]));
        assert!(target(&game, &list[0], ID, &list).is_err());
        assert!(target(&game, "../x.asi", ID, &entries(&["../x.asi"])).is_err());
    }
    #[test]
    fn several_json_patches_or_plugin_variants_are_offered_as_choices() {
        let game = module();
        let list = entries(&["Options/fast.json", "Options/slow.json", "readme.txt"]);
        let resolution = resolve(&list, &game, ID, None).unwrap().unwrap();
        assert_eq!(resolution.files.unwrap(), vec![("Options/fast.json".to_string(), "mods/fast.json".to_string())]);
        let both = resolve(&list, &game, ID, Some(&list[..2])).unwrap().unwrap();
        assert_eq!(both.files.unwrap().len(), 2);
        assert!(!resolve(&list, &game, ID, Some(&[])).unwrap().unwrap().issues.is_empty());
        let variants = entries(&["Fast/plugin.asi", "Fast/plugin.ini", "Slow/plugin.asi"]);
        let chosen = resolve(&variants, &game, ID, Some(&["Slow/".to_string()])).unwrap().unwrap();
        assert_eq!(chosen.files.unwrap(), vec![("Slow/plugin.asi".to_string(), "bin64/plugin.asi".to_string())]);
        let clash = resolve(&variants, &game, ID, Some(&["Fast/".to_string(), "Slow/".to_string()])).unwrap().unwrap();
        assert!(!clash.issues.is_empty());
        assert!(resolve(&entries(&["one.json"]), &game, ID, None).unwrap().is_none());
        assert!(resolve(&entries(&["Mod/plugin.asi", "Mod/plugins/extra.asi"]), &game, ID, None).unwrap().is_none());
    }
}
