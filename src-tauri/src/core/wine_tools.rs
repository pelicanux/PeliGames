//! Launcher-owned prefix tools. Catalog text is parsed as data, never sourced or evaluated.
#[path = "wine_tools/output.rs"]
mod output;
#[cfg(target_os = "linux")]
#[path = "wine_tools/process.rs"]
mod process;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
static BUSY: AtomicBool = AtomicBool::new(false);
pub fn busy() -> bool {
    BUSY.load(Ordering::Acquire)
}
static SESSION: Mutex<Option<(String, Arc<AtomicBool>)>> = Mutex::new(None);
pub fn cancel(id: &str) -> Result<(), String> {
    let session = SESSION.lock().map_err(|e|e.to_string())?;
    let (active, cancel) = session.as_ref().ok_or("A operação já foi concluída.")?;
    if active != id {return Err("Esta operação já foi substituída.".into());}
    cancel.store(true, Ordering::Release);
    Ok(())
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut session) = SESSION.lock() {*session=None;}
        BUSY.store(false, Ordering::Release);
    }
}
#[derive(Deserialize)]
pub struct ToolRequest {
    pub path: String,
    pub action: String,
    #[serde(default)]
    pub packages: Vec<String>,
}
#[derive(Clone, Serialize)]
pub struct Package {
    pub id: String,
    pub category: String,
    pub title: String,
    pub installed: bool,
}
#[derive(Serialize)]
pub struct ToolResult {
    pub packages: Vec<Package>,
    pub log: String,
}
pub type Progress = Arc<dyn Fn(String) + Send + Sync>;
fn parse_catalog(text: &str, installed: &[String]) -> Vec<Package> {
    let mut result = Vec::new();
    let mut metadata_selected = false;
    for line in text.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.first() == Some(&"w_metadata") {
            metadata_selected = false;
        }
        if parts.len() >= 3
            && parts[0] == "w_metadata"
            && ["dlls", "fonts"].contains(&parts[2])
            && valid_id(parts[1])
        {
            metadata_selected = true;
            result.push(Package {
                id: parts[1].into(),
                category: parts[2].into(),
                title: parts[1].into(),
                installed: installed.iter().any(|id| id == parts[1]),
            });
        } else if let Some(title) = line
            .trim()
            .strip_prefix("title=\"")
            .and_then(|v| v.strip_suffix('"'))
        {
            if metadata_selected {
                if let Some(last) = result.last_mut() {
                    last.title = title.replace("\\\"", "\"");
                }
            }
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result.dedup_by(|a, b| a.id == b.id);
    result
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() < 100 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
async fn catalog(
    proton: &Path,
    prefix: &Path,
    progress: &Progress,
) -> Result<Vec<Package>, String> {
    progress("Carregando o catálogo de DLLs e fontes do Winetricks…".into());
    let mut text = None;
    for relative in ["protonfixes/winetricks", "files/protonfixes/winetricks"] {
        if let Ok(source) = fs::read_to_string(proton.join(relative)) {
            text = Some(source);
            break;
        }
    }
    if text.is_none() {
        let cache = super::paths::app_root()?.join("tools/winetricks-catalog.txt");
        text = fs::read_to_string(&cache).ok();
        if text.is_none() {
            progress("Consultando o catálogo oficial do Winetricks…".into());
            let source = reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
                .build()
                .map_err(|e| e.to_string())?
                .get(
                    "https://raw.githubusercontent.com/Winetricks/winetricks/master/src/winetricks",
                )
                .send()
                .await
                .map_err(|e| format!("Falha ao obter catálogo: {e}"))?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .text()
                .await
                .map_err(|e| e.to_string())?;
            if source.len() > 4_000_000 || !source.contains("WINETRICKS_VERSION=") {
                return Err("Catálogo Winetricks inválido.".into());
            }
            fs::create_dir_all(cache.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::write(&cache, &source).map_err(|e| e.to_string())?;
            text = Some(source);
        }
    }
    let installed: Vec<String> = fs::read_to_string(prefix.join("winetricks.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    let result = parse_catalog(&text.unwrap_or_default(), &installed);
    if result.is_empty() {
        return Err("Nenhuma DLL ou fonte encontrada no catálogo Winetricks.".into());
    }
    progress(format!("{} componentes disponíveis.", result.len()));
    Ok(result)
}
fn prepare(umu: &Path, proton: &Path, prefix: &Path, action: &str, packages: &[String], join_running: bool) -> Command {
    let mut cmd = Command::new(umu);
    super::env_utils::host_environment(&mut cmd);
    cmd.arg(if matches!(action, "winecfg" | "regedit" | "explorer" | "uninstaller") {
        action
    } else {
        "winetricks"
    });
    if action == "install" {
        cmd.args(packages);
    } else if action == "winetricks-gui" {
        // UMU requires a nonempty Winetricks argument list; --gui explicitly
        // opens the original menus without selecting any installation verb.
        cmd.arg("--gui");
    }
    cmd.env("WINEPREFIX", prefix)
        .env("PROTONPATH", proton)
        .env("GAMEID", "umu-default")
        .env("STORE", "none")
        .env("PROTON_VERB", if join_running {"runinprefix"} else {"waitforexitandrun"})
        .env("UMU_LOG", "1")
        .env_remove("UMU_NO_PROTON")
        .env_remove("WINETRICKS_SUPER_QUIET")
        .current_dir(prefix)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}
async fn cancellable<T>(future: impl std::future::Future<Output=Result<T,String>>, cancel: &AtomicBool) -> Result<T,String> {
    tokio::select! {
        result = future => result,
        _ = async {while !cancel.load(Ordering::Acquire) {tokio::time::sleep(Duration::from_millis(50)).await;}} => Err("PELIGAMES_WINE_TOOL_CANCELLED".into()),
    }
}
pub async fn execute(request: ToolRequest, id: String, progress: Progress) -> Result<ToolResult, String> {
    if BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Uma ferramenta Wine já está em execução.".into());
    }
    let _guard = Guard;
    let cancel = Arc::new(AtomicBool::new(false));
    *SESSION.lock().map_err(|e|e.to_string())? = Some((id.clone(), cancel.clone()));
    if !["catalog", "winecfg", "install", "winetricks-gui", "regedit", "explorer", "uninstaller"].contains(&request.action.as_str()) {
        return Err("Ferramenta inválida.".into());
    }
    if request.action != "install" && !request.packages.is_empty() {
        return Err("Esta ferramenta não aceita uma lista de componentes.".into());
    }
    let (_, prefix, proton, _) = maintenance_entry(&request.path)?;
    let prefix = PathBuf::from(&prefix)
        .canonicalize()
        .map_err(|_| "Prefixo do jogo indisponível.")?;
    if !prefix.join("drive_c").is_dir() {
        return Err("O prefixo Wine ainda não foi inicializado. Execute o jogo antes de usar as ferramentas.".into());
    }
    let proton = super::game_installation::validate_proton(&proton)?;
    if request.action == "catalog" {
        return Ok(ToolResult {
            packages: cancellable(catalog(&proton, &prefix, &progress), &cancel).await?,
            log: String::new(),
        });
    }
    // UMU can expose the same prefix as <root> or its pfx -> . alias.
    let alias = prefix.join("pfx");
    let join_running = super::game_execution::prefix_in_use(&prefix)
        || (alias.is_dir() && super::game_execution::prefix_in_use(&alias));
    if join_running {
        progress("Abrindo a ferramenta no prefixo em uso, sem aguardar o encerramento do jogo.".into());
    }
    if request.action == "install" {
        if request.packages.is_empty() || request.packages.len() > 40 {
            return Err("Selecione de 1 a 40 componentes.".into());
        }
        let available = cancellable(catalog(&proton, &prefix, &progress), &cancel).await?;
        if request
            .packages
            .iter()
            .any(|id| !valid_id(id) || !available.iter().any(|p| &p.id == id))
        {
            return Err("Componente não encontrado no catálogo Winetricks.".into());
        }
    }
    let _prefix_lease = if join_running {None} else {Some(super::game_installation::lock_prefix(&prefix)?)};
    if cancel.load(Ordering::Acquire) {return Err("PELIGAMES_WINE_TOOL_CANCELLED".into());}
    progress("Preparando UMU (o download automático será feito se necessário)…".into());
    // The shared UMU cache preparation may finish in the background; cancellation
    // stops waiting here and never starts a component installer afterward.
    let umu = cancellable(async {
        tokio::task::spawn_blocking(super::game_installation::find_umu).await.map_err(|e|e.to_string())?
    }, &cancel).await?;
    if cancel.load(Ordering::Acquire) {return Err("PELIGAMES_WINE_TOOL_CANCELLED".into());}
    let logs = super::installation_layout::destination_of(&prefix)
        .ok_or("Diretório do jogo inválido.")?
        .join("logs");
    fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let log = logs.join(format!("{}-{}.log", request.action, uuid::Uuid::new_v4()));
    let logfile = Arc::new(std::sync::Mutex::new(
        fs::File::create(&log).map_err(|e| e.to_string())?,
    ));
    progress(format!(
        "Abrindo {} no prefixo {}",
        request.action,
        prefix.display()
    ));
    let cmd = prepare(&umu, &proton, &prefix, &request.action, &request.packages, join_running);
    #[cfg(target_os = "linux")]
    let exit = tokio::task::spawn_blocking(move || process::run(cmd, id, cancel, progress, logfile))
        .await.map_err(|e|e.to_string())?;
    #[cfg(not(target_os = "linux"))]
    let exit: Result<(), String> = {let _=(cmd,id,cancel,progress,logfile);Err("Ferramentas Wine requerem Linux.".into())};
    exit.map_err(|e| format!("{e} Log: {}", log.display()))?;
    Ok(ToolResult {
        packages: vec![],
        log: log.to_string_lossy().into_owned(),
    })
}

/// Resolve only registered game identities for Wine tools and launcher shortcuts.
pub fn maintenance_entry(path: &str) -> Result<(String,String,String,String),String> {
    if let Some(id) = path.strip_prefix("nexus:") { return crate::commands::nexus_local::maintenance_entry(id); }
    if let Some(id) = path.strip_prefix("external:") { return crate::commands::external_library::maintenance_entry(id); }
    let entry = super::installed_library::list(false)?.into_iter().find(|entry|entry.path == path).ok_or("Jogo não registrado na biblioteca.")?;
    Ok((entry.name, entry.prefix, entry.proton, entry.executable))
}

#[cfg(test)]
mod maintenance_tests {
    use super::*;
    #[test]
    fn cancellation_is_scoped_to_the_active_request() {
        let flag = Arc::new(AtomicBool::new(false));
        *SESSION.lock().unwrap() = Some(("current".into(), flag.clone()));
        assert!(cancel("previous").is_err());
        assert!(!flag.load(Ordering::Acquire));
        cancel("current").unwrap();
        assert!(flag.load(Ordering::Acquire));
        *SESSION.lock().unwrap() = None;
        assert!(cancel("current").is_err());
    }
    #[test]
    fn tools_keep_the_registered_prefix_and_proton_without_a_shell() {
        for action in ["winecfg", "regedit", "explorer", "uninstaller"] {
            let command = prepare(Path::new("/tools/umu-run"), Path::new("/protons/GE Proton"), Path::new("/games/My Prefix"), action, &[], false);
            assert_eq!(command.get_program(), "/tools/umu-run");
            assert_eq!(command.get_args().collect::<Vec<_>>(), [std::ffi::OsStr::new(action)]);
            assert!(command.get_envs().any(|(key,value)| key == "WINEPREFIX" && value == Some(std::ffi::OsStr::new("/games/My Prefix"))));
            assert!(command.get_envs().any(|(key,value)| key == "PROTONPATH" && value == Some(std::ffi::OsStr::new("/protons/GE Proton"))));
        }
    }
    #[test]
    fn tools_join_the_running_prefix_without_waiting_for_the_game() {
        for action in ["winecfg", "regedit", "explorer", "uninstaller", "winetricks-gui", "install"] {
            let cmd = prepare(Path::new("/umu-run"), Path::new("/proton"), Path::new("/prefix"), action, &[], true);
            assert!(cmd.get_envs().any(|(key,value)|key=="PROTON_VERB" && value==Some(std::ffi::OsStr::new("runinprefix"))));
            assert!(cmd.get_envs().any(|(key,value)|key=="WINEPREFIX" && value==Some(std::ffi::OsStr::new("/prefix"))));
        }
        let cmd = prepare(Path::new("/umu-run"), Path::new("/proton"), Path::new("/prefix"), "winecfg", &[], false);
        assert!(cmd.get_envs().any(|(key,value)|key=="PROTON_VERB" && value==Some(std::ffi::OsStr::new("waitforexitandrun"))));
    }
    #[test]
    fn winetricks_opens_its_original_interface_without_installing_packages() {
        let command = prepare(Path::new("/umu-run"), Path::new("/proton"), Path::new("/prefix"), "winetricks-gui", &[], false);
        assert_eq!(command.get_args().collect::<Vec<_>>(), [std::ffi::OsStr::new("winetricks"), std::ffi::OsStr::new("--gui")]);
    }
}
