//! Pinned stable runtimes, isolated from global Vulkan discovery. Profiles are
//! launch wrappers, so existing Steam/Epic/Pelinstall execution paths own them.
use crate::core::advanced_settings::{AdvancedSettings, LaunchWrapper};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
#[path = "frame_generation_flatpak.rs"]
pub mod flatpak;
#[path = "frame_generation_mangohud.rs"]
mod mangohud;
#[path = "frame_profile_names.rs"]
pub(super) mod profile_names;
#[path = "frame_generation_versions.rs"]
pub mod versions;
static INSTALLING: AtomicBool = AtomicBool::new(false);
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
            gpu: String::new(), base_fps_cap: 0, refresh_threshold: 0,
            ultra_performance: false, allow_fp16: true, live_enabled: true,
            scaling_enabled: false, scaling_method: "native".into(),
            scaling_factor: 1.5, scaling_sharpness: 0.8,
        }
    }
}
fn release(backend: &str) -> Result<(&'static str, &'static str, &'static str), String> {
    match backend{
 "mako"=>Ok(("4.5.0","https://github.com/eugeniosegala/MAKO/releases/download/render-v4.5.0/MAKO-Renderer-v4.5.0-linux.tar.xz","4dd3e70743a08a0f2f0e6006d35d1e08dfcf751932678a6217cbdcf87c784305")),
 "afmf"=>Ok(("1.3.1","https://github.com/serialexperimentslainnnn/afmf-linux/releases/download/v1.3.1/afmf-linux-1.3.1-x86_64.tar.xz","7fafa09021ee94dc5ec8da694cb02fe298b54c114b486611bb24b1e1500edb18")),_=>Err("Tecnologia de geração de quadros inválida.".into())}
}
/// The installed AFMF layer folder (manifest beside its library) for a game launch.
pub(crate) fn afmf_layer_dir() -> Result<PathBuf, String> {
    let path = versions::location("afmf", &versions::current("afmf")?)?;
    validate_runtime_at(&path, "afmf")
        .map_err(|_| "O AFMF está ativado neste jogo, mas não está instalado. Instale-o em Configurações → Mods → AFMF ou na aba Geração de quadros do jogo.".to_string())?;
    afmf_runtime_manifest(&path)
}
/// A bare filename is searched in ld.so's global paths, not beside the JSON.
/// Keep the signed package unchanged and offer a private, adapted manifest.
fn afmf_runtime_manifest(path: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    let path = path.canonicalize().map_err(|e|e.to_string())?;
    let mut manifest: serde_json::Value = serde_json::from_slice(&fs::read(path.join("afmf-linux.json")).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    manifest["layer"]["library_path"] = serde_json::Value::String(path.join("libafmf-linux.so").to_string_lossy().into_owned());
    manifest["layer"]["library_arch"] = serde_json::Value::String("64".into());
    // Explicit activation keeps GE/Steam above AFMF's native
    // presentation threads. The package and older implicit adapter stay intact.
    manifest["file_format_version"] = serde_json::Value::String("1.2.1".into());
    let layer = manifest["layer"].as_object_mut().ok_or("Manifesto AFMF inválido")?;
    layer.remove("enable_environment");
    layer.remove("disable_environment");
    let directory = path.join("peligames-explicit-layer");
    match fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {},
        Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists => {},
        Err(e) => return Err(e.to_string()),
    }
    let meta=fs::symlink_metadata(&directory).map_err(|e|e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.uid()!=unsafe{libc::geteuid()} || meta.mode()&0o077!=0 {
        return Err("Pasta do manifesto AFMF sem permissões privadas.".into());
    }
    let destination=directory.join("afmf-linux.json");
    let bytes=serde_json::to_vec_pretty(&manifest).map_err(|e|e.to_string())?;
    if fs::read(&destination).ok().as_deref()!=Some(bytes.as_slice()) { write(&destination,&bytes,0o600)?; }
    Ok(directory)
}
pub(super) fn root() -> Result<PathBuf, String> {
    Ok(crate::core::paths::app_path("FrameGeneration")?)
}
fn selection_file(profiles:&Path,key:&str,label:&str)->Result<PathBuf,String>{
    let base=crate::core::paths::names::allocate(&profiles.join("selections"),label,&profile_key(key))?;
    let path=base.with_extension("json");let old=profiles.join(profile_key(key)+".selection.json");
    if !path.exists()&&old.is_file(){fs::rename(old,&path).map_err(|e|e.to_string())?;}
    Ok(path)
}
fn module_file(key:&str,label:&str)->Result<PathBuf,String>{
    let folder=crate::core::paths::app_path("GameModules")?;
    let identity=profile_key(key);
    let base=crate::core::paths::names::allocate(&folder,label,&identity)?;
    let path=base.with_extension("json");let old=folder.join(identity+".json");
    if !path.exists()&&old.is_file(){fs::rename(old,&path).map_err(|e|e.to_string())?;}
    Ok(path)
}
fn profile_key(key: &str) -> String {
    format!("{:x}", Sha256::digest(key.as_bytes()))
}
pub(super) fn check_settings(s: &Settings) -> Result<(), String> {
    release(&s.backend)?;
    if s.base_fps_cap > 1000 || s.refresh_threshold > 1000
        || !s.scaling_factor.is_finite() || !(1.0..=2.0).contains(&s.scaling_factor)
        || !s.scaling_sharpness.is_finite() || !(0.0..=1.0).contains(&s.scaling_sharpness)
        || !["native", "mako", "ls1", "ls1-performance"].contains(&s.scaling_method.as_str())
        || !crate::commands::gpu::valid_pci_address(&s.gpu) {
        return Err("Ajustes MAKO inválidos: confira GPU, limites (0–1000), escala (1–2×) e nitidez (0–1).".into());
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
fn dll_valid(path: &Path) -> bool {
    let mut b = [0; 2];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok()
        && b == *b"MZ"
}
/// Compare the DLL's filesystem with the system root, following symlinks.
/// An unavailable path has unknown status and must not produce a warning.
#[tauri::command]
pub async fn frame_generation_dll_external_filesystem(path: String) -> Option<bool> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let dll = fs::metadata(path).ok()?;
            if !dll.is_file() {
                return None;
            }
            let system = fs::metadata("/").ok()?;
            Some(dll.dev() != system.dev())
        }
        #[cfg(not(unix))]
        {
            let _ = path;
            None
        }
    })
    .await
    .ok()
    .flatten()
}
fn detect_dll(backend: &str) -> Option<String> {
    let home = dirs::home_dir()?;
    if backend != "mako" {
        return None;
    }
    let names = &["Lossless.dll"];
    for name in names {
        for lib in super::scanner::steam_libraries(&home) {
            let p = lib.join("steamapps/common/Lossless Scaling").join(name);
            if dll_valid(&p) {
                return Some(p.to_string_lossy().into());
            }
        }
    }
    None
}
/// AFMF's layer: a relative manifest beside a self-contained x86_64 library,
/// dormant until AFMF_ENABLE=1, so the folder can be offered per game.
fn validate_afmf_at(path: &Path) -> Result<(), String> {
    let incomplete = || "Runtime do AFMF incompleto. Instale novamente o componente.".to_string();
    for file in ["libafmf-linux.so", "afmf-linux.json"] {
        let p = path.join(file);
        if !p.is_file() || p.is_symlink() { return Err(incomplete()); }
    }
    let mut header = [0u8; 20];
    fs::File::open(path.join("libafmf-linux.so")).and_then(|mut f| f.read_exact(&mut header)).map_err(|_| incomplete())?;
    if header[..5] != [0x7f, b'E', b'L', b'F', 2] || header[18..20] != [62, 0] {
        return Err("A biblioteca do AFMF não é Linux x86_64.".into());
    }
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(path.join("afmf-linux.json")).map_err(|_| incomplete())?)
        .map_err(|_| "Manifesto do AFMF inválido.".to_string())?;
    let layer = &manifest["layer"];
    if layer["name"] != "VK_LAYER_AFMF" || layer["library_path"] != "libafmf-linux.so" || layer["enable_environment"]["AFMF_ENABLE"] != "1" {
        return Err("Manifesto do AFMF diferente do esperado. Nenhum runtime foi instalado.".into());
    }
    Ok(())
}
/// The AFMF archive keeps its files under one versioned folder: bring them to the stage root.
fn flatten_single_root(stage: &Path) -> Result<(), String> {
    let entries: Vec<_> = fs::read_dir(stage).map_err(|e| e.to_string())?.flatten().collect();
    let [only] = entries.as_slice() else { return Ok(()) };
    if !only.file_type().map_err(|e| e.to_string())?.is_dir() { return Ok(()); }
    let root = only.path();
    for child in fs::read_dir(&root).map_err(|e| e.to_string())?.flatten() {
        fs::rename(child.path(), stage.join(child.file_name())).map_err(|e| e.to_string())?;
    }
    fs::remove_dir(root).map_err(|e| e.to_string())
}
fn validate_runtime_at(path: &Path, backend: &str) -> Result<(), String> {
    release(backend)?;
    if backend == "afmf" {
        return validate_afmf_at(path);
    }
    let files = &[
        "bin/mako-launch",
        "bin/mako-cli",
        "lib/libmako-render.so",
        "lib32/libmako-render.so",
        "share/mako-render/vulkan/implicit_layer.d/VkLayer_MAKO_render.json",
    ];
    for f in files {
        let p = path.join(f);
        if !p.is_file() || p.is_symlink() {
            return Err("Runtime incompleto. Instale novamente o componente.".into());
        }
    }
    Ok(())
}
#[derive(Serialize)]
pub struct State {
    settings: Settings,
    versions: serde_json::Value,
    dll: Option<String>,
    installed: bool,
    warning: String,
    download_url: String,
    install_directory: String,
    package_name: String,
    current_version: String,
    available_version: String,
    update_compatible: bool,
    update_available: bool,
    install_version: String,
    installed_versions: Vec<versions::Installed>,
}
#[tauri::command]
pub async fn frame_generation_state(
    game_key: String,
    backend: String,
    active_profile: Option<String>,
) -> Result<State, String> {
    crate::core::release_features::require()?;
    tauri::async_runtime::spawn_blocking(move||{
 let had_profile=active_profile.is_some();let profiles=root()?.join("profiles");let selected=active_profile.map(PathBuf::from).filter(|p|p.parent()==Some(profiles.as_path()) && p.extension().is_some_and(|e|e=="sh")).map(|p|p.with_extension("json")).map(Ok).unwrap_or_else(||selection_file(&profiles,&game_key,&game_key))?;
 let mut settings:Settings=match fs::read(selected){Ok(b)=>serde_json::from_slice(&b).map_err(|_|"Perfil de geração inválido.")?,Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Settings::default(),Err(e)=>return Err(e.to_string())};
 let legacy_profile=settings.backend!="mako";if legacy_profile{settings=Settings::default();}
 let backend=if backend=="saved"{settings.backend.clone()}else{backend};if settings.backend!=backend{settings.dll.clear();settings.runtime_version.clear();}settings.backend=backend.clone();check_settings(&settings)?;
 let current_version=versions::current(&backend)?;
 if settings.runtime_version.is_empty(){settings.runtime_version=if had_profile{release(&backend)?.0.into()}else{current_version.clone()};}
 let available=versions::latest(&backend)?;
 let installable=if versions::compatible(&backend,&available.version){available.clone()}else{versions::builtin(&backend)?};
 let path=versions::location(&backend,&settings.runtime_version)?;
 Ok(State{download_url:installable.url.clone(),install_directory:if validate_runtime_at(&path,&backend).is_ok(){path.to_string_lossy().into()}else{versions::location(&backend,&installable.version)?.to_string_lossy().into()},package_name:installable.url.rsplit('/').next().unwrap_or_default().into(),dll:detect_dll(&backend),installed:validate_runtime_at(&path,&backend).is_ok(),settings,versions:serde_json::json!({"mako":versions::current("mako")?}),warning:if legacy_profile{"A integração anterior foi removida. Configure o MAKO e salve para substituir o vínculo antigo; nenhuma ativação foi convertida automaticamente.".into()}else{"Use apenas um gerador de quadros. Desative a geração interna do jogo e do OptiScaler; DLSSNR/upscaling sozinho não implica conflito. Compatibilidade depende do jogo. É necessário Vulkan (nativo, DXVK ou VKD3D); OpenGL e WineD3D não são cobertos.".into()},update_available:versions::newer(&available.version,&current_version),install_version:installable.version,update_compatible:versions::compatible(&backend,&available.version),available_version:available.version,current_version,installed_versions:versions::installed(&backend)?})
 }).await.map_err(|e|e.to_string())?
}
fn unpack(bytes: &[u8], stage: &Path) -> Result<(), String> {
    let decoder = xz2::read::XzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    let mut total = 0u64;
    for item in archive.entries().map_err(|e| e.to_string())? {
        let mut item = item.map_err(|e| e.to_string())?;
        let rel = item.path().map_err(|e| e.to_string())?.into_owned();
        if rel.is_absolute()
            || rel.components().any(|c| {
                !matches!(
                    c,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
        {
            return Err("Caminho inseguro no pacote.".into());
        }
        let kind = item.header().entry_type();
        if !(kind.is_dir() || kind.is_file()) {
            return Err("O pacote contém links ou arquivos especiais não permitidos.".into());
        }
        total = total
            .checked_add(item.size())
            .ok_or("Pacote excede o limite.")?;
        if total > 256 * 1024 * 1024 {
            return Err("Pacote excede o limite.".into());
        }
        item.unpack_in(stage).map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[derive(Clone, Serialize)]
pub struct InstallProgress {
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
}
#[tauri::command]
pub async fn install_frame_generation(
    backend: String,
    version: Option<String>,
    on_progress: tauri::ipc::Channel<InstallProgress>,
) -> Result<(), String> {
    crate::core::release_features::require()?;
    if INSTALLING.swap(true, Ordering::AcqRel) {
        return Err("Uma instalação de geração de quadros já está em andamento.".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            INSTALLING.store(false, Ordering::Release);
        }
    }
    let _guard = Guard;
    if std::env::consts::ARCH != "x86_64" {
        return Err("Os pacotes estáveis selecionados exigem Linux x86_64.".into());
    }
    let latest = versions::latest(&backend)?;
    let selected_release = if version.as_deref() == Some(release(&backend)?.0) {
        versions::builtin(&backend)?
    } else {
        latest
    };
    if version
        .as_ref()
        .is_some_and(|v| v != &selected_release.version)
    {
        return Err("O catálogo mudou. Verifique a atualização novamente.".into());
    }
    if !versions::compatible(&backend, &selected_release.version) {
        return Err(
            "Esta versão requer uma atualização do PeliGames para adaptar a integração.".into(),
        );
    }
    let url = selected_release.url.clone();
    let hash = selected_release.hash.clone();
    let _ = on_progress.send(InstallProgress {
        phase: "connecting",
        downloaded: 0,
        total: None,
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let total = response.content_length().filter(|n| *n > 0);
    if total.is_some_and(|n| n > 32 * 1024 * 1024) {
        return Err("Pacote excede o limite de download.".into());
    }
    let mut last_update = std::time::Instant::now();
    let _ = on_progress.send(InstallProgress {
        phase: "downloading",
        downloaded: 0,
        total,
    });
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 32 * 1024 * 1024 {
            return Err("Pacote excede o limite de download.".into());
        }
        bytes.extend_from_slice(&chunk);
        if last_update.elapsed() >= std::time::Duration::from_millis(80) {
            let _ = on_progress.send(InstallProgress {
                phase: "downloading",
                downloaded: bytes.len() as u64,
                total,
            });
            last_update = std::time::Instant::now();
        }
    }
    let downloaded = bytes.len() as u64;
    let _ = on_progress.send(InstallProgress {
        phase: "verifying",
        downloaded,
        total,
    });
    let downloaded_hash = format!("{:x}", Sha256::digest(&bytes));
    if hash.as_ref().is_some_and(|h| &downloaded_hash != h) {
        return Err("A verificação SHA-256 falhou. Nenhum runtime foi instalado.".into());
    }
    let _ = on_progress.send(InstallProgress {
        phase: "extracting",
        downloaded,
        total,
    });
    let install_progress = on_progress.clone();
    let mut installed_release = selected_release.clone();
    installed_release.hash = Some(downloaded_hash);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let destination = versions::location(&backend, &installed_release.version)?;
        fs::create_dir_all(destination.parent().ok_or("Destino inválido")?)
            .map_err(|e| e.to_string())?;
        fs::create_dir_all(root()?).map_err(|e| e.to_string())?;
        let stage = root()?.join(format!(".install-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&stage).map_err(|e| e.to_string())?;
        let result = (|| {
            unpack(&bytes, &stage)?;
            if backend == "afmf" { flatten_single_root(&stage)?; }
            validate_runtime_at(&stage, &backend)?;
            let _ = install_progress.send(InstallProgress {
                phase: "installing",
                downloaded,
                total,
            });
            if destination.exists() && validate_runtime_at(&destination, &backend).is_ok() {
                return versions::publish(&backend, &installed_release);
            }
            for entry in walkdir::WalkDir::new(&stage).contents_first(true) {
                let entry = entry.map_err(|e| e.to_string())?;
                fs::File::open(entry.path())
                    .and_then(|f| f.sync_all())
                    .map_err(|e| e.to_string())?;
            }
            let previous = root()?.join(format!(".previous-{}", uuid::Uuid::new_v4()));
            let replacing = destination.exists();
            if replacing {
                fs::rename(&destination, &previous).map_err(|e| e.to_string())?;
            }
            if let Err(error) = fs::rename(&stage, &destination) {
                if replacing {
                    let _ = fs::rename(&previous, &destination);
                }
                return Err(error.to_string());
            }
            fs::File::open(root()?)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
            versions::publish(&backend, &installed_release)
        })();
        let _ = fs::remove_dir_all(&stage);
        result
    })
    .await
    .map_err(|e| e.to_string())?;
    if result.is_ok() {
        let _ = on_progress.send(InstallProgress {
            phase: "complete",
            downloaded,
            total,
        });
    }
    result
}
fn optiscaler_fg_enabled(text: &str) -> bool {
    let mut frame_gen = false;
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.starts_with('[') {
            frame_gen = line.eq_ignore_ascii_case("[FrameGen]");
            continue;
        }
        if frame_gen {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim().eq_ignore_ascii_case("Enabled")
                    && ["true", "1", "yes", "on"]
                        .contains(&value.trim().to_ascii_lowercase().as_str())
                {
                    return true;
                }
            }
        }
    }
    false
}
fn check_local_fg(root: &Path) -> Result<(), String> {
    let mut folders = vec![root.to_path_buf()];
    if let Some(path) = crate::core::game_executable::best_executable_dir(root, 5) {
        if path != root {
            folders.push(path);
        }
    }
    for folder in folders {
        let path = folder.join("OptiScaler.ini");
        if let Ok(file) = fs::File::open(path) {
            let mut text = String::new();
            file.take(256 * 1024)
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            if optiscaler_fg_enabled(&text) {
                return Err("A configuração do OptiScaler indica FrameGen ativado. Desative-o no OptiScaler antes de ativar MAKO.".into());
            }
        }
    }
    Ok(())
}
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}
fn toml(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
pub(super) fn config(s: &Settings) -> String {
    let mut out=format!("version = 2\n[global]\ndll = {}\nallow_fp16 = {}\n[[profile]]\nname = \"PeliGames\"\nmultiplier = {}\nflow_scale = {:.2}\nperformance_mode = {}\n",toml(&s.dll),s.allow_fp16,s.multiplier,s.flow_scale,s.performance);
    out.push_str(&format!("frame_generation_provisioned = true\nframe_generation_enabled = {}\nadaptive = {}\ntarget_fps = {}\nadaptive_max_multiplier = {}\nbase_fps_cap = {}\nframe_generation_refresh_threshold = {}\nultra_performance = {}\nscaling_enabled = {}\nscaling_method = {}\nscaling_factor = {:.2}\nscaling_sharpness = {:.2}\n",s.live_enabled,s.adaptive,s.target_fps,s.multiplier,s.base_fps_cap,s.refresh_threshold,s.ultra_performance,s.scaling_enabled,toml(&s.scaling_method),s.scaling_factor,s.scaling_sharpness));
    if !s.gpu.is_empty() { out.push_str(&format!("gpu = {}\n",toml(&s.gpu))); }
    out
}

pub(crate) fn conflicts(advanced: &AdvancedSettings) -> Result<(), String> {
    let profiles = root()?.join("profiles");
    for wrapper in advanced.wrappers.iter().filter(|w| w.enabled) {
        let script = Path::new(&wrapper.program);
        if script.parent() == Some(profiles.as_path()) {
            if let Ok(bytes) = fs::read(script.with_extension("json")) {
                if serde_json::from_slice::<Settings>(&bytes).is_ok_and(|s| s.backend != "mako") {
                    return Err("Este jogo usa uma integração de geração de quadros removida. Abra Geração de quadros e salve o MAKO, ou desative o vínculo antigo.".into());
                }
            }
        }
    }

    if advanced.options.get("hdr") == Some(&true) {
        let profiles = root()?.join("profiles");
        for wrapper in advanced.wrappers.iter().filter(|w| w.enabled) {
            let script = Path::new(&wrapper.program);
            if script.parent() == Some(profiles.as_path()) {
                if let Ok(bytes) = fs::read(script.with_extension("json")) {
                    if serde_json::from_slice::<Settings>(&bytes).is_ok_and(|s| s.backend == "mako")
                    {
                        return Err("A integração estável do MAKO usa SDR. Desative HDR antes de ativá-la; o HDR do MAKO ainda é experimental.".into());
                    }
                }
            }
        }
    }

    let mut custom = std::process::Command::new("__framegen_validation__");
    if let Some(line) = &advanced.launch_options {
        super::nexus_launch_options::apply_custom(&mut custom, line)?;
    }
    if custom.get_envs().any(|(key, value)| {
        [
            "PROTON_MLFG_UPGRADE",
            "LOW_LATENCY_LAYER",
            "OBS_VKCAPTURE",
            "PROTON_VKD3D_LOWLATENCY",
            "ENABLE_MAKO",
        ]
        .iter()
        .any(|k| key == std::ffi::OsStr::new(k))
            && value.is_some_and(|v| v == "1")
    }) {
        return Err("As opções de inicialização ativam outro gerador ou uma camada incompatível. Remova essa ativação antes de usar MAKO.".into());
    }

    if advanced.options.get("vkd3d_latency") == Some(&true)
        || advanced.options.get("vulkan_latency") == Some(&true)
    {
        return Err("Desative os modos de baixa latência DX12/Vulkan antes de usar este gerador de quadros.".into());
    }
    if advanced.options.get("obs_capture") == Some(&true) {
        return Err("Desative a captura Vulkan OBS antes de usar este gerador de quadros.".into());
    }
    if advanced.options.get("mlfg") == Some(&true) {
        return Err(
            "Desative a geração ML de quadros nas opções avançadas antes de ativar MAKO.".into(),
        );
    }
    for wrapper in advanced.wrappers.iter().filter(|w| w.enabled) {
        let name = Path::new(&wrapper.program)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if ["mako-launch", "mako-run", "lsfg", "lsfg-vk"].contains(&name.as_str()) {
            return Err(
                "Remova o outro wrapper de geração de quadros antes de ativar este perfil.".into(),
            );
        }
    }
    Ok(())
}
pub(super) fn write(path: &Path, bytes: &[u8], mode: u32) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::create_dir_all(path.parent().ok_or("Perfil inválido.")?).map_err(|e| e.to_string())?;
    let temp = path.with_extension(uuid::Uuid::new_v4().to_string());
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(mode)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}
fn wrapper_script(
    settings: &Settings,
    config_path: &Path,
    runtime_path: &Path,
    launcher_config: &Path,
) -> String {
    if settings.flatpak {
        return flatpak::wrapper(config_path);
    }
    let overlay = settings
        .overlay_enabled
        .then(|| super::frame_overlay_runtime::runner(config_path));
    format!("#!/bin/sh\n# PeliGames per-game MAKO profile; no global activation.\nexport MAKO_CONFIG={} MAKO_PROFILE=PeliGames MAKO_LAUNCH_CONFIG={}\nexport DISABLE_LSFG=1 DISABLE_LSFGVK=1 DISABLE_MAKO=0 ENABLE_MAKO=1 MAKO_ALLOW_COMPETING_LAYERS=0\nunset MAKO_ENV\n{}",quote(&config_path.to_string_lossy()),quote(&launcher_config.to_string_lossy()),mangohud::launch(runtime_path, config_path, overlay.as_deref()))
}
/// Upgrade existing native wrappers in place, keeping Steam's pelirun command
/// stable. Monitoring activation belongs to this invocation, not the FG profile.
pub(crate) fn configure_monitoring(
    command: &mut std::process::Command,
    advanced: &AdvancedSettings,
) -> Result<(), String> {
    command.env_remove(mangohud::ENABLE_ENV);
    let enabled = ["mangohud", "mangohud_env"]
        .iter()
        .any(|key| advanced.options.get(*key) == Some(&true));
    if enabled && advanced.afmf.as_ref().is_some_and(|afmf| afmf.enabled) {
        let directory = root()?.join("layers/afmf-mangohud");
        mangohud::prepare_afmf(&directory)?;
        crate::core::afmf_compatibility::monitoring(command, &directory)?;
    }
    let profiles = root()?.join("profiles");
    for wrapper in advanced.wrappers.iter().filter(|w| w.enabled) {
        let path = Path::new(&wrapper.program);
        if path.parent() != Some(profiles.as_path())
            || path.extension().is_none_or(|ext| ext != "sh")
            || !wrapper.arguments.is_empty()
        {
            continue;
        }
        let Some(mut settings) = fs::read(path.with_extension("json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
            .filter(|s| s.backend == "mako" && s.enabled && !s.flatpak)
        else {
            continue;
        };
        let current = fs::read_to_string(path).map_err(|e| e.to_string())?;
        if !current
            .starts_with("#!/bin/sh\n# PeliGames per-game MAKO profile; no global activation.\n")
        {
            continue;
        }
        // Explicit general-overlay settings supersede the legacy MAKO toggle,
        // including when only monitoring was edited on an existing profile.
        if advanced.overlay_enabled.is_some() {
            settings.overlay_enabled = false;
        }
        let config = path.with_extension("toml");
        if settings.overlay_enabled {
            super::frame_overlay_runtime::install(&config)?;
        }
        if enabled {
            mangohud::prepare(&config)?;
        }
        let version = if settings.runtime_version.is_empty() {
            release(&settings.backend)?.0
        } else {
            &settings.runtime_version
        };
        let runtime = versions::location(&settings.backend, version)?;
        let updated = wrapper_script(
            &settings,
            &config,
            &runtime,
            &path.with_extension("launch.conf"),
        );
        if current != updated {
            write(path, updated.as_bytes(), 0o700)?;
        }
        if enabled {
            command.env(mangohud::ENABLE_ENV, "1");
        }
    }
    Ok(())
}
// Only launch identity changes need another wrapper (and Steam launch options).
// Live controls, overlay opt-in and acknowledgements update the same wrapper.
fn same_launch_profile(previous: &Settings, next: &Settings) -> bool {
    previous.backend == next.backend
        && previous.runtime_version == next.runtime_version
        && previous.dll == next.dll
        && previous.enabled == next.enabled
        && previous.flatpak == next.flatpak
        && previous.gpu == next.gpu && previous.allow_fp16 == next.allow_fp16
        && previous.ultra_performance == next.ultra_performance
        && previous.scaling_enabled == next.scaling_enabled
        && previous.scaling_method == next.scaling_method
        && previous.scaling_factor == next.scaling_factor
        && previous.scaling_sharpness == next.scaling_sharpness
}

/// Edits only reloadable keys, retaining pause state and startup configuration.
fn live_config(text: &str, settings: &Settings) -> Result<String, String> {
    let values = [
        ("multiplier", settings.multiplier.to_string()),
        ("adaptive_max_multiplier", settings.multiplier.to_string()),
        ("target_fps", settings.target_fps.to_string()),
        ("flow_scale", format!("{:.2}", settings.flow_scale)),
        ("performance_mode", settings.performance.to_string()),
        ("adaptive", settings.adaptive.to_string()),
    ];
    let mut counts = [0; 6];
    let mut profile = false;
    let mut output = String::new();
    for line in text.lines() {
        if line.trim().starts_with('[') { profile = line.trim() == "[[profile]]"; }
        let key = line.split_once('=').map(|(key, _)| key.trim());
        if let Some((index, (key, value))) = values.iter().enumerate().find(|(_, (name, _))| profile && Some(*name) == key) {
            counts[index] += 1;
            output.push_str(&format!("{key} = {value}\n"));
        } else { output.push_str(line); output.push('\n'); }
    }
    if counts != [1; 6] { return Err("O perfil MAKO não tem um formato de recarga reconhecido. As configurações foram preservadas.".into()); }
    for (key,value) in [("base_fps_cap",settings.base_fps_cap), ("frame_generation_refresh_threshold",settings.refresh_threshold)] {
        let mut edited = String::new(); let mut in_profile=false; let mut found=false;
        for line in output.lines() {
            if line.trim().starts_with('[') {
                if in_profile && !found { edited.push_str(&format!("{key} = {value}\n")); found=true; }
                in_profile = line.trim() == "[[profile]]";
            }
            if in_profile && line.split_once('=').is_some_and(|(name,_)| name.trim()==key) {
                if found { return Err("Chave MAKO duplicada; perfil preservado.".into()); }
                edited.push_str(&format!("{key} = {value}\n")); found=true;
            } else { edited.push_str(line); edited.push('\n'); }
        }
        if !found && in_profile { edited.push_str(&format!("{key} = {value}\n")); }
        output=edited;
    }
    Ok(output)
}

#[tauri::command]
pub async fn update_frame_generation_live(active_profile: String, settings: Settings) -> Result<(), String> {
    crate::core::release_features::require()?;
    tauri::async_runtime::spawn_blocking(move || {
        if INSTALLING.swap(true,Ordering::AcqRel) { return Err("Aguarde a operação de geração de quadros em andamento.".into()); }
        struct Guard; impl Drop for Guard { fn drop(&mut self) { INSTALLING.store(false,Ordering::Release); } } let _guard=Guard;
        check_settings(&settings)?;
        let script = Path::new(&active_profile);
        let profiles = root()?.join("profiles");
        if script.parent() != Some(profiles.as_path()) || script.extension().and_then(|v|v.to_str()) != Some("sh")
            || !script.file_stem().and_then(|v|v.to_str()).is_some_and(profile_names::valid) {
            return Err("Selecione um perfil de execução MAKO do PeliGames.".into());
        }
        let path=script.with_extension("json");
        for path in [&path,&script.with_extension("toml")] {
            use std::os::unix::fs::MetadataExt;
            let metadata=fs::symlink_metadata(path).map_err(|e|e.to_string())?;
            if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.uid()!=unsafe{libc::geteuid()} || metadata.mode()&0o022!=0 || metadata.len()>65536 {
                return Err("Perfil MAKO sem propriedade ou permissões válidas.".into());
            }
        }
        let mut previous: Settings=serde_json::from_slice(&fs::read(&path).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        if !previous.enabled || previous.backend!="mako" { return Err("O perfil precisa ter MAKO habilitado antes de iniciar o jogo.".into()); }
        // Ignore all non-live fields supplied by the UI. They cannot change the
        // DLL, runner, startup wrapper, overlay opt-in, or another launcher's state.
        let switch_changed=previous.live_enabled!=settings.live_enabled;
        previous.live_enabled=settings.live_enabled;
        previous.base_fps_cap=settings.base_fps_cap; previous.refresh_threshold=settings.refresh_threshold;
        previous.multiplier=settings.multiplier; previous.adaptive=settings.adaptive;
        previous.target_fps=settings.target_fps; previous.flow_scale=settings.flow_scale; previous.performance=settings.performance;
        let toml=script.with_extension("toml");
        let old=fs::read_to_string(&toml).map_err(|e|e.to_string())?;
        let mut updated=live_config(&old,&previous)?;
        if switch_changed {
            updated=updated.lines().map(|line| if line.split_once('=').is_some_and(|(key,_)|key.trim()=="frame_generation_enabled") { format!("frame_generation_enabled = {}",previous.live_enabled) } else { line.to_owned() }).collect::<Vec<_>>().join("\n")+"\n";
        }
        let paused=updated.lines().any(|line|line.split_once('=').is_some_and(|(key,value)|key.trim()=="frame_generation_enabled"&&value.trim()=="false"));
        write(&path,&serde_json::to_vec(&previous).map_err(|e|e.to_string())?,0o600)?;
        write(&toml,updated.as_bytes(),0o600)?;
        write(&script.with_extension("live.json"),&serde_json::to_vec(&serde_json::json!({
            "multiplier":previous.multiplier,"adaptive":previous.adaptive,"target_fps":previous.target_fps,
            "flow_scale":previous.flow_scale,"performance":previous.performance,"paused":paused,
            "base_fps_cap":previous.base_fps_cap,"refresh_threshold":previous.refresh_threshold,
            "scaling_enabled":previous.scaling_enabled,"scaling_method":previous.scaling_method,"scaling_sharpness":previous.scaling_sharpness,"ultra_performance":previous.ultra_performance
        })).map_err(|e|e.to_string())?,0o600)
    }).await.map_err(|e|e.to_string())?
}
fn reusable_profile(
    advanced: &AdvancedSettings,
    settings: &Settings,
    profiles: &Path,
) -> Option<PathBuf> {
    if !settings.enabled {
        return None;
    }
    advanced
        .wrappers
        .iter()
        .filter(|w| w.enabled && w.arguments.is_empty())
        .find_map(|w| {
            let script = Path::new(&w.program);
            let stem = script.file_stem()?.to_str()?;
            if script.parent()? != profiles
                || script.extension()?.to_str()? != "sh"
                || !profile_names::valid(stem)
                || !script.is_file()
            {
                return None;
            }
            let previous: Settings =
                serde_json::from_slice(&fs::read(script.with_extension("json")).ok()?).ok()?;
            same_launch_profile(&previous, settings).then(|| script.with_extension(""))
        })
}
#[tauri::command]
pub async fn prepare_frame_generation(
    game_key: String,
    game_name: Option<String>,
    directory: String,
    mut settings: Settings,
    advanced: AdvancedSettings,
    flatpak: bool,
    executable: Option<String>,
) -> Result<LaunchWrapper, String> {
    crate::core::release_features::require()?;
    tauri::async_runtime::spawn_blocking(move||{
 if INSTALLING.swap(true,Ordering::AcqRel){return Err("Aguarde a instalação ou limpeza do runtime em andamento.".into())}struct Guard;impl Drop for Guard{fn drop(&mut self){INSTALLING.store(false,Ordering::Release);}}let _guard=Guard;
 if advanced.overlay_enabled.is_some(){settings.overlay_enabled=false;}
 settings.flatpak=flatpak;check_settings(&settings)?;if settings.runtime_version.is_empty(){settings.runtime_version=versions::current(&settings.backend)?;}if !versions::compatible(&settings.backend,&settings.runtime_version){return Err("Esta versão de runtime não é compatível com a integração atual.".into())}let runtime_path=versions::location(&settings.backend,&settings.runtime_version)?;let directory=Path::new(&directory).canonicalize().map_err(|_|"A pasta do jogo não está disponível. Verifique o disco e a montagem.")?;if !directory.is_dir(){return Err("Pasta de jogo inválida.".into())}
 if settings.enabled{if settings.backend=="mako" && advanced.options.get("hdr")==Some(&true){return Err("A integração estável do MAKO usa SDR. Desative HDR; o suporte HDR do MAKO ainda é experimental.".into())}check_local_fg(&directory)?;let mut checked=advanced.clone();checked.wrappers.retain(|w|!Path::new(&w.program).parent().is_some_and(|p|root().is_ok_and(|r|p==r.join("profiles"))));conflicts(&checked)?;if !flatpak{validate_runtime_at(&runtime_path,&settings.backend)?;}
 if settings.dll.is_empty(){settings.dll=detect_dll(&settings.backend).ok_or("Instale o Lossless Scaling pela Steam ou localize sua DLL.")?}
 let dll=Path::new(&settings.dll);let expected=vec!["Lossless.dll"];if !dll.is_absolute()||!dll_valid(dll)||!expected.iter().any(|n|dll.file_name().is_some_and(|f|f.to_string_lossy().eq_ignore_ascii_case(n))){return Err("Selecione a DLL válida da sua instalação do Lossless Scaling.".into())}settings.dll=dll.canonicalize().map_err(|e|e.to_string())?.to_string_lossy().into();if flatpak{let status=self::flatpak::inspect(&game_key,&settings.dll,&settings.runtime_version)?;if !status.ready{return Err(status.message)}}}
 let serialized=serde_json::to_vec(&settings).map_err(|e|e.to_string())?;
 let snapshot=profile_key(&format!("{game_key}:{}:{}",settings.runtime_version,String::from_utf8_lossy(&serialized)));
 let profiles=root()?.join("profiles");let existing=reusable_profile(&advanced,&settings,&profiles);let reused=existing.is_some();
 let base=match existing{Some(base)=>base,None=>profile_names::allocate(&profiles,game_name.as_deref().unwrap_or_else(||directory.file_name().and_then(|n|n.to_str()).unwrap_or("jogo")),&snapshot)?};let config_path=base.with_extension("toml");let script_path=base.with_extension("sh");
 let launcher_config=base.with_extension("launch.conf");if !reused{write(&launcher_config,b"version=1\ndisable_steam_overlay=0\n",0o600)?;}
 if settings.enabled && settings.overlay_enabled {super::frame_overlay_runtime::check_game(&directory,executable.as_deref(),flatpak)?;super::frame_overlay_runtime::install(&config_path)?;}
 let script=wrapper_script(&settings,&config_path,&runtime_path,&launcher_config);
 write(&config_path,config(&settings).as_bytes(),0o600)?;write(&script_path,script.as_bytes(),0o700)?;write(&base.with_extension("json"),&serialized,0o600)?;write(&selection_file(&root()?.join("profiles"),&game_key,game_name.as_deref().unwrap_or(&game_key))?,&serialized,0o600)?;
 Ok(LaunchWrapper{program:script_path.to_string_lossy().into(),arguments:String::new(),enabled:settings.enabled})
 }).await.map_err(|e|e.to_string())?
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Paths {
    root: String,
    executable: Option<String>,
    dlss_destination: Option<String>,
}
#[tauri::command]
pub async fn resolve_game_mod_paths(
    directory: String,
    executable: Option<String>,
    game_key: Option<String>,
    legacy_override: Option<String>,
) -> Result<Paths, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&directory)
            .canonicalize()
            .map_err(|_| "Pasta indisponível. Verifique o disco e a montagem.")?;
        let root = super::nexus_discovery::resolve(&root)
            .ok()
            .filter(|(_, module)| module.is_some())
            .map(|(root, _)| root)
            .unwrap_or(root);
        if let Some(key) = game_key {
            let file = module_file(&key,root.file_name().and_then(|v|v.to_str()).unwrap_or("game"))?;
            if let Ok(bytes) = fs::read(file) {
                if let Ok(saved) = serde_json::from_slice::<Paths>(&bytes) {
                    if Path::new(&saved.root) == root
                        && saved.dlss_destination.as_ref().is_some_and(|p| {
                            Path::new(p)
                                .canonicalize()
                                .is_ok_and(|p| p.starts_with(&root) && p.is_dir())
                        })
                    {
                        return Ok(saved);
                    }
                }
            }
        }
        if let Some(path) = legacy_override
            .and_then(|p| PathBuf::from(p).canonicalize().ok())
            .filter(|p| p.starts_with(&root) && p.is_dir())
        {
            return Ok(Paths {
                root: root.to_string_lossy().into(),
                executable: super::analyzer::scan_directory_for_exe(&path)
                    .map(|p| p.to_string_lossy().into()),
                dlss_destination: Some(path.to_string_lossy().into()),
            });
        }
        let exe = executable
            .map(PathBuf::from)
            .filter(|p| p.is_file() && p.starts_with(&root))
            .or_else(|| {
                crate::core::game_executable::best_executable_dir(&root, 5)
                    .and_then(|p| super::analyzer::scan_directory_for_exe(&p))
            });
        Ok(Paths {
            root: root.to_string_lossy().into(),
            dlss_destination: exe
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_string_lossy().into()),
            executable: exe.map(|p| p.to_string_lossy().into()),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn set_game_mod_destination(
    game_key: String,
    directory: String,
    destination: String,
) -> Result<Paths, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = Path::new(&directory)
            .canonicalize()
            .map_err(|_| "A pasta do jogo não está disponível.")?;
        let destination = Path::new(&destination)
            .canonicalize()
            .map_err(|_| "O destino não está disponível.")?;
        if !destination.is_dir() || !destination.starts_with(&root) {
            return Err("Selecione uma pasta dentro desta instalação do jogo.".into());
        }
        let paths = Paths {
            root: root.to_string_lossy().into(),
            executable: super::analyzer::scan_directory_for_exe(&destination)
                .map(|p| p.to_string_lossy().into()),
            dlss_destination: Some(destination.to_string_lossy().into()),
        };
        write(
            &module_file(&game_key,root.file_name().and_then(|v|v.to_str()).unwrap_or("game"))?,
            &serde_json::to_vec(&paths).map_err(|e| e.to_string())?,
            0o600,
        )?;
        Ok(paths)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use crate::core::test_support::RetryOutput;
    use super::*;
    #[test]
    fn live_edits_preserve_pause_dll_runtime_and_unknown_options() {
        let before = Settings { dll: "/games/Lossless.dll".into(), ..Default::default() };
        let text = config(&before).replace("frame_generation_enabled = true", "frame_generation_enabled = false") + "custom_option = 123\n";
        let next = Settings { multiplier: 4, adaptive: true, target_fps: 144, flow_scale: 0.5, performance: true, ..before };
        let updated = live_config(&text, &next).unwrap();
        for line in ["multiplier = 4", "adaptive_max_multiplier = 4", "target_fps = 144", "flow_scale = 0.50", "performance_mode = true", "adaptive = true", "frame_generation_enabled = false", "custom_option = 123", "dll = \"/games/Lossless.dll\""] { assert!(updated.lines().any(|v|v==line), "{line}"); }
        assert!(live_config(&(text.clone()+"multiplier = 2\n"),&next).is_err());
        assert!(live_config(&text.replace("target_fps = 60\n",""),&next).is_err());
    }

    #[test]
    fn additional_controls_validate_generate_and_preserve_launch_identity() {
        let base=Settings::default();
        let mut next=Settings { gpu:"0000:01:00.0".into(), base_fps_cap:60, refresh_threshold:90, allow_fp16:false, ultra_performance:true, live_enabled:false, scaling_enabled:true, scaling_method:"mako".into(), scaling_factor:1.5, scaling_sharpness:0.6, ..base.clone() };
        assert!(check_settings(&next).is_ok());
        let text=config(&next);
        for expected in ["allow_fp16 = false", "base_fps_cap = 60", "frame_generation_refresh_threshold = 90", "ultra_performance = true", "frame_generation_enabled = false", "scaling_method = \"mako\"", "scaling_factor = 1.50", "scaling_sharpness = 0.60", "gpu = \"0000:01:00.0\""] { assert!(text.contains(expected),"{expected}"); }
        assert!(!same_launch_profile(&base,&next));
        next=base.clone();next.base_fps_cap=60;next.refresh_threshold=144;next.live_enabled=false;
        assert!(same_launch_profile(&base,&next));
        next.scaling_factor=f32::NAN;assert!(check_settings(&next).is_err());
        next=base.clone();next.gpu="GPU 0\ninvalid".into();assert!(check_settings(&next).is_err());
        next=base.clone();next.scaling_method="unknown".into();assert!(check_settings(&next).is_err());
        next=base;next.base_fps_cap=1001;assert!(check_settings(&next).is_err());
    }
    #[test]
    fn additional_live_controls_upgrade_legacy_profiles_without_unpausing() {
        let settings=Settings {base_fps_cap:72,refresh_threshold:120,..Default::default()};
        let old=config(&Settings::default()).lines().filter(|line| !line.starts_with("base_fps_cap")&&!line.starts_with("frame_generation_refresh_threshold")).collect::<Vec<_>>().join("\n").replace("frame_generation_enabled = true","frame_generation_enabled = false");
        let text=live_config(&old,&settings).unwrap();
        assert!(text.contains("base_fps_cap = 72"));assert!(text.contains("frame_generation_refresh_threshold = 120"));assert!(text.contains("frame_generation_enabled = false"));
        assert_eq!(live_config(&text,&settings).unwrap(),text);
    }

    #[test]
    fn existing_profiles_do_not_enable_the_internal_layer() {
        let old: Settings = serde_json::from_str(r#"{"backend":"mako","enabled":true}"#).unwrap();
        assert!(!old.overlay_enabled);
        assert!(!Settings::default().overlay_enabled);
    }
    #[test]
    fn live_controls_reuse_launch_identity_but_runtime_changes_do_not() {
        let previous = Settings {
            enabled: true,
            runtime_version: "2.0.0".into(),
            dll: "/games/Lossless.dll".into(),
            ..Default::default()
        };
        let mut next = previous.clone();
        next.multiplier = 4;
        next.flow_scale = 0.5;
        next.performance = true;
        next.overlay_enabled = true;
        assert!(same_launch_profile(&previous, &next));
        next.runtime_version = "2.0.1".into();
        assert!(!same_launch_profile(&previous, &next));
        next = previous.clone();
        next.dll = "/other/Lossless.dll".into();
        assert!(!same_launch_profile(&previous, &next));
        next = previous.clone();
        next.backend = "removed".into();
        assert!(!same_launch_profile(&previous, &next));
        next = previous.clone();
        next.enabled = false;
        assert!(!same_launch_profile(&previous, &next));
    }
    #[test]
    fn wrappers_preserve_arguments_and_quote_paths() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "peligames-framegen-wrapper-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        let config = root.join("a ' quoted profile.toml");
        let runtime = root.join("runtime with spaces");
        let launcher = root.join("launcher.conf");
        let script = root.join("run.sh");
        fs::create_dir_all(runtime.join("bin")).unwrap();
        fs::write(runtime.join("bin/mako-launch"), b"#!/bin/sh\nexec \"$@\"\n").unwrap();
        fs::set_permissions(
            runtime.join("bin/mako-launch"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        fs::write(
            &script,
            wrapper_script(&Settings::default(), &config, &runtime, &launcher),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let result=std::process::Command::new(&script).args(["/usr/bin/python3","-c","import os,sys,json; print(json.dumps([sys.argv[1:],os.environ['MAKO_CONFIG'],os.environ['DISABLE_LSFGVK']]))","arg with spaces","$(do-not-execute)"]).output_retrying();
        assert!(result.status.success());
        let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            data[0],
            serde_json::json!(["arg with spaces", "$(do-not-execute)"])
        );
        assert_eq!(data[1], config.to_string_lossy().as_ref());
        assert_eq!(data[2], "1");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn manual_environment_cannot_bypass_conflict_checks() {
        let mut advanced = AdvancedSettings::default();
        advanced.launch_options = Some("PROTON_MLFG_UPGRADE=1 %command%".into());
        assert!(conflicts(&advanced).is_err());
        advanced.launch_options = Some("TEST=value %command%".into());
        assert!(conflicts(&advanced).is_ok());
    }
    #[test]
    #[ignore = "Requires official pinned runtime archives in an isolated /tmp fixture directory"]
    fn official_archives_and_generated_configs_validate_with_upstream_cli() {
        let fixtures = PathBuf::from(
            std::env::var("PELI_FRAMEGEN_FIXTURES").expect("fixture directory required"),
        );
        assert!(
            fixtures.parent() == Some(Path::new("/tmp"))
                && fixtures
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("peligames-framegen-")
        );
        let root = std::env::temp_dir().join(format!(
            "peligames-framegen-validation-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&root).unwrap();
        for backend in ["mako"] {
            let archive = fs::read(fixtures.join(format!("{backend}.tar.xz"))).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&archive)),
                release(backend).unwrap().2
            );
            let target = root.join(backend);
            fs::create_dir(&target).unwrap();
            unpack(&archive, &target).unwrap();
            validate_runtime_at(&target, backend).unwrap();
            let mut settings = Settings::default();
            settings.backend = backend.into();
            settings.adaptive = backend == "mako";
            let path = root.join(format!("{backend}.toml"));
            fs::write(&path, config(&settings)).unwrap();
            let binary = target.join("bin/mako-cli");
            let result = std::process::Command::new(binary)
                .args(["validate", "--config"])
                .arg(path)
                .output_retrying();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn optiscaler_checks_explicit_activation_without_mislabeling_upscaling_or_auto() {
        assert!(optiscaler_fg_enabled("[FrameGen]\nEnabled = true\n"));
        assert!(optiscaler_fg_enabled("[framegen]\nenabled=1 ; user choice"));
        assert!(!optiscaler_fg_enabled("[FrameGen]\nEnabled=auto"));
        assert!(!optiscaler_fg_enabled(
            "[Upscaling]\nEnabled=true\n[FrameGen]\nEnabled=false"
        ));
    }
    #[tokio::test]
    async fn resolves_supported_game_root_separately_from_shipping_destination() {
        let root =
            std::env::temp_dir().join(format!("peligames-framegen-paths-{}", uuid::Uuid::new_v4()));
        let binary = root.join("Pal/Binaries/Win64");
        fs::create_dir_all(&binary).unwrap();
        fs::write(root.join("Palworld.exe"), b"MZ").unwrap();
        fs::write(binary.join("Palworld-Win64-Shipping.exe"), b"MZ").unwrap();
        let paths = resolve_game_mod_paths(
            binary.to_string_lossy().into(),
            Some(
                binary
                    .join("Palworld-Win64-Shipping.exe")
                    .to_string_lossy()
                    .into(),
            ),
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(paths.root, root.to_string_lossy());
        assert_eq!(
            paths.dlss_destination.as_deref(),
            Some(binary.to_str().unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pins_only_stable_versions() {
        assert!(release("lsfg").is_err());
        assert_eq!(release("mako").unwrap().0, "4.5.0");
        assert!(release("nightly").is_err())
    }
    #[test]
    fn documented_mako_limits_and_sandbox_identity() {
        let mut s = Settings::default();
        for fps in [10, 30, 240, 1000] {
            s.target_fps = fps;
            assert!(check_settings(&s).is_ok());
        }
        for fps in [0, 9, 1001] {
            s.target_fps = fps;
            assert!(check_settings(&s).is_err());
        }
        s.target_fps = 60;
        s.multiplier = 6;
        assert!(check_settings(&s).is_err());
        let original = Settings::default();
        let mut sandbox = original.clone();
        sandbox.flatpak = true;
        assert!(!same_launch_profile(&original, &sandbox));
    }
    #[test]
    #[ignore = "Requires the official MAKO runtime installed locally; validates profiles without launching a game"]
    fn installed_mako_cli_accepts_profiles_and_launcher_environment() {
        let runtime =
            PathBuf::from(std::env::var("PELI_MAKO_RUNTIME").expect("MAKO runtime fixture"));
        validate_runtime_at(&runtime, "mako").unwrap();
        let dir =
            std::env::temp_dir().join(format!("peligames-mako-contract-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let cfg = dir.join("profile.toml");
        for adaptive in [false, true] {
            for target in [10, 60, 1000] {
                let settings = Settings {
                    enabled: true,
                    adaptive,
                    target_fps: target,
                    ..Default::default()
                };
                fs::write(&cfg, config(&settings)).unwrap();
                let result = std::process::Command::new(runtime.join("bin/mako-cli"))
                    .args(["validate", "--config"])
                    .arg(&cfg)
                    .output_retrying();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
            }
        }
        use std::os::unix::fs::PermissionsExt;
        let launch = dir.join("profile.sh");
        let launch_cfg = dir.join("launch.conf");
        fs::write(&launch_cfg, b"version=1\ndisable_steam_overlay=0\n").unwrap();
        fs::write(
            &launch,
            wrapper_script(&Settings::default(), &cfg, &runtime, &launch_cfg),
        )
        .unwrap();
        fs::set_permissions(&launch, fs::Permissions::from_mode(0o700)).unwrap();
        let result=std::process::Command::new(launch).args(["/usr/bin/python3","-c","import os,sys,json; print(json.dumps([os.environ['MAKO_CONFIG'],os.environ['MAKO_PROFILE'],os.environ['VK_IMPLICIT_LAYER_PATH'],sys.argv[1:]]))","arg with spaces"]).output_retrying();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(data[0], cfg.to_string_lossy().as_ref());
        assert_eq!(data[1], "PeliGames");
        assert_eq!(
            data[2],
            runtime
                .join("share/mako-render/vulkan/implicit_layer.d")
                .to_string_lossy()
                .as_ref()
        );
        assert_eq!(data[3], serde_json::json!(["arg with spaces"]));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn settings_validate_and_do_not_mix_backends() {
        let mut s = Settings::default();
        s.multiplier = 0;
        assert!(check_settings(&s).is_err());
        s = Settings::default();
        s.backend = "mako".into();
        s.dll = "/tmp/with \"quotes\"/Lossless.dll".into();
        let c = config(&s);
        assert!(c.contains("adaptive = false"));
        assert!(c.contains("\\\"quotes\\\""));
        let mut a = AdvancedSettings::default();
        a.options.insert("mlfg".into(), true);
        assert!(conflicts(&a).is_err());
        assert_ne!(profile_key("peligames:one"), profile_key("peligames:two"));
    }
    #[test]
    fn archives_reject_traversal_and_links() {
        let mut b = tar::Builder::new(Vec::new());
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Symlink);
        h.set_size(0);
        h.set_mode(0o777);
        h.set_cksum();
        b.append_link(&mut h, "bad", "/tmp").unwrap();
        let tar = b.into_inner().unwrap();
        let mut z = xz2::write::XzEncoder::new(Vec::new(), 1);
        z.write_all(&tar).unwrap();
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&root).unwrap();
        assert!(unpack(&z.finish().unwrap(), &root).is_err());
        fs::remove_dir(root).unwrap();
    }
}

/// No runtime files are installed into the game by MAKO. Leave shared runtimes intact.
pub(super) fn reset_selection(game_key:&str)->Result<(),String>{
    let profiles=root()?.join("profiles");
    let selection=selection_file(&profiles,game_key,game_key)?;
    if selection.exists(){write(&selection,&serde_json::to_vec_pretty(&Settings::default()).map_err(|e|e.to_string())?,0o600)?;}
    Ok(())
}

#[derive(Serialize)]
pub struct AfmfSupport { available: bool, reason: String }
/// Whether AFMF can load in this game: a 64-bit graphics process (a 32-bit
/// Windows game only with the new WoW64) outside a Flatpak sandbox.
#[tauri::command]
pub async fn afmf_support(directory: String, executable: Option<String>, flatpak: bool) -> Result<AfmfSupport, String> {
    crate::core::release_features::require()?;
    tauri::async_runtime::spawn_blocking(move || {
        let answer = |available: bool, reason: &str| AfmfSupport { available, reason: reason.into() };
        if flatpak {
            return Ok(answer(false, "O AFMF ainda não pode ser usado em jogos de launchers Flatpak: a camada fica numa pasta do PeliGames que o sandbox não acessa."));
        }
        let folder = Path::new(&directory).canonicalize().map_err(|_| "Pasta do jogo indisponível. Verifique a montagem do disco.".to_string())?;
        let exe = executable.map(PathBuf::from).and_then(|p| p.canonicalize().ok()).filter(|p| p.is_file() && p.starts_with(&folder))
            .or_else(|| crate::core::game_executable::best_executable_dir(&folder, 5).and_then(|d| super::analyzer::scan_directory_for_exe(&d)))
            .or_else(|| super::analyzer::scan_directory_for_exe(&folder));
        let windows = exe.as_deref().is_some_and(|p| { let mut magic = [0; 2]; fs::File::open(p).and_then(|mut f| f.read_exact(&mut magic)).is_ok() && magic == *b"MZ" });
        Ok(match exe.as_deref().and_then(super::analyzer::analyze_pe_header).as_deref() {
            Some("64-bits") => answer(true, "Compatível: processo gráfico de 64 bits. Precisa de Vulkan, diretamente ou pelo DXVK/VKD3D do Proton."),
            Some("32-bits") if windows => answer(true, "Jogo Windows de 32 bits: o AFMF só funciona com o novo WoW64 (Compatibilidade WoW64, na aba Avançado) e um Proton que o suporte."),
            Some("32-bits") => answer(false, "Jogos Linux de 32 bits não são suportados: o AFMF só é publicado para 64 bits."),
            _ => answer(true, "Não foi possível confirmar a arquitetura do executável. O AFMF exige um processo gráfico de 64 bits com Vulkan."),
        })
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod afmf_tests {
    use super::*;
    fn package(root: &Path, manifest: &str, elf: &[u8]) -> PathBuf {
        let stage = root.join("stage");
        let inner = stage.join("afmf-linux-1.3.1");
        fs::create_dir_all(&inner).unwrap();
        let mut library = elf.to_vec();
        library.resize(64, 0);
        fs::write(inner.join("libafmf-linux.so"), library).unwrap();
        fs::write(inner.join("afmf-linux.json"), manifest).unwrap();
        fs::write(inner.join("LICENSE"), "MIT").unwrap();
        stage
    }
    const MANIFEST: &str = r#"{"layer":{"name":"VK_LAYER_AFMF","library_path":"libafmf-linux.so","enable_environment":{"AFMF_ENABLE":"1"}}}"#;
    const ELF64: [u8; 20] = [0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 62, 0];
    #[test]
    fn the_package_folder_is_flattened_and_its_layer_checked() {
        let root = std::env::temp_dir().join(format!("peligames-afmf-{}", uuid::Uuid::new_v4()));
        let stage = package(&root, MANIFEST, &ELF64);
        assert!(validate_runtime_at(&stage, "afmf").is_err(), "files are still one folder down");
        flatten_single_root(&stage).unwrap();
        validate_runtime_at(&stage, "afmf").unwrap();
        assert!(stage.join("LICENSE").is_file() && !stage.join("afmf-linux-1.3.1").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn private_manifest_resolves_the_library_without_modifying_the_package() {
        let root=std::env::temp_dir().join(format!("peligames-afmf-manifest-{}",uuid::Uuid::new_v4()));
        let stage=package(&root,MANIFEST,&ELF64);flatten_single_root(&stage).unwrap();
        let before=fs::read(stage.join("afmf-linux.json")).unwrap();
        let directory=afmf_runtime_manifest(&stage).unwrap();
        let value:serde_json::Value=serde_json::from_slice(&fs::read(directory.join("afmf-linux.json")).unwrap()).unwrap();
        let library=PathBuf::from(value["layer"]["library_path"].as_str().unwrap());
        assert!(library.is_absolute()&&library.is_file());
        assert_eq!(value["layer"]["library_arch"],"64");
        assert_eq!(value["file_format_version"],"1.2.1");
        assert!(value["layer"].get("enable_environment").is_none());
        assert!(value["layer"].get("disable_environment").is_none());
        assert_eq!(fs::read(stage.join("afmf-linux.json")).unwrap(),before);
        validate_afmf_at(&stage).unwrap();
        assert_eq!(afmf_runtime_manifest(&stage).unwrap(),directory);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn a_layer_that_is_always_on_or_not_x86_64_is_refused() {
        let root = std::env::temp_dir().join(format!("peligames-afmf-{}", uuid::Uuid::new_v4()));
        let always_on = MANIFEST.replace(r#","enable_environment":{"AFMF_ENABLE":"1"}"#, "");
        let stage = package(&root, &always_on, &ELF64);
        flatten_single_root(&stage).unwrap();
        assert!(validate_runtime_at(&stage, "afmf").unwrap_err().contains("Manifesto"));
        fs::remove_dir_all(&root).unwrap();
        let mut arm = ELF64;
        arm[18] = 183;
        let stage = package(&root, MANIFEST, &arm);
        flatten_single_root(&stage).unwrap();
        assert!(validate_runtime_at(&stage, "afmf").unwrap_err().contains("x86_64"));
        fs::remove_dir_all(root).unwrap();
    }
}
