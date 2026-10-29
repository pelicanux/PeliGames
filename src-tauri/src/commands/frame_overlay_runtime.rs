//! Install the exact layer bundled with this launcher, only for opted-in games.
//! Versioned payloads keep wrappers valid across an application update.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

const RUNNER: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/game-overlay/peligames-overlay-run"
));
const RENDERER: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/game-overlay/libpeligames_game_overlay.so"
));
const AUDIT: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/game-overlay/libpeligames_overlay_audit.so"
));
const FILES: [(&str, &[u8]); 3] = [
    ("peligames-overlay-run", RUNNER),
    ("libpeligames_game_overlay.so", RENDERER),
    ("libpeligames_overlay_audit.so", AUDIT),
];
static INSTALL: Mutex<()> = Mutex::new(());
pub(crate) fn bundled() -> bool { FILES.iter().all(|(_,bytes)| bytes.len() >= 64) }

#[derive(serde::Serialize)]
pub struct Support {
    available: bool,
    reason: String,
    legacy_enabled: bool,
}
pub(super) fn check_game(
    directory: &Path,
    executable: Option<&str>,
    flatpak: bool,
) -> Result<(), String> {
    crate::core::release_features::require()?;
    if flatpak {
        return Err("O overlay interno ainda não está disponível no Flatpak.".into());
    }
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("O overlay interno exige Linux x86_64.".into());
    }
    let directory = directory
        .canonicalize()
        .map_err(|_| "Pasta do jogo indisponível. Verifique a montagem do disco.")?;
    let executable = executable
        .map(PathBuf::from)
        .and_then(|p| p.canonicalize().ok())
        .filter(|p| p.is_file() && p.starts_with(&directory))
        .or_else(|| {
            crate::core::game_executable::best_executable_dir(&directory, 5)
                .and_then(|d| super::analyzer::scan_directory_for_exe(&d))
        })
        .or_else(|| super::analyzer::scan_directory_for_exe(&directory));
    match executable.as_deref().and_then(super::analyzer::analyze_pe_header).as_deref() {
        // A PE32 executable can use a 64-bit Unix Vulkan host through the new
        // WoW64 mode. Its Windows architecture alone cannot reject that path.
        // The runner still ships only ELF64 payloads and a library_arch=64
        // manifest: this permits testing WoW64, not native i686 support.
        Some("64-bits") => Ok(()),
        Some("32-bits") if executable.as_deref().is_some_and(|path| {
            let mut magic = [0; 2];
            fs::File::open(path).and_then(|mut file| file.read_exact(&mut magic)).is_ok()
                && magic == *b"MZ"
        }) => Ok(()),
        Some("32-bits") => Err("Processos Linux de 32 bits ainda não são suportados pelo overlay. O teste WoW64 é destinado a executáveis Windows.".into()),
        _ => Err("Não foi possível confirmar a arquitetura do executável deste jogo. O MAKO pode ser usado sem o overlay.".into()),
    }
}
#[tauri::command]
pub async fn frame_overlay_support(
    directory: String,
    executable: Option<String>,
    flatpak: bool,
    advanced: Option<crate::core::advanced_settings::AdvancedSettings>,
    external_id: Option<String>,
) -> Result<Support, String> {
    tauri::async_runtime::spawn_blocking(move || {
        // Flatpak Steam runs the overlay from its own PeliRun folder (`install_sandboxed`).
        let sandboxed_steam = flatpak && external_id.as_deref().is_some_and(|id| id.starts_with("Steam:flatpak:"));
        if sandboxed_steam && super::steam_runtime::flatpak_directory().is_none() {
            return Support { available: false, legacy_enabled: false, reason: "Abra a Steam Flatpak uma vez antes de ativar o overlay.".into() };
        }
        let legacy_enabled=advanced.as_ref().is_some_and(|settings| {
            let Ok(root)=crate::core::paths::app_path("FrameGeneration/profiles") else{return false};
            settings.wrappers.iter().filter(|wrapper|wrapper.enabled).any(|wrapper|{
                let path=Path::new(&wrapper.program);
                if path.parent()!=Some(root.as_path()) || path.extension().is_none_or(|e|e!="sh"){return false;}
                let Ok(file)=fs::File::open(path.with_extension("json")) else{return false};
                let mut bytes=Vec::new();if file.take(65537).read_to_end(&mut bytes).is_err()||bytes.len()>65536{return false;}
                serde_json::from_slice::<serde_json::Value>(&bytes).is_ok_and(|v|v["backend"]=="mako"&&v["enabled"]==true&&v["overlay_enabled"]==true)
            })
        });
        match check_game(Path::new(&directory), executable.as_deref(), flatpak && !sandboxed_steam) {
            Ok(()) => Support { available: true, legacy_enabled, reason: format!("Experimental · Exige Vulkan em processo Linux de 64 bits. Para jogos Windows de 32 bits, procure “Compatibilidade WoW64” na aba Avançado e ative manualmente para testar, se o Proton oferecer suporte. Pode causar incompatibilidade; se o jogo deixar de abrir, desative a opção. Processos Linux de 32 bits ainda não são suportados. Jogos nativos Linux só recebem teclado e mouse no painel com Wayland nativo.{}", if sandboxed_steam { " Na Steam Flatpak, o overlay é copiado para a pasta do PeliRun dentro do sandbox." } else { "" }) },
            Err(reason) => Support { available: false, legacy_enabled, reason },
        }
    }).await.map_err(|e| e.to_string())
}

