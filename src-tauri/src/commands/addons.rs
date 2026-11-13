//! Package management and lazy services. No optional service starts at startup.
#[path = "addons/catalog.rs"]
mod catalog;
#[path = "addons/local.rs"]
mod local;
use crate::core::paths::{app_path, layout};
use peligames_addon_kit::{
    catalog::Catalog,
    manifest::{Manifest, Target},
    package,
    registry::{self, SelectionSnapshot, State, Store},
    runtime::{self, ServiceHost},
};
use serde::Serialize;
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

static WRITE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static SERVICES: OnceLock<ServiceHost> = OnceLock::new();
type ChangeSink = Arc<dyn Fn(SelectionSnapshot) + Send + Sync>;
static CHANGED: OnceLock<ChangeSink> = OnceLock::new();
pub fn set_change_sink(sink: ChangeSink) {
    let _ = CHANGED.set(sink);
}
fn changed(state: &State) {
    if let Some(sink) = CHANGED.get() {
        sink(registry::active_snapshot(state));
    }
}
const HOST: &str = env!("CARGO_PKG_VERSION");
fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}
fn catalog() -> Result<Catalog, String> {
    catalog::load().map(|(catalog, _)| catalog)
}

fn store() -> Result<Store, String> {
    Ok(Store {
        packages: app_path(layout::ADDON_PACKAGES)?,
        state_file: app_path(layout::ADDON_STATE)?,
    })
}
async fn operation<T: Send + 'static>(
    f: impl FnOnce(Store) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let _lock = WRITE.lock().await;
    let store = store()?;
    tauri::async_runtime::spawn_blocking(move || {
        store.recover()?;
        f(store)
    })
    .await
    .map_err(|e| e.to_string())?
}
async fn mutation(
    f: impl FnOnce(Store) -> Result<State, String> + Send + 'static,
) -> Result<State, String> {
    let _lock = WRITE.lock().await;
    let store = store()?;
    let before_store = store.clone();
    let before = tauri::async_runtime::spawn_blocking(move || {
        before_store.recover()?;
        before_store.read()
    })
    .await
    .map_err(|e| e.to_string())??;
    if let Some(services) = SERVICES.get() {
        services
            .reconcile(&before)
            .await
            .map_err(|e| e.to_string())?;
    }
    let state = tauri::async_runtime::spawn_blocking(move || f(store))
        .await
        .map_err(|e| e.to_string())??;
    publish_state(state).await
}
async fn publish_state(state: State) -> Result<State, String> {
    let reconciled = if let Some(services) = SERVICES.get() {
        services.reconcile(&state).await.map_err(|e| e.to_string())
    } else {
        Ok(())
    };
    changed(&state);
    reconciled?;
    tauri::async_runtime::spawn_blocking(refresh_launch_profiles)
        .await
        .map_err(|error| error.to_string())??;
    Ok(state)
}
pub(crate) fn refresh_launch_profiles() -> Result<(), String> {
    #[cfg(test)]
    return Ok(());
    #[cfg(not(test))]
    {
        super::launch_profiles::refresh_all()?;
        super::steam_runtime::refresh_monitoring()
    }
}
#[derive(Serialize)]
pub struct Overview {
    catalog: Catalog,
    catalog_warning: bool,
    state: State,
    platform: String,
    host_version: &'static str,
    services: Vec<runtime::Status>,
    contributions: Vec<peligames_addon_kit::ui::UiContribution>,
    /// Remove each entry when its last bundled integration has been extracted.
    bundled_targets: Vec<Target>,
}
#[tauri::command]
pub async fn addon_store_state() -> Result<Overview, String> {
    operation(|store| {
        let state = store.read()?;
        let contributions = peligames_addon_kit::ui::contributions(&state, HOST, &platform());
        let (catalog, catalog_warning) = catalog::load()?;
        Ok(Overview {
            catalog,
            catalog_warning,
            state,
            platform: platform(),
            host_version: HOST,
            services: SERVICES
                .get()
                .map(ServiceHost::statuses)
                .unwrap_or_default(),
            contributions,
            bundled_targets: vec![
                Target::GameMods,
            ],
        })
    })
    .await
}
#[tauri::command]
pub async fn addon_refresh_catalog() -> Result<Overview, String> {
    let bytes = catalog::fetch().await?;
    operation(move |_| catalog::publish(&bytes)).await?;
    addon_store_state().await
}

