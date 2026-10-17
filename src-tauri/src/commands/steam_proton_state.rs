//! Read the selected Steam tool, rather than the last runner recorded in a prefix.
//! No background scans: configuration contents are cached by file metadata.
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::SystemTime,
};

#[derive(Clone, PartialEq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
    inode: u64,
    changed: (i64, i64),
}
fn stamp(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 4 * 1024 * 1024 {
        return None;
    }
    use std::os::unix::fs::MetadataExt;
    Some(Stamp {
        modified: meta.modified().ok(),
        len: meta.len(),
        inode: meta.ino(),
        changed: (meta.ctime(), meta.ctime_nsec()),
    })
}
type Cache = BTreeMap<PathBuf, (Stamp, Arc<Value>)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
fn document(path: &Path) -> Option<Arc<Value>> {
    let current = stamp(path)?;
    let mut cache = CACHE.get_or_init(Default::default).lock().ok()?;
    if let Some((_, doc)) = cache.get(path).filter(|(old, _)| *old == current) {
        return Some(doc.clone());
    }
    let value = Arc::new(super::external_proton::vdf(&fs::read_to_string(path).ok()?).ok()?);
    if stamp(path)? != current {
        return None;
    }
    if cache.len() >= 128 {
        cache.clear();
    }
    cache.insert(path.into(), (current, value.clone()));
    Some(value)
}
fn field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let mut value = value;
    for key in keys {
        value = value
            .as_object()?
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))?
            .1;
    }
    Some(value)
}
#[derive(Clone)]
pub(super) struct Selection {
    pub proton: Option<String>,
    pub name: String,
}
fn runner(folder: &Path, name: &str) -> Option<Selection> {
    let original = fs::read_to_string(folder.join(".peligames-proton-path")).ok();
    let path = original.as_deref().map(Path::new).unwrap_or(folder);
    if !path.join("proton").is_file() {
        return None;
    }
    Some(Selection {
        proton: Some(path.canonicalize().ok()?.to_string_lossy().into_owned()),
        name: name.into(),
    })
}
pub(super) fn selected(home: &Path, flatpak: bool, app: &str) -> Option<Selection> {
    let root = super::super::steam_installation::roots(home, flatpak)
        .into_iter()
        .find(|r| r.join("config/config.vdf").is_file())?;
    selected_in(&root, app)
}
type SelectionStamp = (Option<Stamp>, Option<Stamp>, Option<Stamp>);
type Selections = BTreeMap<(PathBuf, String), (SelectionStamp, Selection)>;
static SELECTIONS: OnceLock<Mutex<Selections>> = OnceLock::new();
fn selected_in(root: &Path, app: &str) -> Option<Selection> {
    let current = (
        stamp(&root.join("config/config.vdf")),
        stamp(&root.join("config/libraryfolders.vdf")),
        stamp(&root.join("steamapps/libraryfolders.vdf")),
    );
    let mut cache = SELECTIONS.get_or_init(Default::default).lock().ok()?;
    let key = (root.to_path_buf(), app.into());
    if let Some((_, value)) = cache.get(&key).filter(|(old, value)| {
        *old == current
            && value
                .proton
                .as_ref()
                .is_some_and(|p| Path::new(p).join("proton").is_file())
    }) {
        return Some(value.clone());
    }
    let value = resolve_in(root, app)?;
    if cache.len() >= 256 {
        cache.clear();
    }
    cache.insert(key, (current, value.clone()));
    Some(value)
}
fn resolve_in(root: &Path, app: &str) -> Option<Selection> {
    let config = document(&root.join("config/config.vdf"))?;
    let mapping = field(
        &config,
        &[
            "InstallConfigStore",
            "Software",
            "Valve",
            "Steam",
            "CompatToolMapping",
        ],
    )?;
    let name = field(mapping, &[app, "name"])
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            field(mapping, &["0", "name"])
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })?;
    if name.contains(['/', '\\', '\0']) || name == "." || name == ".." {
        return None;
    }
    let display = if name == "proton_experimental" {
        "Proton Experimental".into()
    } else if let Some(version) = name.strip_prefix("proton_") {
        format!("Proton {version}.0")
    } else {
        name.into()
    };
    let tools = root.join("compatibilitytools.d");
    if let Some(tool) = runner(&tools.join(name), &display) {
        return Some(tool);
    }
    let mut libraries = vec![root.to_path_buf()];
    for relative in ["config/libraryfolders.vdf", "steamapps/libraryfolders.vdf"] {
        if let Some(doc) = document(&root.join(relative)) {
            if let Some(folders) = field(&doc, &["libraryfolders"]).and_then(Value::as_object) {
                for folder in folders.values() {
                    if let Some(path) = field(folder, &["path"]).and_then(Value::as_str) {
                        let path = PathBuf::from(path);
                        if path.is_absolute() && !libraries.contains(&path) {
                            libraries.push(path);
                        }
                    }
                }
            }
        }
    }
    // Bounded, shallow lookup restricted to compatibility tool directories.
    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(&tools) {
        candidates.extend(entries.flatten().take(128).map(|e| e.path()));
    }
    for library in libraries {
        let common = library.join("steamapps/common");
        let folder = if name == "proton_experimental" {
            "Proton - Experimental".into()
        } else if let Some(v) = name.strip_prefix("proton_") {
            format!("Proton {v}.0")
        } else {
            name.to_owned()
        };
        if let Some(tool) = runner(&common.join(folder), &display) {
            return Some(tool);
        }
        if let Ok(entries) = fs::read_dir(common) {
            candidates.extend(
                entries
                    .flatten()
                    .filter(|e| e.file_name().to_string_lossy().starts_with("Proton"))
                    .take(64)
                    .map(|e| e.path()),
            );
        }
    }
    for folder in candidates {
        let Some(doc) = document(&folder.join("compatibilitytool.vdf")) else {
            continue;
        };
        let Some(tool) = field(&doc, &["compatibilitytools", "compat_tools", name]) else {
            continue;
        };
        let title = field(tool, &["display_name"])
            .and_then(Value::as_str)
            .unwrap_or(&display);
        let relative = field(tool, &["install_path"])
            .and_then(Value::as_str)
            .unwrap_or(".");
        if let Some(tool) = runner(&folder.join(relative), title) {
            return Some(tool);
        }
    }
    // Still show the newly configured name if that runner is not installed here.
    Some(Selection {
        proton: None,
        name: display,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn config(name: &str) -> String {
        format!("\"InstallConfigStore\" {{\"Software\" {{\"Valve\" {{\"Steam\" {{\"CompatToolMapping\" {{\"42\" {{\"name\" \"{name}\"}}}}}}}}}}}}")
    }
    #[test]
    fn selections_follow_external_libraries_and_stay_separate_by_installation() {
        let home = std::env::temp_dir().join(format!(
            "peligames-steam-selection-sources-{}",
            uuid::Uuid::new_v4()
        ));
        let native = super::super::super::steam_installation::roots(&home, false).remove(0);
        let flatpak = super::super::super::steam_installation::roots(&home, true).remove(0);
        let disk = home.join("disk");
        fs::create_dir_all(disk.join("steamapps/common/Proton - Experimental")).unwrap();
        fs::write(
            disk.join("steamapps/common/Proton - Experimental/proton"),
            "test",
        )
        .unwrap();
        fs::create_dir_all(native.join("config")).unwrap();
        fs::write(
            native.join("config/config.vdf"),
            config("proton_experimental"),
        )
        .unwrap();
        fs::write(
            native.join("config/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\" {{\"0\" {{\"path\" \"{}\"}}}}",
                disk.display()
            ),
        )
        .unwrap();
        fs::create_dir_all(flatpak.join("config")).unwrap();
        fs::create_dir_all(flatpak.join("compatibilitytools.d/GE-Proton")).unwrap();
        fs::write(
            flatpak.join("compatibilitytools.d/GE-Proton/proton"),
            "test",
        )
        .unwrap();
        fs::write(flatpak.join("config/config.vdf"), config("GE-Proton")).unwrap();
        assert!(selected(&home, false, "42")
            .unwrap()
            .proton
            .unwrap()
            .starts_with(disk.to_str().unwrap()));
        assert_eq!(selected(&home, true, "42").unwrap().name, "GE-Proton");
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn changing_the_mapping_refreshes_without_a_game_launch_and_reuses_unchanged_documents() {
        let root =
            std::env::temp_dir().join(format!("peligames-steam-selected-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("steamapps/common/Proton - Experimental")).unwrap();
        fs::write(
            root.join("steamapps/common/Proton - Experimental/proton"),
            "test",
        )
        .unwrap();
        fs::create_dir_all(root.join("compatibilitytools.d/GE-Proton")).unwrap();
        fs::write(root.join("compatibilitytools.d/GE-Proton/proton"), "test").unwrap();
        let path = root.join("config/config.vdf");
        fs::write(&path, config("proton_experimental")).unwrap();
        let first = document(&path).unwrap();
        assert!(Arc::ptr_eq(&first, &document(&path).unwrap()));
        assert_eq!(
            selected_in(&root, "42").unwrap().name,
            "Proton Experimental"
        );
        fs::write(&path, config("GE-Proton")).unwrap();
        let next = selected_in(&root, "42").unwrap();
        assert_eq!(next.name, "GE-Proton");
        assert!(next
            .proton
            .unwrap()
            .ends_with("compatibilitytools.d/GE-Proton"));
        assert!(!Arc::ptr_eq(&first, &document(&path).unwrap()));
        fs::remove_dir_all(root).unwrap();
    }
}