fn version() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        let mut digest = Sha256::new();
        for (name, bytes) in FILES {
            digest.update(name);
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        format!("{:x}", digest.finalize())
    })
}
pub(super) fn runner(config: &Path) -> PathBuf {
    let folder=config.parent().expect("profile parent").parent().expect("component root").join("overlay");
    crate::core::paths::names::find(&folder,version()).unwrap_or_else(||folder.join("not-prepared")).join(FILES[0].0)
}
pub(crate) fn install(config: &Path) -> Result<(), String> {
    crate::core::release_features::require()?;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !cfg!(all(target_os = "linux", target_arch = "x86_64"))
        || FILES.iter().any(|(_, b)| b.is_empty())
    {
        return Err("O overlay interno exige a edição Linux x86_64 do PeliGames.".into());
    }
    let _guard = INSTALL.lock().map_err(|e| e.to_string())?;
    let folder=config.parent().expect("profile parent").parent().expect("component root").join("overlay");
    let directory=crate::core::paths::names::allocate(&folder,&format!("peligames-{}",env!("CARGO_PKG_VERSION")),version())?;
    if directory
        .to_string_lossy()
        .chars()
        .any(|c| c.is_whitespace() || c == ':')
    {
        return Err("O carregamento do overlay exige que a pasta de configuração do PeliGames não contenha espaços ou dois-pontos.".into());
    }
    // Reject symlinks, including the intermediate overlay directory. Never
    // replace another user's files or turn a shared folder into an executable.
    for path in [directory.parent().unwrap(), directory.as_path()] {
        match fs::create_dir(path) {
            Ok(()) => fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.to_string()),
        }
        let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
            return Err("Diretório do overlay sem propriedade ou permissões privadas.".into());
        }
    }
    for (name, bytes) in FILES {
        let path = directory.join(name);
        let intact = fs::symlink_metadata(&path).is_ok_and(|m| {
            m.is_file()
                && m.uid() == unsafe { libc::geteuid() }
                && m.mode() & 0o077 == 0
                && m.mode() & 0o100 != 0
                && m.len() == bytes.len() as u64
        }) && fs::read(&path).is_ok_and(|existing| existing == bytes);
        if !intact {
            super::frame_generation::write(&path, bytes, 0o700)?;
        }
    }
    Ok(())
}

