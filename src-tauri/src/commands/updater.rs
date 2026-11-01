#[path = "backend_folder.rs"]
pub(super) mod backend_folder;
#[path = "backend_release.rs"]
mod backend_release;
use tauri::AppHandle;
use serde::Serialize;
use reqwest::Client;
use std::fs;
use std::path::PathBuf;
use flate2::read::GzDecoder;
use tar::Archive;

#[path = "backend_source.rs"]
mod backend_source;
#[path = "backend_package.rs"]
mod backend_package;
use backend_source::linux_release;

#[tauri::command]
pub async fn get_backend_path(app: tauri::AppHandle, gpu_arch: String) -> Result<String, String> {
    let backend_dir = get_backend_dir(&app)?.join(&gpu_arch);
    Ok(backend_dir.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn open_backend_folder(app: tauri::AppHandle, gpu_arch: String) -> Result<(), String> {
    let result = async {
        let backend_dir = backend_folder::prepare(&get_backend_dir(&app)?, &gpu_arch)?;
        crate::core::logger::log_launcher(&app, "INFO", &format!("Abrindo pasta do mod: {}", backend_dir.display()));
        tauri::async_runtime::spawn_blocking(move || backend_folder::open(&backend_dir))
            .await.map_err(|error| error.to_string())?
    }.await;
    crate::core::logger::log_result(&app, "Abrir pasta do mod", &result);
    result
}

pub fn get_backend_dir(_app: &AppHandle) -> Result<PathBuf, String> {
    let backend_dir = crate::core::paths::mod_root()?.join("backend-files");
    std::fs::create_dir_all(&backend_dir).map_err(|e| format!("Failed creating mod backend directory: {e}"))?;
    Ok(backend_dir)
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::Emitter;

static CANCEL_DOWNLOAD: AtomicBool = AtomicBool::new(false);
static BACKEND_OPERATION: AtomicBool = AtomicBool::new(false);

struct BackendOperation;
impl BackendOperation {
    fn start() -> Result<Self, String> {
        BACKEND_OPERATION.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| Self).map_err(|_| "Já há uma operação de backend em andamento.".into())
    }
}
impl Drop for BackendOperation {
    fn drop(&mut self) { BACKEND_OPERATION.store(false, Ordering::SeqCst); }
}

#[derive(Clone, Serialize)]
pub struct ProgressPayload {
    pub downloaded: u64,
    pub total: u64,
    pub speed_bytes_per_sec: f64,
}

#[derive(Serialize)]
pub struct BackendVersionStatus {
    pub latest: String,
    pub current: Option<String>,
    pub needs_update: bool,
}

#[tauri::command]
pub async fn check_backend_version(app: AppHandle, gpu_arch: String) -> Result<BackendVersionStatus, String> {
    let client = Client::builder()
        .user_agent("dlssnr-installer")
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .map_err(|e| e.to_string())?;

    let release = linux_release(&client, &gpu_arch).await?;

    let backend_dir = get_backend_dir(&app)?.join(&gpu_arch);
    let version_file = backend_dir.join("version.txt");
    
    let current_version = if version_file.exists() {
        fs::read_to_string(&version_file).ok().map(|s| s.trim().to_string())
    } else if backend_dir.join("64").exists() || backend_dir.join("32").exists() {
        Some("local-copy".to_string())
    } else {
        None
    };

    let tag_with_arch = format!("{}-{}", release.tag_name, gpu_arch);
    let needs_update = if let Some(cv) = &current_version {
        if cv.contains("local-copy") {
            false
        } else {
            cv != &tag_with_arch
        }
    } else {
        true
    };

    Ok(BackendVersionStatus {
        latest: tag_with_arch,
        current: current_version,
        needs_update,
    })
}

#[tauri::command]
pub fn cancel_update() {
    CANCEL_DOWNLOAD.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub async fn update_backend(app: AppHandle, gpu_arch: String) -> Result<String, String> {
    let logger_app = app.clone();
    crate::core::logger::log_launcher(&logger_app, "INFO", &format!("Atualização do backend: GPU={gpu_arch}"));
    let result = update_backend_inner(app, gpu_arch).await;
    crate::core::logger::log_result(&logger_app, "Atualização do backend", &result);
    result
}
async fn update_backend_inner(app: AppHandle, gpu_arch: String) -> Result<String, String> {
    let _operation = BackendOperation::start()?;
    CANCEL_DOWNLOAD.store(false, Ordering::SeqCst);

    let client = Client::builder()
        .user_agent("dlssnr-installer")
        .build()
        .map_err(|e| e.to_string())?;

    let release = linux_release(&client, &gpu_arch).await?;

    let tag_with_arch = format!("{}-{}", release.tag_name, gpu_arch);
    let backend_dir = get_backend_dir(&app)?.join(&gpu_arch);

    let linux_assets = release.linux_assets();
    crate::core::logger::log_launcher(&app, "INFO", &format!("Backend Linux selecionado: GPU={gpu_arch}, release={}, pacotes={}", release.tag_name, linux_assets.iter().map(|(asset, _)| asset.name.as_str()).collect::<Vec<_>>().join(", ")));

    // Sum total sizes to give overall progress
    let mut overall_total: u64 = 0;
    for (asset, _) in &linux_assets {
        if let Ok(resp) = client.head(&asset.url).send().await {
            if resp.status().is_success() { overall_total += resp.content_length().unwrap_or(0); }
        }
    }

    let mut overall_downloaded: u64 = 0;
    let overall_start = Instant::now();
    let mut last_emit = Instant::now();

    let stage = backend_package::UpdateStage::new(backend_dir.parent().ok_or("Pasta de backend inválida.")?, &gpu_arch)?;
    for (asset, bitness) in linux_assets {

        let mut download_resp = client.get(&asset.url).send().await.map_err(|e| format!("Error downloading {}: {}", asset.name, e))?.error_for_status().map_err(|e| e.to_string())?;
        
        let checksum = asset.sha256.clone().or_else(|| download_resp.headers().get("x-checksum-sha256")
            .and_then(|value| value.to_str().ok()).map(str::to_owned));
        // Use the actual size if we couldn't get it via HEAD
        let file_total = download_resp.content_length().unwrap_or(0);
        if overall_total == 0 { overall_total = file_total * 2; } // rough fallback

        let mut bytes_vec = Vec::with_capacity(file_total.min(32 * 1024 * 1024) as usize);

        while let Some(chunk) = download_resp.chunk().await.map_err(|e| e.to_string())? {
            if CANCEL_DOWNLOAD.load(Ordering::SeqCst) {
                return Err("Download cancelado pelo usuário.".into());
            }
            
            bytes_vec.extend_from_slice(&chunk);
            overall_downloaded += chunk.len() as u64;

            let now = Instant::now();
            if now.duration_since(last_emit).as_millis() > 100 {
                let elapsed = overall_start.elapsed().as_secs_f64();
                let speed = if elapsed > 0.0 { overall_downloaded as f64 / elapsed } else { 0.0 };
                
                let _ = app.emit("download-progress", ProgressPayload {
                    downloaded: overall_downloaded,
                    total: overall_total,
                    speed_bytes_per_sec: speed,
                });
                last_emit = now;
            }
        }

        if CANCEL_DOWNLOAD.load(Ordering::SeqCst) {
            return Err("Download cancelado pelo usuário.".into());
        }

        backend_package::verify_download(&bytes_vec, checksum.as_deref())?;
        stage.unpack(&asset.name, bitness, &bytes_vec)?;
    }
    if CANCEL_DOWNLOAD.load(Ordering::SeqCst) {
        return Err("Download cancelado pelo usuário.".into());
    }
    let destination = backend_dir.clone();
    tauri::async_runtime::spawn_blocking(move || stage.publish(&destination, &tag_with_arch, || CANCEL_DOWNLOAD.load(Ordering::SeqCst)))
        .await.map_err(|e| e.to_string())??;

    // final emit
    let elapsed = overall_start.elapsed().as_secs_f64();
    let speed = if elapsed > 0.0 { overall_downloaded as f64 / elapsed } else { 0.0 };
    let _ = app.emit("download-progress", ProgressPayload {
        downloaded: overall_downloaded,
        total: overall_total,
        speed_bytes_per_sec: speed,
    });

    Ok(format!("Arquivos atualizados com sucesso na pasta:\n{:?}", backend_dir))
}

#[tauri::command]
pub async fn delete_backend(app: AppHandle, gpu_arch: String) -> Result<String, String> {
    let logger_app = app.clone();
    crate::core::logger::log_launcher(&logger_app, "INFO", &format!("Exclusão do backend: GPU={gpu_arch}"));
    let result = delete_backend_inner(app, gpu_arch).await;
    crate::core::logger::log_result(&logger_app, "Exclusão do backend", &result);
    result
}
async fn delete_backend_inner(app: AppHandle, gpu_arch: String) -> Result<String, String> {
    if !matches!(gpu_arch.as_str(), "rdna3" | "rdna4") { return Err("Arquitetura de backend inválida.".into()); }
    let _operation = BackendOperation::start()?;
    let backend_dir = get_backend_dir(&app)?.join(&gpu_arch);
    if backend_dir.exists() {
        fs::remove_dir_all(&backend_dir).map_err(|e| format!("Falha ao apagar pasta: {}", e))?;
        Ok("Arquivos do backend apagados com sucesso.".to_string())
    } else {
        Ok("Nenhum arquivo de backend encontrado para apagar.".to_string())
    }
}

#[tauri::command]
pub async fn copy_local_backend(app: AppHandle, gpu_arch: String, source_path: String) -> Result<String, String> {
    if !matches!(gpu_arch.as_str(), "rdna3" | "rdna4") { return Err("Arquitetura de backend inválida.".into()); }
    let _operation = BackendOperation::start()?;
    let backend_dir = get_backend_dir(&app)?.join(&gpu_arch);
    
    // Copy source_path to backend_dir
    let source = PathBuf::from(source_path);
    if !source.exists() {
        return Err("A pasta de origem não existe.".into());
    }

    let dest32 = backend_dir.join("32");
    let dest64 = backend_dir.join("64");

    // very simple copy logic: if source has 32 and 64, copy them.
    let src32 = source.join("32");
    let src64 = source.join("64");

    let mut used_tar = false;
    let mut detected_version = "local-copy".to_string();

    if src32.exists() || src64.exists() {
        if src32.exists() {
            fs::create_dir_all(&dest32).map_err(|e| e.to_string())?;
            copy_dir_all(&src32, &dest32).map_err(|e| e.to_string())?;
        }
        if src64.exists() {
            fs::create_dir_all(&dest64).map_err(|e| e.to_string())?;
            copy_dir_all(&src64, &dest64).map_err(|e| e.to_string())?;
        }
    } else {
        // Try finding tar.gz files
        let mut found_tars = Vec::new();
        if let Ok(entries) = fs::read_dir(&source) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if backend_release::linux_bitness(name, false).is_some() {
                            if (gpu_arch == "rdna4" && name.to_ascii_lowercase().contains("rdna3")) || (gpu_arch == "rdna3" && !name.to_ascii_lowercase().contains("rdna3")) {
                                return Err(format!("O pacote {name} não corresponde à GPU selecionada."));
                            }
                            found_tars.push(path.clone());
                            // DLSSNR-AMD-Vulkan-Linux-0.0.3-x86_64.tar.gz -> "v0.0.3"
                            if let Some(start) = name.find("Linux-") {
                                if let Some(end) = name.find("-x86_64").or_else(|| name.find("-i686")) {
                                    let v = &name[start + 6..end];
                                    detected_version = format!("v{}", v);
                                }
                            }
                        }
                    }
                }
            }
        }

        if found_tars.is_empty() {
            return Err("A pasta selecionada deve conter as subpastas '32' e/ou '64', ou os pacotes Linux .tar.gz/.zip do mod.".into());
        }

        // One archive per architecture: never overlay multiple releases.
        for bits in ["32", "64"] {
            if found_tars.iter().filter(|path| path.file_name().and_then(|name| name.to_str()).and_then(|name| backend_release::linux_bitness(name, false)) == Some(bits)).count() > 1 {
                return Err(format!("Há mais de um pacote de {bits} bits. Deixe apenas a versão que deseja importar na pasta selecionada."));
            }
        }
        for tar_path in found_tars {
            let name = tar_path.file_name().unwrap().to_str().unwrap();
            let bitness = if name.contains("i686") { "32" } else { "64" };
            let target_dir = backend_dir.join(bitness);
            fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;

            let file = fs::File::open(&tar_path).map_err(|e| e.to_string())?;
            if name.to_ascii_lowercase().ends_with(".zip") {
                let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
                archive.extract(&target_dir).map_err(|e| e.to_string())?;
            } else {
                let mut archive = Archive::new(GzDecoder::new(file));
                archive.unpack(&target_dir).map_err(|e| format!("Failed unpacking tar: {e}"))?;
            }
        }
        used_tar = true;
    }

    let ready = [&dest64, &dest32].iter().any(|directory| directory.join("install.sh").is_file() && directory.join("model-tools/extract_model.sh").is_file() && directory.join("dlssnr-amd").is_dir());
    if !ready { return Err("Backend incompleto: faltam install.sh, model-tools/extract_model.sh ou dlssnr-amd. Selecione a pasta ou os pacotes Linux completos.".into()); }
    // Keep version metadata beside the imported packages.
    let version_to_write = format!("{}-{}", detected_version, gpu_arch);
    let _ = fs::write(backend_dir.join("version.txt"), version_to_write);

    if used_tar {
        Ok(format!("Arquivos extraídos com sucesso para:\n{:?}", backend_dir))
    } else {
        Ok(format!("Arquivos copiados com sucesso para:\n{:?}", backend_dir))
    }
}

fn copy_dir_all(src: impl AsRef<std::path::Path>, dst: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
