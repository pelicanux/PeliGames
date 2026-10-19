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
) -> Result<Support, String> {
    tauri::async_runtime::spawn_blocking(move || {
        match check_game(Path::new(&directory), executable.as_deref(), flatpak) {
            Ok(()) => Support { available: true, reason: "Experimental · Exige Vulkan em processo Linux de 64 bits. Jogos Windows de 32 bits podem ser testados com o novo WoW64 do Proton; processos Linux de 32 bits ainda não são suportados.".into() },
            Err(reason) => Support { available: false, reason },
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
    config
        .parent()
        .expect("profile parent")
        .parent()
        .expect("FrameGeneration")
        .join("overlay")
        .join(version())
        .join(FILES[0].0)
}
pub(super) fn install(config: &Path) -> Result<(), String> {
    crate::core::release_features::require()?;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if !cfg!(all(target_os = "linux", target_arch = "x86_64"))
        || FILES.iter().any(|(_, b)| b.is_empty())
    {
        return Err("O overlay interno exige a edição Linux x86_64 do PeliGames.".into());
    }
    let _guard = INSTALL.lock().map_err(|e| e.to_string())?;
    let directory = runner(config).parent().unwrap().to_owned();
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

#[cfg(test)]
mod tests {
    use super::*;
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
