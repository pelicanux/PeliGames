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
        // Menu entry: Pelinstall without a file asks for one.
        [flag] if flag == "--install" => info.module = "pelinstall".into(),
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
    // Installed (extracted) AppImage: the command beside `app/` stays valid across updates.
    if running_installed() {
        let command = installed_command(&install_root()?);
        if command.is_file() { return Ok(command); }
    }
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
    if binary.extension().is_some_and(|ext|ext=="AppImage") || std::env::var_os("APPDIR").is_some() {
        command.env_remove("APPIMAGE").env_remove("APPDIR").env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD");
    }
    Ok(command)
}
/// Menu entries belong to the installed copy only: an AppImage opened from
/// Downloads never adds itself to the menu (the Install button does that).
pub fn register_appimage() -> Result<(), String> {
    let root = install_root()?;
    if !running_installed() {
        // Entries an earlier version created for a loose AppImage file.
        let applications = dirs::data_dir().ok_or("Pasta de aplicativos indisponível.")?.join("applications");
        remove_loose_entries(&applications, &installed_command(&root));
        return Ok(());
    }
    // This version opened: the one it replaced is no longer needed.
    let previous = root.join("app.previous");
    if fs::symlink_metadata(&previous).is_ok_and(|meta| meta.is_dir()) { let _ = fs::remove_dir_all(&previous); }
    register_appimage_at(&launcher_binary()?)
}
const MENU_ENTRIES: [&str; 2] = ["peligames-appimage.desktop", "pelinstall-appimage.desktop"];
/// Removes PeliGames' menu entries that do not start the installed command.
fn remove_loose_entries(applications: &Path, installed: &Path) {
    let installed = exec_arg(&installed.to_string_lossy()).unwrap_or_default();
    for name in MENU_ENTRIES {
        let path = applications.join(name);
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let starts_installed = text.lines().any(|line| line.strip_prefix("Exec=").is_some_and(|exec| !installed.is_empty() && exec.starts_with(&installed)));
        if !starts_installed { let _ = fs::remove_file(&path); }
    }
}
/// Menu entries (Games category) for PeliGames and Pelinstall pointing at `binary`.
fn register_appimage_at(binary: &Path) -> Result<(), String> {
    let data = dirs::data_dir().ok_or("Pasta de aplicativos indisponível.")?;
    let applications = data.join("applications"); let icons = data.join("peligames");
    fs::create_dir_all(&applications).and_then(|_|fs::create_dir_all(&icons)).map_err(|e|e.to_string())?;
    let icon = icons.join("appimage-icon.svg");
    fs::write(&icon, include_bytes!("../../../public/peligames.svg")).map_err(|e|e.to_string())?;
    for (name, filename, arguments, extra) in [
        ("PeliGames", "peligames-appimage.desktop", "%u", "MimeType=x-scheme-handler/nxm;\n"),
        ("Pelinstall", "pelinstall-appimage.desktop", "--install %f", "MimeType=application/x-ms-dos-executable;application/x-msdownload;application/x-msi;\n"),
    ] {
        let text=format!("[Desktop Entry]\nVersion=1.0\nType=Application\nName={name}\nComment=Instalando jogos na base da humilhação\nExec={} {arguments}\nIcon={}\nTerminal=false\nCategories=Game;\n{extra}",exec_arg(&binary.to_string_lossy())?,desktop_value(&icon.to_string_lossy()));
        let temporary=applications.join(format!(".{}.desktop",uuid::Uuid::new_v4()));
        fs::write(&temporary,text).and_then(|_|fs::rename(&temporary,applications.join(filename))).map_err(|e|e.to_string())?;
    }
    Ok(())
}

// Installed AppImage, like AppManager's extracted mode: no root, no FUSE.
//   ~/.local/share/PeliGames/app/        the AppImage's contents (run through AppRun)
//   ~/.local/share/PeliGames/peligames   the command menus and shortcuts call
//   ~/.local/bin/peligames               link to that command, for the terminal
// The launcher's data stays in ~/.config/PeliGames, so resetting it keeps the program.

