use crate::core::capture::{
    self, CaptureManager, CaptureOptions, CaptureStatus, CaptureSupport, Clip,
};
use std::sync::{Arc, OnceLock};
static BRIDGE: OnceLock<Result<crate::core::capture_overlay::Bridge, String>> = OnceLock::new();
fn notifier(app: tauri::AppHandle) -> Arc<dyn Fn(CaptureStatus) + Send + Sync> {
    Arc::new(move |status| {
        super::discord_presence::capture(&status);
        let _ = app.emit("peligames-capture", status);
    })
}
#[tauri::command]
pub fn capture_prepare_overlay(
    app: tauri::AppHandle,
    options: CaptureOptions,
) -> Result<(), String> {
    if options.game_key.is_empty() || options.game_name.is_empty() {
        return Err("Escolha um jogo para configurar a captura.".into());
    }
    capture::validate(&options)?;
    let status=manager().status();
    if status.phase!="idle" && status.game_key.as_deref()!=Some(options.game_key.as_str()) {
        return Err("Encerre a captura atual antes de vincular outro jogo.".into());
    }
    let root = crate::core::paths::app_root()?;
    let bindings=capture::context::Bindings::new(root.clone());
    BRIDGE.get_or_init(||crate::core::capture_overlay::Bridge::start(&root,Arc::new(move |command, peer| {
        bindings.reply(manager(),command,peer,notifier(app.clone()),|selected| {
            let _=app.emit("capture-selection-changed",serde_json::json!({"game_key":selected.game_key,"selection":selected.selection}));
        },encoder_info)
    }))).as_ref().map_err(Clone::clone)?;
    capture::save_settings(&options)?;
    Ok(())
}

