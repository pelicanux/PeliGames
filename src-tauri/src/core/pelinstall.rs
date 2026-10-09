//! System entry points, desktop shortcuts and monitored background launches.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime},
};

#[derive(Clone, Default, Serialize)]
pub struct StartupInfo {
    pub module: String,
    pub executable: Option<String>,
    pub entry: Option<String>,
    pub error: Option<String>,
    pub log: Option<String>,
}
pub fn parse_args(args: &[String], installer: bool) -> Result<StartupInfo, String> {
    let mut info = StartupInfo {
        module: if installer { "pelinstall" } else { "launcher" }.into(),
        ..Default::default()
    };
    match args {
        [] => (),
        [flag] if flag == "--launcher" => info.module = "launcher".into(),
        [flag, file] if flag == "--install" => { info.module = "pelinstall".into(); info.executable = Some(file.clone()); },
        [flag, entry] if flag == "--launch-game" => { info.module = "background".into(); info.entry = Some(entry.clone()); },
        [flag, entry] if flag == "--show-game" => { info.module = "launcher".into(); info.entry = Some(entry.clone()); },
        [file] if !file.starts_with('-') => { info.module = "pelinstall".into(); info.executable = Some(file.clone()); },
        _ => return Err("Use pelinstall /caminho/instalador.exe, peligames --install ARQUIVO ou peligames --launch-game EXECUTÁVEL_REGISTRADO.".into()),
    }
    if let Some(file) = &info.executable {
        let absolute = if Path::new(file).is_absolute() {
            PathBuf::from(file)
        } else {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(file)
        };
        info.executable = Some(absolute.to_string_lossy().into_owned());
        if let Err(error) =
            super::game_installation::validate_executable(info.executable.as_deref().unwrap())
        {
            info.error = Some(error);
        }
    }
    Ok(info)
}

