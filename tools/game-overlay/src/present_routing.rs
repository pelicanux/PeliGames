//! Presentation observers surround the generator without moving input/rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Generator {
    Afmf,
    Mako,
}
impl Generator {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Afmf => "AFMF",
            Self::Mako => "MAKO",
        }
    }
    pub(crate) fn counter_is_source(self) -> bool {
        self == Self::Mako
    }
}
pub(crate) fn detect(afmf: bool, layers: &str) -> Option<Generator> {
    if afmf {
        Some(Generator::Afmf)
    } else if layers.split(':').any(|name| name == "VK_LAYER_MAKO_render") {
        Some(Generator::Mako)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mako_is_detected_after_its_launcher_unsets_enable_mako() {
        let mako = detect(false, "VK_LAYER_MAKO_render:VK_LAYER_PELIGAMES_overlay").unwrap();
        assert_eq!(mako.name(), "MAKO");
        assert!(mako.counter_is_source());
        let afmf = detect(true, "VK_LAYER_PELIGAMES_overlay:VK_LAYER_AFMF").unwrap();
        assert_eq!(afmf.name(), "AFMF");
        assert!(!afmf.counter_is_source());
        assert_eq!(detect(false, "VK_LAYER_PELIGAMES_overlay"), None);
        assert_eq!(detect(false, "VK_LAYER_MAKO_spatial_scaling"), None);
    }
}
