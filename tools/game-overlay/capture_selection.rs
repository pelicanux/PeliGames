//! Wire settings shared by the launcher and its in-game capture panel.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Selection {
    pub format: String,
    pub codec: String,
    pub resolution: String,
    pub fps: u16,
    pub rate_control: String,
    pub bitrate_kbps: u32,
    pub quality_preset: String,
    pub desktop_audio: bool,
    pub microphone: bool,
    pub microphone_noise_reduction: bool,
    pub output_source: String,
    pub input_source: String,
}
impl Default for Selection {
    fn default() -> Self {
        Self {
            format: "mkv".into(),
            codec: "auto".into(),
            resolution: "1280x720".into(),
            fps: 60,
            rate_control: "qp".into(),
            bitrate_kbps: 5000,
            quality_preset: "high".into(),
            desktop_audio: true,
            microphone: false,
            microphone_noise_reduction: false,
            output_source: "game:auto".into(),
            input_source: "default_input".into(),
        }
    }
}
impl Selection {
    pub fn validate(&self) -> Result<(), String> {
        if ![24, 30, 60, 90, 120, 144, 165, 240].contains(&self.fps) {
            return Err("FPS de gravação inválido.".into());
        }
        if !matches!(
            self.resolution.as_str(),
            "0x0" | "854x480" | "1280x720" | "1920x1080" | "2560x1440" | "3840x2160"
        ) {
            return Err("Limite de resolução inválido.".into());
        }
        if !matches!(self.rate_control.as_str(), "cbr" | "vbr" | "qp")
            || !(1000..=200000).contains(&self.bitrate_kbps)
            || !matches!(
                self.quality_preset.as_str(),
                "medium" | "high" | "very_high" | "ultra"
            )
        {
            return Err("Qualidade ou bitrate de vídeo inválidos.".into());
        }
        if !matches!(self.format.as_str(), "mp4" | "mkv" | "webm") {
            return Err("Formato de vídeo inválido.".into());
        }
        if !matches!(
            self.codec.as_str(),
            "auto" | "h264" | "hevc" | "av1" | "vp9"
        ) {
            return Err("Codec de vídeo inválido.".into());
        }
        if self.format == "webm" && matches!(self.codec.as_str(), "h264" | "hevc") {
            return Err("WebM exige AV1 ou VP9. Escolha MP4/MKV para H.264 ou HEVC.".into());
        }
        for (source, input) in [(&self.output_source, false), (&self.input_source, true)] {
            let default = if input {
                "default_input"
            } else {
                "default_output"
            };
            let supported = (!input && source == "game:auto") || source == default
                || source.starts_with("device:")
                || (!input && source.starts_with("app:"));
            if !supported
                || source.len() > 512
                || source.ends_with(':')
                || source.chars().any(|c| c.is_control() || c == '|')
            {
                return Err("Fonte de áudio inválida.".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub label: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AudioSources {
    pub outputs: Vec<Source>,
    pub inputs: Vec<Source>,
    pub applications: Vec<Source>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EncoderSupport {
    pub vendor: String,
    pub encoder: String,
    pub gpu: String,
    pub card_path: String,
    pub pci_address: String,
    pub codecs: Vec<String>,
    pub containers: Vec<String>,
}
impl EncoderSupport {
    pub fn resolve(&self, selection: &Selection) -> Result<String, String> {
        selection.validate()?;
        if !self.containers.iter().any(|v| v == &selection.format) {
            return Err("Formato não disponível no backend instalado.".into());
        }
        let allowed = |codec: &str| {
            self.codecs.iter().any(|v| v == codec)
                && (selection.format != "webm" || matches!(codec, "av1" | "vp9"))
        };
        if selection.codec != "auto" {
            return if allowed(&selection.codec) {
                Ok(selection.codec.clone())
            } else {
                Err(format!(
                    "A GPU/driver de captura não confirmou suporte a {} neste formato.",
                    selection.codec.to_uppercase()
                ))
            };
        }
        let priorities = if selection.format == "webm" {
            &["av1", "vp9"][..]
        } else {
            &["av1", "hevc", "h264", "vp9"][..]
        };
        priorities
            .iter()
            .find(|v| allowed(v))
            .map(|v| v.to_string())
            .ok_or_else(|| "Nenhum codec de GPU compatível foi detectado para este formato.".into())
    }
}
