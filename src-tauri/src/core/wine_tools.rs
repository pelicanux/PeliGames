//! Launcher-owned prefix tools. Catalog text is parsed as data, never sourced or evaluated.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
static BUSY: AtomicBool = AtomicBool::new(false);
pub fn busy() -> bool {
    BUSY.load(Ordering::Acquire)
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
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
fn prepare(umu: &Path, proton: &Path, prefix: &Path, action: &str, packages: &[String]) -> Command {
    let mut cmd = Command::new(umu);
    cmd.arg(if action == "winecfg" {
        "winecfg"
    } else {
        "winetricks"
    });
    if action != "winecfg" {
        cmd.args(packages);
    }
    cmd.env("WINEPREFIX", prefix)
        .env("PROTONPATH", proton)
        .env("GAMEID", "umu-default")
        .env("STORE", "none")
        .env("PROTON_VERB", "waitforexitandrun")
        .env("UMU_LOG", "1")
        .env_remove("UMU_NO_PROTON")
        .current_dir(prefix)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}
pub async fn execute(request: ToolRequest, progress: Progress) -> Result<ToolResult, String> {
    if BUSY
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("Uma ferramenta Wine já está em execução.".into());
    }
    let _guard = Guard;
    if !["catalog", "winecfg", "install"].contains(&request.action.as_str()) {
        return Err("Ferramenta inválida.".into());
    }
    let entry = super::installed_library::list(false)?
        .into_iter()
        .find(|e| e.path == request.path)
        .ok_or("Jogo não registrado na biblioteca.")?;
    let prefix = PathBuf::from(&entry.prefix)
        .canonicalize()
        .map_err(|_| "Prefixo do jogo indisponível.")?;
    if !prefix.join("drive_c").is_dir() {
        return Err("O prefixo Wine ainda não foi inicializado. Execute o jogo antes de usar as ferramentas.".into());
    }
    let proton = super::game_installation::validate_proton(&entry.proton)?;
    if request.action == "catalog" {
        return Ok(ToolResult {
            packages: catalog(&proton, &prefix, &progress).await?,
            log: String::new(),
        });
    }
    if super::game_execution::active() || super::game_execution::prefix_in_use(&prefix) {
        return Err(
            "Encerre o jogo e os programas deste prefixo antes de usar as ferramentas.".into(),
        );
    }
    if request.action == "install" {
        if request.packages.is_empty() || request.packages.len() > 40 {
            return Err("Selecione de 1 a 40 componentes.".into());
        }
        let available = catalog(&proton, &prefix, &progress).await?;
        if request
            .packages
            .iter()
            .any(|id| !valid_id(id) || !available.iter().any(|p| &p.id == id))
        {
            return Err("Componente não encontrado no catálogo Winetricks.".into());
        }
    }
    progress("Preparando UMU (o download automático será feito se necessário)…".into());
    let umu = tokio::task::spawn_blocking(super::game_installation::find_umu).await.map_err(|e|e.to_string())??;
    let logs = prefix
        .parent()
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
    let mut cmd = prepare(&umu, &proton, &prefix, &request.action, &request.packages);
    let exit = tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Não foi possível iniciar a ferramenta: {e}"))?;
        let read = |pipe: Box<dyn std::io::Read + Send>,
                    progress: Progress,
                    logfile: Arc<std::sync::Mutex<fs::File>>| {
            std::thread::spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    if let Ok(mut log) = logfile.lock() {
                        let _ = writeln!(log, "{line}");
                    }
                    progress(line);
                }
            })
        };
        let out = read(
            Box::new(child.stdout.take().unwrap()),
            progress.clone(),
            logfile.clone(),
        );
        let err = read(
            Box::new(child.stderr.take().unwrap()),
            progress.clone(),
            logfile,
        );
        let status = child.wait().map_err(|e| e.to_string())?;
        let _ = out.join();
        let _ = err.join();
        if !status.success() {
            return Err(format!(
                "A ferramenta encerrou com código {:?}.",
                status.code()
            ));
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?;
    exit.map_err(|e| format!("{e} Log: {}", log.display()))?;
    Ok(ToolResult {
        packages: vec![],
        log: log.to_string_lossy().into_owned(),
    })
}
