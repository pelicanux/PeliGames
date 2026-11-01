//! Regression cases for external games after their Wine prefix is deleted.
use super::*;
use crate::core::paths;
use std::os::unix::fs::PermissionsExt;

struct Fixture {
    root: PathBuf,
    index: PathBuf,
    prefix: PathBuf,
    exe: PathBuf,
    runner: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "peligames-independent-registration-{}",
            uuid::Uuid::new_v4()
        ));
        let index = root.join("config/library/installed-destinations.json");
        let prefix = root.join("prefix");
        let exe = root.join("external/Game.exe");
        let runner = root.join("runner");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, b"MZfixture").unwrap();
        fs::create_dir_all(&runner).unwrap();
        fs::write(
            runner.join("proton"),
            b"#!/bin/sh\n# PROTON_ENABLE_WAYLAND=1\n",
        )
        .unwrap();
        fs::set_permissions(runner.join("proton"), fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            root,
            index,
            prefix,
            exe,
            runner,
        }
    }
    fn add(&self) -> InstalledEntry {
        add_at(
            &self.index,
            super::super::game_installation::InstallationRequest {
                name: "External Game".into(),
                directory: self.prefix.to_string_lossy().into_owned(),
                executable: self.exe.to_string_lossy().into_owned(),
                proton: self.runner.to_string_lossy().into_owned(),
                cover_url: Some("https://example.com/game.jpg".into()),
            },
        )
        .unwrap()
    }
    fn settings(&self, entry: &InstalledEntry, prefix: &Path) -> EntrySettings {
        EntrySettings {
            path: entry.path.clone(),
            name: entry.name.clone(),
            executable: entry.executable.clone(),
            proton: entry.proton.clone(),
            prefix: Some(prefix.to_string_lossy().into_owned()),
            advanced: Some(entry.advanced.clone()),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn deleted_external_game_prefix_keeps_settings_and_accepts_a_new_path_after_reload() {
    let f = Fixture::new();
    let entry = f.add();
    fs::create_dir_all(f.prefix.join("drive_c")).unwrap();
    fs::write(f.prefix.join("system.reg"), b"old registry").unwrap();
    let mut settings = f.settings(&entry, &f.prefix);
    settings.name = "My external game".into();
    let mut advanced = entry.advanced.clone();
    advanced.options.insert("wayland".into(), true);
    settings.advanced = Some(advanced);
    let saved = update_at(&f.index, settings).unwrap();
    fs::remove_dir_all(&f.prefix).unwrap();
    for rescan in [false, true, false] {
        let games = list_at(&f.index, rescan).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].path, saved.path);
        assert_eq!(games[0].name, saved.name);
        assert_eq!(games[0].proton, saved.proton);
        assert_eq!(games[0].cover_url, saved.cover_url);
        assert_eq!(games[0].advanced.options.get("wayland"), Some(&true));
        assert!(!f.prefix.exists());
    }
    let new = f.root.join("replacement/new-prefix");
    let updated = update_at(&f.index, f.settings(&saved, &new)).unwrap();
    assert_eq!(Path::new(&updated.prefix), new);
    assert!(!new.exists());
    assert!(!f.prefix.exists());
    let reloaded = list_at(&f.index, true).unwrap();
    assert_eq!(reloaded[0].prefix, updated.prefix);
    assert_eq!(fs::read(&f.exe).unwrap(), b"MZfixture");
    let folder = paths::layout::installation_records(&f.index).unwrap();
    let names: Vec<_> = fs::read_dir(&folder)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.contains(&"external-game.json".into()),
        "readable record name: {names:?}"
    );
    assert!(!folder.starts_with(&f.prefix));
    remove_entry_at(&f.index, &entry.path).unwrap();
    assert!(list_at(&f.index, true).unwrap().is_empty());
    assert!(f.exe.is_file());
    assert!(!new.exists());
}
#[test]
fn legacy_registration_migrates_before_the_whole_installation_folder_is_deleted() {
    let f = Fixture::new();
    let legacy = f.root.join("legacy");
    fs::create_dir_all(legacy.join("prefix/drive_c")).unwrap();
    let entry = InstalledEntry {
        discovery_source: None,
        recommended: false,
        shortcut: None,
        profile: None,
        launch_arguments: vec![],
        advanced: Default::default(),
        name: "Legacy external".into(),
        path: f.exe.to_string_lossy().into_owned(),
        directory: f.exe.parent().unwrap().to_string_lossy().into_owned(),
        executable: f.exe.to_string_lossy().into_owned(),
        prefix: legacy.join("prefix").to_string_lossy().into_owned(),
        proton: f.runner.to_string_lossy().into_owned(),
        cover_url: None,
        launcher: "PeliGames".into(),
    };
    let record = InstallationRecord {
        name: entry.name.clone(),
        directory: legacy.to_string_lossy().into_owned(),
        prefix: entry.prefix.clone(),
        proton: entry.proton.clone(),
        installer: "/previous/setup.exe".into(),
        cover_url: None,
        entries: vec![entry.clone()],
        selected_executable: None,
        overrides: vec![],
        baseline: vec![],
        known_shortcuts: None,
        launcher_games: vec![],
        install_shortcuts: None,
        profile_kind: Default::default(),
    };
    write_json(&legacy.join("installation.json"), &record).unwrap();
    write_json(&f.index, &BTreeSet::from([record.directory.clone()])).unwrap();
    assert_eq!(list_at(&f.index, false).unwrap().len(), 1);
    fs::remove_dir_all(&legacy).unwrap();
    assert_eq!(list_at(&f.index, true).unwrap()[0].path, entry.path);
    assert!(!legacy.exists());
    let new = f.root.join("new-prefix");
    update_at(&f.index, f.settings(&entry, &new)).unwrap();
    assert_eq!(
        list_at(&f.index, false).unwrap()[0].prefix,
        new.to_string_lossy()
    );
    assert!(!legacy.exists());
}
#[test]
fn deleting_only_the_portable_manifest_does_not_erase_the_registration() {
    let f = Fixture::new();
    let entry = f.add();
    fs::remove_file(f.prefix.join("installation.json")).unwrap();
    assert_eq!(list_at(&f.index, false).unwrap()[0].path, entry.path);
    assert!(f.prefix.join("installation.json").is_file());
    let updated = update_at(&f.index, f.settings(&entry, &f.root.join("new-prefix"))).unwrap();
    assert!(f.prefix.join("installation.json").is_file());
    assert_eq!(updated.proton, entry.proton);
}
#[test]
fn uninstall_with_an_already_deleted_prefix_forgets_only_its_own_registration() {
    let f = Fixture::new();
    let entry = f.add();
    fs::remove_dir_all(&f.prefix).unwrap();
    let untouched = f.root.join("another-prefix");
    fs::create_dir(&untouched).unwrap();
    fs::write(untouched.join("save.dat"), b"keep").unwrap();
    uninstall_with_shortcuts_at(&f.index, &entry.path, &[], UninstallOptions::default()).unwrap();
    assert!(destinations(&f.index).unwrap().is_empty());
    assert!(registry::read(&f.index, &f.prefix).unwrap().is_none());
    assert_eq!(fs::read(&f.exe).unwrap(), b"MZfixture");
    assert_eq!(fs::read(untouched.join("save.dat")).unwrap(), b"keep");
}
#[test]
fn older_launcher_edits_are_detected_without_discarding_the_independent_record() {
    let f = Fixture::new();
    let entry = f.add();
    let mut portable: InstallationRecord =
        serde_json::from_slice(&fs::read(f.prefix.join("installation.json")).unwrap()).unwrap();
    portable.entries[0].name = "Edited by older launcher".into();
    write_json(&f.prefix.join("installation.json"), &portable).unwrap();
    assert_eq!(
        list_at(&f.index, false).unwrap()[0].name,
        "Edited by older launcher"
    );
    fs::remove_dir_all(&f.prefix).unwrap();
    assert_eq!(
        list_at(&f.index, false).unwrap()[0].name,
        "Edited by older launcher"
    );
    assert_eq!(list_at(&f.index, false).unwrap()[0].path, entry.path);
}
#[test]
fn equally_named_installations_get_separate_readable_records() {
    let f = Fixture::new();
    let one = f.add();
    let second = f.root.join("second-prefix");
    let two = add_at(
        &f.index,
        super::super::game_installation::InstallationRequest {
            name: one.name.clone(),
            directory: second.to_string_lossy().into_owned(),
            executable: f.exe.to_string_lossy().into_owned(),
            proton: one.proton.clone(),
            cover_url: None,
        },
    )
    .unwrap();
    fs::remove_dir_all(&f.prefix).unwrap();
    fs::remove_dir_all(&second).unwrap();
    assert_eq!(list_at(&f.index, false).unwrap().len(), 2);
    assert_ne!(one.path, two.path);
    let root = paths::layout::installation_records(&f.index).unwrap();
    assert!(root.join("external-game.json").is_file());
    assert!(root.join("external-game-2.json").is_file());
    remove_entry_at(&f.index, &one.path).unwrap();
    assert_eq!(list_at(&f.index, false).unwrap()[0].path, two.path);
}

