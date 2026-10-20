use tauri::AppHandle;
use crate::core::script_runner::execute_standalone_install;
use crate::commands::updater::get_backend_dir;

/// Tauri Command: install_mod
/// 
/// Orchestrates a standalone installation by extracting the bundled resources
/// to a temporary directory, preparing the model, and executing the bash script.
#[tauri::command]
pub async fn install_mod(
    app: AppHandle,
    game_dir: String,
    route: String,
    bitness: String,
    gpu_arch: String,
    dll_path: Option<String>,
    mut bin_path: Option<String>,
    shortcut_key: String,
    neural_startup: Option<bool>,
    int4: Option<bool>,
    renodx: Option<bool>,
    model: Option<crate::core::script_runner::NeuralModel>,
    lumenite: Option<bool>,
) -> Result<String, String> {
    crate::core::logger::log_launcher(&app, "INFO", &format!("Operação no jogo: rota={route}, arquitetura={bitness}, GPU={gpu_arch}, diretório={game_dir}"));
    let result = async {
    backend_arch(&gpu_arch, &bitness)?;
    if bin_path.as_deref().and_then(|path| std::path::Path::new(path).parent()) == Some(crate::core::paths::mod_root()?.as_path()) {
        bin_path = check_cached_bin(app.clone(), bitness.clone(), gpu_arch.clone())?;
        if route != "remove" && bin_path.is_none() { return Err("MODEL_BIN_NOT_FOUND".into()); }
    }
    // Validate the selected model before copying resources or starting the installer.
    if route != "remove" {
        if let Some(ref bin) = bin_path {
            if !std::path::Path::new(bin).is_file() {
                return Err("MODEL_BIN_NOT_FOUND".to_string());
            }
        }
    }
    let base_dir = get_backend_dir(&app)?;
    let resource_path = base_dir.join(&gpu_arch).join(&bitness);

    if !backend_ready(&resource_path) {
        return Err("Arquivos do backend não encontrados! Clique no botão de Atualizar no topo direito para baixar os arquivos necessários.".to_string());
    }

    let install_game = game_dir.clone();
    let install_route = route.clone();
    let output = tauri::async_runtime::spawn_blocking(move || execute_standalone_install(
        &resource_path, &install_game, &install_route, dll_path.as_ref(), bin_path.as_ref(), &shortcut_key,
        crate::core::script_runner::InstallExtras { int4, renodx, model, lumenite },
    )).await.map_err(|error| error.to_string())??;
    if route != "remove" {
        if let Some(enabled) = neural_startup {
            super::neural_settings::set_enabled(std::path::Path::new(&game_dir), enabled)?;
        }
    }
    Ok(output)
    }.await;
    super::analyzer::invalidate_game(&game_dir);
    crate::core::logger::log_result(&app, "Instalação/remoção do mod", &result);
    result
}

