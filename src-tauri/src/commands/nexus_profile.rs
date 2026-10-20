//! Valheim native BepInEx profiles: owned files only, never deploy into the game.
use super::nexus_local::NexusGame;
use std::{
    collections::HashSet,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
};

pub(super) fn adapter(game: &NexusGame) -> String {
    game.game["directory"].as_str()
        .and_then(|root| super::nexus_discovery::at(Path::new(root)))
        .filter(|definition| definition.platform == game.platform)
        .map(|definition| definition.domain.to_string()).unwrap_or_default()
}
pub(super) fn directory(game: &NexusGame) -> Result<PathBuf, String> {
    super::nexus_storage::directory(game)
}
pub(super) fn profile(game: &NexusGame) -> Result<PathBuf, String> {
    let revision = game
        .profile
        .as_deref()
        .ok_or("Instale o BepInEx e os mods primeiro.")?;
    uuid::Uuid::parse_str(revision).map_err(|_| "Perfil inválido.")?;
    Ok(directory(game)?.join("profiles").join(revision))
}
pub(super) fn executable(game: &NexusGame) -> Result<PathBuf, String> {
    if game.platform != "native" {
        return Err("A instalação de mods nesta etapa suporta Valheim nativo Linux. Proton ainda não está habilitado.".into());
    }
    let folder = game
        .game
        .get("directory")
        .and_then(|v| v.as_str())
        .ok_or("Pasta do jogo ausente.")?;
    if !super::nexus_discovery::at(Path::new(folder)).is_some_and(|definition| definition.domain == "valheim") {
        return Err("A pasta não contém os arquivos exigidos do Valheim nativo Linux.".into());
    }
    let exe = fs::canonicalize(Path::new(folder).join("valheim.x86_64")).map_err(|_| {
        "Este perfil suporta o Valheim nativo Linux. Escolha sua pasta de instalação.".to_string()
    })?;
    let mut header = [0u8; 4];
    fs::File::open(&exe)
        .and_then(|mut f| f.read_exact(&mut header))
        .map_err(|e| e.to_string())?;
    if header != *b"\x7fELF" {
        return Err("O executável não é o Valheim nativo Linux.".into());
    }
    Ok(exe)
}
fn live_pid(game: &NexusGame) -> Option<i32> {
    let pid: i32 = fs::read_to_string(directory(game).ok()?.join("session.pid"))
        .ok()?
        .trim()
        .parse()
        .ok()?;
    if pid <= 1 {
        return None;
    }
    let expected = format!(
        "DOORSTOP_TARGET_ASSEMBLY={}/BepInEx/core/BepInEx.Preloader.dll",
        profile(game).ok()?.display()
    );
    let env = fs::read(format!("/proc/{pid}/environ")).ok()?;
    let script = profile(game)
        .ok()?
        .join("start_game_bepinex.sh")
        .to_string_lossy()
        .to_string();
    let cmdline = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    if env.split(|b| *b == 0).any(|v| v == expected.as_bytes())
        || cmdline.split(|b| *b == 0).any(|v| v == script.as_bytes())
    {
        Some(pid)
    } else {
        None
    }
}
pub(super) fn running(game: &NexusGame) -> bool {
    if super::nexus_deployment::supports(game) {
        return super::nexus_deployment::running(game);
    }
    live_pid(game).is_some()
}
/// `running`, for a refresh that checks every game against one process snapshot.
pub(super) fn running_in(snapshot: &super::nexus_deployment::ProcessSnapshot, game: &NexusGame) -> bool {
    if super::nexus_deployment::supports(game) {
        return super::nexus_deployment::running_in(snapshot, game);
    }
    live_pid(game).is_some()
}
pub(super) fn idle(game: &NexusGame) -> Result<(), String> {
    if super::nexus_deployment::supports(game) {
        return super::nexus_deployment::idle(game);
    }
    if running(game) {
        Err("Feche o jogo antes de alterar os mods.".into())
    } else {
        Ok(())
    }
}
fn config_copy(old: &Path, new: &Path) -> Result<(), String> {
    if !old.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(old) {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_symlink() {
            return Err("Link recusado na configuração do perfil.".into());
        }
        let dest = new.join(entry.path().strip_prefix(old).map_err(|e| e.to_string())?);
        if entry.file_type().is_dir() {
            fs::create_dir_all(dest).map_err(|e| e.to_string())?;
        } else {
            fs::copy(entry.path(), dest).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
/// Build a new revision before changing library.json. A failed extraction leaves the old profile usable.
pub(super) fn build(game: &NexusGame) -> Result<String, String> {
    if super::nexus_deployment::supports(game) {
        return super::nexus_deployment::build(game);
    }
    executable(game)?;
    idle(game)?;
    let definition = super::nexus_discovery::at(Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?))
        .ok_or("Módulo do jogo não identificado.")?;
    let revision = uuid::Uuid::new_v4().to_string();
    let destination = directory(game)?.join("profiles").join(&revision);
    fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
    let result = (|| {
        let mut written = HashSet::new();
        let mut total = 0u64;
        for item in game.mods.iter().filter(|m| m.enabled) {
            uuid::Uuid::parse_str(&item.id).map_err(|_| "Mod inválido.")?;
            let mut zip = super::nexus_archive::open(game,item)?;
            let selected = if let Some(files) = item.installed_files.as_ref().or(item.fomod_files.as_ref()) {
                super::nexus_fomod::validate_installation_files(&mut zip,&definition,files)?;
                Some(files.clone())
            } else {
                let resolved = super::nexus_fomod::resolve(&mut zip,&definition,Path::new(game.game["directory"].as_str().unwrap()),&item.id,item.fomod_selection.as_deref(),Some(&super::nexus_loot::PluginOracle::new(game,&definition)))?;
                if !resolved.issues.is_empty() {return Err(resolved.issues.join("\n"));}
                resolved.files
            };
            if zip.len() > 10000 {
                return Err("Arquivo com entradas demais.".into());
            }
            let entries = (0..zip.len()).map(|index| zip.by_index(index)
                .map(|entry| entry.name().replace('\\', "/")).map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let mut recognized = false;
            for index in 0..zip.len() {
                let entry = zip.by_index(index).map_err(|e| e.to_string())?;
                let name = entry.name().replace('\\', "/");
                if name.starts_with('/')
                    || name.split('/').any(|v| v == ".." || v.contains(':'))
                    || entry
                        .unix_mode()
                        .map(|m| m & 0o170000 == 0o120000)
                        .unwrap_or(false)
                {
                    return Err(format!("Caminho inseguro no ZIP: {name}"));
                }
                if entry.is_dir() {
                    continue;
                }
                drop(entry);
                let outputs = if let Some(selected) = &selected {selected.iter().filter(|(source,_)|source == &name).map(|(_,target)|target.clone()).collect::<Vec<_>>()} else {definition.route_package(&name,&item.id,&entries)?.into_iter().collect()};
                for output in outputs {
                let entry = zip.by_index(index).map_err(|e|e.to_string())?;
                if output.ends_with(".dll") || output.ends_with(".so") {
                    recognized = true;
                }
                total = total
                    .checked_add(entry.size())
                    .ok_or("Arquivo grande demais.")?;
                if entry.size() > 100 * 1024 * 1024 || total > 512 * 1024 * 1024 {
                    return Err("O perfil excede o limite de extração de 512 MB.".into());
                }
                if !written.insert(output.clone()) {
                    return Err(format!(
                        "Dois arquivos tentam substituir o mesmo caminho: {output}"
                    ));
                }
                let path = destination.join(&output);
                fs::create_dir_all(path.parent().ok_or("Caminho inválido.")?)
                    .map_err(|e| e.to_string())?;
                let mut target = fs::File::create(path).map_err(|e| e.to_string())?;
                let declared = entry.size();
                let copied = std::io::copy(&mut entry.take(declared + 1), &mut target)
                    .map_err(|e| e.to_string())?;
                if copied != declared {
                    return Err("Tamanho inválido na entrada do ZIP.".into());
                }
                }
            }
            if !recognized {
                return Err(format!(
                    "{} não contém um pacote BepInEx compatível com este perfil.",
                    item.name
                ));
            }
        }
        if let Ok(previous) = profile(game) {
            config_copy(
                &previous.join("BepInEx/config"),
                &destination.join("BepInEx/config"),
            )?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = destination.join("start_game_bepinex.sh");
            if script.exists() {
                fs::set_permissions(script, fs::Permissions::from_mode(0o755))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&destination);
        return Err(error);
    }
    Ok(revision)
}
pub(super) fn discard(game: &NexusGame, revision: &str) {
    if uuid::Uuid::parse_str(revision).is_ok() {
        if let Ok(dir) = directory(game) {
            let _ = fs::remove_dir_all(dir.join("profiles").join(revision));
            // Keep VFS writable layers: they can contain game-generated/user files.
        }
    }
}
pub(super) fn launch(game: &NexusGame) -> Result<(), String> {
    super::steam_readiness::check(
        Path::new(game.game["directory"].as_str().unwrap_or("")),
        game.game["app_id"].as_str(),
    )?;
    if super::nexus_deployment::supports(game) {
        return super::nexus_deployment::launch(game);
    }
    idle(game)?;
    let exe = executable(game)?;
    // Avoid a second Valheim process, including one started directly by Steam.
    if let Ok(entries) = fs::read_dir("/proc") {
        for entry in entries.filter_map(Result::ok) {
            if entry.file_name().to_string_lossy().parse::<u32>().is_ok()
                && fs::read_link(entry.path().join("exe")).ok().as_ref() == Some(&exe)
            {
                return Err(
                    "O Valheim já está aberto. Feche-o antes de iniciar este perfil.".into(),
                );
            }
        }
    }
    if game.profile.is_none() && !game.mods.iter().any(|item|item.installed) {
        let mut command=Command::new(&exe);
        command.current_dir(exe.parent().ok_or("Pasta inválida.")?).env("SteamAppId","892970").env("SteamGameId","892970");
        super::nexus_launch_options::apply(game,&mut command,exe.parent().ok_or("Pasta inválida.")?)?;
        return spawn(game,command);
    }
    let runtime = profile(game)?;
    for required in [
        "start_game_bepinex.sh",
        "doorstop_libs/libdoorstop_x64.so",
        "BepInEx/core/BepInEx.Preloader.dll",
    ] {
        if !runtime.join(required).is_file() {
            return Err(
                "Instale e ative o BepInExPack_Valheim com suporte a Linux antes de iniciar."
                    .into(),
            );
        }
    }
    let plugins: Vec<_> = walkdir::WalkDir::new(runtime.join("BepInEx/plugins"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    if plugins.iter().any(|p| p == "CDEV.EquipmentSlots.dll")
        && !plugins.iter().any(|p| p == "Jotunn.dll")
    {
        return Err("EquipmentSlots exige Jötunn. Instale e ative essa dependência.".into());
    }
    let mut command = Command::new(runtime.join("start_game_bepinex.sh"));
    command
        .arg(&exe)
        .current_dir(exe.parent().ok_or("Pasta inválida.")?)
        .env("SteamAppId", "892970")
        .env("SteamGameId", "892970")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD")
        .env_remove("APPIMAGE")
        .env_remove("APPDIR");
    super::nexus_launch_options::apply(game, &mut command, exe.parent().ok_or("Pasta inválida.")?)?;
    spawn(game, command)
}
/// Files of the launch sessions shown by the log window (not the event history).
const SESSION_FILES: [&str; 5] = ["current.log", "previous.log", "tool.log", "session-started.txt", "session-started-ms.txt"];
/// Starts the launcher with empty logs: the next launch begins a new session.
pub(super) fn clear_session_logs(game: &NexusGame) -> Result<(), String> {
    clear_session_files(&directory(game)?.join("logs"))
}
fn clear_session_files(logs: &Path) -> Result<(), String> {
    for name in SESSION_FILES {
        let file = logs.join(name);
        // Only regular files; a link is never followed or removed here.
        if fs::symlink_metadata(&file).is_ok_and(|meta| meta.is_file()) {
            fs::remove_file(&file).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn rotate_execution(logs: &Path) -> Result<(), String> {
    let current = logs.join("current.log");
    let previous = logs.join("previous.log");
    if current.exists() {
        if previous.exists() { fs::remove_file(&previous).map_err(|e| e.to_string())?; }
        fs::rename(current, previous).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub(super) fn spawn(game: &NexusGame, command: Command) -> Result<(), String> {
    spawn_with_session(game, command, true)
}
pub(super) fn spawn_tool(game: &NexusGame, command: Command) -> Result<(), String> {
    spawn_with_session(game, command, false)
}
fn spawn_with_session(game: &NexusGame, command: Command, new_session: bool) -> Result<(), String> {
    let mut command = super::nexus_launch_options::advanced_command(game,command)?;
    let dir = directory(game)?;
    let logs = dir.join("logs");
    fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let current = logs.join(if new_session { "current.log" } else { "tool.log" });
    if new_session { rotate_execution(&logs)?; }
    if new_session { super::nexus_loader_logs::reset(game)?; }
    let mut log = fs::File::create(&current).map_err(|e| e.to_string())?;
    // Wine exposes DLL load provenance and exception frames without the full Proton trace.
    if game.platform != "native" {
        command.env("WINEDEBUG", "-all,err+all,warn+seh,trace+loaddll");
    }
    let context = super::nexus_reports::launch_context(game, &command);
    log.write_all(context.as_bytes()).map_err(|e|e.to_string())?;
    if new_session {
        let started = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?;
        fs::write(logs.join("session-started.txt"), started.as_secs().to_string()).map_err(|e|e.to_string())?;
        fs::write(logs.join("session-started-ms.txt"), started.as_millis().to_string()).map_err(|e|e.to_string())?;
    }
    drop(log);
    command
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD")
        .env_remove("APPIMAGE")
        .env_remove("APPDIR")
        .env("PROTON_LOG","0");
    crate::core::bounded_logs::capture(&mut command,&current)?;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // The game must survive closing the launcher or its parent terminal.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let mut child = command.spawn().map_err(|e| {
        let detail = format!("Não foi possível iniciar o jogo: {e}");
        super::nexus_reports::event(game,"ERROR","criar processo",&detail);
        detail
    })?;
    let pid = child.id();
    super::nexus_reports::event(game,"INFO","processo iniciado",&format!("PID={pid}; contexto e DLLs em {}", current.display()));
    if let Err(e) = fs::write(dir.join("session.pid"), pid.to_string()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e.to_string());
    }
    let diagnostic = game.clone();
    std::thread::spawn(move || {
        match child.wait() {
            Ok(status) => super::nexus_reports::event(&diagnostic,if status.success(){"INFO"}else{"ERROR"},"processo finalizado",&status.to_string()),
            Err(error) => super::nexus_reports::event(&diagnostic,"ERROR","aguardar processo",&error.to_string()),
        }
        /* Keep PID file: /proc verification handles stale or reused PIDs. */
    });
    Ok(())
}
pub(super) fn stop(game: &NexusGame) -> Result<(), String> {
    if super::nexus_deployment::supports(game) {
        return super::nexus_deployment::stop(game);
    }
    let pid = live_pid(game).ok_or("Este perfil não está em execução.")?;
    #[cfg(unix)]
    if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}
pub(super) fn logs(game: &NexusGame, previous: bool) -> Result<String, String> {
    let file = directory(game)?.join("logs").join(if previous {
        "previous.log"
    } else {
        "current.log"
    });
    if !file.exists() {
        return Ok(if previous { "Nenhuma execução anterior desde que o launcher foi aberto.".into() } else { super::nexus_loader_logs::NO_SESSION.into() });
    }
    read_log_file(&file)
}
pub(super) fn read_log_file(file: &Path) -> Result<String, String> {
    let mut input = match fs::File::open(file) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok("Nenhum log disponível.".into())
        }
        Err(e) => return Err(e.to_string()),
    };
    let len = input.metadata().map_err(|e| e.to_string())?.len();
    let mut bytes = Vec::new();
    // Keep startup failures as well as the latest output; a noisy session must not
    // hide the exception that prevented a loader from starting.
    if len > 256 * 1024 {
        (&mut input).take(64 * 1024).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(b"\n[... trecho intermediario omitido ...]\n");
        input.seek(SeekFrom::Start(len.saturating_sub(192 * 1024))).map_err(|e|e.to_string())?;
        input.take(192 * 1024).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    } else {
        input.take(256 * 1024).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<_> = text.lines().collect();
    if lines.len() <= 1500 {return Ok(text.into_owned());}
    Ok(format!("{}\n[... linhas intermediárias omitidas ...]\n{}",
        lines[..300].join("\n"),lines[lines.len()-1200..].join("\n")))
}
#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn launcher_start_clears_launch_logs_but_keeps_events_and_links() {
        let root = std::env::temp_dir().join(format!("peligames-session-logs-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        for name in SESSION_FILES { fs::write(root.join(name), "old").unwrap(); }
        fs::write(root.join("events.log"), "history").unwrap();
        clear_session_files(&root).unwrap();
        assert!(SESSION_FILES.iter().all(|name| !root.join(name).exists()));
        assert!(root.join("events.log").exists());
        #[cfg(unix)] {
            let outside = root.with_extension("outside");
            fs::write(&outside, "keep").unwrap();
            std::os::unix::fs::symlink(&outside, root.join("current.log")).unwrap();
            clear_session_files(&root).unwrap();
            assert!(root.join("current.log").is_symlink() && outside.exists());
            fs::remove_file(outside).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn each_launch_retains_only_one_previous_execution() {
        let root = std::env::temp_dir().join(format!("log-rotation-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        rotate_execution(&root).unwrap();
        fs::write(root.join("current.log"), "first execution").unwrap();
        rotate_execution(&root).unwrap();
        assert_eq!(fs::read_to_string(root.join("previous.log")).unwrap(), "first execution");
        fs::write(root.join("current.log"), "second execution").unwrap();
        rotate_execution(&root).unwrap();
        assert_eq!(fs::read_to_string(root.join("previous.log")).unwrap(), "second execution");
        assert!(!root.join("current.log").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn large_log_retains_startup_error_and_latest_output() {
        let file=std::env::temp_dir().join(format!("peligames-log-{}.txt",uuid::Uuid::new_v4()));
        let mut data=String::from("STARTUP_EXCEPTION important loader failure\n");
        for _ in 0..10_000 {data.push_str("repeated runtime output that must not hide startup failures\n");}
        data.push_str("LATEST_OUTPUT\n");
        fs::write(&file,data).unwrap();
        let text=read_log_file(&file).unwrap();
        fs::remove_file(file).unwrap();
        assert!(text.contains("STARTUP_EXCEPTION"));assert!(text.contains("LATEST_OUTPUT"));
        assert!(text.len()<256*1024+1024);assert!(text.contains("omitid"));
    }
}