// Escape Desktop Entry values and Exec arguments, never shell-interpolate paths.
fn desktop_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
pub(crate) fn exec_arg(value: &str) -> Result<String, String> {
    if value.contains(['\0', '\n', '\r']) {
        return Err("Caminho inválido para o atalho.".into());
    }
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\\\\\"),
            '"' | '`' | '$' => {
                quoted.push_str("\\\\");
                quoted.push(ch);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    Ok(quoted)
}
pub fn launcher_binary() -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    if let (Some(image), Some(appdir)) = (std::env::var_os("APPIMAGE"), std::env::var_os("APPDIR")) {
        if let Some(image) = appimage_binary(&current, Path::new(&appdir), Path::new(&image)) { return Ok(image); }
    }
    for name in ["peligames", "PeliGames", "dlssnr-x-amd"] {
        let candidate = current
            .parent()
            .ok_or("Pasta do programa indisponível.")?
            .join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    // Pelinstall contains the same shared launcher runtime as the main binary.
    Ok(current)
}
fn appimage_binary(current: &Path, appdir: &Path, image: &Path) -> Option<PathBuf> {
    if !image.is_absolute() || !image.is_file() { return None; }
    let directory = appdir.canonicalize().ok()?;
    current.canonicalize().ok()?.starts_with(directory).then(||image.to_path_buf())
}
fn launcher_command() -> Result<Command, String> {
    let binary = launcher_binary()?;
    let mut command = Command::new(&binary);
    if binary.extension().is_some_and(|ext|ext=="AppImage") {
        command.env_remove("APPIMAGE").env_remove("APPDIR").env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD");
    }
    Ok(command)
}
pub fn register_appimage() -> Result<(), String> {
    let binary = launcher_binary()?;
    if !binary.extension().is_some_and(|ext|ext=="AppImage") { return Ok(()); }
    let data = dirs::data_dir().ok_or("Pasta de aplicativos indisponível.")?;
    let applications = data.join("applications"); let icons = data.join("peligames");
    fs::create_dir_all(&applications).and_then(|_|fs::create_dir_all(&icons)).map_err(|e|e.to_string())?;
    let icon = icons.join("appimage-icon.svg");
    fs::write(&icon, include_bytes!("../../../public/peligames.svg")).map_err(|e|e.to_string())?;
    for (name, filename, arguments, extra) in [
        ("PeliGames", "peligames-appimage.desktop", "%u", "MimeType=x-scheme-handler/nxm;\n"),
        ("Pelinstall", "pelinstall-appimage.desktop", "--install %f", "NoDisplay=true\nMimeType=application/x-ms-dos-executable;application/x-msdownload;application/x-msi;\n"),
    ] {
        let text=format!("[Desktop Entry]\nVersion=1.0\nType=Application\nName={name}\nExec={} {arguments}\nIcon={}\nTerminal=false\nCategories=Game;\n{extra}",exec_arg(&binary.to_string_lossy())?,desktop_value(&icon.to_string_lossy()));
        let temporary=applications.join(format!(".{}.desktop",uuid::Uuid::new_v4()));
        fs::write(&temporary,text).and_then(|_|fs::rename(&temporary,applications.join(filename))).map_err(|e|e.to_string())?;
    }
    Ok(())
}
fn shortcut_text(name: &str, entry: &str, launcher: &Path, icon: &str) -> Result<String, String> {
    Ok(format!("[Desktop Entry]\nVersion=1.0\nType=Application\nName={}\nComment=Executar com PeliGames\nExec={} --launch-game {}\nTerminal=false\nIcon={}\nCategories=Game;\n", desktop_value(name), exec_arg(&launcher.to_string_lossy())?, exec_arg(entry)?, desktop_value(icon)))
}
fn write_shortcut_at(
    directory: &Path,
    name: &str,
    entry: &str,
    launcher: &Path,
    icon: &str,
) -> Result<PathBuf, String> {
    use std::io::Write;
    let text = shortcut_text(name, entry, launcher, icon)?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    // A unique file never overwrites another user's desktop shortcut.
    let path = directory.join(format!("peligames-{}.desktop", uuid::Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(path)
}
// Search only standard per-user shortcut folders, never the entire home directory.
pub(crate) fn shortcut_directories() -> Vec<PathBuf> {
    let mut directories = vec![];
    if let Some(desktop) = dirs::desktop_dir().or_else(|| dirs::home_dir().map(|home| home.join("Desktop"))) { directories.push(desktop); }
    if let Some(data) = dirs::data_dir() { directories.push(data.join("applications")); }
    if let Some(home) = dirs::home_dir() {
        // Include the former default desktop if XDG_DESKTOP_DIR was changed.
        directories.push(home.join("Desktop"));
        directories.push(home.join(".local/share/applications"));
        directories.push(home.join("Applications"));
    }
    directories.sort(); directories.dedup(); directories
}
fn is_entry_shortcut(file: &Path, entry: &str) -> Result<bool, String> {
    if file.extension().and_then(|ext| ext.to_str()) != Some("desktop") { return Ok(false); }
    let metadata = fs::symlink_metadata(file).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 65536 { return Ok(false); }
    let mut options = fs::OpenOptions::new(); options.read(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW); }
    let opened = options.open(file).map_err(|e| e.to_string())?;
    let mut text = String::new();
    if let Err(error) = opened.take(65537).read_to_string(&mut text) {
        if error.kind() == std::io::ErrorKind::InvalidData { return Ok(false); }
        return Err(error.to_string());
    }
    if text.len() > 65536 { return Ok(false); }
    shortcut_matches_text(&text, entry)
}
// Desktop entries may be rewritten by a desktop editor with unquoted arguments.
fn desktop_exec_args(command: &str) -> Option<Vec<String>> {
    let mut args = Vec::new(); let mut token = String::new();
    let mut quoted = false; let mut escaped = false; let mut started = false;
    for ch in command.chars() {
        if escaped { token.push(ch); escaped = false; started = true; }
        else if ch == '\\' { escaped = true; }
        else if ch == '"' { quoted = !quoted; started = true; }
        else if ch.is_whitespace() && !quoted { if started { args.push(token.replace("%%", "%")); token.clear(); started = false; } }
        else { token.push(ch); started = true; }
    }
    if escaped || quoted { return None; }
    if started { args.push(token.replace("%%", "%")); }
    Some(args)
}
fn shortcut_matches_text(text: &str, entry: &str) -> Result<bool, String> {
    let mut section = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') { section = line == "[Desktop Entry]"; continue; }
        if section {
            if let Some(command) = line.strip_prefix("Exec=") {
                if let Some(args) = desktop_exec_args(command) {
                    let launcher = args.first().and_then(|arg| Path::new(arg).file_name()).and_then(|name| name.to_str());
                    if launcher.is_some_and(|name| matches!(name, "peligames" | "pelinstall" | "PeliGames" | "Pelinstall") || (name.to_ascii_lowercase().starts_with("peligames") && name.ends_with(".AppImage"))) && args.len() == 3 && args[1] == "--launch-game" && args[2] == entry { return Ok(true); }
                }
            }
        }
    }
    Ok(false)
}
fn shortcut_paths_at(directories: &[PathBuf], entry: &str) -> Result<Vec<String>, String> {
    let mut found = std::collections::BTreeSet::new();
    for directory in directories {
        if !directory.exists() { continue; }
        let canonical = directory.canonicalize().map_err(|e| e.to_string())?;
        for file in fs::read_dir(canonical).map_err(|e| e.to_string())? {
            let path = file.map_err(|e| e.to_string())?.path();
            if is_entry_shortcut(&path, entry)? { found.insert(path.to_string_lossy().into_owned()); }
        }
    }
    Ok(found.into_iter().collect())
}
pub fn shortcut_paths(entry: &str) -> Result<Vec<String>, String> { shortcut_paths_at(&shortcut_directories(), entry) }
pub(crate) fn remove_shortcuts_at(directories: &[PathBuf], entry: &str) -> Result<usize, String> {
    let found = shortcut_paths_at(directories, entry)?; let mut removed = 0;
    for file in found {
        let path = Path::new(&file);
        if is_entry_shortcut(path, entry)? { fs::remove_file(path).map_err(|e| e.to_string())?; removed += 1; }
    }
    Ok(removed)
}
pub fn remove_shortcuts(entry: &str) -> Result<usize, String> {
    if !super::installed_library::list(false)?.iter().any(|game| game.path == entry) { return Err("Jogo não registrado na biblioteca.".into()); }
    remove_shortcuts_at(&shortcut_directories(), entry)
}
// Rewrite only shortcuts owned by this exact library entry, preserving their paths.
fn update_shortcuts_at(directories: &[PathBuf], entry: &str, name: &str, launcher: &Path, icon: &str) -> Result<usize, String> {
    use std::io::Write;
    let text = shortcut_text(name, entry, launcher, icon)?;
    let mut updated = 0;
    for path in shortcut_paths_at(directories, entry)? {
        let mut options = fs::OpenOptions::new(); options.read(true).write(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW); }
        let mut file = options.open(path).map_err(|e| e.to_string())?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > 65536 { continue; }
        let mut current = String::new();
        (&mut file).take(65537).read_to_string(&mut current).map_err(|e| e.to_string())?;
        if !shortcut_matches_text(&current, entry)? { continue; }
        file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.set_len(text.len() as u64).map_err(|e| e.to_string())?;
        updated += 1;
    }
    Ok(updated)
}
pub fn update_shortcuts(path: &str) -> Result<usize, String> {
    let entry = super::installed_library::list(false)?.into_iter().find(|entry| entry.path == path)
        .ok_or("Jogo não registrado na biblioteca.")?;
    let icon = super::executable_icon::shortcut_icon(Path::new(&entry.executable));
    let icon = icon.map(|path| path.to_string_lossy().into_owned()).unwrap_or_else(|| "applications-games".into());
    update_shortcuts_at(&shortcut_directories(), &entry.path, &entry.name, &launcher_binary()?, &icon)
}
pub fn create_shortcut(path: &str) -> Result<String, String> {
    let entry = super::installed_library::list(false)?
        .into_iter()
        .find(|entry| entry.path == path)
        .ok_or("Jogo não registrado na biblioteca.")?;
    let desktop = dirs::desktop_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Desktop")))
        .ok_or("Área de trabalho indisponível.")?;
    let icon = super::executable_icon::shortcut_icon(Path::new(&entry.executable));
    let icon = icon.as_ref().map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "applications-games".into());
    write_shortcut_at(&desktop, &entry.name, &entry.path, &launcher_binary()?, &icon)
        .map(|path| path.to_string_lossy().into_owned())
}
pub fn open_launcher(entry: Option<&str>) -> Result<(), String> {
    #[cfg(unix)]
    if super::launcher_instance::focus_running(entry)? { return Ok(()); }
    let mut command = launcher_command()?;
    if let Some(entry) = entry {
        command.arg("--show-game").arg(entry);
    } else {
        command.arg("--launcher");
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Não foi possível abrir o PeliGames: {e}"))?;
    Ok(())
}

pub fn launch_entry(path: &str) -> Result<(), String> {
    if !super::installed_library::list(false)?.iter().any(|e| e.path == path) { return Err("Executável não registrado.".into()); }
    launcher_command()?.arg("--launch-game").arg(path)
        .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .spawn().map_err(|e| format!("Não foi possível iniciar: {e}"))?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct LaunchReport {
    pub entry: String,
    pub error: String,
    pub log: Option<String>,
}
fn report_path(entry: &str) -> Result<PathBuf, String> {
    Ok(super::paths::app_root()?.join("launch-reports")
        .join(format!("{:x}.json", Sha256::digest(entry.as_bytes()))))
}
pub fn save_report(report: &LaunchReport) -> Result<(), String> {
    let path = report_path(&report.entry)?;
    let dir = path.parent().ok_or("Registro inválido.")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let temp = dir.join(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(
        &temp,
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}
pub fn report_for(entry: &str) -> Option<LaunchReport> {
    let bytes = fs::read(report_path(entry).ok()?).ok()?;
    let report: LaunchReport = serde_json::from_slice(&bytes).ok()?;
    (report.entry == entry).then_some(report)
}
fn fatal_session_logs(log: &str, since: SystemTime) -> Option<String> {
    if let Some(error) = fatal_log(log) {
        return Some(error);
    }
    // Proton also writes steam-*.log separately from UMU's stdout/stderr.
    let logs = Path::new(log).parent()?;
    for file in fs::read_dir(logs).ok()?.flatten().take(128) {
        if !file.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let name = file.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".log") || name.starts_with("installer-") {
            continue;
        }
        if !file
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified >= since)
        {
            continue;
        }
        if let Some(error) = fatal_log(file.path().to_str()?) {
            return Some(error);
        }
    }
    None
}
fn fatal_log(log: &str) -> Option<String> {
    let mut file = fs::File::open(log).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(65536)))
        .ok()?;
    let mut bytes = Vec::new();
    file.take(65536).read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
    [
        "wine: could not load kernel32.dll",
        "wine: failed to load",
        "unhandled exception",
        "fatal error:",
    ]
    .into_iter()
    .find(|needle| text.contains(needle))
    .map(|_| {
        "O log registrou uma possível falha durante a execução. Verifique os detalhes no PeliGames."
            .into()
    })
}
pub fn monitor_background(entry: &str) -> Result<(), LaunchReport> {
    let report = |error: String, log: Option<String>| LaunchReport {
        entry: entry.into(),
        error,
        log,
    };
    let started_at = SystemTime::now();
    let initial =
        super::game_execution::start(entry.into()).map_err(|error| report(error, None))?;
    // A repeated desktop invocation may resolve to a monitor in another process.
    if super::game_execution::local_status().ok().flatten().is_none_or(|s| s.id != initial.id) {
        if let Ok(path) = report_path(entry) { let _ = fs::remove_file(path); }
        return Ok(());
    }
    loop {
        let state = super::game_execution::local_status()
            .map_err(|error| report(error, Some(initial.log.clone())))?
            .ok_or_else(|| {
                report(
                    "Monitor de execução indisponível.".into(),
                    Some(initial.log.clone()),
                )
            })?;
        if !state.active() {
            if state.state == "cancelled" {
                if let Ok(path) = report_path(entry) { let _ = fs::remove_file(path); }
                return Ok(());
            }
            if let Some(error) = state.error {
                let detail = fatal_session_logs(&state.log, started_at).map(|hint| format!("{error} {hint}")).unwrap_or(error);
                return Err(report(detail, Some(state.log)));
            }
            if let Ok(path) = report_path(entry) {
                let _ = fs::remove_file(path);
            }
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
