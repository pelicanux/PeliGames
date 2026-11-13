#[derive(serde::Serialize, Clone, Debug)]
pub struct GameInstallationDetails {
    pub status: String,
    pub installed_dll: Option<String>,
    pub dlssnr_installed: bool,
}

pub fn details_in_directory(dir: &std::path::Path) -> GameInstallationDetails {
    let has = |names: &[&str]| names.iter().any(|name| dir.join(name).is_file());
    // Proxy DLL names alone are not evidence: games, DXVK and other tools ship them too.
    let plugin = dir.join("OptiScaler/plugins/dlssnr-amd.asi").is_file();
    let optiscaler = plugin
        || has(&[
            "OptiScaler.dll",
            "OptiScaler.ini",
            "dlssnr_core.dll",
            "nvngx.dll_dlssnr.dll",
            "nvngx_dlssnr.dll",
        ]);
    let addon = has(&["dlssnr_amd.addon64", "dlssnr_amd.addon32"]);
    let manifest = dir.join("dlssnr-amd-install.txt").is_file()
        && crate::neural_models::find(&dir.join("dlssnr-amd"), crate::neural_models::ORIGINAL)
            .is_some();
    let proxy_names = [
        "vulkan-1.dll",
        "winevulkan.dll",
        "dxgi.dll",
        "d3d9.dll",
        "winmm.dll",
        "version.dll",
        "d3d12.dll",
        "d3d11.dll",
        "dinput8.dll",
        "ReShade64.dll",
        "ReShade32.dll",
        "OptiScaler.dll",
    ];
    // Prefer the loader recorded by the installer over similarly named original game DLLs.
    use std::io::Read;
    let mut recorded_files = String::new();
    if let Ok(file) = std::fs::File::open(dir.join("dlssnr-amd-install.txt")) {
        let _ = file.take(64 * 1024).read_to_string(&mut recorded_files);
    }
    let proxy = proxy_names
        .iter()
        .find(|name| {
            dir.join(name).is_file() && recorded_files.lines().any(|line| line.trim() == **name)
        })
        .or_else(|| proxy_names.iter().find(|name| dir.join(name).is_file()));
    let status = if proxy.is_none() || !(optiscaler || addon || manifest) {
        "Não Instalado"
    } else if optiscaler {
        "OptiScaler"
    } else if matches!(proxy.copied(), Some("vulkan-1.dll" | "winevulkan.dll")) {
        "Vulkan/DX9"
    } else {
        "ReShade"
    };
    let installed_dll = if status == "Não Instalado" {
        None
    } else {
        proxy.map(|name| (*name).to_string())
    };
    let dlssnr_installed = proxy.is_some()
        && (plugin
            || addon
            || manifest
            || has(&[
                "dlssnr_core.dll",
                "nvngx.dll_dlssnr.dll",
                "nvngx_dlssnr.dll",
            ]));
    GameInstallationDetails {
        status: status.to_string(),
        installed_dll,
        dlssnr_installed,
    }
}

pub fn installed_directory(directory: &std::path::Path) -> Option<std::path::PathBuf> {
    (details_in_directory(directory).status != "Não Instalado").then(|| directory.to_path_buf())
}