/// `~/.local/share/PeliGames`: the installed program, apart from its data.
pub fn install_root() -> Result<PathBuf, String> {
    dirs::data_dir().map(|data| data.join("PeliGames")).ok_or_else(|| "Pasta de aplicativos indisponível.".into())
}
fn installed_command(root: &Path) -> PathBuf { root.join("peligames") }
/// Whether this launcher runs from the installed (extracted) AppImage.
pub fn running_installed() -> bool {
    let (Ok(root), Ok(current)) = (install_root(), std::env::current_exe()) else { return false; };
    let (Ok(app), Ok(current)) = (root.join("app").canonicalize(), current.canonicalize()) else { return false; };
    current.starts_with(app)
}
#[derive(Serialize)]
pub struct AppImageStatus { pub appimage: bool, pub installed: bool, pub target: String }
/// Whether this launcher runs from an AppImage file, and whether it is the installed copy.
pub fn appimage_status() -> Result<AppImageStatus, String> {
    let installed = running_installed();
    let target = install_root()?.join("app");
    Ok(AppImageStatus { appimage: std::env::var_os("APPIMAGE").is_some() && !installed, installed, target: target.to_string_lossy().into_owned() })
}
/// Installs the running AppImage: extracts it in `~/.local/share/PeliGames/app`,
/// adds the commands and menu entries, moves the game shortcuts to the new
/// command and removes the AppImage file. Returns the command to start.
pub fn install_appimage() -> Result<PathBuf, String> {
    let source = PathBuf::from(std::env::var_os("APPIMAGE").ok_or("O PeliGames não está rodando como AppImage.")?);
    let bin = dirs::executable_dir().or_else(|| dirs::home_dir().map(|home| home.join(".local/bin"))).ok_or("Pasta pessoal indisponível.")?;
    let command = install_appimage_at(&source, &install_root()?, &bin)?;
    register_appimage_at(&command)?;
    refresh_game_shortcuts(&command);
    Ok(command)
}
/// Updates the installed AppImage from a downloaded one; the previous version
/// stays in `app.previous` until the new one opens. Returns the command to start.
pub fn update_installed(image: &Path) -> Result<PathBuf, String> {
    let root = install_root()?;
    extract_appimage(image, &root)?;
    Ok(installed_command(&root))
}
/// Puts the previous version back after an update that could not start.
pub fn restore_previous() -> Result<(), String> {
    let root = install_root()?;
    let (app, previous) = (root.join("app"), root.join("app.previous"));
    if !previous.is_dir() { return Err("Versão anterior indisponível.".into()); }
    let failed = root.join(format!(".app-failed-{}", uuid::Uuid::new_v4()));
    if app.exists() { fs::rename(&app, &failed).map_err(|e| e.to_string())?; }
    fs::rename(&previous, &app).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(failed);
    Ok(())
}
fn install_appimage_at(source: &Path, root: &Path, bin: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;
    let app = extract_appimage(source, root)?;
    // The command keeps the absolute path: AppRun finds its folder from `$0`, not from a link.
    let command = installed_command(root);
    let quoted = app.join("AppRun").to_string_lossy().replace('\'', "'\\''");
    let script = format!("#!/bin/sh\n# PeliGames instalado (AppImage extraído). Apague {} para remover.\nexec '{quoted}' \"$@\"\n", root.display().to_string().replace('\n', " "));
    let staged = root.join(format!(".peligames-{}", uuid::Uuid::new_v4()));
    let result = fs::write(&staged, script)
        .and_then(|_| fs::set_permissions(&staged, fs::Permissions::from_mode(0o755)))
        .and_then(|_| fs::rename(&staged, &command));
    if let Err(error) = result { let _ = fs::remove_file(&staged); return Err(error.to_string()); }
    // Terminal command: a link, never replacing a file of the user's own.
    if fs::create_dir_all(bin).is_ok() {
        let link = bin.join("peligames");
        match fs::symlink_metadata(&link) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => { let _ = std::os::unix::fs::symlink(&command, &link); }
            Ok(meta) if meta.file_type().is_symlink() => { let _ = fs::remove_file(&link); let _ = std::os::unix::fs::symlink(&command, &link); }
            _ => (),
        }
        // The 0.3.2 test build installed the AppImage file itself here.
        for legacy in ["PeliGames.AppImage", "PeliGames.AppImage.previous"] {
            let path = bin.join(legacy);
            if fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file()) { let _ = fs::remove_file(path); }
        }
    }
    // The contents are installed: the AppImage file is no longer needed (as a move).
    if !source.starts_with(root) && fs::symlink_metadata(source).is_ok_and(|meta| meta.is_file()) { let _ = fs::remove_file(source); }
    Ok(command)
}
/// Extracts `image` with its own runtime (`--appimage-extract`, no FUSE or
/// unsquashfs needed) and swaps it in as `root/app`, keeping the version it
/// replaces in `root/app.previous`.
fn extract_appimage(image: &Path, root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let staging = root.join(format!(".app-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        let output = Command::new(image).arg("--appimage-extract").current_dir(&staging)
            .env_remove("APPIMAGE").env_remove("APPDIR").env_remove("LD_LIBRARY_PATH").env_remove("LD_PRELOAD")
            .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).output()
            .map_err(|e| format!("Não foi possível extrair o AppImage: {e}"))?;
        let extracted = staging.join("squashfs-root");
        if !output.status.success() || !extracted.join("AppRun").is_file() {
            return Err(format!("Não foi possível extrair o AppImage: {}", String::from_utf8_lossy(&output.stderr).trim()));
        }
        let (app, previous) = (root.join("app"), root.join("app.previous"));
        if fs::symlink_metadata(&previous).is_ok_and(|meta| meta.is_dir()) { fs::remove_dir_all(&previous).map_err(|e| e.to_string())?; }
        // A running version keeps working after its folder is renamed.
        if app.exists() { fs::rename(&app, &previous).map_err(|e| e.to_string())?; }
        if let Err(error) = fs::rename(&extracted, &app) {
            if previous.is_dir() { let _ = fs::rename(&previous, &app); }
            return Err(error.to_string());
        }
        Ok(app)
    })();
    let _ = fs::remove_dir_all(&staging);
    result
}
/// Desktop and menu shortcuts of library games now start `launcher`.
fn refresh_game_shortcuts(launcher: &Path) {
    let Ok(entries) = super::installed_library::list(false) else { return; };
    for entry in entries {
        let icon = super::executable_icon::shortcut_icon(Path::new(&entry.executable));
        let icon = icon.map(|path| path.to_string_lossy().into_owned()).unwrap_or_else(|| "applications-games".into());
        let _ = update_shortcuts_at(&shortcut_directories(), &entry.path, &entry.name, launcher, &icon);
    }
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

#[cfg(test)]
mod appimage_install_tests {
    use super::*;
    #[test]
    fn loose_appimage_menu_entries_are_removed_and_installed_ones_kept() {
        let base = std::env::temp_dir().join(format!("peligames-menu-{}", uuid::Uuid::new_v4()));
        let applications = base.join("applications");
        fs::create_dir_all(&applications).unwrap();
        let installed = base.join("data/PeliGames/peligames");
        let entry = |exec: &str| format!("[Desktop Entry]\nType=Application\nName=PeliGames\nExec={exec} %u\n");
        // Left by an AppImage opened from Downloads: removed.
        fs::write(applications.join("peligames-appimage.desktop"), entry("\"/home/user/Downloads/PeliGames_0.3.4_amd64.AppImage\"")).unwrap();
        // Pointing at the installed command: kept.
        fs::write(applications.join("pelinstall-appimage.desktop"), entry(&exec_arg(&installed.to_string_lossy()).unwrap())).unwrap();
        // Another program's entry: never touched.
        fs::write(applications.join("other.desktop"), entry("\"/home/user/Downloads/Other.AppImage\"")).unwrap();
        remove_loose_entries(&applications, &installed);
        assert!(!applications.join("peligames-appimage.desktop").exists());
        assert!(applications.join("pelinstall-appimage.desktop").exists());
        assert!(applications.join("other.desktop").exists());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn appimage_installs_extracted_with_a_command_and_updates_in_place() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("peligames-appimage-{}", uuid::Uuid::new_v4()));
        // A stand-in AppImage: its runtime only needs to answer --appimage-extract.
        let image = |name: &str, version: &str| {
            let path = base.join("Downloads").join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, format!("#!/bin/sh\n[ \"$1\" = --appimage-extract ] || exit 1\nmkdir -p squashfs-root/usr/bin\nprintf '#!/bin/sh\\n' > squashfs-root/AppRun\nchmod +x squashfs-root/AppRun\necho {version} > squashfs-root/version\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        };
        let (root, bin) = (base.join("share/PeliGames"), base.join("bin"));
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("PeliGames.AppImage"), b"0.3.2").unwrap();
        std::os::unix::fs::symlink("PeliGames.AppImage", bin.join("peligames")).unwrap();
        let source = image("PeliGames_0.3.3_amd64.AppImage", "1");
        let command = install_appimage_at(&source, &root, &bin).unwrap();
        assert_eq!(fs::read_to_string(root.join("app/version")).unwrap().trim(), "1");
        assert!(!source.exists(), "o AppImage some depois de extraído");
        assert!(!bin.join("PeliGames.AppImage").exists(), "a cópia da 0.3.2 sai de ~/.local/bin");
        assert_eq!(fs::read_link(bin.join("peligames")).unwrap(), command);
        assert!(fs::read_to_string(&command).unwrap().contains(&format!("exec '{}'", root.join("app/AppRun").display())));
        assert_eq!(fs::metadata(&command).unwrap().permissions().mode() & 0o777, 0o755);
        // An update swaps the folder and keeps the version it replaced.
        extract_appimage(&image("PeliGames_0.3.4_amd64.AppImage", "2"), &root).unwrap();
        assert_eq!(fs::read_to_string(root.join("app/version")).unwrap().trim(), "2");
        assert_eq!(fs::read_to_string(root.join("app.previous/version")).unwrap().trim(), "1");
        // A broken image changes nothing; no staging folder is left behind.
        let broken = base.join("Downloads/broken.AppImage");
        fs::write(&broken, "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(&broken, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(extract_appimage(&broken, &root).is_err());
        assert_eq!(fs::read_to_string(root.join("app/version")).unwrap().trim(), "2");
        assert!(fs::read_dir(&root).unwrap().all(|entry| !entry.unwrap().file_name().to_string_lossy().starts_with(".app-")));
        // A user's own `peligames` file is never replaced.
        fs::remove_file(bin.join("peligames")).unwrap();
        fs::write(bin.join("peligames"), b"#!/bin/sh").unwrap();
        install_appimage_at(&image("again.AppImage", "3"), &root, &bin).unwrap();
        assert_eq!(fs::read(bin.join("peligames")).unwrap(), b"#!/bin/sh");
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn pelinstall_opens_without_a_file_from_the_menu() {
        assert_eq!(parse_args(&["--install".into()], false).unwrap().module, "pelinstall");
    }
}
