use crate::{release, validate_runtime_at, versions};
use peligames_archive::unpack_tar_xz as unpack;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
static INSTALLING: AtomicBool = AtomicBool::new(false);
pub type Progress = Arc<dyn Fn(InstallProgress) + Send + Sync>;
#[derive(Clone, Serialize)]
pub struct InstallProgress {
    pub phase: &'static str,
    pub downloaded: u64,
    pub total: Option<u64>,
}
pub async fn install(
    storage: PathBuf,
    backend: String,
    version: Option<String>,
    on_progress: Progress,
) -> Result<(), String> {
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
    let latest = versions::latest(&storage, &backend)?;
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
    on_progress(InstallProgress {
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
    on_progress(InstallProgress {
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
            on_progress(InstallProgress {
                phase: "downloading",
                downloaded: bytes.len() as u64,
                total,
            });
            last_update = std::time::Instant::now();
        }
    }
    let downloaded = bytes.len() as u64;
    on_progress(InstallProgress {
        phase: "verifying",
        downloaded,
        total,
    });
    let downloaded_hash = format!("{:x}", Sha256::digest(&bytes));
    if hash.as_ref().is_some_and(|h| &downloaded_hash != h) {
        return Err("A verificação SHA-256 falhou. Nenhum runtime foi instalado.".into());
    }
    on_progress(InstallProgress {
        phase: "extracting",
        downloaded,
        total,
    });
    let install_progress = on_progress.clone();
    let mut installed_release = selected_release.clone();
    installed_release.hash = Some(downloaded_hash);
    let result = tokio::task::spawn_blocking(move || {
        peligames_addon_kit::files::safe_directory(&storage, true)?;
        let _lock = peligames_mod_config::DirectoryLock::acquire(&storage)?;
        let destination = versions::location(&storage, &backend, &installed_release.version)?;
        peligames_addon_kit::files::safe_directory(
            destination.parent().ok_or("Destino inválido")?,
            true,
        )?;
        fs::create_dir_all(storage.clone()).map_err(|e| e.to_string())?;
        let stage = storage
            .clone()
            .join(format!(".install-mako-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&stage).map_err(|e| e.to_string())?;
        let result = (|| {
            unpack(&bytes, &stage)?;
            validate_runtime_at(&stage, &backend)?;
            install_progress(InstallProgress {
                phase: "installing",
                downloaded,
                total,
            });
            if destination.exists() && validate_runtime_at(&destination, &backend).is_ok() {
                return versions::publish(&storage, &backend, &installed_release);
            }
            for entry in walkdir::WalkDir::new(&stage).contents_first(true) {
                let entry = entry.map_err(|e| e.to_string())?;
                fs::File::open(entry.path())
                    .and_then(|f| f.sync_all())
                    .map_err(|e| e.to_string())?;
            }
            let previous = storage
                .clone()
                .join(format!(".previous-mako-{}", uuid::Uuid::new_v4()));
            let replacing = destination.exists();
            if replacing {
                peligames_addon_kit::files::ensure_unmapped(std::slice::from_ref(&destination))?;
                fs::rename(&destination, &previous).map_err(|e| e.to_string())?;
            }
            if let Err(error) = fs::rename(&stage, &destination) {
                if replacing {
                    let _ = fs::rename(&previous, &destination);
                }
                return Err(error.to_string());
            }
            fs::File::open(storage.clone())
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
            versions::publish(&storage, &backend, &installed_release)?;
            if replacing {
                fs::remove_dir_all(&previous).map_err(|e| e.to_string())?;
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&stage);
        result
    })
    .await
    .map_err(|e| e.to_string())?;
    if result.is_ok() {
        on_progress(InstallProgress {
            phase: "complete",
            downloaded,
            total,
        });
    }
    result
}
