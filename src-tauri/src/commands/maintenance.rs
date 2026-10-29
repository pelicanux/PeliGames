//! Explicit, previewed maintenance. Never discover deletion targets by globbing.
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::Manager;
pub mod reset;
mod pelirun;
static BUSY: AtomicBool = AtomicBool::new(false);
#[derive(Serialize)]
pub struct Plan {
    action: String,
    cleanup_data: bool,
    paths: Vec<String>,
    mod_games: Vec<super::nexus_durable::Review>,
    mod_revision: Option<String>,
    installed_games: Vec<crate::core::installed_library::ResetGame>,
    /// PeliRun folders, deleted only when the person also asks to remove it.
    pelirun: Vec<String>,
}

fn base(path: Option<PathBuf>) -> Result<PathBuf, String> {
    let path = path.ok_or("Diretório do usuário indisponível.")?;
    if !path.is_absolute() || path == Path::new("/") {
        return Err("Diretório de manutenção inválido.".into());
    }
    // Resolve XDG base aliases once; child symlinks are removed, never followed.
    if path.exists() {
        path.canonicalize().map_err(|e| e.to_string())
    } else {
        Ok(path)
    }
}
fn reset_paths(config: &Path, data: &Path, cache: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for root in [config, data, cache] {
        for name in ["com.pelicano.peligames", "com.pelicano.dlssnr-installer"] {
            paths.push(root.join(name));
        }
    }
    for name in ["PeliGames", "peligames", "dlssnr-x-amd"] {
        paths.push(config.join(name));
    }
    paths.push(cache.join("PeliGames"));
    paths.push(cache.join("peligames"));
    paths
}
fn owned_desktop(path: &Path, binary: &Path) -> bool {
    if !fs::symlink_metadata(path)
        .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink() && m.len() <= 65536)
    {
        return false;
    }
    let Ok(quoted) = crate::core::pelinstall::exec_arg(&binary.to_string_lossy()) else {
        return false;
    };
    fs::read_to_string(path).is_ok_and(|text| {
        text.lines().any(|line| {
            let Some(exec) = line.strip_prefix("Exec=") else {
                return false;
            };
            exec.strip_prefix(&quoted)
                .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(' '))
        })
    })
}

fn targets(action: &str) -> Result<Vec<PathBuf>, String> {
    let config = base(dirs::config_dir())?;
    let data = base(dirs::data_dir())?;
    let cache = base(dirs::cache_dir())?;
    let install = data.join("PeliGames");
    let mut paths = match action {
        "reset" => {
            let paths = reset_paths(&config, &data, &cache);
            // An unusual XDG setup must never turn Reset into Uninstall.
            if paths.iter().any(|p| install.starts_with(p)) {
                return Err("Os diretórios XDG de dados e configuração se sobrepõem. Redefinição recusada para preservar a instalação.".into());
            }
            paths
        }
        "uninstall" => {
            let status = crate::core::pelinstall::appimage_status()?;
            if !status.installed && !status.appimage {
                return Err(
                    "Esta instalação deve ser removida pelo gerenciador de pacotes do sistema."
                        .into(),
                );
            }
            let mut paths = Vec::new();
            if status.installed {
                paths.push(install.clone());
            } else {
                let image =
                    PathBuf::from(std::env::var_os("APPIMAGE").ok_or("AppImage indisponível.")?);
                if !image.is_absolute()
                    || !fs::symlink_metadata(&image)
                        .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
                {
                    return Err("O AppImage atual não é um arquivo regular.".into());
                }
                paths.push(image);
            }
            let binary = if status.installed {
                install.join("peligames")
            } else {
                paths[0].clone()
            };
            // Remove only shortcuts pointing to this exact installation, including
            // game shortcuts. A .deb/.rpm or another AppImage keeps its entries.
            for directory in [Some(data.join("applications")), dirs::desktop_dir()]
                .into_iter()
                .flatten()
            {
                if let Ok(entries) = fs::read_dir(&directory) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|ext| ext == "desktop")
                            && owned_desktop(&path, &binary)
                        {
                            paths.push(path);
                        }
                    }
                }
            }
            if status.installed {
                let bin = base(
                    dirs::executable_dir()
                        .or_else(|| dirs::home_dir().map(|p| p.join(".local/bin"))),
                )?;
                for name in ["peligames", "pelinstall"] {
                    let path = bin.join(name);
                    if fs::read_link(&path).is_ok_and(|target| target == binary) {
                        paths.push(path);
                    }
                }
                paths.push(data.join("peligames/appimage-icon.svg"));
            }
            paths
        }
        _ => return Err("Operação de manutenção inválida.".into()),
    };
    if action == "reset" {
        for program in [
            std::env::current_exe().ok(),
            std::env::var_os("APPIMAGE").map(PathBuf::from),
        ]
        .into_iter()
        .flatten()
        {
            if paths.iter().any(|p| program.starts_with(p)) {
                return Err("O executável está dentro de um diretório de dados. Mova-o antes de redefinir para preservar o programa.".into());
            }
        }
    }
    for path in &paths {
        validate_ancestors(path)?;
    }
    paths.sort();
    paths.dedup();
    Ok(paths
        .into_iter()
        .filter(|p| fs::symlink_metadata(p).is_ok())
        .collect())
}
fn validate_ancestors(path: &Path) -> Result<(), String> {
    for parent in path.parent().into_iter().flat_map(|p| p.ancestors()) {
        if fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "A limpeza recusou um diretório intermediário que é link: {}",
                parent.display()
            ));
        }
    }
    Ok(())
}
fn strings(paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}
fn operation_paths(action: &str, cleanup_data: bool) -> Result<Vec<PathBuf>, String> {
    let mut paths = targets(action)?;
    if action == "uninstall" && cleanup_data { paths.extend(targets("reset")?); }
    paths.sort(); paths.dedup();
    Ok(paths)
}
#[tauri::command]
pub fn launcher_maintenance_plan(action: String, cleanup_data: Option<bool>) -> Result<Plan, String> {
    let cleaning = action == "reset" || cleanup_data.unwrap_or(false);
    Ok(Plan {
        paths: strings(operation_paths(&action, cleaning)?),
        mod_games: if cleaning {super::nexus_durable::review()?} else {vec![]},
        mod_revision: if cleaning {Some(super::nexus_durable::reset_revision()?)}else{None},
        installed_games: if cleaning { crate::core::installed_library::reset_inventory(&[])? } else {vec![]},
        pelirun: if cleaning { strings(pelirun::folders()?) } else { vec![] },
        action,
        cleanup_data: cleaning,
    })
}

