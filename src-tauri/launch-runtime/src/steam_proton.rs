//! Select a Proton per launch without editing Steam's configuration or prefix.
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SteamProton {
    pub app_id: u32,
    pub directory: String,
    pub runtime: Option<String>,
}
impl SteamProton {
    pub fn validate(&self) -> Result<(), String> {
        if self.app_id == 0
            || !Path::new(&self.directory).is_absolute()
            || self.directory.contains(['\0', ':'])
            || self
                .runtime
                .as_ref()
                .is_some_and(|r| !Path::new(r).is_absolute() || r.contains(['\0', ':']))
        {
            return Err("Seleção de Proton do perfil Steam inválida.".into());
        }
        Ok(())
    }
}
