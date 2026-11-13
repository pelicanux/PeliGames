use serde::{Deserialize, Serialize};

/// afmf-linux options. Read once at game start; there is no live control.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    /// auto, quality or performance.
    pub performance_mode: String,
    /// auto, standard or high.
    pub search_mode: String,
    /// blend or repeat.
    pub fast_motion: String,
    /// Real frame rate below which no frame is generated; 0 always generates.
    pub min_fps: u32,
    pub hud_detect: bool,
    pub pacing: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            performance_mode: "auto".into(),
            search_mode: "auto".into(),
            fast_motion: "blend".into(),
            min_fps: 30,
            hud_detect: true,
            pacing: true,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !["auto", "quality", "performance"].contains(&self.performance_mode.as_str())
            || !["auto", "standard", "high"].contains(&self.search_mode.as_str())
            || !["blend", "repeat"].contains(&self.fast_motion.as_str())
        {
            return Err("Opção do AFMF inválida.".into());
        }
        if self.min_fps > 240 {
            return Err("Informe o FPS mínimo do AFMF entre 0 e 240.".into());
        }
        Ok(())
    }
    /// The variables afmf-linux reads, without the layer path.
    pub fn environment(&self, gamescope: bool) -> Vec<(&'static str, String)> {
        let flag = |on: bool| if on { "1" } else { "0" }.to_string();
        let mut env = vec![
            ("AFMF_ENABLE", "1".to_string()),
            ("AFMF_PERFORMANCE_MODE", self.performance_mode.clone()),
            ("AFMF_SEARCH_MODE", self.search_mode.clone()),
            ("AFMF_FAST_MOTION_RESPONSE", self.fast_motion.clone()),
            ("AFMF_MIN_FPS", self.min_fps.to_string()),
            ("AFMF_HUD_DETECT", flag(self.hud_detect)),
            ("AFMF_PACING", flag(self.pacing)),
            // Never stack another frame generator layer.
            ("DISABLE_LSFGVK", "1".to_string()),
        ];
        if gamescope {
            env.push(("AFMF_GAMESCOPE", "1".to_string()));
        }
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn afmf_variables_follow_the_game_settings() {
        let afmf = Settings {
            enabled: true,
            performance_mode: "quality".into(),
            min_fps: 45,
            hud_detect: false,
            ..Default::default()
        };
        let env: std::collections::BTreeMap<_, _> = afmf.environment(true).into_iter().collect();
        assert_eq!(env["AFMF_ENABLE"], "1");
        assert_eq!(env["AFMF_PERFORMANCE_MODE"], "quality");
        assert_eq!(env["AFMF_MIN_FPS"], "45");
        assert_eq!(env["AFMF_HUD_DETECT"], "0");
        assert_eq!(env["AFMF_PACING"], "1");
        assert_eq!(env["AFMF_GAMESCOPE"], "1");
        assert_eq!(env["DISABLE_LSFGVK"], "1");
        assert!(!afmf
            .environment(false)
            .iter()
            .any(|(k, _)| *k == "AFMF_GAMESCOPE"));
    }
}
