use crate::{
    catalog::{Artifact, Catalog, Entry},
    manifest::Manifest,
    package,
    protocol::{Initialize, Request, Response},
    registry::{self, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const HOST: &str = "0.5.5";
const PLATFORM: &str = "linux-x86_64";

#[test]
fn native_launch_contract_requires_a_service_and_bounds_opaque_settings() {
    use crate::{launch, manifest::Capability};
    let mut recipe = manifest("native-example", "0.1.0");
    recipe.capabilities.push(Capability::NativeLaunch);
    recipe.validate().unwrap();
    recipe.entrypoints.service = None;
    assert!(recipe.validate().is_err());
    let mut values = launch::Selections::from([(
        "native-example".into(),
        launch::Settings {
            enabled: true,
            group: None,
            data: json!({"enabled":true}),
        },
    )]);
    launch::validate(&values).unwrap();
    values.get_mut("native-example").unwrap().data = json!("x".repeat(32 * 1024));
    assert!(launch::validate(&values).is_err());
    values.clear();
    values.insert(
        "../other".into(),
        launch::Settings {
            enabled: false,
            group: None,
            data: json!(null),
        },
    );
    assert!(launch::validate(&values).is_err());
}
fn manifest(id: &str, version: &str) -> Manifest {
    let mut manifest: Manifest =
        serde_json::from_str(include_str!("../../template/manifest.json")).unwrap();
    manifest.id = id.into();
    manifest.version = version.into();
    manifest
}
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "peligames-addon-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self { root }
    }
    fn store(&self) -> Store {
        Store {
            packages: self.root.join("addons"),
            state_file: self.root.join("settings/addons.json"),
        }
    }
    fn archive(&self, manifest: Manifest) -> (Entry, Vec<u8>) {
        let payload = self
            .root
            .join(format!("payload-{}-{}", manifest.id, manifest.version));
        fs::create_dir_all(payload.join("bin")).unwrap();
        fs::write(
            payload.join("bin/example-service"),
            format!("independent payload {} {}", manifest.id, manifest.version),
        )
        .unwrap();
        fs::write(payload.join("LICENSE"), "GPL-3.0-only fixture").unwrap();
        Self::write_ui(&payload, &manifest);
        let bytes = package::pack(manifest.clone(), &payload).unwrap();
        (entry(manifest, &bytes), bytes)
    }
    fn write_ui(payload: &Path, manifest: &Manifest) {
        if let Some(path) = &manifest.entrypoints.ui {
            let path = payload.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "export function createAddon(host) { return host; }").unwrap();
        }
    }
    fn prepare(&self, manifest: Manifest) -> package::Receipt {
        let (entry, bytes) = self.archive(manifest);
        package::prepare(&self.store().packages, &entry, PLATFORM, &bytes).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn entry(manifest: Manifest, bytes: &[u8]) -> Entry {
    Entry { manifest, artifacts: BTreeMap::from([(PLATFORM.into(), Artifact {
        url: "https://github.com/pelicanux/PeliGames/releases/download/addons-example/example.zip".into(),
        bytes: bytes.len() as u64, sha256: package::digest(bytes),
    })]) }
}
fn rewrite(
    bytes: &[u8],
    mut change: impl FnMut(String, Vec<u8>) -> (String, Vec<u8>),
    extra: Option<(&str, &[u8])>,
) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::FileOptions::default();
    for i in 0..input.len() {
        let mut file = input.by_index(i).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        let (name, data) = change(file.name().into(), data);
        output.start_file(name, options).unwrap();
        output.write_all(&data).unwrap();
    }
    if let Some((name, data)) = extra {
        output.start_file(name, options).unwrap();
        output.write_all(data).unwrap();
    }
    output.finish().unwrap().into_inner()
}