fn backend_arch(gpu_arch: &str, bitness: &str) -> Result<(), String> {
    if !matches!(gpu_arch, "rdna3" | "rdna4") || !matches!(bitness, "32" | "64") { return Err("Arquitetura inválida.".into()); }
    Ok(())
}
fn backend_ready(directory: &std::path::Path) -> bool {
    directory.join("install.sh").is_file() && directory.join("model-tools/extract_model.sh").is_file() && directory.join("dlssnr-amd").is_dir()
}
#[tauri::command]
pub fn check_cached_bin(app: AppHandle, bitness: String, gpu_arch: String) -> Result<Option<String>, String> {
    backend_arch(&gpu_arch, &bitness)?;
    let model = get_backend_dir(&app)?.join(gpu_arch).join(bitness).join("dlssnr-amd/dlssnr.bin");
    Ok(model.is_file().then(|| model.to_string_lossy().to_string()))
}
#[derive(serde::Serialize)]
pub struct BackendSetupStatus { path: String, ready: bool, cached: Vec<String> }
#[tauri::command]
pub fn get_backend_setup_status(app: AppHandle, gpu_arch: String) -> Result<BackendSetupStatus, String> {
    backend_arch(&gpu_arch, "64")?;
    let root = get_backend_dir(&app)?.join(gpu_arch);
    let directories: Vec<_> = ["64", "32"].into_iter().map(|bits| root.join(bits)).filter(|dir| backend_ready(dir)).collect();
    let cached = directories.iter().map(|dir| dir.join("dlssnr-amd/dlssnr.bin")).filter(|file| file.is_file()).map(|file| file.to_string_lossy().to_string()).collect();
    Ok(BackendSetupStatus {path:root.to_string_lossy().to_string(),ready:!directories.is_empty(),cached})
}
#[tauri::command]
pub async fn import_model_bins(app: AppHandle, gpu_arch: String, bin_path: String) -> Result<String, String> {
    backend_arch(&gpu_arch, "64")?;
    let root = get_backend_dir(&app)?.join(gpu_arch);
    tauri::async_runtime::spawn_blocking(move || {
        let directories: Vec<_> = ["64", "32"].into_iter().map(|bits| root.join(bits)).filter(|dir| backend_ready(dir)).collect();
        let first = directories.first().ok_or("Baixe e verifique o backend antes de importar os modelos.")?.join("dlssnr-amd");
        crate::core::neural_models::stage(std::path::Path::new(&bin_path), &first)?;
        for directory in directories.iter().skip(1) { crate::core::neural_models::cache(&first, &directory.join("dlssnr-amd"))?; }
        Ok(first.join("dlssnr.bin").to_string_lossy().to_string())
    }).await.map_err(|error| error.to_string())?
}

#[derive(serde::Serialize)]
pub struct NeuralModelStatus { pub small: bool, pub original: bool, pub selectable: bool, pub unified_networks: bool, pub lumenite: bool, pub lumenite_selected: bool }

#[tauri::command]
pub fn get_neural_model_status(app: AppHandle, gpu_arch: String, bitness: String, bin_path: Option<String>, game_dir: Option<String>) -> Result<NeuralModelStatus, String> {
    if !matches!(gpu_arch.as_str(), "rdna3" | "rdna4") || !matches!(bitness.as_str(), "32" | "64") {
        return Err("Arquitetura inválida.".into());
    }
    let resource = get_backend_dir(&app)?.join(gpu_arch).join(bitness);
    let script = std::fs::read_to_string(resource.join("install.sh")).unwrap_or_default();
    let canonical = resource.join("dlssnr-amd");
    let selected = bin_path.as_deref().map(std::path::Path::new);
    let old_root = crate::core::paths::mod_root()?;
    let model_dir = if canonical.join("dlssnr.bin").is_file() { Some(canonical.as_path()) }
        else { selected.and_then(|p| p.parent()).filter(|p| *p != old_root.as_path()) };
    let has = |name| model_dir.map(|dir| dir.join(name).is_file()).unwrap_or(false);
    Ok(NeuralModelStatus {
        small: has("dlssnr-small.bin") && has("dlssnr.bin"), original: has("dlssnr.bin"),
        selectable: crate::core::script_runner::selectable_models(&script),
        unified_networks: crate::core::script_runner::unified_networks(&script),
        lumenite: crate::core::script_runner::supports_lumenite(&script),
        lumenite_selected: match game_dir { Some(directory) => super::neural_settings::lumenite_selected(std::path::Path::new(&directory))?, None => false },
    })
}

use crate::core::script_runner::inject_shortcut_key_pub;

#[tauri::command]
pub fn update_shortcut_key_in_game(game_dir: String, shortcut_key: String, route: String) -> Result<(), String> {
    inject_shortcut_key_pub(&game_dir, &shortcut_key, &route)
}

use crate::commands::analyzer::scan_directory_for_exe;

#[derive(serde::Serialize)]
pub struct GameInstallationDetails {
    status: String,
    installed_dll: Option<String>,
    dlssnr_installed: bool,
}