/// The general overlay has its own installation boundary, independent of MAKO.
pub(crate) fn standalone_runner() -> Result<PathBuf,String> {
    install_standalone()?;
    Ok(runner(&crate::core::paths::app_path("Overlay/profiles/standalone.toml")?))
}
pub(crate) fn install_standalone() -> Result<(),String> {
    use std::os::unix::fs::{MetadataExt,PermissionsExt};
    let root=crate::core::paths::app_path("Overlay")?;
    match fs::create_dir(&root){Ok(())=>fs::set_permissions(&root,fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?,Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{},Err(e)=>return Err(e.to_string())}
    let meta=fs::symlink_metadata(&root).map_err(|e|e.to_string())?;
    if !meta.is_dir()||meta.file_type().is_symlink()||meta.uid()!=unsafe{libc::geteuid()}||meta.mode()&0o077!=0{return Err("Pasta do overlay sem permissões privadas.".into());}
    install(&root.join("profiles/standalone.toml"))
}

/// Flatpak Steam sees only its own data, never `~/.config/PeliGames`. The
/// payload and the shortcut go beside its PeliRun (`root`), and the profile
/// calls that copy. Returns the runner inside the sandbox.
pub(crate) fn install_sandboxed(root: &Path) -> Result<PathBuf, String> {
    let config = root.join("profiles/standalone.toml");
    install(&config)?;
    mirror_preferences(root)?;
    Ok(runner(&config))
}
/// Copies the shortcut preferences into a sandboxed overlay root.
pub(crate) fn mirror_preferences(root: &Path) -> Result<(), String> {
    let source = crate::core::paths::app_path("frame-overlay.json")?;
    let target = crate::core::paths::layout::join(&root, "frame-overlay.json");
    match fs::read(&source) {
        Ok(bytes) if bytes.len() <= 65536 => super::frame_generation::write(&target, &bytes, 0o600),
        Ok(_) => Err("Preferências do overlay grandes demais.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match fs::remove_file(&target) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
        Err(e) => Err(e.to_string()),
    }
}

#[derive(serde::Serialize, Debug, PartialEq)]
pub struct Session {
    /// Seconds since the Unix epoch, from the file name.
    started: u64,
    /// "ok", "warning" or "error".
    level: &'static str,
    summary: String,
    lines: Vec<String>,
}
/// What one diagnosis file says, in plain terms.
fn diagnose(text: &str) -> (&'static str, String, Vec<String>) {
    // Every process of the launch appends here (a launcher, then the game): keep
    // who said what, and let one process's success outweigh another's refusal.
    let mut entries: Vec<(String, String)> = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with("cmd\t")) {
        let (process, message) = line.split_once('\t').unwrap_or(("", line));
        let entry = (process.trim().to_string(), message.trim().to_string());
        if !entry.1.is_empty() && !entries.contains(&entry) && entries.len() < 40 { entries.push(entry); }
    }
    let lines: Vec<String> = entries.iter().map(|(process, message)| match process.as_str() {
        "" | "runner" => message.clone(),
        process => format!("{process}: {message}"),
    }).collect();
    let has = |needle: &str| entries.iter().any(|(_, m)| m.contains(needle));
    if let Some(error) = entries.iter().find_map(|(_, m)| m.strip_prefix("Runner: overlay não carregado: ")) {
        return ("error", format!("O overlay não foi carregado: {error}"), lines);
    }
    if has("Entrada do jogo reconhecida") {
        return ("ok", "Funcionou na última execução: painel pronto e atalho disponível.".into(), lines);
    }
    if let Some(reason) = entries.iter().find_map(|(_, m)| m.strip_prefix("Swapchain: ").filter(|r| !r.starts_with("compatível")).and_then(|r| r.split(';').next())) {
        let hint = if reason.contains("rota de entrada") { " Jogos nativos Linux só têm entrada no Wayland nativo; em X11/XWayland, o overlay atende jogos executados pelo Wine/Proton." } else { "" };
        return ("error", format!("O jogo usa uma imagem que o overlay ainda não cobre ({reason}).{hint}"), lines);
    }
    if has("Renderer preparado") || has("Aguardando uma rota de entrada") {
        return ("warning", "O painel foi desenhado, mas o overlay não recebeu teclado e mouse do jogo. Em X11/XWayland isso só funciona com jogos executados pelo Wine/Proton; jogos nativos Linux precisam do Wayland nativo.".into(), lines);
    }
    if has("Rota de entrada Wayland não suportada") {
        return ("warning", "O compositor ou o jogo usa uma rota de entrada Wayland não suportada.".into(), lines);
    }
    if has("Runner: camada Vulkan e entrada preparadas") {
        return ("warning", "O overlay foi preparado, mas o jogo não criou uma tela Vulkan com ele. Causas comuns: jogo em OpenGL, jogo Windows de 32 bits sem o novo WoW64, ou fechado antes de abrir a janela.".into(), lines);
    }
    ("warning", "A sessão não registrou nenhuma etapa do overlay.".into(), lines)
}
fn session_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = crate::core::paths::app_root().ok().into_iter().collect();
    roots.extend(super::steam_runtime::flatpak_directory());
    roots
}
/// The newest diagnosis whose command ran something inside `directory`.
fn last_session_in(roots: &[PathBuf], directory: &Path) -> Option<Session> {
    let wanted: Vec<PathBuf> = [Some(directory.to_owned()), directory.canonicalize().ok()].into_iter().flatten().collect();
    let mut files: Vec<PathBuf> = roots.iter().filter_map(|root| fs::read_dir(crate::core::paths::layout::join(&root, "overlay-logs")).ok())
        .flat_map(|entries| entries.flatten().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "log")).collect();
    files.sort_by_key(|p| std::cmp::Reverse(p.file_name().map(|n| n.to_owned())));
    for path in files {
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if !meta.is_file() { continue; }
        let mut bytes = Vec::new();
        if fs::File::open(&path).and_then(|f| f.take(256 * 1024).read_to_end(&mut bytes)).is_err() { continue; }
        let text = String::from_utf8_lossy(&bytes);
        let ours = text.lines().filter_map(|l| l.strip_prefix("cmd\t"))
            .any(|word| wanted.iter().any(|dir| Path::new(word).starts_with(dir)));
        if !ours { continue; }
        let started = path.file_name()?.to_str()?.split('-').next()?.parse().unwrap_or(0);
        let (level, summary, lines) = diagnose(&text);
        return Some(Session { started, level, summary, lines });
    }
    None
}
pub(super) fn report_session(directory: &Path) -> Option<Session> {
    last_session_in(&session_roots(), directory)
}
/// The last overlay launch of the game in `directory`, for the Performance panel.
#[tauri::command]
pub async fn frame_overlay_last_session(directory: String) -> Result<Option<Session>, String> {
    tauri::async_runtime::spawn_blocking(move || report_session(Path::new(&directory)))
        .await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sessions_explain_the_last_launch_of_each_game() {
        let root = std::env::temp_dir().join(format!("peli-overlay-sessions-{}", uuid::Uuid::new_v4()));
        let logs = crate::core::paths::layout::join(&root, "overlay-logs");
        fs::create_dir_all(&logs).unwrap();
        fs::write(logs.join("000000000100-1.log"), "cmd\t/games/Hades/Hades.exe\nrunner\tRunner: camada Vulkan e entrada preparadas.\nHades.exe\tRenderer preparado; painel inicia oculto.\nHades.exe\tEntrada do jogo reconhecida; atalho do overlay disponível com o jogo em foco.\n").unwrap();
        fs::write(logs.join("000000000200-2.log"), "cmd\t/games/Hades2/Hades2.exe\nrunner\tRunner: overlay não carregado: Biblioteca sem propriedade/permissões seguras.\n").unwrap();
        fs::write(logs.join("000000000300-3.log"), "cmd\t/games/Native/game\nrunner\tRunner: camada Vulkan e entrada preparadas.\ngame\tSwapchain: superfície sem uma rota de entrada compatível; superfície=Unavailable\n").unwrap();
        let roots = [root.clone()];
        let hades = last_session_in(&roots, Path::new("/games/Hades")).unwrap();
        assert_eq!((hades.started, hades.level), (100, "ok"));
        let hades2 = last_session_in(&roots, Path::new("/games/Hades2")).unwrap();
        assert_eq!(hades2.level, "error");
        assert!(hades2.summary.contains("permissões seguras"));
        let native = last_session_in(&roots, Path::new("/games/Native")).unwrap();
        assert!(native.summary.contains("Wayland nativo"));
        assert_eq!(last_session_in(&roots, Path::new("/games/Celeste")), None);
        // OpenGL or 32-bit without WoW64: prepared, but no Vulkan swapchain.
        assert!(diagnose("runner\tRunner: camada Vulkan e entrada preparadas.\n").1.contains("OpenGL"));
        // A launcher window refused, then the game it opens works: the launch succeeded.
        let (level, _, lines) = diagnose("runner\tRunner: camada Vulkan e entrada preparadas.\nLauncher.exe\tSwapchain: flags de criação da swapchain ainda não cobertas; superfície=Xlib(1)\nGame.exe\tRenderer preparado; painel inicia oculto.\nGame.exe\tEntrada do jogo reconhecida; atalho do overlay disponível com o jogo em foco.\n");
        assert_eq!(level, "ok");
        assert!(lines.iter().any(|l| l.starts_with("Launcher.exe: Swapchain")) && lines.iter().any(|l| l.starts_with("Game.exe: Entrada")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn windows_32_bit_games_can_opt_in_for_wow64_testing() {
        let root = std::env::temp_dir().join(format!("peli-overlay-abi-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let exe = root.join("Game.exe");
        let mut pe = vec![0u8; 512];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..68].copy_from_slice(b"PE\0\0");
        pe[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
        fs::write(&exe, &pe).unwrap();
        assert!(check_game(&root, exe.to_str(), false).is_ok());
        assert!(check_game(&root, exe.to_str(), true).is_err());
        pe[68..70].copy_from_slice(&0x014cu16.to_le_bytes());
        fs::write(&exe, &pe).unwrap();
        assert!(check_game(&root, exe.to_str(), false).is_ok());
        assert!(check_game(&root, exe.to_str(), true).is_err());
        // Unknown architectures must not become eligible as a side effect.
        pe[68..70].copy_from_slice(&0xffffu16.to_le_bytes());
        fs::write(&exe, &pe).unwrap();
        assert!(check_game(&root, exe.to_str(), false).is_err());
        // The analyzer also recognizes ELF. Allowing PE32 must not allow ELF32.
        pe[..4].copy_from_slice(b"\x7fELF");
        pe[4] = 1;
        fs::write(&exe, &pe).unwrap();
        assert!(check_game(&root, exe.to_str(), false).unwrap_err().contains("Linux de 32 bits"));
        pe[4] = 2;
        fs::write(&exe, &pe).unwrap();
        assert!(check_game(&root, exe.to_str(), false).is_ok());
        assert!(check_game(&root.join("missing"), exe.to_str(), false).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