#[test]
fn catalog_and_template_validate_without_publishing_examples() {
    let catalog: Catalog = serde_json::from_str(include_str!("../../config/catalog.json")).unwrap();
    catalog.validate().unwrap();
    assert!(catalog.addons.is_empty());
    manifest("example-service", "0.1.0").validate().unwrap();
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../config/manifest.schema.json")).unwrap();
    let value = serde_json::to_value(manifest("example-service", "0.1.0")).unwrap();
    let fields = value
        .as_object()
        .unwrap()
        .keys()
        .collect::<std::collections::BTreeSet<_>>();
    let properties = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect::<std::collections::BTreeSet<_>>();
    assert!(fields.is_subset(&properties));
    assert_eq!(
        properties
            .difference(&fields)
            .map(|v| v.as_str())
            .collect::<Vec<_>>(),
        vec!["category", "component_directory", "group"]
    );
    assert_eq!(
        fields
            .into_iter()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect()
    );
}
fn ui_manifest() -> Manifest {
    let mut m = manifest("ui-example", "1.0.0");
    m.entrypoints.ui = Some("ui/index.js".into());
    m.contributions = vec![crate::manifest::Contribution {
        target: crate::manifest::Target::Settings,
        route: "preferences".into(),
        title: "Example preferences".into(),
        descriptions: Default::default(),
    }];
    m
}

#[test]
fn localized_card_descriptions_preserve_old_recipes_and_reject_invalid_metadata() {
    let mut recipe = ui_manifest();
    let old = serde_json::to_value(&recipe).unwrap();
    assert!(old["contributions"][0].get("descriptions").is_none());
    assert_eq!(serde_json::from_value::<Manifest>(old).unwrap(), recipe);
    recipe.contributions[0].descriptions = BTreeMap::from([
        ("en".into(), "Frame generation".into()),
        ("pt-BR".into(), "Geração de quadros".into()),
    ]);
    recipe.validate().unwrap();
    let text = serde_json::to_string(&recipe).unwrap();
    assert_eq!(serde_json::from_str::<Manifest>(&text).unwrap(), recipe);
    for (language, text) in [
        ("pt_BR", "Invalid tag".into()),
        ("en", " ".into()),
        ("en", "x".repeat(321)),
    ] {
        recipe.contributions[0].descriptions = BTreeMap::from([(language.into(), text)]);
        assert!(recipe.validate().is_err());
    }
}

