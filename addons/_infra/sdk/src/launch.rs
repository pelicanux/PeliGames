//! Launcher-neutral configuration and verified native launch dispatch.
use crate::{
    manifest::{Capability, Category},
    registry::{self, Store},
    Result,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub group: Option<Category>,
    pub data: Value,
}
pub type Selections = BTreeMap<String, Settings>;

pub fn validate(settings: &Selections) -> Result<()> {
    if settings.len() > 16 {
        return Err("Use up to 16 launch integrations".into());
    }
    for (id, settings) in settings {
        crate::manifest::identifier(id)?;
        if serde_json::to_vec(&settings.data)
            .map_err(|e| e.to_string())?
            .len()
            > 32 * 1024
        {
            return Err("Launch integration configuration is too large".into());
        }
    }
    Ok(())
}

pub struct Dispatch {
    pub executable: PathBuf,
    pub component_directory: Option<String>,
}
/// Missing or disabled packages are bypassed, so removal never strands a game.
pub fn dispatch(store: &Store, id: &str, host: &str, platform: &str) -> Result<Option<Dispatch>> {
    crate::manifest::identifier(id)?;
    let state = store.read()?;
    if !state.addons.get(id).is_some_and(|entry| entry.enabled) {
        return Ok(None);
    }
    let selection = registry::resolve_selection(&state, id, host, platform)?;
    let manifest = selection.get(id).ok_or("Missing launch integration")?;
    if !manifest.capabilities.contains(&Capability::NativeLaunch) {
        return Err("This integration cannot prepare game launches".into());
    }
    let executable = registry::entrypoint_at(store, &state, id, true, host, platform)?;
    Ok(Some(Dispatch {
        executable,
        component_directory: manifest.component_directory.clone(),
    }))
}

/// Recognize the old managed namespace without interpreting component settings.
pub fn legacy_path(root: &std::path::Path, program: &std::path::Path) -> bool {
    let directories = [
        root.join("components/FrameGeneration/profiles"),
        root.join("FrameGeneration/profiles"),
    ];
    program.extension().is_some_and(|ext| ext == "sh")
        && directories
            .iter()
            .any(|dir| program.parent() == Some(dir.as_path()))
}

/// Version-zero managed wrappers stored their integration identifier in a JSON
/// sidecar. Read only this envelope; the selected service owns its settings.
pub fn legacy_profile(
    root: &std::path::Path,
    program: &std::path::Path,
) -> Result<Option<(String, serde_json::Value)>> {
    if !legacy_path(root, program) {
        return Ok(None);
    }
    match std::fs::symlink_metadata(program.with_extension("json")) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        _ => {}
    }
    crate::files::check_directory(program.parent().ok_or("Invalid managed profile")?)?;
    let bytes = crate::package::bounded_read(&program.with_extension("json"), 64 * 1024)?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let id = value["backend"]
        .as_str()
        .ok_or("Missing legacy integration identity")?
        .to_owned();
    crate::manifest::identifier(&id)?;
    Ok(Some((
        id,
        serde_json::json!({"enabled":value["enabled"]==true,"legacy_profile":program,"overlay_enabled":value["overlay_enabled"]==true}),
    )))
}

#[cfg(test)]
mod legacy_tests {
    use super::*;
    #[test]
    fn legacy_envelope_preserves_identity_and_rejects_foreign_or_redirected_paths() {
        let root =
            std::env::temp_dir().join(format!("peligames-legacy-envelope-{}", std::process::id()));
        let profiles = root.join("components/FrameGeneration/profiles");
        std::fs::create_dir_all(&profiles).unwrap();
        let script = profiles.join("fixture-game.sh");
        std::fs::write(script.with_extension("json"),br#"{"backend":"fixture-native","enabled":true,"overlay_enabled":true,"private":"not forwarded"}"#).unwrap();
        let (id, data) = legacy_profile(&root, &script).unwrap().unwrap();
        assert_eq!(id, "fixture-native");
        assert_eq!(data["enabled"], true);
        assert!(data.get("private").is_none());
        assert_eq!(data["legacy_profile"], script.to_str().unwrap());
        assert!(legacy_profile(&root, &root.join("foreign.sh"))
            .unwrap()
            .is_none());
        std::fs::write(
            script.with_extension("json"),
            br#"{"backend":"../../bad","enabled":true}"#,
        )
        .unwrap();
        assert!(legacy_profile(&root, &script).is_err());
        #[cfg(unix)]
        {
            std::fs::remove_file(script.with_extension("json")).unwrap();
            std::fs::write(root.join("redirect.json"), b"{}").unwrap();
            std::os::unix::fs::symlink(root.join("redirect.json"), script.with_extension("json"))
                .unwrap();
            assert!(legacy_profile(&root, &script).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
