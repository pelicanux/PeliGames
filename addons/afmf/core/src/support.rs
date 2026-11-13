//! Advisory compatibility, separated from executable discovery in the host.
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Executable {
    pub bits: Option<u8>,
    pub windows: bool,
    pub flatpak: bool,
}

#[derive(Serialize)]
pub struct Support {
    pub available: bool,
    pub code: &'static str,
    pub reason: &'static str,
}

pub fn inspect(executable: Executable) -> Result<Support, String> {
    if executable
        .bits
        .is_some_and(|bits| ![32, 64].contains(&bits))
    {
        return Err("Invalid executable architecture".into());
    }
    let (available, code, reason) = match executable {
        Executable {flatpak: true, ..} => (false, "flatpak", "O AFMF ainda não pode ser usado em jogos de launchers Flatpak: a camada fica numa pasta do PeliGames que o sandbox não acessa."),
        Executable {bits: Some(64), ..} => (true, "supported", "Compatível: processo gráfico de 64 bits. Precisa de Vulkan, diretamente ou pelo DXVK/VKD3D do Proton."),
        Executable {bits: Some(32), windows: true, ..} => (true, "requires-wow64", "Jogo Windows de 32 bits: o AFMF só funciona com o novo WoW64 (Compatibilidade WoW64, na aba Avançado) e um Proton que o suporte."),
        Executable {bits: Some(32), ..} => (false, "native-32-bit", "Jogos Linux de 32 bits não são suportados: o AFMF só é publicado para 64 bits."),
        _ => (true, "unknown", "Não foi possível confirmar a arquitetura do executável. O AFMF exige um processo gráfico de 64 bits com Vulkan."),
    };
    Ok(Support {
        available,
        code,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn advisory_results_distinguish_native_windows_and_unconfirmed_architectures() {
        for (bits, windows, available, code) in [
            (Some(64), false, true, "supported"),
            (Some(32), true, true, "requires-wow64"),
            (Some(32), false, false, "native-32-bit"),
            (None, false, true, "unknown"),
        ] {
            let result = inspect(Executable {
                bits,
                windows,
                flatpak: false,
            })
            .unwrap();
            assert_eq!(result.available, available);
            assert_eq!(result.code, code);
        }
    }
    #[test]
    fn sandbox_and_malformed_probe_results_cannot_be_treated_as_supported() {
        let result = inspect(Executable {
            bits: Some(64),
            windows: true,
            flatpak: true,
        })
        .unwrap();
        assert!(!result.available);
        assert_eq!(result.code, "flatpak");
        assert!(inspect(Executable {
            bits: Some(16),
            windows: true,
            flatpak: false
        })
        .is_err());
    }
}
