use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub backend: String,
    pub runtime_version: String,
    pub enabled: bool,
    pub overlay_enabled: bool,
    pub flatpak: bool,
    pub dll: String,
    pub multiplier: u8,
    pub adaptive: bool,
    pub target_fps: u16,
    pub flow_scale: f32,
    pub performance: bool,
    pub gpu: String,
    pub base_fps_cap: u16,
    pub refresh_threshold: u16,
    pub ultra_performance: bool,
    pub allow_fp16: bool,
    pub live_enabled: bool,
    pub scaling_enabled: bool,
    pub scaling_method: String,
    pub scaling_factor: f32,
    pub scaling_sharpness: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            backend: "mako".into(),
            runtime_version: String::new(),
            enabled: false,
            overlay_enabled: false,
            flatpak: false,
            dll: String::new(),
            multiplier: 2,
            adaptive: false,
            target_fps: 60,
            flow_scale: 1.0,
            performance: false,
            gpu: String::new(),
            base_fps_cap: 0,
            refresh_threshold: 0,
            ultra_performance: false,
            allow_fp16: true,
            live_enabled: true,
            scaling_enabled: false,
            scaling_method: "native".into(),
            scaling_factor: 1.5,
            scaling_sharpness: 0.8,
        }
    }
}
pub fn check_settings(s: &Settings) -> Result<(), String> {
    if s.backend != "mako" {
        return Err("Invalid generator".into());
    }
    if s.base_fps_cap > 1000
        || s.refresh_threshold > 1000
        || !s.scaling_factor.is_finite()
        || !(1.0..=2.0).contains(&s.scaling_factor)
        || !s.scaling_sharpness.is_finite()
        || !(0.0..=1.0).contains(&s.scaling_sharpness)
        || !["native", "mako", "ls1", "ls1-performance"].contains(&s.scaling_method.as_str())
        || !valid_pci_address(&s.gpu)
    {
        return Err(
            "Ajustes MAKO inválidos: confira GPU, limites (0–1000), escala (1–2×) e nitidez (0–1)."
                .into(),
        );
    }
    if !(2..=5).contains(&s.multiplier)
        || !(10..=1000).contains(&s.target_fps)
        || !s.flow_scale.is_finite()
        || !(0.25..=1.0).contains(&s.flow_scale)
    {
        return Err("Informe multiplicador de 2× a 5×, alvo entre 10 e 1000 FPS e escala de movimento entre 25% e 100%.".into());
    }
    Ok(())
}
pub use peligames_mod_config::pci::valid_pci_address;
