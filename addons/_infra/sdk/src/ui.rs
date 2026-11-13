//! Verified, self-contained UI modules. No package path is accepted from the UI.
use crate::{
    manifest::{Contribution, Target},
    package,
    registry::{self, SelectionSnapshot, State, Store},
    Result,
};
use serde::Serialize;

pub const MAX_UI_BYTES: u64 = 2 * 1024 * 1024;
#[derive(Clone, Serialize)]
pub struct UiContribution {
    pub id: String,
    pub version: String,
    pub name: String,
    pub repository: String,
    pub category: Option<crate::manifest::Category>,
    pub group: Option<String>,
    pub contribution: Contribution,
}
#[derive(Serialize)]
pub struct UiBundle {
    pub id: String,
    pub version: String,
    pub host_api: u32,
    pub route: String,
    pub target: Target,
    pub selection: SelectionSnapshot,
    pub source: String,
    pub source_sha256: String,
}
pub fn contributions(state: &State, host: &str, platform: &str) -> Vec<UiContribution> {
    let mut result = Vec::new();
    for id in registry::active_snapshot(state).keys() {
        let Ok(selected) = registry::resolve_selection(state, id, host, platform) else {
            continue;
        };
        let Some(manifest) = selected.get(id) else {
            continue;
        };
        if manifest.entrypoints.ui.is_none() {
            continue;
        }
        result.extend(
            manifest
                .contributions
                .iter()
                .cloned()
                .map(|contribution| UiContribution {
                    id: id.clone(),
                    version: manifest.version.clone(),
                    name: manifest.name.clone(),
                    repository: manifest.repository.clone(),
                    category: manifest.category.clone(),
                    group: manifest.group.clone(),
                    contribution,
                }),
        );
    }
    result
}
pub fn load(
    store: &Store,
    id: &str,
    target: Target,
    route: &str,
    host: &str,
    platform: &str,
) -> Result<UiBundle> {
    let state = store.read()?;
    let selected = registry::resolve_selection(&state, id, host, platform)?;
    let manifest = selected.get(id).ok_or("Missing UI add-on")?;
    if !manifest
        .contributions
        .iter()
        .any(|c| c.target == target && c.route == route)
    {
        return Err("UI route is not declared by this add-on".into());
    }
    let entrypoint = registry::entrypoint_at(store, &state, id, false, host, platform)?;
    let receipt = store.metadata(&state, id, &manifest.version)?;
    let file = manifest
        .entrypoints
        .ui
        .as_ref()
        .ok_or("Missing UI entrypoint")?;
    let expected = receipt
        .contents
        .files
        .get(file)
        .ok_or("Undeclared UI file")?;
    let bytes = package::bounded_read(&entrypoint, MAX_UI_BYTES)?;
    let source_sha256 = package::digest(&bytes);
    if bytes.len() as u64 != expected.bytes || source_sha256 != expected.sha256 {
        return Err("UI source changed while loading".into());
    }
    let source = String::from_utf8(bytes).map_err(|_| "UI entrypoint is not UTF-8 JavaScript")?;
    if source.trim().is_empty() {
        return Err("Empty add-on UI module".into());
    }
    Ok(UiBundle {
        id: id.into(),
        version: manifest.version.clone(),
        host_api: manifest.host_api,
        route: route.into(),
        target,
        selection: registry::dependency_snapshot(&state, id, host, platform)?,
        source,
        source_sha256,
    })
}
