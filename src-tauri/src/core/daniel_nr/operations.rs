use super::*;
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    os::unix::{fs::OpenOptionsExt, process::CommandExt},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn python() -> Command {
    let mut command = Command::new("python3");
    command
        .arg("-B")
        .env_remove("PYTHONHOME")
        .env_remove("PYTHONPATH")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD")
        .env_remove("LD_AUDIT");
    command
}
pub(super) fn python_ready() -> bool {
    python().args(["-c", "import sys,importlib.util;sys.exit(0 if sys.version_info >= (3,12) and (sys.version_info >= (3,14) or importlib.util.find_spec('zstandard')) else 1)"])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}
pub(super) fn run(
    base: &Path,
    script: &str,
    arguments: &[String],
    log_path: &Path,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    let needs_compression = script == "setup.py";
    let version_ready = python()
        .args([
            "-c",
            "import sys;sys.exit(0 if sys.version_info >= (3,12) else 1)",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    if !version_ready || (needs_compression && !python_ready()) {
        return Err("Instale Python 3.12 ou superior. Python 3.12/3.13 também requer o módulo zstandard do sistema.".into());
    }
    if !component::ready(base) {
        return Err("Prepare a ponte AMDNR-Linux antes de continuar.".into());
    }
    let log = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&log_path)
        .map_err(|e| e.to_string())?;
    if !log.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Log da operação inválido.".into());
    }
    let mut command = python();
    command
        .arg(component(base).join("linux").join(script))
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .process_group(0);
    // The upstream proxy switch uses its controller journals. Keep its state
    // private to this component instead of creating a second launcher profile.
    if script == "manage.py" && arguments.first().is_some_and(|arg| arg == "switch-proxy") {
        command
            .env("XDG_DATA_HOME", base.join("launcher-state"))
            .env("XDG_CACHE_HOME", base.join("launcher-cache"));
    }
    let mut child = ChildGroup(
        command
            .spawn()
            .map_err(|e| format!("Não foi possível iniciar a instalação: {e}"))?,
    );
    let started = Instant::now();
    let mut previous = String::new();
    loop {
        match child.0.try_wait() {
            Ok(Some(status)) => {
                let _ = super::super::bounded_logs::compact(log_path);
                if status.success() {
                    return Ok(());
                }
                let tail = text(log_path)
                    .lines()
                    .rev()
                    .take(8)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(format!("A operação Daniel NR falhou.\n{tail}"));
            }
            Ok(None) => (),
            Err(error) => {
                let _ = child.0.kill();
                let _ = child.0.wait();
                return Err(error.to_string());
            }
        }
        if started.elapsed() > Duration::from_secs(1800) {
            // The child owns this process group; never target unrelated Wine sessions.
            unsafe {
                libc::kill(-(child.0.id() as i32), libc::SIGKILL);
            }
            let _ = child.0.wait();
            return Err("A operação excedeu 30 minutos. Os downloads verificados serão reutilizados na próxima tentativa.".into());
        }
        let latest = text(log_path)
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or_default()
            .chars()
            .take(300)
            .collect::<String>();
        if latest != previous {
            progress(latest.clone());
            previous = latest;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
pub fn prepare(
    base: &Path,
    compiler: &Path,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    if package_ready(base) {
        return Ok(());
    }
    if !compiler_valid(compiler) {
        return Err("Selecione um d3dcompiler_47.dll nativo Microsoft de 64 bits de um jogo seu; a DLL interna do Wine não é compatível.".into());
    }
    progress("Preparando Daniel 0.6.0 e o runtime ROCm privado…".into());
    run(
        base,
        "setup.py",
        &[
            "prepare".into(),
            "--compiler".into(),
            string(compiler),
            "--daniel-runtime".into(),
            DANIEL_VERSION.into(),
            "--output".into(),
            string(&package(base)),
            "--cache".into(),
            string(&base.join("downloads")),
        ],
        &base.join("prepare.log"),
        progress,
    )?;
    if !package_ready(base) {
        return Err(
            "O pacote preparado está incompleto ou não selecionou o backend Daniel.".into(),
        );
    }
    Ok(())
}
pub fn install(
    base: &Path,
    game: &Game,
    dx12: bool,
    proxy: &str,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    install_version(base, game, dx12, proxy, runtimes::Runtime::V06, progress)
}
pub fn install_version(
    base: &Path,
    game: &Game,
    dx12: bool,
    proxy: &str,
    runtime: runtimes::Runtime,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    super::proxy::validate(proxy, true)?;
    let (directory, executable) = target(game)?;
    super::super::installed_library::ensure_path_writable(&directory)?;
    ensure_closed(&executable)?;
    let root = Path::new(&game.directory)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let root = if root.is_file() {
        root.parent().ok_or("Jogo sem pasta.")?
    } else {
        root.as_path()
    };
    if recorded_executable(root, None)?.is_some_and(|path| path != executable) {
        return Err(
            "Remova o teste do outro executável desta instalação antes de mudar o destino.".into(),
        );
    }
    if !dx12 {
        return Err("Confirme que o jogo está configurado para DirectX 12.".into());
    }
    if let Some(reason) = conflict(&directory, owned(&directory, &executable)) {
        return Err(reason);
    }
    if directory.join(JOURNAL).exists() {
        return Err("Daniel NR já está instalado. Remova o teste antes de reinstalar.".into());
    }
    if !runtimes::ready(base, runtime) {
        return Err("Prepare o pacote Daniel NR antes de instalar no jogo.".into());
    }
    if !cfg!(test)
        && fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kfd")
            .is_err()
    {
        return Err("Sem acesso à GPU via /dev/kfd. Confira as permissões de GPU da distribuição antes de instalar.".into());
    }
    if owned(&directory, &executable) && !directory.join(".amdnr-linux-backup").exists() {
        fs::remove_file(directory.join(RECEIPT)).map_err(|e| e.to_string())?;
    }
    if runtime == runtimes::Runtime::V08 {
        runtimes::verify(&runtime.package(base))?;
    }
    let receipt = Receipt {
        version: 1,
        executable: string(&executable),
        backend: runtime.version().into(),
    };
    let mut record = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(directory.join(RECEIPT))
        .map_err(|e| e.to_string())?;
    record
        .write_all(&serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    progress(
        "Instalando o teste ao lado do executável e preservando os arquivos originais…".into(),
    );
    let log_path = game_log(base, &directory, &game.name)?;
    let result = run(
        base,
        "manage.py",
        &[
            "install".into(),
            string(&runtime.package(base)),
            string(&directory),
            "--proxy".into(),
            proxy.into(),
            "--exe".into(),
            string(&executable),
        ],
        &log_path,
        progress,
    );
    if result.is_err() && !directory.join(JOURNAL).exists() {
        let _ = fs::remove_file(directory.join(RECEIPT));
    }
    result?;
    if runtime == runtimes::Runtime::V08 {
        set_startup(game, true)?;
    }
    Ok(())
}

/// The upstream operation commits the renamed DLL and its ownership journal
/// together. Its launch wrapper reads proxy.json on the next run, so the
/// PeliRun wrapper path and all other launch settings remain valid.
pub fn switch_proxy(
    base: &Path,
    game: &Game,
    proxy: &str,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    super::proxy::validate(proxy, false)?;
    let (directory, executable) = target(game)?;
    super::super::installed_library::ensure_path_writable(&directory)?;
    ensure_closed(&executable)?;
    if !owned(&directory, &executable) || !directory.join(JOURNAL).is_file() {
        return Err("Esta instalação não pertence ao teste Daniel NR.".into());
    }
    progress("Alterando a DLL de carregamento e preservando os backups…".into());
    run(
        base,
        "manage.py",
        &[
            "switch-proxy".into(),
            string(&directory),
            "--proxy".into(),
            proxy.into(),
            "--exe".into(),
            string(&executable),
        ],
        &game_log(base, &directory, &game.name)?,
        progress,
    )
}

pub(super) fn hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
/// Preserve edited settings before restoring originals. Other modified files
/// still go through the upstream uninstaller's normal refusal; never use force.
pub(super) fn preserve_settings(base: &Path, directory: &Path, game: &Game) -> Result<(), String> {
    for rel in [
        "OptiScaler.ini",
        "AMDNR-NeuralHub-080/settings.ini",
        "AMDNR-NeuralHub-080/neuralhub.ini",
    ] {
        if fs::symlink_metadata(directory.join(rel)).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(
                "O arquivo de ajustes é um link; preserve-o antes de remover o teste.".into(),
            );
        }
    }
    let path = directory.join(JOURNAL);
    if !path.exists() {
        return Ok(());
    }
    let mut journal: serde_json::Value =
        serde_json::from_str(&text(&path)).map_err(|e| e.to_string())?;
    let files = journal
        .get_mut("files")
        .and_then(|v| v.as_array_mut())
        .ok_or("Registro sem arquivos.")?;
    let mut changed = false;
    for entry in files {
        let rel = entry
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if ![
            "OptiScaler.ini",
            "AMDNR-NeuralHub-080/settings.ini",
            "AMDNR-NeuralHub-080/neuralhub.ini",
        ]
        .contains(&rel.as_str())
        {
            continue;
        }
        let config = directory.join(&rel);
        if fs::symlink_metadata(&config).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(
                "O arquivo de ajustes é um link; preserve-o antes de remover o teste.".into(),
            );
        }
        if !config.exists() {
            continue;
        }
        let current = hash(&config)?;
        if entry.get("sha256").and_then(|v| v.as_str()) == Some(current.as_str()) {
            continue;
        }
        let backups = base.join("backups");
        fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
        let backup = super::super::paths::names::allocate(
            &backups,
            &format!("{}-ajustes", super::super::paths::names::slug(&game.name)),
            &format!("{}:{}:{}", string(directory), rel, current),
        )?;
        let output = backup.join(&rel);
        fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::copy(&config, &output).map_err(|e| e.to_string())?;
        entry["sha256"] = serde_json::Value::String(current);
        changed = true;
    }
    if !changed {
        return Ok(());
    }
    let temporary = directory.join(".peligames-daniel-journal.tmp");
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        out.write_all(&serde_json::to_vec_pretty(&journal).map_err(|e| e.to_string())?)
            .and_then(|_| out.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temporary, &path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

pub fn remove(
    base: &Path,
    game: &Game,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    let (directory, executable) = target(game)?;
    super::super::installed_library::ensure_path_writable(&directory)?;
    ensure_closed(&executable)?;
    if !owned(&directory, &executable) {
        return Err("Esta instalação não foi criada pelo teste do PeliGames.".into());
    }
    preserve_settings(base, &directory, game)?;
    progress("Restaurando arquivos originais e alterações do prefixo…".into());
    run(
        base,
        "manage.py",
        &["uninstall".into(), string(&directory)],
        &game_log(base, &directory, &game.name)?,
        progress,
    )?;
    fs::remove_file(directory.join(RECEIPT)).map_err(|e| e.to_string())
}

pub(super) fn game_log(base: &Path, directory: &Path, name: &str) -> Result<PathBuf, String> {
    super::super::paths::names::allocate(&base.join("logs"), name, &string(directory))
        .map(|p| p.with_extension("log"))
}

struct ChildGroup(std::process::Child);
impl Drop for ChildGroup {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.wait();
        }
    }
}

pub fn set_startup(game: &Game, enabled: bool) -> Result<(), String> {
    let (directory, executable) = target(game)?;
    super::super::installed_library::ensure_path_writable(&directory)?;
    ensure_closed(&executable)?;
    if !owned(&directory, &executable) {
        return Err("Esta instalação não pertence ao teste Daniel NR.".into());
    }
    let path = directory.join("OptiScaler.ini");
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("Ajustes Daniel NR não são um arquivo regular.".into());
    }
    let current = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut next = peligames_mod_config::ini::put(
        &current,
        "DlssNr",
        "Enabled",
        Some(if enabled { "true" } else { "false" }),
    );
    if runtimes::installed(&directory, &executable) == Some(runtimes::Runtime::V08) {
        next = peligames_mod_config::ini::put(
            &next,
            "Libraries",
            "NvngxPath",
            Some(&format!(
                "Z:{}",
                directory
                    .join(runtimes::HUB)
                    .join("neuralhub_host.dll")
                    .to_string_lossy()
                    .replace('/', "\\")
            )),
        );
        for (key, value) in [
            ("NeuralHubEnabled", "true"),
            ("NeuralHubNrSelected", "true"),
            ("NeuralHubNrEnabled", if enabled { "true" } else { "false" }),
        ] {
            next = peligames_mod_config::ini::put(&next, "DLSS", key, Some(value));
        }
    }
    let temporary = directory.join(".peligames-daniel-settings.tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(next.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::set_permissions(&temporary, meta.permissions()).map_err(|e| e.to_string())?;
        fs::rename(&temporary, &path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