// Arguments are separate OS strings, never substituted into shell source.
// Uninstall only: reset uses the recoverable transaction in reset.rs.
// Wait for webviews and native writers to close before removing the installation.
const CLEANUP: &str = r#"
parent="$1"; shift
attempts=0
while kill -0 "$parent" 2>/dev/null; do
  attempts=$((attempts + 1))
  [ "$attempts" -lt 120 ] || exit 1
  sleep 1
done
for target do
  /bin/rm -rf -- "$target" || exit 1
done
# Check every path again after the entire cleanup, including dangling symlinks.
# Report incomplete uninstall rather than treating a leftover as success.
for target do
  if [ -e "$target" ] || [ -L "$target" ]; then
    printf 'Remoção incompleta: %s\n' "$target" >&2
    exit 1
  fi
done
"#;
fn idle(app: &tauri::AppHandle) -> Result<(), String> {
    if crate::core::game_execution::active()
        || crate::core::game_installation::installation_active()
        || super::proton::download_active(&app.state::<super::proton::ProtonDownloadState>())
    {
        return Err(
            "Encerre os jogos e aguarde as instalações e downloads antes de continuar.".into(),
        );
    }
    if super::nexus_downloads::list_nexus_downloads(app.state())?
        .iter()
        .any(|job| {
            matches!(
                job.status.as_str(),
                "queued" | "waiting" | "authorizing" | "downloading" | "importing" | "installing"
            )
        })
    {
        return Err("Conclua ou cancele os downloads Nexus antes de continuar.".into());
    }
    // Separate Pelinstall windows/processes can still hold or write these files.
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    for process in fs::read_dir("/proc").map_err(|e| e.to_string())?.flatten() {
        if process
            .file_name()
            .to_string_lossy()
            .parse::<u32>()
            .ok()
            .is_some_and(|pid| pid != std::process::id())
            && fs::read_link(process.path().join("exe")).is_ok_and(|path| path == executable)
        {
            return Err(
                "Feche as outras janelas e instâncias do PeliGames/Pelinstall antes de continuar."
                    .into(),
            );
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn launcher_maintenance_apply(
    app: tauri::AppHandle,
    action: String,
    confirmed_paths: Vec<String>,
    cleanup_data: Option<bool>,
    remove_mods: Option<bool>,
    preserve_accounts: Option<bool>,
    preserve_games: Option<bool>,
    preserve_mangohud: Option<bool>,
    confirmed_mod_revision: Option<String>,
    remove_pelirun: Option<bool>,
) -> Result<(), String> {
    if BUSY.swap(true, Ordering::AcqRel) {
        return Err("Manutenção já em andamento.".into());
    }
    struct Guard;
    impl Drop for Guard {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::Release);
        }
    }
    let _guard = Guard;
    idle(&app)?;
    let cleaning = action == "reset" || cleanup_data.unwrap_or(false);
    let paths = strings(operation_paths(&action, cleaning)?);
    if paths != confirmed_paths {
        return Err(
            "Os caminhos mudaram. Abra a confirmação novamente para revisar a lista atualizada."
                .into(),
        );
    }
    if cleaning {
        if reset::launcher_reset_status()?.state != "none" {
            return Err("Conclua a redefinição anterior antes de iniciar outra.".into());
        }
        if confirmed_mod_revision.as_deref()!=Some(super::nexus_durable::reset_revision()?.as_str()) {
            return Err("A biblioteca de mods mudou. Abra a confirmação novamente para revisar os jogos antes de redefinir.".into());
        }
        for game in super::nexus_durable::review()? {
            if paths.iter().any(|path|Path::new(&game.directory).starts_with(path)) {
                return Err(format!("A pasta de {} está dentro da área de redefinição. Mova o jogo para uma pasta externa antes de redefinir para preservar seus arquivos e mods.",game.name));
            }
        }
        // Validate the full disk/game inventory before touching credentials or mods.
        let remove=remove_mods.unwrap_or(false);
        if let Some(game)=super::nexus_durable::review()?.into_iter().find(|g| !g.available && (remove || !g.portable)) {
            return Err(format!("Não foi possível acessar “{}”, em {}. Dispositivo registrado: {}. Verifique se o dispositivo está conectado e montado no caminho original. A redefinição foi interrompida para preservar os mods.",game.name,game.directory,game.device));
        }
        // First of the irreversible steps: it refuses with Steam open before anything else changes.
        if remove_pelirun.unwrap_or(false) { tauri::async_runtime::spawn_blocking(pelirun::remove).await.map_err(|e| e.to_string())??; }
        if preserve_games.unwrap_or(false) { crate::core::installed_library::reset_inventory(&paths)?; }
        if !preserve_accounts.unwrap_or(false) {
            // Revoke where possible; local keyring deletion is mandatory.
            super::matrix_chat::disconnect_matrix_chat().await?;
            super::discord_account::disconnect_discord_account().await?;
            super::nexus_account::disconnect_nexus_account().await?;
            tauri::async_runtime::spawn_blocking(|| {
                super::nexus_account::credential_for(
                    "clear",
                    None,
                    "matrix-rtc-crypto-store",
                    "PeliGames — Matrix RTC",
                )
            })
            .await
            .map_err(|e| e.to_string())??;
        }
    }
    idle(&app)?;
    if cleaning {
        tauri::async_runtime::spawn_blocking(move || super::nexus_durable::prepare_reset(remove_mods.unwrap_or(false))).await.map_err(|e|e.to_string())??;
        let reset_paths = strings(targets("reset")?);
        let uninstall_paths = if action == "uninstall" { strings(targets("uninstall")?) } else { vec![] };
        return reset::close(&app, &reset_paths, preserve_accounts.unwrap_or(false), preserve_games.unwrap_or(false), preserve_mangohud.unwrap_or(false), &uninstall_paths);
    }
    launch_cleanup(&app, &paths)
}
fn launch_cleanup(app: &tauri::AppHandle, paths: &[String]) -> Result<(), String> {
    let mut helper = Command::new("/bin/sh");
    crate::core::env_utils::host_environment(&mut helper);
    helper
        .env_remove("APPIMAGE")
        .env_remove("APPDIR")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD");
    let log = std::env::temp_dir().join("peligames-maintenance.log");
    crate::core::bounded_logs::capture(&mut helper,&log)?;
    helper
        .arg("-c")
        .arg(CLEANUP)
        .arg("peligames-maintenance")
        .arg(std::process::id().to_string())
        .args(paths)
        .current_dir("/")
        .stdin(Stdio::null());
    helper
        .spawn()
        .map_err(|e| format!("Não foi possível iniciar a limpeza: {e}"))?;
    app.exit(0);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_keeps_installation_and_external_games() {
        let paths = reset_paths(
            Path::new("/user/config"),
            Path::new("/user/data"),
            Path::new("/user/cache"),
        );
        assert!(paths.contains(&PathBuf::from("/user/config/PeliGames")));
        assert!(!paths
            .iter()
            .any(|p| Path::new("/user/data/PeliGames/app").starts_with(p)));
        assert!(!paths
            .iter()
            .any(|p| Path::new("/games/example").starts_with(p)));
    }
    #[test]
    fn uninstall_only_owns_shortcuts_for_its_exact_binary() {
        let root =
            std::env::temp_dir().join(format!("peligames-shortcut-reset-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("game.desktop");
        fs::write(
            &path,
            "[Desktop Entry]\nExec=\"/data/PeliGames/peligames\" --launch-game /games/test\n",
        )
        .unwrap();
        assert!(owned_desktop(&path, Path::new("/data/PeliGames/peligames")));
        assert!(!owned_desktop(&path, Path::new("/usr/bin/peligames")));
        assert!(!owned_desktop(&path, Path::new("/data/PeliGames/peli")));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cleanup_treats_paths_literally_and_does_not_follow_symlinks() {
        let root =
            std::env::temp_dir().join(format!("peligames-maintenance-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("external")).unwrap();
        fs::write(root.join("external/keep"), "keep").unwrap();
        let target = root.join("settings $(touch unsafe)");
        fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(root.join("external"), target.join("linked-game")).unwrap();
        // A deliberately nonexistent PID makes the fixture helper run immediately.
        assert!(Command::new("/bin/sh")
            .arg("-c")
            .arg(CLEANUP)
            .arg("test")
            .arg("2147483647")
            .arg(&target)
            .status()
            .unwrap()
            .success());
        assert!(!target.exists());
        assert!(root.join("external/keep").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
