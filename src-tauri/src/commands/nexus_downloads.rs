use crate::core::nxm;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
#[derive(Clone, Serialize)]
pub struct Download {
    pub id: String,
    pub domain: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub game_id: Option<String>,
    pub name: String,
    pub status: String,
    pub received: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    pub requirements: Option<super::nexus_catalog::Requirements>,
    pub auto_install: bool,
    pub mod_entry_id: Option<String>,
}
struct Job {
    info: Download,
    request: Option<nxm::Request>,
    cancel: Arc<AtomicBool>,
    batch: Option<String>,
}
#[derive(Default)]
pub struct Downloads(Mutex<Vec<Job>>);
fn cancelled(flag: &AtomicBool) -> Result<(), String> {
    if flag.load(Ordering::Relaxed) {
        Err("Download cancelado.".into())
    } else {
        Ok(())
    }
}
async fn responsive<T>(
    future: impl std::future::Future<Output = Result<T, String>>,
    cancel: &AtomicBool,
) -> Result<T, String> {
    let mut future = Box::pin(future);
    loop {
        cancelled(cancel)?;
        if let Ok(result) = tokio::time::timeout(Duration::from_millis(100), &mut future).await {
            return result;
        }
    }
}
fn record_download(info: &Download) {
    if let Some(id)=&info.game_id {
        if let Ok(game)=super::nexus_local::diagnostic_game(id) {
            super::nexus_reports::event(&game,if info.status=="error" {"ERROR"} else {"INFO"},"download",&format!("{} | {}/mods/{} file={} | estado={} | bytes={} | {}",info.name,info.domain,info.mod_id,info.file_id,info.status,info.received,info.error.as_deref().unwrap_or("")));
        }
    }
}
fn update(app: &tauri::AppHandle, id: &str, action: impl FnOnce(&mut Download)) {
    let changed = if let Ok(mut jobs) = app.state::<Downloads>().0.lock() {
        jobs.iter_mut().find(|j|j.info.id==id).and_then(|job| {
            let previous=(job.info.status.clone(),job.info.error.clone());action(&mut job.info);
            (previous != (job.info.status.clone(),job.info.error.clone())).then(||job.info.clone())
        })
    } else {None};
    if let Some(info)=changed {
        record_download(&info);
        if matches!(info.status.as_str(), "imported" | "installed") && !app.state::<Downloads>().0.lock().map(|jobs| jobs.iter().any(|job| job.batch.is_some() && matches!(job.info.status.as_str(), "queued" | "authorizing" | "waiting"))).unwrap_or(true) {
            super::browser::focus_launcher(app);
        }
    }
}
pub fn receive(app: &tauri::AppHandle, value: &str) -> Result<(), String> {
    let result = receive_inner(app, value);
    if result.is_ok() {
        super::browser::focus_launcher(app);
    }
    if let Err(error) = &result {
        let state = app.state::<Downloads>();
        if let Ok(mut jobs) = state.0.lock() {
            if jobs.len() < 32 {
                jobs.push(Job {
                    info: Download {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain: String::new(),
                        mod_id: 0,
                        file_id: 0,
                        game_id: None,
                        name: "Nexus Mods".into(),
                        status: "error".into(),
                        received: 0,
                        total: None,
                        error: Some(error.clone()),
                        requirements: None,
                        auto_install: false,
                        mod_entry_id: None,
                    },
                    request: None,
                    cancel: Arc::new(AtomicBool::new(false)),
                    batch: None,
                });
            }
        }
        let _ = app.emit("nexus-download-request", "");
    }
    result
}
fn receive_inner(app: &tauri::AppHandle, value: &str) -> Result<(), String> {
    let request = nxm::parse(value)?;
    let workspace = super::nexus_local::load_nexus_workspace()?;
    let matches: Vec<_> = workspace
        .games
        .iter()
        .filter(|g| g.nexus_domain == request.domain)
        .collect();
    let game_id = if matches.len() == 1 {
        Some(matches[0].id.clone())
    } else {
        None
    };
    let id = uuid::Uuid::new_v4().to_string();
    let state = app.state::<Downloads>();
    let mut jobs = state.0.lock().map_err(|_| "Fila Nexus indisponível.")?;
    // Match only the exact file the user selected, preserving their chosen installation.
    if let Some(job) = jobs.iter_mut().find(|j| {
        j.info.status == "authorizing"
            && j.info.domain == request.domain
            && j.info.mod_id == request.mod_id
            && j.info.file_id == request.file_id
    }) {
        let id = job.info.id.clone();
        let game_id = job
            .info
            .game_id
            .clone()
            .ok_or("Instalação Nexus ausente.")?;
        let domain = request.domain.clone();
        job.request = Some(request);
        job.info.status = "waiting".into();
        drop(jobs);
        begin(app.clone(), id, game_id)?;
        let _ = app.emit("nexus-download-request", &domain);
        return Ok(());
    }
    if jobs.iter().any(|j| {
        j.info.domain == request.domain
            && j.info.mod_id == request.mod_id
            && j.info.file_id == request.file_id
            && [
                "preparing",
                "queued",
                "authorizing",
                "waiting",
                "downloading",
                "importing",
                "installing",
            ]
            .contains(&j.info.status.as_str())
    }) {
        return Ok(());
    }
    if jobs.len() >= 32 {
        prune_jobs(&mut jobs);
    }
    if jobs.len() >= 32 {
        return Err("Fila Nexus cheia. Aguarde os downloads atuais.".into());
    }
    let domain = request.domain.clone();
    jobs.push(Job {
        info: Download {
            id: id.clone(),
            domain: domain.clone(),
            mod_id: request.mod_id,
            file_id: request.file_id,
            game_id: game_id.clone(),
            name: format!("Mod {} · arquivo {}", request.mod_id, request.file_id),
            status: "waiting".into(),
            received: 0,
            total: None,
            error: None,
            requirements: None,
            auto_install: false,
            mod_entry_id: None,
        },
        request: Some(request),
        cancel: Arc::new(AtomicBool::new(false)),
        batch: None,
    });
    drop(jobs);
    let _ = app.emit("nexus-download-request", &domain);
    if let Some(game_id) = game_id {
        begin(app.clone(), id, game_id)?;
    }
    Ok(())
}
fn begin(app: tauri::AppHandle, id: String, game_id: String) -> Result<(), String> {
    let workspace = super::nexus_local::load_nexus_workspace()?;
    let game = workspace
        .games
        .iter()
        .find(|g| g.id == game_id)
        .ok_or("Adicione o jogo à biblioteca Nexus antes de baixar.")?;
    let state = app.state::<Downloads>();
    let mut jobs = state.0.lock().map_err(|_| "Fila Nexus indisponível.")?;
    let job = jobs
        .iter_mut()
        .find(|j| j.info.id == id)
        .ok_or("Download não encontrado.")?;
    if job.info.status != "waiting" {
        return Err("Este download já foi processado.".into());
    }
    if game.nexus_domain != job.info.domain {
        return Err("O arquivo pertence a outro jogo Nexus.".into());
    }
    let request = job
        .request
        .take()
        .ok_or("Autorização de download indisponível.")?;
    job.info.game_id = Some(game_id.clone());
    job.info.status = "downloading".into();
    let cancel = job.cancel.clone();
    let diagnostic = job.info.clone();
    drop(jobs);
    record_download(&diagnostic);
    let _ = app.emit("nexus-download-updated", ());
    super::nexus_browser::dismiss(&app, Some(&id));
    advance_after_capture(&app);
    tauri::async_runtime::spawn(async move {
        let result = transfer(&app, &id, &game_id, request, &cancel).await;
        update(&app, &id, |info| match result {
            Ok(()) => {
                info.status = if info.auto_install {
                    "installed"
                } else {
                    "imported"
                }
                .into()
            }
            Err(error) => {
                info.status = if cancel.load(Ordering::Relaxed) {
                    "cancelled"
                } else {
                    "error"
                }
                .into();
                info.error = Some(error);
            }
        });
        let _ = app.emit("nexus-download-updated", ());
    });
    Ok(())
}
// A private temporary directory is deleted on every error, cancellation and success.
struct Temporary(std::path::PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
async fn transfer(
    app: &tauri::AppHandle,
    id: &str,
    game_id: &str,
    request: nxm::Request,
    cancel: &AtomicBool,
) -> Result<(), String> {
    cancelled(cancel)?;
    request.check_expiry()?;
    let account = responsive(super::nexus_account::load_nexus_account(), cancel)
        .await?
        .ok_or("Conecte sua conta na aba Conta Nexus.")?;
    if request.user_id.is_some_and(|u| u != account.user_id) {
        return Err("Este link foi autorizado para outra conta Nexus. Use a mesma conta no navegador e no launcher.".into());
    }
    if request.key.is_none() && !account.is_premium {
        return Err("Na conta gratuita, clique em Mod Manager Download no site para autorizar este arquivo.".into());
    }
    let requirements = responsive(
        async { Ok(super::nexus_catalog::requirements(&request.domain, request.mod_id).await) },
        cancel,
    )
    .await?;
    update(app, id, |info| {
        info.requirements = Some(requirements.clone())
    });
    let file = responsive(
        super::nexus_account::metadata(&format!(
            "games/{}/mods/{}/files/{}.json",
            request.domain, request.mod_id, request.file_id
        )),
        cancel,
    )
    .await?;
    let filename = file["file_name"]
        .as_str()
        .ok_or("Nome de arquivo Nexus ausente.")?;
    let mod_info = responsive(super::nexus_account::metadata(&format!("games/{}/mods/{}.json", request.domain, request.mod_id)), cancel).await?;
    if filename.len() > 240
        || filename.contains(['/', '\\', '\0', '\n', '\r'])
        || !super::nexus_archive::supported_filename(filename)
    {
        return Err("Formato de arquivo de mod não suportado.".into());
    }
    const MAX: u64 = 2 * 1024 * 1024 * 1024;
    let expected_size = file["size_in_bytes"].as_u64().filter(|size| *size > 0);
    if expected_size.is_some_and(|size| size > MAX) {
        return Err("Arquivo excede o limite de 2 GiB por download.".into());
    }
    cancelled(cancel)?;
    request.check_expiry()?;
    let links = responsive(
        super::nexus_account::metadata(&request.download_endpoint()),
        cancel,
    )
    .await?;
    let url = links
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .filter_map(|i| i["URI"].as_str())
                .filter_map(|s| reqwest::Url::parse(s).ok())
                .find(nxm::trusted_download)
        })
        .ok_or("O Nexus não forneceu um endereço HTTPS de download compatível.")?;
    // This client has no API-key headers. Redirects remain on Nexus HTTPS hosts.
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(900))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !nxm::trusted_download(attempt.url()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Falha ao iniciar download.")?;
    let mut response = responsive(
        async {
            client
                .get(url)
                .send()
                .await
                .map_err(|_| "Falha ao conectar ao servidor de download Nexus.".to_string())
        },
        cancel,
    )
    .await?;
    if response.status().as_u16() != 200 {
        return Err("Download recusado pelo servidor. Gere um novo link pelo site.".into());
    }
    if response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|t| t.contains("text/html") || t.contains("application/json"))
    {
        return Err("O servidor retornou uma página em vez do arquivo.".into());
    }
    let total = response.content_length();
    if total.is_some_and(|n| n > MAX) {
        return Err("Arquivo excede o limite de 2 GiB por download.".into());
    }
    let game = super::nexus_local::diagnostic_game(game_id)?;
    let directory = super::nexus_profile::directory(&game)?.join("downloads").join(id);
    fs::create_dir_all(directory.parent().ok_or("Pasta de download inválida.")?)
        .map_err(|_| "Não foi possível criar a pasta de download.")?;
    fs::create_dir(&directory).map_err(|_| "Não foi possível criar o download temporário.")?;
    let temporary = Temporary(directory);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary.0, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Falha ao proteger o download.")?;
    }
    let path = temporary.0.join(filename);
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| "Não foi possível salvar o download.")?;
    update(app, id, |info| {
        info.name = filename.into();
        info.total = total;
    });
    let mut received = 0u64;
    let mut reported = Instant::now();
    while let Some(chunk) = responsive(
        async {
            response
                .chunk()
                .await
                .map_err(|_| "Download interrompido. Tente novamente pelo site.".to_string())
        },
        cancel,
    )
    .await?
    {
        cancelled(cancel)?;
        received += chunk.len() as u64;
        if received > MAX {
            return Err("Arquivo excede o limite de 2 GiB por download.".into());
        }
        output
            .write_all(&chunk)
            .map_err(|_| "Não foi possível salvar o arquivo. Verifique o espaço disponível.")?;
        if reported.elapsed() > Duration::from_millis(200) {
            update(app, id, |info| info.received = received);
            reported = Instant::now();
        }
    }
    if received == 0
        || total.is_some_and(|n| n != received)
        || expected_size.is_some_and(|n| n != received)
    {
        return Err("Download incompleto.".into());
    }
    output
        .sync_all()
        .map_err(|_| "Não foi possível concluir o arquivo.")?;
    drop(output);
    cancelled(cancel)?;
    {
        let state = app.state::<Downloads>();
        let mut jobs = state.0.lock().map_err(|_| "Fila Nexus indisponível.")?;
        cancelled(cancel)?;
        let job = jobs
            .iter_mut()
            .find(|j| j.info.id == id)
            .ok_or("Download não encontrado.")?;
        job.info.received = received;
        job.info.status = "importing".into();
    }
    let workspace = super::nexus_local::import_archive(
        game_id.into(),
        path.to_string_lossy().into(),
        Some(super::nexus_local::NexusSource {
            name: mod_info["name"].as_str().unwrap_or_default().into(),
            version: file["version"].as_str().unwrap_or_default().into(),
            domain: request.domain.clone(),
            mod_id: request.mod_id,
            file_id: request.file_id,
            requirements,
        }),
    )
    .await?;
    let mod_id = workspace
        .games
        .iter()
        .find(|g| g.id == game_id)
        .and_then(|g| g.mods.last())
        .ok_or("Mod importado não encontrado.")?
        .id
        .clone();
    let mut auto_install = false;
    update(app, id, |info| {
        info.mod_entry_id = Some(mod_id.clone());
        auto_install = info.auto_install;
    });
    if auto_install {
        let plan =
            super::nexus_local::plan_nexus_installation(game_id.into(), mod_id.clone(), None)
                .await?;
        if plan.fomod.is_some() {
            // Interactive installers stay imported until the user confirms the choices in Mods.
            update(app, id, |info| info.auto_install = false);
            return Ok(());
        }
        update(app, id, |info| info.status = "installing".into());
        super::nexus_local::install_nexus_mod(game_id.into(), mod_id, None)
            .await
            .map_err(|e| format!("Arquivo importado, mas a instalação falhou: {e}"))?;
    }
    Ok(())
}
#[tauri::command]
pub async fn request_nexus_dependency(
    app: tauri::AppHandle,
    game_id: String,
    game_domain: String,
    mod_id: u64,
    file_id: u64,
) -> Result<Option<String>, String> {
    prepare_download(app, game_id, game_domain, mod_id, file_id, None).await
}
async fn prepare_download(
    app: tauri::AppHandle,
    game_id: String,
    game_domain: String,
    mod_id: u64,
    file_id: u64,
    batch: Option<String>,
) -> Result<Option<String>, String> {
    super::nexus_catalog::domain(&game_domain)?;
    if mod_id == 0 || file_id == 0 {
        return Err("Arquivo Nexus inválido.".into());
    }
    let workspace = super::nexus_local::load_nexus_workspace()?;
    let game = workspace
        .games
        .iter()
        .find(|g| g.id == game_id)
        .ok_or("Jogo não encontrado.")?;
    if game.nexus_domain != game_domain {
        return Err("A dependência pertence a outro jogo Nexus.".into());
    }
    if game.running {
        return Err("Feche o jogo antes de instalar dependências.".into());
    }
    if batch.is_none() && game.adapter.is_empty() {
        return Err("Instalação automática indisponível para este jogo.".into());
    }
    if batch.is_none() && game.mods.iter().any(|m| {
        m.installed
            && m.enabled
            && m.nexus_source
                .as_ref()
                .is_some_and(|s| s.domain == game_domain && s.mod_id == mod_id)
    }) {
        return Err("Esta dependência já está instalada e ativa.".into());
    }
    let account = super::nexus_account::load_nexus_account()
        .await?
        .ok_or("Conecte sua conta Nexus.")?;
    let details = super::nexus_catalog::get_nexus_catalog_mod(game_domain.clone(), mod_id).await?;
    let file = details
        .files
        .iter()
        .find(|f| f.file_id == file_id)
        .ok_or("Arquivo não disponível para download.")?;
    if !super::nexus_archive::supported_filename(&file.file_name) {
        return Err("Este arquivo não é um pacote ou mod reconhecido.".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    {
        let state = app.state::<Downloads>();
        let mut jobs = state.0.lock().map_err(|_| "Fila Nexus indisponível.")?;
        if jobs.iter().any(|j| {
            j.info.domain == game_domain
                && j.info.mod_id == mod_id
                && j.info.file_id == file_id
                && [
                    "preparing",
                    "queued",
                    "authorizing",
                    "waiting",
                    "downloading",
                    "importing",
                    "installing",
                ]
                .contains(&j.info.status.as_str())
        }) {
            return Err("Esta dependência já está na fila de instalação.".into());
        }
        if jobs.len() >= 32 {
            prune_jobs(&mut jobs);
        }
        if jobs.len() >= 32 {
            return Err("Fila Nexus cheia.".into());
        }
        jobs.push(Job {
            info: Download {
                id: id.clone(),
                domain: game_domain.clone(),
                mod_id,
                file_id,
                game_id: Some(game_id.clone()),
                name: file.name.clone(),
                status: if batch.is_some() {
                    "preparing"
                } else if account.is_premium {
                    "waiting"
                } else {
                    "authorizing"
                }
                .into(),
                received: 0,
                total: None,
                error: None,
                requirements: Some(details.requirements),
                auto_install: batch.is_none(),
                mod_entry_id: None,
            },
            request: if account.is_premium {
                Some(nxm::parse(&format!(
                    "nxm://{game_domain}/mods/{mod_id}/files/{file_id}"
                ))?)
            } else {
                None
            },
            cancel: Arc::new(AtomicBool::new(false)),
            batch: batch.clone(),
        });
    }
    let _ = app.emit("nexus-download-updated", ());
    if batch.is_some() {
        Ok(None)
    } else if account.is_premium {
        begin(app, id, game_id)?;
        Ok(None)
    } else {
        let result = super::nexus_browser::open(app.clone(), format!("https://www.nexusmods.com/{game_domain}/mods/{mod_id}?tab=files&file_id={file_id}&nmm=1"), Some(id.clone())).await;
        if let Err(error) = &result { update(&app, &id, |info| { info.status = "error".into(); info.error = Some(error.clone()); }); }
        result.map(|_| None)
    }
}
#[derive(Deserialize)]
pub struct QueueFile {
    domain: String,
    mod_id: u64,
    file_id: u64,
}
#[tauri::command]
pub async fn queue_nexus_downloads(
    app: tauri::AppHandle,
    game_id: String,
    files: Vec<QueueFile>,
) -> Result<(), String> {
    if files.is_empty() || files.len() > 16 {
        return Err("Selecione de 1 a 16 mods por fila.".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    if files
        .iter()
        .any(|f| !seen.insert((f.domain.clone(), f.mod_id, f.file_id)))
    {
        return Err("Arquivo duplicado na seleção.".into());
    }
    super::nexus_account::load_nexus_account()
        .await?
        .ok_or("Conecte sua conta Nexus.")?;
    let batch = uuid::Uuid::new_v4().to_string();
    for file in files {
        if let Err(error) = prepare_download(
            app.clone(),
            game_id.clone(),
            file.domain,
            file.mod_id,
            file.file_id,
            Some(batch.clone()),
        )
        .await
        {
            if let Ok(mut jobs) = app.state::<Downloads>().0.lock() {
                jobs.retain(|j| j.batch.as_deref() != Some(&batch));
            }
            let _ = app.emit("nexus-download-updated", ());
            return Err(error);
        }
    }
    {
        let state = app.state::<Downloads>();
        let mut jobs = state.0.lock().map_err(|_| "Fila Nexus indisponível.")?;
        for job in jobs
            .iter_mut()
            .filter(|j| j.batch.as_deref() == Some(&batch))
        {
            job.info.status = "queued".into();
        }
    }
    advance_batch(&app);
    let _ = app.emit("nexus-download-updated", ());
    Ok(())
}
fn next_batch_index(jobs: &[Job]) -> Option<usize> {
    // Only the outstanding browser authorization blocks the next page.
    // Captured transfers continue in the background.
    if jobs.iter().any(|job| job.batch.is_some() && matches!(job.info.status.as_str(), "authorizing" | "waiting")) { return None; }
    jobs.iter().position(|job| job.info.status == "queued" && !jobs.iter().any(|previous| previous.batch == job.batch && previous.info.status == "error"))
}
fn advance_after_capture(app: &tauri::AppHandle) {
    super::browser::focus_launcher(app);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // Let the launcher display the captured/cancelled item before activating
        // the next browser page. The queue lock prevents duplicate page opens.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        advance_batch(&handle);
    });
}
fn prune_jobs(jobs: &mut Vec<Job>) {
    let pending: std::collections::BTreeSet<_> = jobs
        .iter()
        .filter(|j| j.info.status == "queued")
        .filter_map(|j| j.batch.clone())
        .collect();
    jobs.retain(|j| {
        [
            "preparing",
            "queued",
            "authorizing",
            "waiting",
            "downloading",
            "importing",
            "installing",
        ]
        .contains(&j.info.status.as_str())
            || j.info.status == "error"
                && j.batch
                    .as_ref()
                    .is_some_and(|group| pending.contains(group))
    });
}
// Runs in the backend so closing the selection popup does not interrupt the queue.
fn advance_batch(app: &tauri::AppHandle) {
    let next = {
        let state = app.state::<Downloads>();
        let Ok(mut jobs) = state.0.lock() else {
            return;
        };
        let Some(index) = next_batch_index(&jobs) else {
            return;
        };
        let job = &mut jobs[index];
        let premium = job.request.is_some();
        job.info.status = if premium { "waiting" } else { "authorizing" }.into();
        (
            job.info.id.clone(),
            job.info.game_id.clone().unwrap_or_default(),
            premium,
            format!(
                "https://www.nexusmods.com/{}/mods/{}?tab=files&file_id={}&nmm=1",
                job.info.domain, job.info.mod_id, job.info.file_id
            ),
        )
    };
    let result = if next.2 {
        begin(app.clone(), next.0.clone(), next.1)
    } else {
        let handle=app.clone();let id=next.0.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error)=super::nexus_browser::open(handle.clone(),next.3,Some(id.clone())).await {
                update(&handle,&id,|info| {info.status="error".into();info.error=Some(error);});
                let _=handle.emit("nexus-download-updated",());
            }
        });
        Ok(())
    };
    if let Err(error) = result {
        update(app, &next.0, |info| {
            info.status = "error".into();
            info.error = Some(error)
        });
    }
    let _ = app.emit("nexus-download-updated", ());
}

