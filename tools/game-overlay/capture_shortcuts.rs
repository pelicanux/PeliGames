//! Shared capture bindings. Empty values deliberately leave actions unassigned.
use serde::{Deserialize, Serialize};
#[path = "shortcut.rs"]
mod key;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub recording: String,
    pub replay: String,
    pub save_replay: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            recording: "Alt_L+F6".into(),
            replay: "Alt_L+F7".into(),
            save_replay: "Alt_L+F8".into(),
        }
    }
}
impl Preferences {
    pub fn bindings(&self) -> impl Iterator<Item = (&str, &str)> {
        [
            ("toggle-recording", self.recording.as_str()),
            ("toggle-replay", self.replay.as_str()),
            ("save-replay", self.save_replay.as_str()),
        ]
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = Vec::new();
        for (_, value) in self.bindings() {
            let parsed = key::parse(value)?;
            if seen.contains(&parsed) {
                return Err("Escolha uma combinação diferente para cada ação de captura.".into());
            }
            if parsed.0 == 37 && parsed.1 & 9 != 0 {
                return Err("Esse atalho já abre a busca da biblioteca.".into());
            }
            seen.push(parsed);
        }
        Ok(())
    }
    pub fn conflict(&self, value: &str) -> Option<&str> {
        let value = key::parse(value).ok()?;
        self.bindings()
            .find(|(_, v)| key::parse(v) == Ok(value))
            .map(|(action, _)| action)
    }
    pub fn parsed(&self) -> Vec<(u32, u8, &'static str)> {
        [
            ("toggle-recording", &self.recording),
            ("toggle-replay", &self.replay),
            ("save-replay", &self.save_replay),
        ]
        .into_iter()
        .filter_map(|(action, value)| {
            key::parse(value)
                .ok()
                .map(|(key, mods)| (key, mods, action))
        })
        .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_bindings_and_saved_custom_choices_are_preserved() {
        let p = Preferences::default();
        p.validate().unwrap();
        assert_eq!(p.parsed(), vec![(64,4,"toggle-recording"),(65,4,"toggle-replay"),(66,4,"save-replay")]);
        let restored: Preferences = serde_json::from_str(r#"{"recording":"Control_L+F9","replay":"","save_replay":"Shift_L+F10"}"#).unwrap();
        assert_eq!(restored.recording,"Control_L+F9");
        assert!(restored.replay.is_empty());
        assert_eq!(restored.save_replay,"Shift_L+F10");
        let missing: Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(missing.parsed(),p.parsed());
    }
    #[test]
    fn disabled_bindings_and_conflicts_use_physical_keys() {
        let mut p = Preferences { recording: String::new(), replay: String::new(), save_replay: String::new() };
        assert!(p.bindings().next().is_none());
        p.validate().unwrap();
        p.recording = "Control_L+F9".into();
        p.replay = "F9+Control_R".into();
        assert!(p.validate().is_err());
        p.replay = "Shift_L+F9".into();
        p.validate().unwrap();
        assert_eq!(p.conflict("F9+Control_R"), Some("toggle-recording"));
        p.save_replay = "Escape".into();
        assert!(p.validate().is_err());
        p.save_replay = "Control_L+Shift_L+k".into();
        assert!(p.validate().is_err());
    }
}
