use super::{Error, ErrorKind, Result};
use crate::{
    manifest::Manifest,
    registry::{self, State, Store},
};
use std::path::PathBuf;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Selection {
    pub manifest: Manifest,
    pub versions: registry::SelectionSnapshot,
}
impl Selection {
    pub fn read(state: &State, id: &str, host: &str, platform: &str) -> Result<Self> {
        if !state.addons.get(id).is_some_and(|a| a.enabled) {
            return Err(Error::new(
                ErrorKind::Unavailable,
                "Add-on is disabled or not installed",
            ));
        }
        let selection =
            registry::resolve_selection(state, id, host, platform).map_err(Error::verification)?;
        let manifest = selection
            .get(id)
            .ok_or_else(|| Error::verification("Missing selected manifest"))?
            .clone();
        if manifest.entrypoints.service.is_none() {
            return Err(Error::new(ErrorKind::Unavailable, "Add-on has no service"));
        }
        let versions = registry::dependency_snapshot(state, id, host, platform)
            .map_err(Error::verification)?;
        Ok(Self { manifest, versions })
    }
    pub fn current(&self, state: &State) -> bool {
        registry::selection_current(state, &self.versions)
    }
    pub async fn verify(&self, store: Store, host: String, platform: String) -> Result<Verified> {
        let expected = self.clone();
        tokio::task::spawn_blocking(move || {
            let state = store.read().map_err(Error::verification)?;
            if !expected.current(&state) {
                return Err(Error::new(ErrorKind::Stopped, "Add-on selection changed"));
            }
            let executable = registry::entrypoint_at(
                &store,
                &state,
                &expected.manifest.id,
                true,
                &host,
                &platform,
            )
            .map_err(Error::verification)?;
            let directory = store
                .packages
                .join("installed")
                .join(&expected.manifest.id)
                .join(&expected.manifest.version);
            Ok(Verified {
                executable,
                directory,
            })
        })
        .await
        .map_err(|_| Error::new(ErrorKind::Verification, "Add-on verification task failed"))?
    }
}
pub(super) struct Verified {
    pub executable: PathBuf,
    pub directory: PathBuf,
}
