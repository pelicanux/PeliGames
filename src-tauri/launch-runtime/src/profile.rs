//! Shared on-disk contract. The application produces it; the standalone helper
//! consumes it without loading the GUI, contacting Steam, or evaluating a shell.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;

pub const VERSION: u32 = 1;
pub const UNIVERSAL_VERSION: u32 = 2;
pub const MARKER: &str = "__PELIGAMES_COMMAND__";
const LEGACY_MARKER: &str = "__PELIGAMES_STEAM_COMMAND__";
pub const MAX_BYTES: u64 = 512 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub version: u32,
    #[serde(default)]
    pub app_id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation: Option<String>,
    /// The complete command supplied by any launcher replaces exactly one marker.
    pub command: Vec<String>,
    /// None explicitly removes an inherited variable, unlike an omitted key.
    pub environment: BTreeMap<String, Option<String>>,
    pub extra_args: Vec<String>,
    /// GUI-only information for rebuilding a profile after a global HUD edit.
    #[serde(default)]
    pub source: serde_json::Value,
}

pub fn app_id(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|id| *id != 0 && id.to_string() == value)
        .ok_or_else(|| "Steam AppID inválido.".into())
}

pub fn profile_id(value: &str) -> Result<&str, String> {
    if value.len() == 11 && value.starts_with("pg-") && value[3..].bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
        Ok(value)
    } else { Err("Identificador PeliRun inválido.".into()) }
}

impl Profile {
    pub fn validate_id(&self, expected: &str) -> Result<(), String> {
        profile_id(expected)?;
        if self.version != UNIVERSAL_VERSION || self.id.as_deref() != Some(expected)
            || self.installation.as_ref().is_none_or(|p| !std::path::Path::new(p).is_absolute()) {
            return Err("Perfil PeliRun incompatível ou de outra instalação.".into());
        }
        self.validate_command()
    }
    pub fn validate(&self, expected: u32) -> Result<(), String> {
        if self.version != VERSION || self.app_id != expected || expected == 0 {
            return Err("Perfil de inicialização incompatível ou de outro jogo.".into());
        }
        self.validate_command()
    }

    fn validate_command(&self) -> Result<(), String> {
        if self.command.is_empty()
            || self.command[0].is_empty()
            || self.command.len() + self.extra_args.len() > 4096
            || self
                .command
                .iter()
                .filter(|word| matches!(word.as_str(), MARKER | LEGACY_MARKER))
                .count()
                != 1
            || self
                .command
                .iter()
                .chain(&self.extra_args)
                .any(|word| word.contains('\0'))
        {
            return Err("Comando de inicialização inválido.".into());
        }
        for (key, value) in &self.environment {
            let valid = !key.is_empty()
                && key.bytes().enumerate().all(|(i, c)| {
                    c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                });
            if !valid || value.as_ref().is_some_and(|value| value.contains('\0')) {
                return Err("Variável de inicialização inválida.".into());
            }
        }
        Ok(())
    }

    pub fn expand(&self, original: &[OsString]) -> Result<Vec<OsString>, String> {
        if self.version == UNIVERSAL_VERSION {
            self.validate_id(self.id.as_deref().ok_or("Perfil sem identificador.")?)?;
        } else { self.validate(self.app_id)?; }
        if original.is_empty() || original[0].is_empty() {
            return Err("O launcher não forneceu o comando do jogo.".into());
        }
        let mut words = Vec::new();
        for word in &self.command {
            if word == MARKER || word == LEGACY_MARKER {
                words.extend_from_slice(original);
            } else {
                words.push(OsString::from(word.as_str()));
            }
        }
        words.extend(
            self.extra_args
                .iter()
                .map(|word| OsString::from(word.as_str())),
        );
        Ok(words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile {
            version: VERSION,
            app_id: 42,
            id: None, installation: None,
            command: vec!["/path with spaces/wrapper".into(), MARKER.into()],
            environment: BTreeMap::from([("MANGOHUD".into(), None)]),
            extra_args: vec!["$(touch /never-executed)".into(), "two words".into()],
            source: serde_json::Value::Null,
        }
    }

    #[test]
    fn command_preserves_steam_runtime_proton_arguments_and_literal_text() {
        let original: Vec<OsString> = [
            "/steam/pressure-vessel",
            "/proton/proton",
            "waitforexitandrun",
            "game.exe",
            "-old",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        let words = profile().expand(&original).unwrap();
        assert_eq!(words[0], "/path with spaces/wrapper");
        assert_eq!(&words[1..6], original.as_slice());
        assert_eq!(words[6], "$(touch /never-executed)");
        assert_eq!(words[7], "two words");
    }

    #[test]
    fn rejects_wrong_game_version_duplicate_marker_and_invalid_environment() {
        let mut value = profile();
        assert!(value.validate(43).is_err());
        value.version += 1;
        assert!(value.validate(42).is_err());
        value.version = VERSION;
        value.command.push(MARKER.into());
        assert!(value.validate(42).is_err());
        value.command.pop();
        value
            .environment
            .insert("X;cmd".into(), Some("value".into()));
        assert!(value.validate(42).is_err());
        for value in ["0", "042", "../42", "42/other", "4294967296"] {
            assert!(app_id(value).is_err());
        }
    }

    #[test]
    fn environment_removal_survives_serialization() {
        let saved = serde_json::to_vec(&profile()).unwrap();
        let loaded: Profile = serde_json::from_slice(&saved).unwrap();
        assert_eq!(loaded.environment.get("MANGOHUD"), Some(&None));
    }
}