fn details_in_directory(dir: &std::path::Path) -> GameInstallationDetails {
    let has = |names: &[&str]| names.iter().any(|name| dir.join(name).is_file());
    // Proxy DLL names alone are not evidence: games, DXVK and other tools ship them too.
    let optiscaler = has(&["OptiScaler.dll", "OptiScaler.ini", "dlssnr_core.dll", "nvngx.dll_dlssnr.dll", "nvngx_dlssnr.dll"]);
    let addon = has(&["dlssnr_amd.addon64", "dlssnr_amd.addon32"]);
    let manifest = dir.join("dlssnr-amd-install.txt").is_file()
        && dir.join("dlssnr-amd/dlssnr.bin").is_file();
    let proxy_names = ["vulkan-1.dll", "winevulkan.dll", "dxgi.dll", "d3d9.dll", "winmm.dll", "version.dll", "d3d12.dll", "d3d11.dll", "dinput8.dll", "ReShade64.dll", "ReShade32.dll", "OptiScaler.dll"];
    // Prefer the loader recorded by the installer over similarly named original game DLLs.
    use std::io::Read;
    let mut recorded_files = String::new();
    if let Ok(file) = std::fs::File::open(dir.join("dlssnr-amd-install.txt")) {
        let _ = file.take(64 * 1024).read_to_string(&mut recorded_files);
    }
    let proxy = proxy_names.iter().find(|name| dir.join(name).is_file()
        && recorded_files.lines().any(|line| line.trim() == **name))
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
    let installed_dll = if status == "Não Instalado" { None } else { proxy.map(|name| (*name).to_string()) };
    let dlssnr_installed = proxy.is_some() && (addon || manifest
        || has(&["dlssnr_core.dll", "nvngx.dll_dlssnr.dll", "nvngx_dlssnr.dll"]));
    GameInstallationDetails { status: status.to_string(), installed_dll, dlssnr_installed }
}

pub(crate) fn installed_mod_directory(base_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    if details_in_directory(base_dir).status != "Não Instalado" { return Some(base_dir.to_path_buf()); }
    let exe = scan_directory_for_exe(base_dir)?;
    let dir = exe.parent()?;
    if details_in_directory(dir).status != "Não Instalado" { Some(dir.to_path_buf()) } else { None }
}

fn installation_details(game_dir: &str) -> GameInstallationDetails {
    let base_dir = std::path::Path::new(game_dir);
    if !base_dir.exists() {
        return GameInstallationDetails { status: "Não Instalado".to_string(), installed_dll: None, dlssnr_installed: false };
    }
    let root_details = details_in_directory(base_dir);
    // install.sh writes into the selected folder, even when the largest EXE is nested.
    // The manifest/add-on/core files identify that folder without another directory walk.
    let root_has_mod = ["dlssnr-amd-install.txt", "dlssnr_core.dll", "OptiScaler.ini", "dlssnr_amd.addon64", "dlssnr_amd.addon32", "ReShade.ini"].iter().any(|name| base_dir.join(name).is_file());
    if root_has_mod && root_details.status != "Não Instalado" { return root_details; }

    // Retain detection for existing installations next to a nested game executable.
    let exe_path = scan_directory_for_exe(base_dir);
    if let Some(dir) = exe_path.as_ref().and_then(|exe| exe.parent()) {
        if dir != base_dir {
            let nested_details = details_in_directory(dir);
            if nested_details.status != "Não Instalado" { return nested_details; }
        }
    }
    root_details
}

