//! IPC for the opt-in Daniel NR test. Downloads never run during status queries.
use crate::core::daniel_nr::{self, ComponentState, Game, State};
use serde::Serialize;
use std::{
    fs,
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter};
static BUSY: AtomicBool = AtomicBool::new(false);
struct Operation;
impl Drop for Operation {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}
#[derive(Clone, Serialize)]
struct Progress {
    directory: String,
    message: String,
}

fn lock(root: &Path) -> Result<fs::File, String> {
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(root)
        .map_err(|e| e.to_string())?;
    let meta = fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o022 != 0
    {
        return Err("Pasta do componente Daniel NR inválida ou compartilhada para escrita.".into());
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join("operation.lock"))
        .map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.uid() != unsafe { libc::geteuid() } {
        return Err("Bloqueio Daniel NR inválido.".into());
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("Outra operação Daniel NR está em andamento.".into());
    }
    Ok(file)
}
#[tauri::command]
pub async fn daniel_nr_state(game: Game) -> Result<State, String> {
    crate::core::release_features::require_daniel()?;
    tauri::async_runtime::spawn_blocking(move || daniel_nr::state(&daniel_nr::root()?, &game))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn daniel_nr_component_state() -> Result<ComponentState, String> {
    crate::core::release_features::require_daniel()?;
    tauri::async_runtime::spawn_blocking(move || {
        Ok(daniel_nr::component_state(&daniel_nr::root()?))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[derive(Serialize)]
#[serde(untagged)]
pub enum OperationState {
    Game(Box<State>),
    Component(ComponentState),
}
fn validate_scope(operation: &str, has_game: bool) -> Result<(), String> {
    if !has_game && !matches!(operation, "prepare" | "import-runtime" | "remove-package") {
        return Err("Selecione um jogo para alterar sua instalação.".into());
    }
    if has_game && operation == "remove-package" {
        return Err("Remova pacotes em Complementos; a instalação do jogo é independente.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn daniel_nr_operate(
    app: AppHandle,
    game: Option<Game>,
    operation: String,
    compiler: Option<String>,
    dx12: bool,
    proxy: Option<String>,
    runtime: Option<String>,
    source: Option<String>,
) -> Result<OperationState, String> {
    crate::core::release_features::require_daniel()?;
    validate_scope(&operation, game.is_some())?;
    if !matches!(
        operation.as_str(),
        "prepare"
            | "install"
            | "remove"
            | "enable"
            | "disable"
            | "switch-proxy"
            | "switch-version"
            | "recover-version"
            | "import-runtime"
            | "remove-package"
    ) {
        return Err("Operação Daniel NR inválida.".into());
    }
    let runtime = daniel_nr::runtimes::Runtime::parse(runtime.as_deref().unwrap_or("0.6.0"))?;
    BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "Outra operação Daniel NR está em andamento.")?;
    let _operation = Operation;
    let root = daniel_nr::root()?;
    let _lock = lock(&root)?;
    let progress_app = app.clone();
    let progress_game = game
        .as_ref()
        .map(|g| g.directory.clone())
        .unwrap_or_default();
    let progress = move |message: String| {
        let _ = progress_app.emit(
            "danielNrProgress",
            Progress {
                directory: progress_game.clone(),
                message,
            },
        );
    };
    if matches!(
        operation.as_str(),
        "prepare" | "install" | "remove" | "switch-proxy" | "switch-version" | "recover-version"
    ) {
        daniel_nr::download(&root, &progress).await?;
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let (_operation, _lock) = (_operation, _lock);
        if !matches!(
            operation.as_str(),
            "prepare" | "import-runtime" | "remove-package" | "recover-version"
        ) {
            let current = daniel_nr::state(&root, game.as_ref().ok_or("Selecione um jogo.")?)?;
            if current.pending_version_change {
                return Err("Há uma troca interrompida. Use Recuperar versão anterior.".into());
            }
        }
        match operation.as_str() {
            "prepare" => {
                if runtime != daniel_nr::runtimes::Runtime::V06 {
                    return Err(
                        "Importe o pacote experimental preparado para a versão 0.8.0.".into(),
                    );
                }
                daniel_nr::prepare(
                    &root,
                    Path::new(
                        compiler
                            .as_deref()
                            .ok_or("Selecione d3dcompiler_47.dll para preparar o teste.")?,
                    ),
                    &progress,
                )?;
            }
            "import-runtime" => daniel_nr::runtimes::import(
                &root,
                Path::new(
                    source
                        .as_deref()
                        .ok_or("Escolha a pasta do pacote preparado.")?,
                ),
                &progress,
            )?,
            "remove-package" => daniel_nr::package_removal::remove(&root, runtime, &progress)?,
            "switch-version" => daniel_nr::runtime_switch::switch(
                &root,
                game.as_ref().ok_or("Selecione um jogo.")?,
                runtime,
                &progress,
            )?,
            "recover-version" => daniel_nr::runtime_switch::recover(
                &root,
                game.as_ref().ok_or("Selecione um jogo.")?,
                &progress,
            )?,
            "install" => daniel_nr::install_version(
                &root,
                game.as_ref().ok_or("Selecione um jogo.")?,
                dx12,
                proxy.as_deref().unwrap_or("auto"),
                runtime,
                &progress,
            )?,
            "switch-proxy" => daniel_nr::switch_proxy(
                &root,
                game.as_ref().ok_or("Selecione um jogo.")?,
                proxy.as_deref().ok_or("Selecione a DLL de carregamento.")?,
                &progress,
            )?,
            "remove" => {
                daniel_nr::remove(&root, game.as_ref().ok_or("Selecione um jogo.")?, &progress)?
            }
            "enable" => daniel_nr::set_startup(game.as_ref().ok_or("Selecione um jogo.")?, true)?,
            "disable" => daniel_nr::set_startup(game.as_ref().ok_or("Selecione um jogo.")?, false)?,
            _ => unreachable!(),
        }
        match game {
            Some(game) => daniel_nr::state(&root, &game).map(|s| OperationState::Game(Box::new(s))),
            None => Ok(OperationState::Component(daniel_nr::component_state(&root))),
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    crate::core::logger::log_result(
        &app,
        "Daniel NR experimental",
        &result
            .as_ref()
            .map(|_| "Operação concluída")
            .map_err(Clone::clone),
    );
    result
}
#[tauri::command]
pub async fn daniel_nr_diagnostics(game: Game) -> Result<String, String> {
    crate::core::release_features::require_daniel()?;
    tauri::async_runtime::spawn_blocking(move || daniel_nr::diagnostics(&daniel_nr::root()?, &game))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn public_release_rejects_daniel_access_before_resolving_paths() {
        if crate::core::release_features::daniel_enabled() {
            return;
        }
        let error = super::daniel_nr_component_state()
            .await
            .err()
            .expect("disabled component");
        assert!(error.contains("desativado"));
        let game = crate::core::daniel_nr::Game {
            directory: "/does-not-exist".into(),
            executable: None,
            name: "Fixture".into(),
            flatpak: false,
            selected_executable: false,
        };
        assert!(super::daniel_nr_state(game.clone())
            .await
            .err()
            .unwrap()
            .contains("desativado"));
        assert!(super::daniel_nr_diagnostics(game)
            .await
            .unwrap_err()
            .contains("desativado"));
    }
    #[test]
    fn shared_preparation_cannot_change_any_game_installation() {
        for operation in ["prepare", "import-runtime", "remove-package"] {
            assert!(super::validate_scope(operation, false).is_ok());
        }
        assert!(super::validate_scope("remove-package", true).is_err());
        for operation in [
            "install",
            "remove",
            "enable",
            "disable",
            "switch-proxy",
            "switch-version",
            "recover-version",
        ] {
            assert!(
                super::validate_scope(operation, false).is_err(),
                "{operation} needs a game"
            );
            assert!(super::validate_scope(operation, true).is_ok());
        }
    }
}
