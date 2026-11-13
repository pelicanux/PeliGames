use crate::{
    manifest::{Manifest, FORMAT_VERSION},
    package::{self, Receipt},
    Result,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub schema_version: u32,
    pub selection_completed: bool,
    pub addons: BTreeMap<String, Installed>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema_version: FORMAT_VERSION,
            selection_completed: false,
            addons: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Installed {
    pub selected: Option<String>,
    pub enabled: bool,
    pub versions: BTreeMap<String, InstalledVersion>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledVersion {
    #[serde(default)]
    pub local: bool,
    pub manifest: Manifest,
    pub archive_sha256: String,
    pub contents_sha256: String,
    pub consumers: BTreeSet<String>,
}

/// Caller serializes access across operations. Paths are supplied by the host's
/// shared storage map; this library never discovers or reads a personal profile.
#[derive(Clone)]
pub struct Store {
    pub packages: PathBuf,
    pub state_file: PathBuf,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionIdentity {
    pub version: String,
    pub digest: String,
}
pub type SelectionSnapshot = BTreeMap<String, SelectionIdentity>;
pub fn active_snapshot(state: &State) -> SelectionSnapshot {
    state
        .addons
        .iter()
        .filter_map(|(id, addon)| {
            if !addon.enabled {
                return None;
            }
            let version = addon.selected.as_ref()?;
            let recorded = addon.versions.get(version)?;
            Some((
                id.clone(),
                SelectionIdentity {
                    version: version.clone(),
                    digest: recorded.archive_sha256.clone(),
                },
            ))
        })
        .collect()
}
pub fn selection_current(state: &State, expected: &SelectionSnapshot) -> bool {
    let current = active_snapshot(state);
    expected
        .iter()
        .all(|(id, identity)| current.get(id) == Some(identity))
}
pub fn dependency_snapshot(
    state: &State,
    id: &str,
    host: &str,
    platform: &str,
) -> Result<SelectionSnapshot> {
    let selection = resolve_selection(state, id, host, platform)?;
    Ok(active_snapshot(state)
        .into_iter()
        .filter(|(id, _)| selection.contains_key(id))
        .collect())
}
impl Store {
    /// A crash between quarantining a directory and committing the registry
    /// leaves the old registration intact. Restore that exact version only.
    pub fn recover(&self) -> Result<()> {
        let state = self.read()?;
        for (id, installed) in &state.addons {
            for version in installed.versions.keys() {
                let directory = package::version_directory(&self.packages, id, version)?;
                let parent = directory.parent().ok_or("Invalid version directory")?;
                let quarantine = parent.join(format!("remove-{version}"));
                if fs::symlink_metadata(&directory).is_ok() || !quarantine.exists() {
                    continue;
                }
                package::safe_directory(&quarantine, false)?;
                let receipt: Receipt = serde_json::from_slice(&package::bounded_read(
                    &quarantine.join("receipt.json"),
                    1024 * 1024,
                )?)
                .map_err(|e| e.to_string())?;
                let registered = &installed.versions[version];
                if receipt.contents.manifest != registered.manifest
                    || receipt.archive_sha256 != registered.archive_sha256
                    || package::digest(
                        &serde_json::to_vec(&receipt.contents).map_err(|e| e.to_string())?,
                    ) != registered.contents_sha256
                {
                    return Err("Interrupted add-on removal has an invalid receipt".into());
                }
                fs::rename(quarantine, directory).map_err(|e| e.to_string())?;
            }
        }
        // The registry commit may have completed just before a crash, leaving
        // only the quarantined, unregistered bytes to clean up.
        let installed_root = self.packages.join("installed");
        if !installed_root.exists() {
            return Ok(());
        }
        package::safe_directory(&installed_root, false)?;
        for item in fs::read_dir(&installed_root).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let Some(id) = item.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if crate::manifest::identifier(&id).is_err() {
                continue;
            }
            package::safe_directory(&item.path(), false)?;
            for child in fs::read_dir(item.path()).map_err(|e| e.to_string())? {
                let child = child.map_err(|e| e.to_string())?;
                let name = child.file_name();
                let Some(version) = name.to_str().and_then(|name| name.strip_prefix("remove-"))
                else {
                    continue;
                };
                package::version_directory(&self.packages, &id, version)?;
                if state
                    .addons
                    .get(&id)
                    .is_some_and(|a| a.versions.contains_key(version))
                {
                    continue;
                }
                package::safe_directory(&child.path(), false)?;
                let receipt: Receipt = serde_json::from_slice(&package::bounded_read(
                    &child.path().join("receipt.json"),
                    1024 * 1024,
                )?)
                .map_err(|e| e.to_string())?;
                receipt.contents.manifest.validate()?;
                if receipt.contents.manifest.id != id
                    || receipt.contents.manifest.version != version
                {
                    return Err("Unregistered add-on quarantine has an invalid identity".into());
                }
                fs::remove_dir_all(child.path()).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    pub fn read(&self) -> Result<State> {
        match fs::symlink_metadata(&self.state_file) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
            Err(e) => return Err(e.to_string()),
            Ok(_) => (),
        }
        package::safe_directory(
            self.state_file.parent().ok_or("Missing state directory")?,
            false,
        )?;
        let state: State =
            serde_json::from_slice(&package::bounded_read(&self.state_file, 4 * 1024 * 1024)?)
                .map_err(|e| e.to_string())?;
        if state.schema_version != FORMAT_VERSION || state.addons.len() > 128 {
            return Err("Unsupported or oversized add-on state".into());
        }
        // Structural validation only: a damaged optional package must not prevent
        // reading the store and disabling/removing that package.
        for (id, installed) in &state.addons {
            crate::manifest::identifier(id)?;
            if installed.versions.len() > 32
                || installed
                    .selected
                    .as_ref()
                    .is_some_and(|v| !installed.versions.contains_key(v))
                || installed.enabled && installed.selected.is_none()
            {
                return Err("Invalid add-on selection".into());
            }
            for (version, item) in &installed.versions {
                package::version_directory(&self.packages, id, version)?;
                item.manifest.validate()?;
                if item.manifest.id != *id || item.manifest.version != *version {
                    return Err("Invalid registered add-on identity".into());
                }
            }
        }
        Ok(state)
    }
    fn save(&self, state: &State) -> Result<()> {
        let parent = self.state_file.parent().ok_or("Missing state directory")?;
        package::safe_directory(parent, true)?;
        if fs::symlink_metadata(&self.state_file)
            .is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
        {
            return Err("Invalid add-on state file".into());
        }
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let temporary = parent.join(format!(
            "addons-write-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut created = false;
        let result = (|| {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut output = options.open(&temporary).map_err(|e| e.to_string())?;
            created = true;
            output
                .write_all(&serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            output.sync_all().map_err(|e| e.to_string())?;
            fs::rename(&temporary, &self.state_file).map_err(|e| e.to_string())
        })();
        if created && result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
    pub fn verified(&self, state: &State, id: &str, version: &str) -> Result<Receipt> {
        self.check_receipt(
            state,
            id,
            version,
            package::verify(&self.packages, id, version)?,
        )
    }
    pub(crate) fn metadata(&self, state: &State, id: &str, version: &str) -> Result<Receipt> {
        self.check_receipt(
            state,
            id,
            version,
            package::receipt(&self.packages, id, version)?,
        )
    }
    fn check_receipt(
        &self,
        state: &State,
        id: &str,
        version: &str,
        receipt: Receipt,
    ) -> Result<Receipt> {
        let recorded = state
            .addons
            .get(id)
            .and_then(|a| a.versions.get(version))
            .ok_or("Add-on version is not registered")?;
        if receipt.local != recorded.local
            || receipt.archive_sha256 != recorded.archive_sha256
            || receipt.contents.manifest != recorded.manifest
            || package::digest(&serde_json::to_vec(&receipt.contents).map_err(|e| e.to_string())?)
                != recorded.contents_sha256
        {
            return Err("Installed add-on receipt changed".into());
        }
        Ok(receipt)
    }
    fn dependencies(&self, state: &State, host: &str, platform: &str) -> Result<()> {
        for (id, _) in state.addons.iter().filter(|(_, addon)| addon.enabled) {
            resolve_selection(state, id, host, platform)?;
        }
        Ok(())
    }
    /// Commit a complete, already-prepared selection once. A failed dependency
    /// check or write keeps the previous selection; prepared bytes remain cached.
    pub fn register(&self, receipts: &[Receipt], host: &str, platform: &str) -> Result<State> {
        let mut state = self.read()?;
        for receipt in receipts {
            let manifest = &receipt.contents.manifest;
            let actual = package::verify(&self.packages, &manifest.id, &manifest.version)?;
            if actual.local != receipt.local
                || actual.archive_sha256 != receipt.archive_sha256
                || actual.contents != receipt.contents
            {
                return Err("Prepared add-on does not match its receipt".into());
            }
            let addon = state.addons.entry(manifest.id.clone()).or_default();
            if addon.versions.len() >= 32 && !addon.versions.contains_key(&manifest.version) {
                return Err("Remove unused versions before installing another".into());
            }
            let consumers = addon
                .versions
                .get(&manifest.version)
                .map(|v| v.consumers.clone())
                .unwrap_or_default();
            addon.versions.insert(
                manifest.version.clone(),
                InstalledVersion {
                    local: receipt.local,
                    manifest: manifest.clone(),
                    archive_sha256: receipt.archive_sha256.clone(),
                    contents_sha256: package::digest(
                        &serde_json::to_vec(&receipt.contents).map_err(|e| e.to_string())?,
                    ),
                    consumers,
                },
            );
            addon.selected = Some(manifest.version.clone());
            addon.enabled = true;
        }
        if state.addons.len() > 128 {
            return Err("Too many installed add-ons".into());
        }
        self.dependencies(&state, host, platform)?;
        self.save(&state)?;
        Ok(state)
    }
    pub fn select(&self, id: &str, version: &str, host: &str, platform: &str) -> Result<State> {
        let mut state = self.read()?;
        self.verified(&state, id, version)?
            .contents
            .manifest
            .compatible(host, platform)?;
        state
            .addons
            .get_mut(id)
            .ok_or("Unknown installed add-on")?
            .selected = Some(version.into());
        self.dependencies(&state, host, platform)?;
        self.save(&state)?;
        Ok(state)
    }
    pub fn enable(&self, id: &str, enabled: bool, host: &str, platform: &str) -> Result<State> {
        let mut state = self.read()?;
        if enabled {
            let version = state
                .addons
                .get(id)
                .and_then(|a| a.selected.as_deref())
                .ok_or("No add-on version selected")?;
            self.verified(&state, id, version)?;
        }
        state
            .addons
            .get_mut(id)
            .ok_or("Unknown installed add-on")?
            .enabled = enabled;
        self.dependencies(&state, host, platform)?;
        self.save(&state)?;
        Ok(state)
    }
    pub fn complete_selection(&self) -> Result<State> {
        let mut state = self.read()?;
        state.selection_completed = true;
        self.save(&state)?;
        Ok(state)
    }
    /// The host pins versions referenced by profiles/active jobs. Removing the
    /// integration never means deleting a mod already installed inside a game.
    pub fn pin(&self, id: &str, version: &str, consumer: &str, pinned: bool) -> Result<()> {
        if consumer.trim().is_empty() || consumer.len() > 240 {
            return Err("Invalid add-on consumer".into());
        }
        let mut state = self.read()?;
        let item = state
            .addons
            .get_mut(id)
            .and_then(|a| a.versions.get_mut(version))
            .ok_or("Unknown add-on version")?;
        if pinned {
            item.consumers.insert(consumer.into());
        } else {
            item.consumers.remove(consumer);
        }
        self.save(&state)
    }
    pub fn remove(&self, id: &str, version: &str) -> Result<State> {
        let mut state = self.read()?;
        let addon = state.addons.get_mut(id).ok_or("Unknown installed add-on")?;
        let selected = addon.selected.as_deref() == Some(version);
        let record = addon
            .versions
            .get(version)
            .ok_or("Unknown add-on version")?;
        if selected && addon.enabled || !record.consumers.is_empty() {
            return Err("Disable the add-on and release its profile/job references before removing this version".into());
        }
        let directory = package::version_directory(&self.packages, id, version)?;
        let parent = directory.parent().ok_or("Invalid add-on directory")?;
        if parent.exists() {
            package::safe_directory(parent, false)?;
        }
        let trash = parent.join(format!("remove-{version}"));
        if fs::symlink_metadata(&trash).is_ok() {
            return Err("An add-on removal needs recovery".into());
        }
        let exists = match fs::symlink_metadata(&directory) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            _ => return Err("Invalid add-on directory".into()),
        };
        if exists {
            fs::rename(&directory, &trash).map_err(|e| e.to_string())?;
        }
        addon.versions.remove(version);
        if selected {
            addon.selected = None;
        }
        if addon.versions.is_empty() {
            state.addons.remove(id);
        }
        if let Err(error) = self.save(&state) {
            if exists {
                fs::rename(&trash, &directory)
                    .map_err(|e| format!("{error}; rollback failed: {e}"))?;
            }
            return Err(error);
        }
        if exists {
            fs::remove_dir_all(&trash)
                .map_err(|e| format!("Add-on unregistered; cached files need cleanup: {e}"))?;
        }
        Ok(state)
    }
}

/// Resolve only this integration's dependency closure. An unrelated add-on that
/// became incompatible after a launcher update must not block healthy services.
pub fn resolve_selection(
    state: &State,
    id: &str,
    host: &str,
    platform: &str,
) -> Result<BTreeMap<String, Manifest>> {
    fn visit(
        state: &State,
        id: &str,
        host: &str,
        platform: &str,
        visiting: &mut BTreeSet<String>,
        resolved: &mut BTreeMap<String, Manifest>,
    ) -> Result<Manifest> {
        if let Some(manifest) = resolved.get(id) {
            return Ok(manifest.clone());
        }
        if !visiting.insert(id.into()) {
            return Err("Cyclic installed add-on dependency".into());
        }
        let addon = state
            .addons
            .get(id)
            .filter(|a| a.enabled)
            .ok_or_else(|| format!("Required add-on is disabled or missing: {id}"))?;
        let version = addon
            .selected
            .as_deref()
            .ok_or("No selected add-on version")?;
        let manifest = &addon
            .versions
            .get(version)
            .ok_or("Unknown add-on version")?
            .manifest;
        manifest.compatible(host, platform)?;
        if manifest.id != id || manifest.version != version {
            return Err("Invalid registered add-on identity".into());
        }
        for (dependency, requirement) in &manifest.dependencies {
            let candidate = visit(state, dependency, host, platform, visiting, resolved)?;
            if !VersionReq::parse(requirement)
                .map_err(|e| e.to_string())?
                .matches(&Version::parse(&candidate.version).map_err(|e| e.to_string())?)
            {
                return Err(format!("{id} needs a different version of {dependency}"));
            }
        }
        visiting.remove(id);
        resolved.insert(id.into(), manifest.clone());
        Ok(manifest.clone())
    }
    let mut resolved = BTreeMap::new();
    visit(
        state,
        id,
        host,
        platform,
        &mut BTreeSet::new(),
        &mut resolved,
    )?;
    Ok(resolved)
}

/// Resolve a service/UI only after both state and payload integrity checks.
pub fn entrypoint(
    store: &Store,
    id: &str,
    service: bool,
    host: &str,
    platform: &str,
) -> Result<PathBuf> {
    let state = store.read()?;
    entrypoint_at(store, &state, id, service, host, platform)
}

/// Keep verification and path resolution tied to one selection snapshot. The
/// host coordinates mutation/stop so its immutable version stays present.
pub(crate) fn entrypoint_at(
    store: &Store,
    state: &State,
    id: &str,
    service: bool,
    host: &str,
    platform: &str,
) -> Result<PathBuf> {
    let addon = state
        .addons
        .get(id)
        .filter(|a| a.enabled)
        .ok_or("Add-on is disabled or not installed")?;
    let version = addon
        .selected
        .as_deref()
        .ok_or("No add-on version selected")?;
    let selection = resolve_selection(state, id, host, platform)?;
    for (dependency, manifest) in &selection {
        store.verified(state, dependency, &manifest.version)?;
    }
    let manifest = selection.get(id).ok_or("Missing selected add-on")?;
    let entry = if service {
        manifest.entrypoints.service.clone()
    } else {
        manifest.entrypoints.ui.clone()
    }
    .ok_or("Missing add-on entrypoint")?;
    Ok(package::version_directory(&store.packages, id, version)?.join(entry))
}
