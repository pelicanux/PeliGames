//! Experimental Daniel NR through the pinned AMDNR Linux bridge.
mod component;
mod operations;
pub mod package_removal;
mod proxy;
pub mod runtime_switch;
pub mod runtimes;
#[cfg(test)]
mod tests;

pub use component::download;
pub use operations::{install, install_version, prepare, remove, set_startup, switch_proxy};
pub use proxy::LoadingDllState;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};
pub const VERSION: &str = "0.3.6";
pub const DANIEL_VERSION: &str = "0.6.0";
const RECEIPT: &str = ".peligames-daniel-nr.json";
const JOURNAL: &str = ".amdnr-linux-install.json";

#[derive(Clone, Deserialize, Serialize)]
pub struct Game {
    pub directory: String,
    pub executable: Option<String>,
    #[serde(default)]
    pub selected_executable: bool,
    pub name: String,
    #[serde(default)]
    pub flatpak: bool,
}
#[derive(Serialize)]
pub struct State {
    pub component_ready: bool,
    pub package_ready: bool,
    pub installed: bool,
    pub enabled: bool,
    pub directory: String,
    pub executable: String,
    pub wrapper: String,
    pub compiler: Option<String>,
    pub python_ready: bool,
    pub gpu_access: bool,
    pub conflict: Option<String>,
    pub version: &'static str,
    pub component_version: &'static str,
    pub loading_dll: LoadingDllState,
    pub runtimes: Vec<runtimes::RuntimeState>,
    pub pending_version_change: bool,
}
/// Shared preparation status; independent of any game's executable or prefix.
#[derive(Serialize)]
pub struct ComponentState {
    pub runtimes: Vec<runtimes::RuntimeState>,
    pub python_ready: bool,
    pub component_version: &'static str,
}
pub fn component_state(base: &Path) -> ComponentState {
    ComponentState {
        runtimes: runtimes::states(base),
        python_ready: operations::python_ready(),
        component_version: VERSION,
    }
}
#[derive(Deserialize, Serialize)]
struct Receipt {
    version: u32,
    executable: String,
    backend: String,
}
pub fn root() -> Result<PathBuf, String> {
    super::paths::app_path("Mod/DanielNR")
}
fn component(root: &Path) -> PathBuf {
    root.join(format!("AMDNR-Linux-v{VERSION}"))
}
fn package(root: &Path) -> PathBuf {
    root.join("prepared")
        .join(format!("daniel-{DANIEL_VERSION}"))
}
fn text(path: &Path) -> String {
    super::bounded_logs::read(path).unwrap_or_default()
}
fn string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Read headers only; never load a Windows binary to identify its architecture.
fn pe64(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut dos = [0u8; 64];
    if file.read_exact(&mut dos).is_err() || &dos[..2] != b"MZ" {
        return false;
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if offset > 1024 * 1024 || file.seek(SeekFrom::Start(offset)).is_err() {
        return false;
    }
    let mut pe = [0u8; 26];
    file.read_exact(&mut pe).is_ok() && &pe[..6] == b"PE\0\0\x64\x86" && &pe[24..26] == b"\x0b\x02"
}
fn compiler_valid(path: &Path) -> bool {
    if !pe64(path) {
        return false;
    }
    let mut bytes = Vec::new();
    if let Ok(file) = fs::File::open(path) {
        let _ = file.take(1024).read_to_end(&mut bytes);
    }
    !String::from_utf8_lossy(&bytes)
        .to_ascii_lowercase()
        .contains("wine builtin")
        && !String::from_utf8_lossy(&bytes)
            .to_ascii_lowercase()
            .contains("wine placeholder")
}
fn find_compiler(root: &Path) -> Option<PathBuf> {
    walkdir::WalkDir::new(root)
        .max_depth(6)
        .follow_links(false)
        .into_iter()
        .take(20_000)
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("d3dcompiler_47.dll")
                && compiler_valid(e.path())
        })
        .map(|e| e.into_path())
}
fn target(game: &Game) -> Result<(PathBuf, PathBuf), String> {
    if game.flatpak {
        return Err("O teste Daniel NR ainda não suporta launchers Flatpak.".into());
    }
    let requested = Path::new(&game.directory);
    if !requested.is_absolute() {
        return Err("Escolha a pasta do jogo com um caminho absoluto.".into());
    }
    let root = requested
        .canonicalize()
        .map_err(|e| format!("Pasta do jogo indisponível: {e}"))?;
    let root = if root.is_file() {
        root.parent().ok_or("Jogo sem pasta.")?.to_path_buf()
    } else {
        root
    };
    let explicit = game
        .executable
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .map(|p| if p.is_absolute() { p } else { root.join(p) });
    let recorded = if game.selected_executable {
        None
    } else {
        recorded_executable(&root, explicit.as_deref())?
    };
    let executable = if let Some(path) = recorded {
        path
    } else if let Some(path) = explicit.filter(|p| p.is_file()) {
        path
    } else {
        let folder = super::game_executable::best_executable_dir(&root, 8)
            .ok_or("Executável Windows não encontrado. Escolha o executável do jogo.")?;
        super::installed_library::discover_game_executables(&folder, 0, 10_000, 128)?
            .files
            .into_iter()
            .filter(|p| pe64(p))
            .max_by_key(|p| p.metadata().map(|m| m.len()).unwrap_or(0))
            .ok_or("O teste requer um executável Windows de 64 bits.")?
    }
    .canonicalize()
    .map_err(|e| e.to_string())?;
    if !executable.starts_with(&root)
        || !pe64(&executable)
        || !executable
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
    {
        return Err("Selecione um executável Windows de 64 bits dentro desta instalação.".into());
    }
    Ok((
        executable.parent().ok_or("Executável sem pasta.")?.into(),
        executable,
    ))
}
fn owned(directory: &Path, executable: &Path) -> bool {
    runtimes::installed(directory, executable).is_some()
}
fn conflict(directory: &Path, owned: bool) -> Option<String> {
    if owned {
        return None;
    }
    let files = [
        JOURNAL,
        RECEIPT,
        ".amdnr-linux-backup",
        "OptiScaler.ini",
        "OptiScaler.dll",
        "ReShade.ini",
        "dlssnr-amd-install.txt",
        "dlssnr_amd.addon64",
        "dlssnr_core.dll",
        "OptiScaler/plugins/dlssnr-amd.asi",
    ];
    files.iter().find(|n| directory.join(n).exists()).map(|name|
        format!("Outro mod ou uma instalação anterior foi detectada ({name}). Remova-o pela aba correspondente antes de instalar Daniel NR."))
}
pub fn state(base: &Path, game: &Game) -> Result<State, String> {
    let recovered = runtime_switch::recoverable_game(base, game)?;
    let game = &recovered;
    let (directory, executable) = target(game)?;
    let managed = owned(&directory, &executable);
    Ok(State {
        component_ready: component::ready(base),
        package_ready: package_ready(base),
        installed: managed && directory.join(JOURNAL).is_file(),
        enabled: {
            let ini = text(&directory.join("OptiScaler.ini"));
            let flags = peligames_mod_config::ini::sections(&ini);
            startup_enabled(&ini)
                && (runtimes::installed(&directory, &executable) != Some(runtimes::Runtime::V08)
                    || peligames_mod_config::ini::flag(
                        peligames_mod_config::ini::get(&flags, "DLSS", "NeuralHubNrEnabled"),
                        false,
                    ))
        },
        directory: string(&directory),
        executable: string(&executable),
        wrapper: string(&directory.join("amdnr-linux/run.sh")),
        compiler: find_compiler(Path::new(&game.directory)).map(|p| string(&p)),
        python_ready: operations::python_ready(),
        gpu_access: fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kfd")
            .is_ok(),
        conflict: if managed
            && directory.join(JOURNAL).is_file()
            && !directory.join("amdnr-linux/run.sh").is_file()
        {
            Some("A instalação está incompleta. Remova o teste para restaurar os arquivos antes de reinstalar.".into())
        } else {
            conflict(&directory, managed)
        },
        version: runtimes::installed(&directory, &executable)
            .unwrap_or(runtimes::Runtime::V06)
            .version(),
        runtimes: runtimes::states(base),
        pending_version_change: runtime_switch::pending(base, &directory),
        component_version: VERSION,
        loading_dll: proxy::state(&directory, managed && directory.join(JOURNAL).is_file()),
    })
}
fn package_ready(base: &Path) -> bool {
    let root = package(base);
    let components: serde_json::Value =
        serde_json::from_str(&text(&root.join("amdnr-linux/components.json"))).unwrap_or_default();
    components.get("neural_runtime").and_then(|v| v.as_str()) == Some("daniel")
        && components
            .pointer("/daniel/version")
            .and_then(|v| v.as_str())
            == Some(DANIEL_VERSION)
        && [
            "winmm.dll",
            "OptiScaler.ini",
            "dlssnr_amd_pass1.dll",
            "dlssnr_amd_pass2.dll",
            "dlssnr_amd_pass3.dll",
            "dlssnr_on_amd_weights.bin",
            "amdnr-linux/run.sh",
            "amdnr-linux/rocm/lib/libamdhip64.so.7",
        ]
        .iter()
        .all(|p| root.join(p).is_file())
}
fn ensure_closed(executable: &Path) -> Result<(), String> {
    let full = string(executable);
    let entries = fs::read_dir("/proc").map_err(|e| e.to_string())?;
    for entry in entries.flatten().filter(|e| {
        e.file_name()
            .to_string_lossy()
            .bytes()
            .all(|c| c.is_ascii_digit())
    }) {
        // Wine maps the PE file even though /proc/<pid>/exe identifies its loader.
        if fs::read_to_string(entry.path().join("maps")).is_ok_and(|maps| {
            maps.lines().any(|line| {
                line.ends_with(&format!(" {full}")) || line.ends_with(&format!(" {full} (deleted)"))
            })
        }) {
            return Err("Feche o jogo antes de alterar Daniel NR.".into());
        }
    }
    Ok(())
}
pub fn diagnostics(base: &Path, game: &Game) -> Result<String, String> {
    let (directory, _) = target(game)?;
    let (_, executable) = target(game)?;
    let runtime = runtimes::installed(&directory, &executable)
        .unwrap_or(runtimes::Runtime::V06)
        .version();
    let mut result = format!(
        "PELIGAMES — DANIEL NR EXPERIMENTAL\nRuntime: {runtime}\nPonte AMDNR-Linux: {VERSION}\n"
    );
    for (label, path) in [
        ("Preparo do componente", base.join("prepare.log")),
        (
            "Instalação deste jogo",
            operation_log(base, &directory).unwrap_or_else(|| base.join("logs/not-created.log")),
        ),
        ("Inicialização", directory.join("amdnr-linux/launch.log")),
        ("Daniel NR", directory.join("dlssnr_on_amd.log")),
        (
            "Daniel 0.8",
            directory.join(runtimes::HUB).join("dlssnr_on_amd.log"),
        ),
        ("Daniel pré-SR", directory.join("amd_presr.log")),
        ("Ponte HIP", directory.join("amd_bridge.log")),
        ("OptiScaler", directory.join("OptiScaler.log")),
    ] {
        let scope = if path.starts_with(&directory) {
            &directory
        } else {
            base
        };
        let content = scoped_log(scope, &path);
        result.push_str(&format!(
            "\n--- {label} ---\n{}\n",
            if content.is_empty() {
                "Log ainda não disponível."
            } else {
                &content
            }
        ));
    }
    Ok(super::report_privacy::sanitize(&result))
}