#[derive(Serialize)]
pub struct ExecutableProbe {
    bits: Option<u8>,
    windows: bool,
}
#[tauri::command]
pub async fn addon_game_probe(executable: String) -> Result<ExecutableProbe, String> {
    tauri::async_runtime::spawn_blocking(move || {
        use std::io::Read;
        let path = std::path::Path::new(&executable);
        if !path.is_absolute() {
            return Err("An absolute executable path is required".into());
        }
        let bits = match super::analyzer::analyze_pe_header(path).as_deref() {
            Some("64-bits") => Some(64),
            Some("32-bits") => Some(32),
            _ => None,
        };
        let mut magic = [0; 2];
        let windows = std::fs::File::open(path)
            .and_then(|mut file| file.read_exact(&mut magic))
            .is_ok()
            && magic == *b"MZ";
        Ok(ExecutableProbe { bits, windows })
    })
    .await
    .map_err(|error| error.to_string())?
}
#[derive(Serialize)]
pub struct PlannedAddon {
    manifest: Manifest,
    download_bytes: u64,
}
#[tauri::command]
pub async fn addon_install_plan(ids: Vec<String>) -> Result<Vec<PlannedAddon>, String> {
    let catalog = catalog()?;
    let platform = platform();
    Ok(catalog
        .plan(&ids, HOST, &platform)?
        .iter()
        .map(|entry| PlannedAddon {
            manifest: entry.manifest.clone(),
            download_bytes: entry.artifacts[&platform].bytes,
        })
        .collect())
}
#[derive(Clone, Serialize)]
pub struct Progress {
    id: String,
    phase: &'static str,
    downloaded: u64,
    total: u64,
}
async fn download(
    artifact: &peligames_addon_kit::catalog::Artifact,
    id: &str,
    progress: &tauri::ipc::Channel<Progress>,
) -> Result<Vec<u8>, String> {
    artifact.validate()?;
    let client = reqwest::Client::builder()
        .user_agent("PeliGames")
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() >= 5
                || url.scheme() != "https"
                || !matches!(
                    url.host_str(),
                    Some(
                        "github.com"
                            | "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                    )
                )
            {
                attempt.error("Invalid add-on download redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(&artifact.url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    if response
        .content_length()
        .is_some_and(|n| n != artifact.bytes)
    {
        return Err("Incorrect add-on download size".into());
    }
    let mut bytes = Vec::new();
    let mut report_at = std::time::Instant::now();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() as u64 + chunk.len() as u64 > artifact.bytes {
            return Err("Add-on download exceeds declared size".into());
        }
        bytes.extend_from_slice(&chunk);
        if report_at.elapsed() >= Duration::from_millis(200) {
            let _ = progress.send(Progress {
                id: id.into(),
                phase: "downloading",
                downloaded: bytes.len() as u64,
                total: artifact.bytes,
            });
            report_at = std::time::Instant::now();
        }
    }
    Ok(bytes)
}
#[tauri::command]
pub async fn addon_install(
    ids: Vec<String>,
    on_progress: tauri::ipc::Channel<Progress>,
) -> Result<State, String> {
    let _lock = WRITE.lock().await;
    let catalog = catalog()?;
    let platform = platform();
    let plan = catalog.plan(&ids, HOST, &platform)?;
    let store = store()?;
    let recovery = Store {
        packages: store.packages.clone(),
        state_file: store.state_file.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || recovery.recover())
        .await
        .map_err(|e| e.to_string())??;
    let mut receipts = Vec::new();
    for entry in plan {
        let cache_root = store.packages.clone();
        let cache_entry = entry.clone();
        let cache_platform = platform.clone();
        if let Some(receipt) = tauri::async_runtime::spawn_blocking(move || {
            package::cached(&cache_root, &cache_entry, &cache_platform)
        })
        .await
        .map_err(|e| e.to_string())??
        {
            receipts.push(receipt);
            continue;
        }
        let artifact = &entry.artifacts[&platform];
        let _ = on_progress.send(Progress {
            id: entry.manifest.id.clone(),
            phase: "connecting",
            downloaded: 0,
            total: artifact.bytes,
        });
        let bytes = download(artifact, &entry.manifest.id, &on_progress).await?;
        let _ = on_progress.send(Progress {
            id: entry.manifest.id.clone(),
            phase: "verifying",
            downloaded: bytes.len() as u64,
            total: artifact.bytes,
        });
        let root = store.packages.clone();
        let entry = entry.clone();
        let target = platform.clone();
        receipts.push(
            tauri::async_runtime::spawn_blocking(move || {
                package::prepare(&root, &entry, &target, &bytes)
            })
            .await
            .map_err(|e| e.to_string())??,
        );
    }
    // No part of the selection is enabled until every dependency is prepared.
    let state =
        tauri::async_runtime::spawn_blocking(move || store.register(&receipts, HOST, &platform))
            .await
            .map_err(|e| e.to_string())??;
    publish_state(state).await
}
#[tauri::command]
pub async fn addon_local_plan(path: String) -> Result<local::Plan, String> {
    tauri::async_runtime::spawn_blocking(move || local::inspect(&path, HOST, &platform()))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn addon_import_local(path: String, sha256: String) -> Result<State, String> {
    mutation(move |store| {
        let bytes = local::read(&path)?;
        let receipt = package::prepare_local(&store.packages, &bytes, &sha256, HOST, &platform())?;
        store.register(&[receipt], HOST, &platform())
    })
    .await
}
#[tauri::command]
pub async fn addon_select_version(id: String, version: String) -> Result<State, String> {
    mutation(move |store| store.select(&id, &version, HOST, &platform())).await
}
#[tauri::command]
pub async fn addon_set_enabled(id: String, enabled: bool) -> Result<State, String> {
    mutation(move |store| store.enable(&id, enabled, HOST, &platform())).await
}
#[tauri::command]
pub async fn addon_remove_version(id: String, version: String) -> Result<State, String> {
    mutation(move |store| store.remove(&id, &version)).await
}
#[tauri::command]
pub async fn addon_complete_selection() -> Result<State, String> {
    mutation(|store| store.complete_selection()).await
}

#[tauri::command]
pub async fn addon_service_call(
    id: String,
    method: String,
    params: serde_json::Value,
    expected_selection: Option<SelectionSnapshot>,
) -> Result<serde_json::Value, runtime::Error> {
    let pending = {
        let _lock = WRITE.lock().await;
        let store = store().map_err(|_| {
            runtime::Error::new(
                runtime::ErrorKind::Verification,
                "Could not open the add-on store",
            )
        })?;
        let snapshot = store.clone();
        let state = tauri::async_runtime::spawn_blocking(move || snapshot.read())
            .await
            .map_err(|_| {
                runtime::Error::new(runtime::ErrorKind::Verification, "Add-on state task failed")
            })?
            .map_err(|_| {
                runtime::Error::new(
                    runtime::ErrorKind::Verification,
                    "Could not read the add-on store",
                )
            })?;
        if expected_selection
            .as_ref()
            .is_some_and(|expected| !registry::selection_current(&state, expected))
        {
            return Err(runtime::Error::new(
                runtime::ErrorKind::Stopped,
                "Add-on UI selection changed",
            ));
        }
        let selected = registry::resolve_selection(&state, &id, HOST, &platform())
            .map_err(|message| runtime::Error::new(runtime::ErrorKind::Verification, message))?;
        let manifest = selected.get(&id).ok_or_else(|| {
            runtime::Error::new(runtime::ErrorKind::Verification, "Missing add-on selection")
        })?;
        let storage = manifest
            .component_directory
            .clone()
            .unwrap_or_else(|| format!("{}/{id}", layout::ADDON_COMPONENTS));
        // The caller never chooses managed storage. Recipes retain existing component
        // directories, so installing an integration does not move live configuration.
        let storage = app_path(&storage)
            .map_err(|message| runtime::Error::new(runtime::ErrorKind::Verification, message))?;
        if method == "game.mutate" {
            if !manifest
                .capabilities
                .contains(&peligames_addon_kit::manifest::Capability::GameFiles)
            {
                return Err(runtime::Error::new(
                    runtime::ErrorKind::InvalidRequest,
                    "Game file access is not declared",
                ));
            }
            let directory = params
                .get("directory")
                .and_then(serde_json::Value::as_str)
                .filter(|value| std::path::Path::new(value).is_absolute())
                .ok_or_else(|| {
                    runtime::Error::new(
                        runtime::ErrorKind::InvalidRequest,
                        "Choose an absolute game directory",
                    )
                })?
                .to_owned();
            tauri::async_runtime::spawn_blocking(move || {
                crate::core::installed_library::ensure_path_writable(std::path::Path::new(
                    &directory,
                ))?;
                super::analyzer::invalidate_game(&directory);
                Ok::<(), String>(())
            })
            .await
            .map_err(|_| {
                runtime::Error::new(runtime::ErrorKind::Verification, "Game access task failed")
            })?
            .map_err(|message| runtime::Error::new(runtime::ErrorKind::InvalidRequest, message))?;
        }
        if manifest.category==Some(peligames_addon_kit::manifest::Category::Captures)&&manifest.capabilities.contains(&peligames_addon_kit::manifest::Capability::BackgroundService){super::capture::watch(id.clone());}
        SERVICES.get_or_init(ServiceHost::default).submit(
            store,
            &state,
            &id,
            &method,
            serde_json::json!({"host":{"component_directory":storage},"request":params}),
            HOST,
            &platform(),
        )?
    };
    pending.wait().await
}
#[tauri::command]
pub async fn addon_service_stop(
    id: String,
    expected_selection: Option<SelectionSnapshot>,
) -> Result<(), runtime::Error> {
    let _lock = WRITE.lock().await;
    if let Some(services) = SERVICES.get() {
        let store=store().map_err(|e|runtime::Error::new(runtime::ErrorKind::Verification,e))?;
        let state=store.read().map_err(|e|runtime::Error::new(runtime::ErrorKind::Verification,e))?;
        if state.addons.get(&id).filter(|entry|entry.enabled).and_then(|entry|entry.selected.as_ref().and_then(|v|entry.versions.get(v))).is_some_and(|entry|entry.manifest.capabilities.contains(&peligames_addon_kit::manifest::Capability::BackgroundService)){return Ok(());}
        if expected_selection
            .as_ref()
            .is_some_and(|expected| !services.uses_selection(&id, expected))
        {
            return Ok(());
        }
        services.stop(&id).await?;
    }
    Ok(())
}
#[tauri::command]
pub async fn addon_ui_contributions() -> Result<Vec<peligames_addon_kit::ui::UiContribution>, String>
{
    operation(|store| {
        Ok(peligames_addon_kit::ui::contributions(
            &store.read()?,
            HOST,
            &platform(),
        ))
    })
    .await
}
#[tauri::command]
pub async fn addon_ui_load(
    id: String,
    target: peligames_addon_kit::manifest::Target,
    route: String,
) -> Result<peligames_addon_kit::ui::UiBundle, String> {
    operation(move |store| {
        peligames_addon_kit::ui::load(&store, &id, target, &route, HOST, &platform())
    })
    .await
}
#[tauri::command]
pub fn addon_service_status() -> Vec<runtime::Status> {
    SERVICES
        .get()
        .map(ServiceHost::statuses)
        .unwrap_or_default()
}

/// Called by the native event loop, outside an async command, before it exits.
pub fn shutdown() {
    if let Some(services) = SERVICES.get() {
        if let Err(error) = tauri::async_runtime::block_on(services.shutdown()) {
            eprintln!("Add-on shutdown: {error}");
        }
    }
}

/// Reset an installed integration through its own service; the host never runs
/// an upstream mod installer or decides which of its files should be deleted.
pub(crate) async fn remove_game_group(
    group: &str,
    directory: &str,
    bits: &str,
) -> Result<(), String> {
    let group = group.to_owned();
    let ids = operation(move |store| {
        let state = store.read()?;
        Ok(state
            .addons
            .iter()
            .filter_map(|(id, addon)| {
                addon
                    .selected
                    .as_ref()
                    .and_then(|v| addon.versions.get(v))
                    .filter(|version| {
                        addon.enabled && version.manifest.group.as_deref() == Some(group.as_str())
                    })
                    .map(|_| id.clone())
            })
            .collect::<Vec<_>>())
    })
    .await?;
    if ids.is_empty() {
        return Err("Enable the game's add-on before removing its managed mod files".into());
    }
    for id in ids {
        let response = addon_service_call(
            id.clone(),
            "game.mutate".into(),
            serde_json::json!({"directory":directory,"bits":bits,"action":{"operation":"remove"}}),
            None,
        )
        .await
        .map_err(|e| e.to_string())?;
        let job = response
            .get("job")
            .and_then(serde_json::Value::as_u64)
            .ok_or("The integration did not return a removal job")?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
        loop {
            let status = addon_service_call(
                id.clone(),
                "job.inspect".into(),
                serde_json::json!({"id":job}),
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
            if status["id"].as_u64() != Some(job) {
                return Err("The integration operation changed".into());
            }
            if status["done"] == true {
                if let Some(error) = status["error"].as_str() {
                    return Err(error.into());
                }
                break;
            }
            if std::time::Instant::now() >= deadline {
                return Err("Integration removal timed out".into());
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn addon_game_refresh(directory: String) -> Result<(), String> {
    if !std::path::Path::new(&directory).is_absolute() {
        return Err("Choose an absolute game directory".into());
    }
    tauri::async_runtime::spawn_blocking(move || super::analyzer::invalidate_game(&directory))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn addon_machine_context()->Result<serde_json::Value,String>{
 let gpus=super::gpu::detect_linux_gpus().await?;
 let libraries=tauri::async_runtime::spawn_blocking(||dirs::home_dir().map(|home|super::scanner::steam_libraries(&home)).unwrap_or_default()).await.map_err(|e|e.to_string())?;
 Ok(serde_json::json!({"gpus":gpus,"steam_libraries":libraries}))
}

/// The host understands only the legacy launch envelope, not module settings.
#[tauri::command]
pub async fn addon_launch_settings(mut advanced:crate::core::advanced_settings::AdvancedSettings)->Result<peligames_addon_kit::launch::Selections,String>{
 crate::core::addon_launch::migrate(&mut advanced)?;Ok(advanced.addon_launch)
}

/// Capture callers resolve a capability/category, never a vendor-specific module.
pub(crate) async fn capture_provider()->Result<Option<String>,String>{operation(|store|{
 let state=store.read()?;let mut found=None;
 for(id,entry)in state.addons.iter().filter(|(_,entry)|entry.enabled){if let Some(manifest)=entry.selected.as_ref().and_then(|v|entry.versions.get(v)).map(|v|&v.manifest){if manifest.category==Some(peligames_addon_kit::manifest::Category::Captures)&&manifest.capabilities.contains(&peligames_addon_kit::manifest::Capability::BackgroundService){if found.is_some(){return Err("Enable only one capture provider".into());}found=Some(id.clone());}}}Ok(found)
}).await}

#[tauri::command]pub async fn addon_open_file(path:String)->Result<(),String>{let file=std::path::Path::new(&path);if !file.is_absolute()||!file.is_file(){return Err("Choose an existing absolute file path".into());}super::backend_folder::open(file)}


pub(crate) fn capture_shortcuts_enabled()->bool{
 store().and_then(|store|store.read()).is_ok_and(|state|state.addons.values().filter(|entry|entry.enabled).any(|entry|entry.selected.as_ref().and_then(|v|entry.versions.get(v)).is_some_and(|entry|entry.manifest.category==Some(peligames_addon_kit::manifest::Category::Captures)&&entry.manifest.capabilities.contains(&peligames_addon_kit::manifest::Capability::BackgroundService))))
}