#[tauri::command]
pub async fn get_game_installation_details(game_dir: String) -> Result<GameInstallationDetails, String> {
    tauri::async_runtime::spawn_blocking(move || installation_details(&game_dir))
        .await.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn check_game_installation(game_dir: String) -> Result<String, String> {
    Ok(get_game_installation_details(game_dir).await?.status)
}

#[tauri::command]
pub async fn get_installed_dll(game_dir: String) -> Result<Option<String>, String> {
    Ok(get_game_installation_details(game_dir).await?.installed_dll)
}

#[tauri::command]
pub async fn extract_dll(app: AppHandle, dll_path: String, gpu_arch: String, bitness: String) -> Result<String, String> {
    backend_arch(&gpu_arch, &bitness)?;
    extract_dll_wizard(app.clone(), dll_path, gpu_arch.clone()).await?;
    check_cached_bin(app, bitness, gpu_arch)?.ok_or_else(|| "Backend da arquitetura solicitada não está instalado.".into())
}

#[tauri::command]
pub async fn extract_dll_wizard(
    app: AppHandle,
    dll_path: String,
    gpu_arch: String,
) -> Result<String, String> {
    use std::fs;
    use std::env;
    use crate::core::script_runner;

    backend_arch(&gpu_arch, "64")?;
    let root = get_backend_dir(&app)?.join(&gpu_arch);
    let resource_path = ["64", "32"].into_iter().map(|bits| root.join(bits)).find(|dir| backend_ready(dir))
        .ok_or("Backend não encontrado ou incompleto. Baixe e verifique os arquivos primeiro.")?;
    let temp_game_dir = env::temp_dir().join(format!("dummy-game-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_game_dir).map_err(|e| e.to_string())?;

    // Run the heavy I/O work in a blocking thread pool to avoid freezing the UI
    let resource_path_clone = resource_path.clone();
    let temp_game_dir_clone = temp_game_dir.clone();
    let dll_path_clone = dll_path.clone();
    let backend_root = root.clone();

    let res = tokio::task::spawn_blocking(move || {
        script_runner::execute_standalone_install(
            &resource_path_clone,
            &temp_game_dir_clone.to_string_lossy(),
            "reshade-feeder",
            Some(&dll_path_clone),
            None,
            "Insert",
            script_runner::InstallExtras { int4: Some(false), renodx: Some(false), ..Default::default() },
        )?;
        for bits in ["64", "32"] {
            let directory = backend_root.join(bits);
            if directory != resource_path_clone && backend_ready(&directory) {
                crate::core::neural_models::cache(&resource_path_clone.join("dlssnr-amd"), &directory.join("dlssnr-amd"))?;
            }
        }
        Ok::<(), String>(())
    }).await.map_err(|e| format!("Task error: {}", e))?;

    let _ = fs::remove_dir_all(&temp_game_dir);

    if res.is_err() {
        return Err(res.unwrap_err());
    }

    Ok(resource_path.join("dlssnr-amd/dlssnr.bin").to_string_lossy().to_string())
}

#[cfg(test)]
mod modern_library_detection_tests {
    use super::details_in_directory;
    use std::{fs, path::PathBuf, sync::atomic::{AtomicU64, Ordering}};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new(files: &[&str]) -> Self {
            let path = std::env::temp_dir().join(format!("peligames-library-detection-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap();
            for file in files { fs::write(path.join(file), []).unwrap(); }
            Self(path)
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    #[test]
    fn setup_requires_the_installer_extractor_and_model_directory() {
        let fixture = Fixture::new(&["install.sh"]);
        assert!(!super::backend_ready(&fixture.0));
        fs::create_dir_all(fixture.0.join("model-tools")).unwrap();
        fs::write(fixture.0.join("model-tools/extract_model.sh"), []).unwrap();
        assert!(!super::backend_ready(&fixture.0));
        fs::create_dir(fixture.0.join("dlssnr-amd")).unwrap();
        assert!(super::backend_ready(&fixture.0));
        assert!(super::backend_arch("rdna4", "64").is_ok());
        assert!(super::backend_arch("rdna3", "32").is_ok());
        assert!(super::backend_arch("../other", "64").is_err());
        assert!(super::backend_arch("rdna4", "../other").is_err());
    }
    #[test]
    fn generic_optiscaler_is_not_dlssnr() {
        let fixture = Fixture::new(&["OptiScaler.dll", "OptiScaler.ini"]);
        let details = details_in_directory(&fixture.0);
        assert_eq!(details.status, "OptiScaler");
        assert!(!details.dlssnr_installed);
    }
    #[test]
    fn detects_dlssnr_with_a_loader() {
        for core in ["dlssnr_core.dll", "dlssnr_amd.addon64", "nvngx_dlssnr.dll"] {
            let fixture = Fixture::new(&["dxgi.dll", core]);
            let details = details_in_directory(&fixture.0);
            assert!(details.dlssnr_installed);
            assert!(details.installed_dll.is_some());
        }
    }
    #[test]
    fn proxy_or_core_alone_does_not_enable_the_tab() {
        for file in ["dxgi.dll", "dlssnr_amd.addon64", "dlssnr_core.dll"] {
            let fixture = Fixture::new(&[file]);
            assert!(!details_in_directory(&fixture.0).dlssnr_installed);
        }
    }
}