use tauri::Emitter;
fn manager() -> &'static Arc<CaptureManager> {
    static MANAGER: OnceLock<Arc<CaptureManager>> = OnceLock::new();
    MANAGER.get_or_init(|| Arc::new(CaptureManager::default()))
}
#[tauri::command]
pub async fn capture_support() -> Result<CaptureSupport, String> {
    tauri::async_runtime::spawn_blocking(capture::support)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn capture_status() -> CaptureStatus {
    manager().status()
}
#[tauri::command]
pub async fn capture_start(
    app: tauri::AppHandle,
    options: CaptureOptions,
    replay: bool,
) -> Result<CaptureStatus, String> {
    capture_prepare_overlay(app.clone(), options.clone())?;
    let manager = manager().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.start(
            options,
            replay,
            Arc::new(move |status| {
                super::discord_presence::capture(&status);
                let _ = app.emit("peligames-capture", status);
            }),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn capture_action(
    app: tauri::AppHandle,
    action: String,
) -> Result<CaptureStatus, String> {
    let manager = manager().clone();
    tauri::async_runtime::spawn_blocking(move || {
        manager.action(
            &action,
            Arc::new(move |status| {
                super::discord_presence::capture(&status);
                let _ = app.emit("peligames-capture", status);
            }),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn capture_clips(game_key: String, game_name: String) -> Result<Vec<Clip>, String> {
    tauri::async_runtime::spawn_blocking(move || capture::clips(&game_key, &game_name))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub fn capture_folder(game_key: String, game_name: String) -> Result<String, String> {
    Ok(capture::output_folder(&game_key, &game_name)?
        .to_string_lossy()
        .into_owned())
}
pub fn shutdown() {
    if let Some(Ok(bridge)) = BRIDGE.get() {
        bridge.shutdown();
    }
    manager().shutdown();
}

#[tauri::command]
pub async fn capture_runtime_status() -> Result<crate::core::capture_runtime::RuntimeStatus, String>
{
    tauri::async_runtime::spawn_blocking(crate::core::capture_runtime::status)
        .await
        .map_err(|e| e.to_string())?
}
static INSTALLING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
struct InstallGuard;
impl Drop for InstallGuard {
    fn drop(&mut self) {
        INSTALLING.store(false, std::sync::atomic::Ordering::Release);
    }
}
#[tauri::command]
pub async fn capture_runtime_install(
    app: tauri::AppHandle,
) -> Result<crate::core::capture_runtime::RuntimeStatus, String> {
    use crate::core::capture_runtime as runtime;
    if INSTALLING.swap(true, std::sync::atomic::Ordering::AcqRel) {
        return Err("Já existe uma instalação de captura em andamento.".into());
    }
    let guard = InstallGuard;
    if manager().status().phase != "idle" {
        return Err("Encerre a captura antes de instalar ou reparar o componente.".into());
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(runtime::URL)
        .send()
        .await
        .map_err(|e| format!("Falha no download do gravador: {e}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let total = response.content_length();
    let limit = 80 * 1024 * 1024;
    if total.is_some_and(|size| size > limit) {
        return Err("Pacote de captura excede o tamanho permitido.".into());
    }
    let mut bytes = Vec::new();
    let mut last = std::time::Instant::now();
    let _ = app.emit(
        "capture-runtime-progress",
        runtime::Progress {
            phase: "downloading",
            downloaded: 0,
            total,
        },
    );
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > limit as usize {
            return Err("Pacote de captura excede o tamanho permitido.".into());
        }
        bytes.extend_from_slice(&chunk);
        if last.elapsed() > std::time::Duration::from_millis(150) {
            let _ = app.emit(
                "capture-runtime-progress",
                runtime::Progress {
                    phase: "downloading",
                    downloaded: bytes.len() as u64,
                    total,
                },
            );
            last = std::time::Instant::now();
        }
    }
    let _ = app.emit(
        "capture-runtime-progress",
        runtime::Progress {
            phase: "verifying",
            downloaded: bytes.len() as u64,
            total,
        },
    );
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        runtime::install_bytes(
            &bytes,
            |progress| {
                let _ = app.emit("capture-runtime-progress", progress);
            },
            || manager().status().phase != "idle",
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn capture_runtime_remove() -> Result<crate::core::capture_runtime::RuntimeStatus, String>
{
    if INSTALLING.load(std::sync::atomic::Ordering::Acquire) {
        return Err("Aguarde a instalação em andamento.".into());
    }
    tauri::async_runtime::spawn_blocking(|| {
        crate::core::capture_runtime::remove(|| manager().status().phase != "idle")
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn capture_audio_sources() -> Result<capture::selection::AudioSources, String> {
    tauri::async_runtime::spawn_blocking(capture::audio_sources)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn capture_settings(game_key:String)->Result<Option<CaptureOptions>,String>{
    capture::saved_settings(&game_key)
}
#[tauri::command]
pub fn capture_selection(game_key:String)->Option<capture::selection::Selection>{capture::saved_settings(&game_key).ok().flatten().map(|options|options.selection)}

fn encoder_info()->Result<capture::selection::EncoderSupport,String>{
 let mut info=capture::encoder_support()?;
 if let Ok(gpus)=super::gpu::detect(){if let Some(gpu)=gpus.iter().find(|gpu|gpu.pci_address==info.pci_address){info.gpu=gpu.model.clone();}}
 Ok(info)
}
#[tauri::command]
pub async fn capture_encoder_support()->Result<capture::selection::EncoderSupport,String>{tauri::async_runtime::spawn_blocking(encoder_info).await.map_err(|e|e.to_string())?}

#[tauri::command]
pub fn capture_shortcut_preferences()->Result<capture::shortcuts::Preferences,String>{capture::shortcut_preferences()}
pub(super) fn check_shortcut_conflict(value:&str)->Result<(),String>{
    if capture::shortcut_preferences()?.conflict(value).is_some(){return Err("Essa combinação já está atribuída à captura de vídeo do PeliGames.".into());}Ok(())
}
#[tauri::command]
pub fn configure_capture_shortcuts(preferences:capture::shortcuts::Preferences)->Result<(),String>{
    preferences.validate()?;
    let overlay=super::frame_overlay_shortcut::frame_overlay_preferences().preferences.shortcut;
    if preferences.conflict(&overlay).is_some(){return Err("Essa combinação já abre o Overlay PeliGames.".into());}
    for (name,value) in super::mangohud::load_mangohud_profile()?.settings {
        if (name.starts_with("toggle_")||name=="reset_fps_metrics") && value.as_str().is_some_and(|v|preferences.conflict(v).is_some()){return Err(format!("Essa combinação já está atribuída ao MangoHUD ({name})."));}
    }
    super::frame_generation::write(&crate::core::paths::app_root()?.join("capture-shortcuts.json"),&serde_json::to_vec(&preferences).map_err(|e|e.to_string())?,0o600)
}

#[tauri::command]
pub async fn capture_storage_preferences()->Result<capture::storage::StorageInfo,String>{
    tauri::async_runtime::spawn_blocking(capture::storage::info).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn configure_capture_storage(app:tauri::AppHandle,directory:Option<String>)->Result<capture::storage::StorageInfo,String>{
    let result=tauri::async_runtime::spawn_blocking(move||manager().configure_storage(directory)).await.map_err(|e|e.to_string())??;
    let _=app.emit("capture-storage-changed", &result);Ok(result)
}

#[tauri::command]
pub async fn capture_clip_details(app:tauri::AppHandle,game_key:String,game_name:String,path:String)->Result<capture::library::Details,String>{
    use tauri::Manager;
    let generate=manager().status().phase=="idle";
    let details=tauri::async_runtime::spawn_blocking(move||capture::library::details(&capture::output_folder(&game_key,&game_name)?,std::path::Path::new(&path),generate)).await.map_err(|e|e.to_string())??;
    if let Some(path)=&details.thumbnail{app.asset_protocol_scope().allow_file(path).map_err(|e|e.to_string())?;}Ok(details)
}
#[tauri::command]
pub async fn capture_clip_title(app:tauri::AppHandle,game_key:String,game_name:String,path:String,title:String)->Result<capture::library::Renamed,String>{
    let result=tauri::async_runtime::spawn_blocking(move||manager().rename_clip(&game_key,&game_name,std::path::Path::new(&path),&title)).await.map_err(|e|e.to_string())??;
    notifier(app)(manager().status());Ok(result)
}
#[tauri::command]
pub async fn capture_clip_open(game_key:String,game_name:String,path:String)->Result<(),String>{
    tauri::async_runtime::spawn_blocking(move||{
        let file=capture::library::validate_file(&capture::output_folder(&game_key,&game_name)?,std::path::Path::new(&path))?;
        super::updater::backend_folder::open(&file)
    }).await.map_err(|e|e.to_string())?
}