#[test]
fn recreated_prefix_runs_the_external_game_with_the_saved_runner() {
    use crate::core::test_support::RetryOutput;
    let f = Fixture::new();
    let entry = f.add();
    fs::remove_dir_all(&f.prefix).unwrap();
    let wine = f.root.join("isolated-wine");
    fs::create_dir_all(wine.join("bin")).unwrap();
    for binary in ["wine", "wineserver"] {
        let file = wine.join("bin").join(binary);
        fs::write(&file,b"#!/bin/sh\nmkdir -p \"$WINEPREFIX/drive_c\"\nprintf registry > \"$WINEPREFIX/system.reg\"\nprintf '%s|%s' \"$WINEPREFIX\" \"$1\"\n").unwrap();
        fs::set_permissions(file, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let new = f.root.join("replacement/prefix");
    let mut settings = f.settings(&entry, &new);
    settings.proton = wine.to_string_lossy().into_owned();
    let updated = update_at(&f.index, settings).unwrap();
    assert!(!new.exists());
    let reloaded = list_at(&f.index, false)
        .unwrap()
        .into_iter()
        .find(|e| e.path == updated.path)
        .unwrap();
    let mut run =
        super::super::game_installation::prepare_registered_entry(reloaded, None, vec![]).unwrap();
    assert!(!new.join("system.reg").exists());
    let output = run.command.output_retrying();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{}|{}", new.display(), f.exe.display())
    );
    assert!(new.join("drive_c").is_dir());
    assert!(new.join("system.reg").is_file());
    assert!(!f.prefix.exists());
    assert_eq!(fs::read(&f.exe).unwrap(), b"MZfixture");
}
#[test]
fn restoring_an_unchanged_portable_copy_does_not_undo_the_new_prefix() {
    let f = Fixture::new();
    let entry = f.add();
    let original = fs::read(f.prefix.join("installation.json")).unwrap();
    fs::remove_dir_all(&f.prefix).unwrap();
    let new = f.root.join("replacement");
    update_at(&f.index, f.settings(&entry, &new)).unwrap();
    fs::create_dir(&f.prefix).unwrap();
    fs::write(f.prefix.join("installation.json"), original).unwrap();
    assert_eq!(
        list_at(&f.index, false).unwrap()[0].prefix,
        new.to_string_lossy()
    );
}
#[test]
fn a_redirected_registry_cannot_write_into_an_unrelated_folder() {
    let f = Fixture::new();
    let entry = f.add();
    let records = paths::layout::installation_records(&f.index).unwrap();
    fs::rename(&records, records.with_file_name("saved-records")).unwrap();
    let other = f.root.join("untouched");
    fs::create_dir(&other).unwrap();
    std::os::unix::fs::symlink(&other, &records).unwrap();
    assert!(update_at(&f.index, f.settings(&entry, &f.root.join("new-prefix"))).is_err());
    assert!(fs::read_dir(&other).unwrap().next().is_none());
}

#[test]
fn a_redirected_installation_cannot_commit_a_failed_settings_change() {
    let f = Fixture::new();
    let entry = f.add();
    let original = f.root.join("saved-prefix");
    fs::rename(&f.prefix, &original).unwrap();
    let other = f.root.join("untouched");
    fs::create_dir(&other).unwrap();
    std::os::unix::fs::symlink(&other, &f.prefix).unwrap();
    assert!(update_at(&f.index, f.settings(&entry, &f.root.join("new-prefix"))).is_err());
    assert!(fs::read_dir(&other).unwrap().next().is_none());
    fs::remove_file(&f.prefix).unwrap();
    fs::rename(original, &f.prefix).unwrap();
    assert_eq!(list_at(&f.index, false).unwrap()[0].prefix, entry.prefix);
}

#[test]
fn deleting_the_central_record_restores_the_latest_settings_from_the_portable_copy() {
    let f = Fixture::new();
    let entry = f.add();
    let exe = f.root.join("external/Updated.exe");
    fs::write(&exe, b"MZupdated").unwrap();
    let mut settings = f.settings(&entry, &f.root.join("replacement"));
    settings.name = "Updated external game".into();
    settings.executable = exe.to_string_lossy().into_owned();
    let runner = f.root.join("updated-runner");
    fs::create_dir(&runner).unwrap();
    fs::copy(f.runner.join("proton"), runner.join("proton")).unwrap();
    settings.proton = runner.to_string_lossy().into_owned();
    settings
        .advanced
        .as_mut()
        .unwrap()
        .options
        .insert("wayland".into(), true);
    let updated = update_at(&f.index, settings).unwrap();
    let records = paths::layout::installation_records(&f.index).unwrap();
    fs::remove_dir_all(&records).unwrap();
    let recovered = list_at(&f.index, false).unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].path, updated.path);
    assert_eq!(recovered[0].prefix, updated.prefix);
    assert_eq!(recovered[0].executable, updated.executable);
    assert_eq!(recovered[0].name, updated.name);
    assert_eq!(recovered[0].proton, updated.proton);
    assert_eq!(recovered[0].advanced.options.get("wayland"), Some(&true));
    let central = paths::names::find(&records, &f.prefix.to_string_lossy()).unwrap();
    assert!(central.with_extension("json").is_file());
    fs::remove_file(f.prefix.join("installation.json")).unwrap();
    assert_eq!(
        list_at(&f.index, false).unwrap()[0].executable,
        updated.executable
    );
    let portable: InstallationRecord =
        serde_json::from_slice(&fs::read(f.prefix.join("installation.json")).unwrap()).unwrap();
    assert_eq!(portable.entries[0].prefix, updated.prefix);
    assert_eq!(portable.entries[0].executable, updated.executable);
    assert_eq!(portable.entries[0].proton, updated.proton);
}

#[test]
fn reset_inventory_does_not_recreate_a_missing_portable_record() {
    let f = Fixture::new();
    f.add();
    fs::remove_file(f.prefix.join("installation.json")).unwrap();
    assert!(registry::peek(&f.index, &f.prefix).unwrap().is_some());
    assert!(!f.prefix.join("installation.json").exists());
}