/// Read-only lookup, so opening a report never creates a log or registry entry.
pub fn operation_log(base: &Path, directory: &Path) -> Option<PathBuf> {
    let directory = directory.canonicalize().ok()?;
    super::paths::names::find(&base.join("logs"), &string(&directory))
        .map(|p| p.with_extension("log"))
}

fn scoped_log(root: &Path, path: &Path) -> String {
    let Ok(root) = root.canonicalize() else {
        return String::new();
    };
    if !path.canonicalize().is_ok_and(|p| p.starts_with(&root)) {
        return String::new();
    }
    let text = text(path);
    let mut start = text.len().saturating_sub(64 * 1024);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].into()
}

fn recorded_executable(root: &Path, preferred: Option<&Path>) -> Result<Option<PathBuf>, String> {
    let mut found = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .max_depth(10)
        .follow_links(false)
        .into_iter()
        .take(20_000)
        .filter_map(Result::ok)
    {
        if entry.file_name() != RECEIPT || !entry.file_type().is_file() {
            continue;
        }
        let Ok(receipt) = serde_json::from_str::<Receipt>(&text(entry.path())) else {
            continue;
        };
        let path = PathBuf::from(&receipt.executable);
        if receipt.version == 1
            && runtimes::Runtime::parse(&receipt.backend).is_ok()
            && path.starts_with(root)
            && path.parent() == entry.path().parent()
            && pe64(&path)
        {
            found.push(path);
        }
    }
    found.sort();
    found.dedup();
    if let Some(preferred) = preferred.and_then(|p| p.canonicalize().ok()) {
        if found.contains(&preferred) {
            return Ok(Some(preferred));
        }
    }
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        _ => Err(
            "Há mais de um teste nesta pasta. Escolha o executável para identificar a instalação."
                .into(),
        ),
    }
}

fn startup_enabled(text: &str) -> bool {
    let sections = peligames_mod_config::ini::sections(text);
    peligames_mod_config::ini::flag(
        peligames_mod_config::ini::get(&sections, "DlssNr", "Enabled"),
        false,
    )
}