#[tauri::command]
pub fn list_nexus_downloads(state: tauri::State<'_, Downloads>) -> Result<Vec<Download>, String> {
    Ok(state
        .0
        .lock()
        .map_err(|_| "Fila Nexus indisponível.")?
        .iter()
        .map(|j| j.info.clone())
        .collect())
}
#[tauri::command]
pub fn start_nexus_download(
    app: tauri::AppHandle,
    download_id: String,
    game_id: String,
) -> Result<(), String> {
    begin(app, download_id, game_id)
}

pub(super) fn browser_closed(app: &tauri::AppHandle, id: &str) {
    update(app, id, mark_browser_closed);
    let _ = app.emit("nexus-download-updated", ());
}

fn mark_browser_closed(info: &mut Download) {
    if info.status == "authorizing" {
        info.status = "error".into();
        info.error = Some("A janela Nexus foi fechada antes da confirmação. Tente novamente ou cancele este item.".into());
    }
}

#[tauri::command]
pub async fn retry_nexus_download(app: tauri::AppHandle, window: tauri::Webview, download_id: String, external: bool) -> Result<(), String> {
    if window.label() != "main" { return Err("Comando disponível somente no launcher.".into()); }
    if external { register_nexus_handler().await?; }
    let url = {
        let state = app.state::<Downloads>();
        let mut jobs = state.0.lock().map_err(|_| "Fila Nexus indisponível.")?;
        if jobs.iter().any(|j| j.info.id != download_id && matches!(j.info.status.as_str(), "authorizing" | "waiting")) {
            return Err("Confirme ou cancele o arquivo atual antes de abrir outro.".into());
        }
        let job = jobs.iter_mut().find(|j| j.info.id == download_id).ok_or("Download não encontrado.")?;
        if !matches!(job.info.status.as_str(), "authorizing" | "error") || job.cancel.load(Ordering::Relaxed) || job.info.received > 0 {
            return Err("Este download não está aguardando autorização.".into());
        }
        job.info.status = "authorizing".into(); job.info.error = None;
        format!("https://www.nexusmods.com/{}/mods/{}?tab=files&file_id={}&nmm=1", job.info.domain, job.info.mod_id, job.info.file_id)
    };
    let result = if external {
        super::nexus_browser::dismiss(&app, Some(&download_id));
        super::browser::open(app.clone(), url).await
    } else { super::nexus_browser::open(app.clone(), url, Some(download_id.clone())).await };
    if let Err(error) = &result { update(&app, &download_id, |info| { info.status = "error".into(); info.error = Some(error.clone()); }); }
    let _ = app.emit("nexus-download-updated", ());
    result
}
#[tauri::command]
pub fn cancel_nexus_download(
    app: tauri::AppHandle,
    state: tauri::State<'_, Downloads>,
    download_id: String,
) -> Result<(), String> {
    let mut jobs = state.0.lock().map_err(|_| "Fila Nexus indisponível.")?;
    let job = jobs.iter_mut().find(|job| job.info.id == download_id).ok_or("Download não encontrado.")?;
    if !matches!(job.info.status.as_str(), "queued" | "authorizing" | "waiting" | "downloading" | "error") { return Ok(()); }
    job.cancel.store(true, Ordering::Relaxed);
    job.request = None;
    job.info.status = "cancelled".into();
    let diagnostic = job.info.clone();
    drop(jobs);
    record_download(&diagnostic);
    super::nexus_browser::dismiss(&app, Some(&download_id));
    advance_after_capture(&app);
    let _ = app.emit("nexus-download-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn register_nexus_handler() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        let binary = crate::core::pelinstall::launcher_binary()?;
        let directory = dirs::data_dir().ok_or("Pasta de aplicativos indisponível.")?.join("applications");
        fs::create_dir_all(&directory).map_err(|_| "Não foi possível criar a pasta de aplicativos.")?;
        let path = directory.join("peligames-nxm.desktop");
        let temporary = directory.join(format!(".peligames-nxm-{}.desktop", uuid::Uuid::new_v4()));
        // xdg-utils 1.2.1 checks only the first word of Exec without unquoting it.
        // A fixed env executable keeps that check valid and preserves quoted paths.
        let text = format!("[Desktop Entry]\nType=Application\nName=PeliGames — Nexus Mods\nExec=/usr/bin/env {} %u\nIcon=peligames\nTerminal=false\nNoDisplay=true\nMimeType=x-scheme-handler/nxm;\n", crate::core::pelinstall::exec_arg(&binary.to_string_lossy())?);
        fs::write(&temporary, text).and_then(|_| fs::rename(&temporary, &path)).map_err(|_| "Não foi possível registrar o aplicativo.")?;
        let mut command = std::process::Command::new("xdg-mime");
        let result = command.env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD").args(["default", "peligames-nxm.desktop", "x-scheme-handler/nxm"]).output().map_err(|_| "Instale xdg-utils para registrar os links Nexus.")?;
        if !result.status.success() { return Err("Não foi possível configurar o aplicativo padrão para links Nexus.".into()); }
        let checked = std::process::Command::new("xdg-mime").env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD").args(["query", "default", "x-scheme-handler/nxm"]).output().map_err(|_| "Não foi possível verificar a associação Nexus.")?;
        if !checked.status.success() || String::from_utf8_lossy(&checked.stdout).trim() != "peligames-nxm.desktop" { return Err("O sistema não confirmou o PeliGames como aplicativo padrão para links Nexus.".into()); }
        let _ = std::process::Command::new("update-desktop-database").env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD").arg(&directory).output();
        Ok(())
    }).await.map_err(|_| "Falha ao registrar links Nexus.")?
}

#[cfg(test)]
mod browser_tests {
    use super::*;
    fn download(status: &str) -> Download {
        Download { id: "test".into(), domain: "cyberpunk2077".into(), mod_id: 4197, file_id: 1,
            game_id: Some("game".into()), name: "TweakXL".into(), status: status.into(), received: 0,
            total: None, error: None, requirements: None, auto_install: false, mod_entry_id: None }
    }
    #[test]
    fn closing_before_capture_makes_authorization_retryable() {
        let mut info = download("authorizing"); mark_browser_closed(&mut info);
        assert_eq!(info.status, "error"); assert!(info.error.is_some());
    }
    #[test]
    fn closing_browser_does_not_interrupt_captured_or_cancelled_jobs() {
        for status in ["queued", "waiting", "downloading", "importing", "installing", "imported", "installed", "cancelled"] {
            let mut info = download(status); mark_browser_closed(&mut info);
            assert_eq!(info.status, status); assert!(info.error.is_none());
        }
    }
}