#[test]
fn component_storage_is_relative_and_optional_game_categories_are_portable() {
    use crate::manifest::{Category, Contribution, Target};
    let mut m = ui_manifest();
    m.category = Some(Category::Mods);
    m.group = Some("dlssnr".into());
    m.component_directory = Some("components/OptiScaler".into());
    for target in [
        Target::GameMods,
        Target::GameFrameGeneration,
        Target::GameCaptures,
        Target::GameOverview,
    ] {
        m.contributions = vec![Contribution {
            target,
            route: "view".into(),
            title: "View".into(),
            descriptions: Default::default(),
        }];
        m.validate().unwrap();
        let encoded = serde_json::to_vec(&m).unwrap();
        assert_eq!(serde_json::from_slice::<Manifest>(&encoded).unwrap(), m);
    }
    for invalid in [
        "components/../settings",
        "/components/OptiScaler",
        "settings/OptiScaler",
        "components\\OptiScaler",
        "components/",
    ] {
        m.component_directory = Some(invalid.into());
        assert!(m.validate().is_err(), "{invalid}");
    }
    m.component_directory = None;
    m.group = Some("Invalid group".into());
    assert!(m.validate().is_err());
}
#[test]
fn ui_loader_verifies_declared_route_package_and_dependency_identity() {
    let f = Fixture::new();
    let store = f.store();
    let receipt = f.prepare(ui_manifest());
    store.register(&[receipt], HOST, PLATFORM).unwrap();
    let bundle = crate::ui::load(
        &store,
        "ui-example",
        crate::manifest::Target::Settings,
        "preferences",
        HOST,
        PLATFORM,
    )
    .unwrap();
    assert!(bundle.source.contains("createAddon"));
    assert_eq!(bundle.selection["ui-example"].version, "1.0.0");
    assert_eq!(
        package::digest(bundle.source.as_bytes()),
        bundle.source_sha256
    );
    assert!(crate::ui::load(
        &store,
        "ui-example",
        crate::manifest::Target::GameMods,
        "preferences",
        HOST,
        PLATFORM
    )
    .is_err());
    assert!(crate::ui::load(
        &store,
        "ui-example",
        crate::manifest::Target::Settings,
        "../ui/index.js",
        HOST,
        PLATFORM
    )
    .is_err());
    fs::write(
        store
            .packages
            .join("installed/ui-example/1.0.0/ui/index.js"),
        "tampered",
    )
    .unwrap();
    assert!(crate::ui::load(
        &store,
        "ui-example",
        crate::manifest::Target::Settings,
        "preferences",
        HOST,
        PLATFORM
    )
    .is_err());
}
#[test]
fn ui_loader_refuses_disabled_oversized_or_non_utf8_source() {
    for bytes in [vec![255], vec![b'x'; crate::ui::MAX_UI_BYTES as usize + 1]] {
        let f = Fixture::new();
        let store = f.store();
        let m = ui_manifest();
        f.archive(m.clone());
        let payload = f.root.join("payload-ui-example-1.0.0");
        fs::write(payload.join("ui/index.js"), &bytes).unwrap();
        let bytes = package::pack(m.clone(), &payload).unwrap();
        let e = entry(m, &bytes);
        let receipt = package::prepare(&store.packages, &e, PLATFORM, &bytes).unwrap();
        store.register(&[receipt], HOST, PLATFORM).unwrap();
        assert!(crate::ui::load(
            &store,
            "ui-example",
            crate::manifest::Target::Settings,
            "preferences",
            HOST,
            PLATFORM
        )
        .is_err());
    }
    let f = Fixture::new();
    let store = f.store();
    store
        .register(&[f.prepare(ui_manifest())], HOST, PLATFORM)
        .unwrap();
    assert_eq!(
        crate::ui::contributions(&store.read().unwrap(), HOST, PLATFORM).len(),
        1
    );
    store.enable("ui-example", false, HOST, PLATFORM).unwrap();
    assert!(crate::ui::contributions(&store.read().unwrap(), HOST, PLATFORM).is_empty());
    assert!(crate::ui::load(
        &store,
        "ui-example",
        crate::manifest::Target::Settings,
        "preferences",
        HOST,
        PLATFORM
    )
    .is_err());
}
#[test]
fn manifest_rejects_ambiguous_ids_paths_and_incompatible_versions() {
    for id in ["../escape", "OptiScaler", "a--b", "-bad", "a/b", "1bad"] {
        assert!(manifest(id, "1.0.0").validate().is_err());
    }
    for version in ["01.0.0", "1.0", "1.0.0+build", "../../1"] {
        assert!(manifest("valid", version).validate().is_err());
    }
    for path in [
        "../escape",
        "/bin/helper",
        "bin\\helper",
        "C:/helper",
        "bin//helper",
        "bin/../helper",
        "helper.",
    ] {
        let mut m = manifest("valid", "1.0.0");
        m.entrypoints.service = Some(path.into());
        assert!(m.validate().is_err());
    }
    assert!(manifest("valid", "1.0.0")
        .compatible("0.4.0", PLATFORM)
        .is_err());
    assert!(manifest("valid", "1.0.0")
        .compatible(HOST, "macos-aarch64")
        .is_err());
}
#[test]
fn planner_resolves_dependencies_before_download_and_rejects_cycles_missing_artifacts() {
    let f = Fixture::new();
    let (dep, _) = f.archive(manifest("dependency", "1.2.0"));
    let mut dependent = manifest("dependent", "1.0.0");
    dependent
        .dependencies
        .insert("dependency".into(), "^1.0".into());
    let (dependent, _) = f.archive(dependent);
    let mut catalog = Catalog {
        schema_version: 1,
        addons: vec![dependent, dep],
    };
    assert_eq!(
        catalog
            .plan(&["dependent".into()], HOST, PLATFORM)
            .unwrap()
            .iter()
            .map(|e| e.manifest.id.as_str())
            .collect::<Vec<_>>(),
        ["dependency", "dependent"]
    );
    catalog.addons[1].artifacts.clear();
    assert!(catalog
        .plan(&["dependent".into()], HOST, PLATFORM)
        .unwrap_err()
        .contains("No published"));
    catalog.addons[1]
        .manifest
        .dependencies
        .insert("dependent".into(), "^1".into());
    assert!(catalog.validate().unwrap_err().contains("Cyclic"));
}
#[test]
fn catalog_refuses_foreign_download_origin_or_duplicate_ids() {
    let f = Fixture::new();
    let (mut e, _) = f.archive(manifest("official", "1.0.0"));
    e.artifacts.get_mut(PLATFORM).unwrap().url =
        "https://github.com/untrusted/repo/releases/download/v1/file.zip".into();
    assert!(Catalog {
        schema_version: 1,
        addons: vec![e.clone()]
    }
    .validate()
    .is_err());
    e.artifacts.clear();
    assert!(Catalog {
        schema_version: 1,
        addons: vec![e.clone(), e]
    }
    .validate()
    .is_err());
}
#[test]
fn archive_checksum_failure_has_no_installation_side_effects() {
    let f = Fixture::new();
    let (e, mut bytes) = f.archive(manifest("example", "1.0.0"));
    bytes[0] ^= 1;
    assert!(package::prepare(&f.store().packages, &e, PLATFORM, &bytes).is_err());
    assert!(!f.store().packages.exists());
    assert!(!f.store().state_file.exists());
}
#[test]
fn prepared_packages_are_reused_only_when_they_match_the_reviewed_artifact() {
    let f = Fixture::new();
    let store = f.store();
    let (mut e, bytes) = f.archive(manifest("example", "1.0.0"));
    assert!(package::cached(&store.packages, &e, PLATFORM)
        .unwrap()
        .is_none());
    package::prepare(&store.packages, &e, PLATFORM, &bytes).unwrap();
    assert!(package::cached(&store.packages, &e, PLATFORM)
        .unwrap()
        .is_some());
    assert!(store.read().unwrap().addons.is_empty());
    e.artifacts.get_mut(PLATFORM).unwrap().sha256 = "0".repeat(64);
    assert!(package::cached(&store.packages, &e, PLATFORM).is_err());
}
#[test]
fn trusted_but_malformed_archives_cannot_escape_or_smuggle_extra_files() {
    let f = Fixture::new();
    let (e, bytes) = f.archive(manifest("example", "1.0.0"));
    for malicious in [
        "../outside",
        "bin\\escape",
        "/outside",
        "undeclared",
        "LICENSE",
    ] {
        let modified = rewrite(&bytes, |n, b| (n, b), Some((malicious, b"bad")));
        let e = entry(e.manifest.clone(), &modified);
        assert!(package::prepare(&f.store().packages, &e, PLATFORM, &modified).is_err());
    }
    assert!(!f.root.join("outside").exists());
    assert!(!f.store().packages.join("installed/example/1.0.0").exists());
}
#[test]
fn file_checksum_and_catalog_identity_are_verified_after_unpacking() {
    let f = Fixture::new();
    let (e, bytes) = f.archive(manifest("example", "1.0.0"));
    let modified = rewrite(
        &bytes,
        |n, mut b| {
            if n == "bin/example-service" {
                b[0] ^= 1;
            }
            (n, b)
        },
        None,
    );
    let e = entry(e.manifest, &modified);
    assert!(
        package::prepare(&f.store().packages, &e, PLATFORM, &modified)
            .unwrap_err()
            .contains("checksum")
    );
    let mut e = entry(e.manifest, &bytes);
    e.manifest.author = "A different author".into();
    assert!(package::prepare(&f.store().packages, &e, PLATFORM, &bytes)
        .unwrap_err()
        .contains("catalog"));
}
#[test]
fn real_package_round_trip_keeps_versions_and_rollback_preserves_profile_data() {
    let f = Fixture::new();
    let store = f.store();
    let first = f.prepare(manifest("example", "0.6.0"));
    assert!(store.read().unwrap().addons.is_empty()); // prepared does not mean activated
    store.register(&[first], HOST, PLATFORM).unwrap();
    fs::create_dir_all(f.root.join("profiles/game-modules")).unwrap();
    fs::write(
        f.root.join("profiles/game-modules/my-game.json"),
        "keep profile",
    )
    .unwrap();
    let newer = f.prepare(manifest("example", "0.8.0"));
    store.register(&[newer], HOST, PLATFORM).unwrap();
    assert_eq!(
        store.read().unwrap().addons["example"].selected.as_deref(),
        Some("0.8.0")
    );
    store.select("example", "0.6.0", HOST, PLATFORM).unwrap();
    store.remove("example", "0.8.0").unwrap();
    assert!(
        registry::entrypoint(&store, "example", true, HOST, PLATFORM)
            .unwrap()
            .is_file()
    );
    assert!(store.remove("example", "0.6.0").is_err());
    store.enable("example", false, HOST, PLATFORM).unwrap();
    assert!(registry::entrypoint(&store, "example", true, HOST, PLATFORM).is_err());
    store.remove("example", "0.6.0").unwrap();
    assert!(store.read().unwrap().addons.is_empty());
    assert_eq!(
        fs::read_to_string(f.root.join("profiles/game-modules/my-game.json")).unwrap(),
        "keep profile"
    );
}
#[test]
fn failed_upgrade_keeps_previous_selection_and_detects_payload_tampering() {
    let f = Fixture::new();
    let store = f.store();
    store
        .register(&[f.prepare(manifest("example", "0.6.0"))], HOST, PLATFORM)
        .unwrap();
    let mut newer = manifest("example", "0.8.0");
    newer.dependencies.insert("missing".into(), "^1".into());
    assert!(store.register(&[f.prepare(newer)], HOST, PLATFORM).is_err());
    assert_eq!(
        store.read().unwrap().addons["example"].selected.as_deref(),
        Some("0.6.0")
    );
    let executable = registry::entrypoint(&store, "example", true, HOST, PLATFORM).unwrap();
    fs::write(executable, "changed payload").unwrap();
    assert!(registry::entrypoint(&store, "example", true, HOST, PLATFORM).is_err());
    // A corrupt optional package remains removable.
    store.enable("example", false, HOST, PLATFORM).unwrap();
    store.remove("example", "0.6.0").unwrap();
}
#[test]
fn dependent_addon_and_profile_pins_protect_required_versions() {
    let f = Fixture::new();
    let store = f.store();
    let dep = f.prepare(manifest("dependency", "1.0.0"));
    let mut addon = manifest("dependent", "1.0.0");
    addon
        .dependencies
        .insert("dependency".into(), "^1.0".into());
    store
        .register(&[dep, f.prepare(addon)], HOST, PLATFORM)
        .unwrap();
    assert!(store.enable("dependency", false, HOST, PLATFORM).is_err());
    store
        .pin("dependency", "1.0.0", "game/my-game", true)
        .unwrap();
    store.enable("dependent", false, HOST, PLATFORM).unwrap();
    store.enable("dependency", false, HOST, PLATFORM).unwrap();
    assert!(store.remove("dependency", "1.0.0").is_err());
    store
        .pin("dependency", "1.0.0", "game/my-game", false)
        .unwrap();
    store.remove("dependency", "1.0.0").unwrap();
}
#[test]
fn loading_a_healthy_addon_does_not_require_an_unrelated_incompatible_addon() {
    let f = Fixture::new();
    let store = f.store();
    let mut outdated = manifest("outdated", "1.0.0");
    outdated.host_version = ">=0.5.5, <0.5.6".into();
    store
        .register(
            &[f.prepare(manifest("healthy", "1.0.0")), f.prepare(outdated)],
            HOST,
            PLATFORM,
        )
        .unwrap();
    assert!(
        registry::entrypoint(&store, "healthy", true, "0.5.6", PLATFORM)
            .unwrap()
            .is_file()
    );
    assert!(registry::entrypoint(&store, "outdated", true, "0.5.6", PLATFORM).is_err());
}
#[test]
fn interrupted_removal_is_restored_and_externally_deleted_packages_can_be_forgotten() {
    let f = Fixture::new();
    let store = f.store();
    store
        .register(&[f.prepare(manifest("example", "1.0.0"))], HOST, PLATFORM)
        .unwrap();
    store.enable("example", false, HOST, PLATFORM).unwrap();
    let version = store.packages.join("installed/example/1.0.0");
    fs::rename(&version, version.with_file_name("remove-1.0.0")).unwrap();
    store.recover().unwrap();
    assert!(version.is_dir());
    fs::remove_dir_all(store.packages.join("installed/example")).unwrap();
    store.remove("example", "1.0.0").unwrap();
    assert!(store.read().unwrap().addons.is_empty());
}
#[test]
fn committed_removal_cleans_quarantined_bytes_after_interruption() {
    let f = Fixture::new();
    let store = f.store();
    store
        .register(&[f.prepare(manifest("example", "1.0.0"))], HOST, PLATFORM)
        .unwrap();
    store.enable("example", false, HOST, PLATFORM).unwrap();
    let parent = store.packages.join("installed/example");
    fs::rename(parent.join("1.0.0"), parent.join("remove-1.0.0")).unwrap();
    let mut state = store.read().unwrap();
    state.addons.remove("example");
    fs::write(&store.state_file, serde_json::to_vec(&state).unwrap()).unwrap();
    store.recover().unwrap();
    assert!(!parent.join("remove-1.0.0").exists());
    assert!(store.read().unwrap().addons.is_empty());
}
#[test]
fn installation_does_not_silently_replace_an_existing_version() {
    let f = Fixture::new();
    let store = f.store();
    let receipt = f.prepare(manifest("example", "1.0.0"));
    let mut m = receipt.contents.manifest.clone();
    m.author = "Changed publisher".into();
    let payload = f.root.join("replacement");
    fs::create_dir_all(payload.join("bin")).unwrap();
    fs::write(payload.join("bin/example-service"), "different payload").unwrap();
    fs::write(payload.join("LICENSE"), "GPL fixture").unwrap();
    Fixture::write_ui(&payload, &m);
    let bytes = package::pack(m.clone(), &payload).unwrap();
    let e = entry(m, &bytes);
    assert!(package::prepare(&store.packages, &e, PLATFORM, &bytes)
        .unwrap_err()
        .contains("cannot be replaced"));
    assert_eq!(
        package::verify(&store.packages, "example", "1.0.0")
            .unwrap()
            .archive_sha256,
        receipt.archive_sha256
    );
}
#[cfg(unix)]
#[test]
fn symlinks_in_package_storage_never_redirect_installs_or_removal() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, f.store().packages).unwrap();
    let (e, bytes) = f.archive(manifest("example", "1.0.0"));
    assert!(package::prepare(&f.store().packages, &e, PLATFORM, &bytes).is_err());
    assert!(fs::read_dir(outside).unwrap().next().is_none());
}
#[test]
fn protocol_checks_identity_limits_ids_and_null_results() {
    assert_eq!(
        Response::parse(
            &serde_json::to_string(&Response::success(3, json!(null))).unwrap(),
            3
        )
        .unwrap(),
        json!(null)
    );
    assert!(Response::parse(r#"{"jsonrpc":"2.0","id":3,"result":null,"error":null}"#, 3).is_err());
    assert!(Response::parse(r#"{"jsonrpc":"2.0","id":2,"result":42}"#, 3).is_err());
    assert!(Response::parse(r#"{"jsonrpc":"2.0","id":3}"#, 3).is_err());
    assert!(Response::parse(r#"{"jsonrpc":"2.0","id":3,"error":null}"#, 3).is_err());
    assert!(Request::parse(&"x".repeat(crate::protocol::MAX_MESSAGE_BYTES + 1)).is_err());
    assert!(Request::parse(r#"{"jsonrpc":"1.0","id":1,"method":"echo","params":null}"#).is_err());
    assert!(Initialize {
        host_api: 2,
        addon_id: "example".into(),
        addon_version: "1.0.0".into()
    }
    .validate("example", "1.0.0")
    .is_err());
}

#[test]
fn local_package_import_uses_the_registry_without_a_catalog_and_can_be_removed() {
    let f = Fixture::new();
    let (_, bytes) = f.archive(manifest("local-experiment", "0.1.0"));
    let inspected = package::inspect_local(&bytes).unwrap();
    assert_eq!(inspected.manifest.id, "local-experiment");
    assert!(!f.store().packages.exists());
    let receipt = package::prepare_local(
        &f.store().packages,
        &bytes,
        &package::digest(&bytes),
        HOST,
        PLATFORM,
    )
    .unwrap();
    assert!(receipt.local);
    f.store().register(&[receipt], HOST, PLATFORM).unwrap();
    let state = f.store().read().unwrap();
    assert!(state.addons["local-experiment"].enabled);
    assert!(state.addons["local-experiment"].versions["0.1.0"].local);
    assert!(
        package::verify(&f.store().packages, "local-experiment", "0.1.0")
            .unwrap()
            .local
    );
}
#[test]
fn local_package_rejects_changed_files_or_incompatible_hosts_before_selection() {
    let f = Fixture::new();
    let (_, bytes) = f.archive(manifest("experiment", "0.1.0"));
    let hash = package::digest(&bytes);
    let changed = rewrite(
        &bytes,
        |name, data| {
            if name == "LICENSE" {
                (name, b"changed".to_vec())
            } else {
                (name, data)
            }
        },
        None,
    );
    assert!(package::prepare_local(&f.store().packages, &changed, &hash, HOST, PLATFORM).is_err());
    assert!(package::prepare_local(
        &f.store().packages,
        &changed,
        &package::digest(&changed),
        HOST,
        PLATFORM
    )
    .is_err());
    assert!(package::prepare_local(&f.store().packages, &bytes, &hash, "9.0.0", PLATFORM).is_err());
    assert!(
        package::prepare_local(&f.store().packages, &bytes, &hash, HOST, "windows-x86_64").is_err()
    );
    assert!(f.store().read().unwrap().addons.is_empty());
    let receipt =
        package::prepare_local(&f.store().packages, &bytes, &hash, HOST, PLATFORM).unwrap();
    f.store().register(&[receipt], HOST, PLATFORM).unwrap();
    assert!(f.store().read().unwrap().addons["experiment"].enabled);
}

#[test]
fn local_package_removal_preserves_other_versions() {
    let f = Fixture::new();
    for version in ["0.1.0", "0.2.0"] {
        let (_, bytes) = f.archive(manifest("experiment", version));
        let receipt = package::prepare_local(
            &f.store().packages,
            &bytes,
            &package::digest(&bytes),
            HOST,
            PLATFORM,
        )
        .unwrap();
        f.store().register(&[receipt], HOST, PLATFORM).unwrap();
    }
    f.store().remove("experiment", "0.1.0").unwrap();
    assert!(package::verify(&f.store().packages, "experiment", "0.2.0").is_ok());
    assert!(f.store().remove("experiment", "0.2.0").is_err());
}
