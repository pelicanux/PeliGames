//! Copies of launcher-owned prefixes; external game files remain explicitly shared.
//! No shell or hardlink fallback.
use super::{
    game_profiles::{self, GameProfile, ProfileKind},
    installed_library::{self, InstalledEntry},
};
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
static ACTIVE: Mutex<Option<(String, Arc<AtomicBool>)>> = Mutex::new(None);
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut slot) = ACTIVE.lock() {
            *slot = None;
        }
    }
}
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CopyKind {
    Full,
    Reflink,
    Overlay,
}
#[derive(Serialize)]
pub struct CopyOptions {
    pub parent: String,
    pub filesystem: String,
    pub reflink: bool,
    pub overlay: bool,
    pub reason: Option<String>,
    pub shared_game_directory: Option<String>,
}
#[derive(Deserialize)]
pub struct CopyRequest {
    pub path: String,
    pub name: String,
    pub parent: String,
    pub kind: CopyKind,
    #[serde(default)]
    pub acknowledged: bool,
}
#[derive(Serialize, Deserialize)]
struct Metadata {
    version: u32,
    source: String,
    kind: CopyKind,
    #[serde(default)]
    shared_game_directory: Option<String>,
}
fn source(path: &str) -> Result<(InstalledEntry, PathBuf, PathBuf), String> {
    let (entry, directory) = installed_library::copy_source(path)?;
    if let Some(profile) = &entry.profile {
        game_profiles::ensure_ready(profile)?;
    }
    let prefix = Path::new(&entry.prefix)
        .canonicalize()
        .map_err(|_| "Prefixo indisponível.")?;
    let executable = Path::new(&entry.executable)
        .canonicalize()
        .map_err(|_| "Executável indisponível.")?;
    if !prefix.join("drive_c").is_dir() || !executable.is_file() {
        return Err("O prefixo Wine ou executável deste jogo não está disponível.".into());
    }
    Ok((entry, directory, prefix))
}
fn shared_game_directory(executable: &str, prefix: &Path) -> Result<Option<String>, String> {
    let executable = Path::new(executable).canonicalize().map_err(|_| "Executável indisponível.")?;
    Ok(if executable.starts_with(prefix) {None} else {Some(executable.parent().ok_or("Pasta do jogo inválida.")?.to_string_lossy().into_owned())})
}
fn copied_executable(executable: &Path, base: &Path, prefix: &Path) -> PathBuf {
    executable.strip_prefix(base).map(|relative|prefix.join(relative)).unwrap_or_else(|_|executable.to_path_buf())
}
fn filesystem(path: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return "desconhecido".into();
        };
        let mut info: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(path.as_ptr(), &mut info) } == 0 {
            return match info.f_type as i64 {
                0x9123683E => "Btrfs",
                0x58465342 => "XFS",
                0xEF53 => "ext4/ext3/ext2",
                0xCA451A4E => "bcachefs",
                _ => "outro",
            }
            .into();
        }
    }
    "desconhecido".into()
}
fn same_device(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        return fs::metadata(a)
            .ok()
            .zip(fs::metadata(b).ok())
            .is_some_and(|(a, b)| a.dev() == b.dev());
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        false
    }
}
// A filesystem name alone does not prove that reflink is enabled (e.g. XFS).
fn probe_reflink(parent: &Path) -> bool {
    let probe = parent.join(format!(".peligames-copy-check-{}", uuid::Uuid::new_v4()));
    if fs::create_dir(&probe).is_err() {
        return false;
    }
    let result = (|| {
        fs::write(probe.join("source"), b"PeliGames")?;
        Command::new("cp")
            .args(["--reflink=always", "--"])
            .arg(probe.join("source"))
            .arg(probe.join("copy"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
    })();
    let _ = fs::remove_dir_all(probe);
    result.unwrap_or(false)
}
pub fn options(path: &str, parent: Option<&str>) -> Result<CopyOptions, String> {
    let (entry, directory, prefix) = source(path)?;
    let parent = parent
        .map(PathBuf::from)
        .unwrap_or(
            directory
                .parent()
                .ok_or("Pasta de origem inválida.")?
                .to_path_buf(),
        )
        .canonicalize()
        .map_err(|_| "A pasta de destino não está disponível.")?;
    if !parent.is_dir() || parent.starts_with(&prefix) {
        return Err("Escolha uma pasta fora da instalação original.".into());
    }
    let reflink = same_device(&prefix, &parent)
        && game_profiles::reflink_available(&prefix)
        && probe_reflink(&parent);
    Ok(CopyOptions {
        parent: parent.to_string_lossy().into_owned(),
        filesystem: filesystem(&parent),
        reflink,
        overlay: entry.profile.as_ref().is_none_or(|p|p.kind != ProfileKind::Overlay) && game_profiles::overlay_available(),
        reason: None,
        shared_game_directory: shared_game_directory(&entry.executable, &prefix)?,
    })
}
pub fn cancel(id: &str) -> Result<(), String> {
    let slot = ACTIVE.lock().map_err(|e| e.to_string())?;
    let (active, flag) = slot.as_ref().ok_or("A cópia já foi concluída.")?;
    if active != id {
        return Err("Outra cópia está em execução.".into());
    }
    flag.store(true, Ordering::Release);
    Ok(())
}
fn check_cancel(flag: &AtomicBool) -> Result<(), String> {
    if flag.load(Ordering::Acquire) {
        Err("Cópia cancelada. A instalação original foi preservada.".into())
    } else {
        Ok(())
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 100
        && name != "."
        && name != ".."
        && !name
            .chars()
            .any(|c| c.is_control() || ['/', '\\', ':', ','].contains(&c))
}
fn copy_tree(
    source: &Path,
    destination: &Path,
    reflink: bool,
    cancel: &AtomicBool,
) -> Result<(), String> {
    copy_path(source, destination, reflink, cancel, true)
}
fn copy_path(source: &Path, destination: &Path, reflink: bool, cancel: &AtomicBool, contents: bool) -> Result<(), String> {
    let error_path =
        std::env::temp_dir().join(format!("peligames-copy-error-{}", uuid::Uuid::new_v4()));
    let error_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&error_path)
        .map_err(|e| e.to_string())?;
    fs::remove_file(&error_path).map_err(|e| e.to_string())?;
    let mut child = Command::new("cp")
        .args([
            "-a",
            if reflink {
                "--reflink=always"
            } else {
                "--reflink=never"
            },
            "--",
        ])
        .arg(if contents {source.join(".")} else {source.to_path_buf()})
        .arg(destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(error_file.try_clone().map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| format!("Não foi possível iniciar a cópia: {e}"))?;
    loop {
        if cancel.load(Ordering::Acquire) {
            let _ = child.kill();
            let _ = child.wait();
            return check_cancel(cancel);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(());
                }
                use std::io::{Read, Seek, SeekFrom};
                let mut file = error_file;
                let _ = file.seek(SeekFrom::Start(0));
                let mut text = String::new();
                let _ = file.take(8192).read_to_string(&mut text);
                return Err(format!("Falha ao copiar: {}", text.trim()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e.to_string());
            }
        }
    }
}
fn has_nested_mounts(root: &Path) -> bool {
    has_other_nested_mounts(root, None)
}
fn has_other_nested_mounts(root: &Path, allowed: Option<&Path>) -> bool {
    fs::read_to_string("/proc/self/mountinfo")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_whitespace().nth(4))
        .any(|path| {
            let path = PathBuf::from(
                path.replace("\\040", " ")
                    .replace("\\011", "\t")
                    .replace("\\012", "\n")
                    .replace("\\134", "\\"),
            );
            path != root && path.starts_with(root) && allowed != Some(path.as_path())
        })
}
fn copy_flat_overlay(profile: &GameProfile, destination: &Path, reflink: bool, cancel: &AtomicBool) -> Result<(), String> {
    let prefix = profile.prefix();
    let merged = Path::new(&profile.root).join("prefix");
    for entry in fs::read_dir(&prefix).map_err(|e|e.to_string())? {
        check_cancel(cancel)?;
        let entry = entry.map_err(|e|e.to_string())?;
        let name = entry.file_name();
        if [".peligames-overlay", "installation.json", "copy.json"].iter().any(|skip|name == *skip) { continue; }
        let path = entry.path();
        let kind = entry.file_type().map_err(|e|e.to_string())?;
        // Materialize only our directory links; preserve user/Wine links as links.
        let our_directory = kind.is_symlink() && path.is_dir()
            && fs::read_link(&path).is_ok_and(|link|prefix.join(link) == merged.join(&name));
        let source = if our_directory {merged.join(&name)} else {path};
        copy_path(&source, &destination.join(name), reflink, cancel, our_directory || kind.is_dir())?;
    }
    Ok(())
}
fn rebase_links(source: &Path, prefix: &Path) -> Result<(), String> {
    #[cfg(unix)]
    for item in walkdir::WalkDir::new(prefix).follow_links(false) {
        let item = item.map_err(|e| e.to_string())?;
        if item.file_type().is_symlink() {
            let target = fs::read_link(item.path()).map_err(|e| e.to_string())?;
            if target.is_absolute() {
                if let Ok(relative) = target.strip_prefix(source) {
                    fs::remove_file(item.path()).map_err(|e| e.to_string())?;
                    std::os::unix::fs::symlink(prefix.join(relative), item.path())
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}
pub fn create(
    request: CopyRequest,
    id: String,
    progress: impl Fn(&str),
) -> Result<InstalledEntry, String> {
    uuid::Uuid::parse_str(&id).map_err(|_| "Identificador de operação inválido.")?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = ACTIVE.lock().map_err(|e| e.to_string())?;
        if slot.is_some() {
            return Err("Já há uma cópia em andamento.".into());
        }
        *slot = Some((id, cancel.clone()));
    }
    let _guard = Guard;
    let name = request.name.trim();
    if !valid_name(name) {
        return Err("Use um nome de pasta válido, com até 100 caracteres.".into());
    }
    let (entry, original_directory, base) = source(&request.path)?;
    if super::wine_tools::busy() || super::game_execution::prefix_in_use(&base) {
        return Err("Feche o jogo e as ferramentas Wine desse prefixo antes de copiar.".into());
    }
    let lock_root = entry
        .profile
        .as_ref()
        .map(|profile| Path::new(&profile.root))
        .unwrap_or(&base);
    let _lease = super::game_installation::lock_prefix(lock_root)?;
    let _mounted_prefix_lease = if lock_root != base {
        Some(super::game_installation::lock_prefix(&base)?)
    } else {
        None
    };
    let own_mount = entry.profile.as_ref().filter(|p|p.flat_copy).map(|p|Path::new(&p.root).join("prefix"));
    if has_other_nested_mounts(&base, own_mount.as_deref()) {
        return Err("O prefixo contém outras pastas montadas. Desmonte-as antes de copiar.".into());
    }
    let options = options(&request.path, Some(&request.parent))?;
    let parent = PathBuf::from(&options.parent);
    if parent.starts_with(&original_directory) {
        return Err("O destino não pode ficar dentro da instalação original.".into());
    }
    if request.kind == CopyKind::Reflink && !options.reflink {
        return Err("O destino não oferece reflink no mesmo sistema de arquivos da origem.".into());
    }
    if (request.kind == CopyKind::Overlay || options.shared_game_directory.is_some()) && !request.acknowledged {
        return Err("Confirme que compreendeu as dependências e os arquivos compartilhados antes de copiar.".into());
    }
    if request.kind == CopyKind::Overlay && entry.profile.as_ref().is_some_and(|p|p.kind == ProfileKind::Overlay) {
        return Err("Crie os próximos overlays pela base original. Para duplicar alterações de um overlay, use cópia completa ou reflink.".into());
    }
    if request.kind == CopyKind::Overlay && !options.overlay {
        return Err("Overlay requer fuse-overlayfs, fusermount3 e acesso a /dev/fuse.".into());
    }
    check_cancel(&cancel)?;
    let destination = parent.join(name);
    fs::create_dir(&destination)
        .map_err(|e| format!("Não foi possível criar a pasta da cópia (ela deve ser nova): {e}"))?;
    let mut profile: Option<GameProfile> = None;
    let result = (|| {
        progress(if options.shared_game_directory.is_some(){"Copiando o prefixo; a pasta externa do jogo será compartilhada…"}else{"Copiando o jogo e o prefixo…"});
        let prefix = if request.kind == CopyKind::Overlay {
            let created = game_profiles::create_copy_overlay(&base, &destination)?;
            let prefix = created.prefix();
            profile = Some(created);
            prefix
        } else {
            let prefix = destination.clone();
            if let Some(source) = entry.profile.as_ref().filter(|p|p.flat_copy) {
                copy_flat_overlay(source, &prefix, request.kind == CopyKind::Reflink, &cancel)?;
            } else {
                copy_tree(&base, &prefix, request.kind == CopyKind::Reflink, &cancel)?;
            }
            for name in ["installation.json", "copy.json"] {
                match fs::remove_file(prefix.join(name)) {
                    Ok(()) => {},
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
                    Err(e) => return Err(e.to_string()),
                }
            }
            prefix
        };
        fs::write(destination.join(super::installation_layout::OWNED_MARKER), b"PeliGames\n").map_err(|e|e.to_string())?;
        check_cancel(&cancel)?;
        if let Some(source) = entry.profile.as_ref().filter(|p|p.flat_copy) {
            rebase_links(&Path::new(&source.root).join("prefix"), &prefix)?;
        }
        rebase_links(&base, &prefix)?;
        let mut executable = Path::new(&entry.executable)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if let Some(source) = entry.profile.as_ref().filter(|p|p.flat_copy) {
            executable = copied_executable(&executable, &Path::new(&source.root).join("prefix"), &base);
        }
        let executable = copied_executable(&executable, &base, &prefix);
        let mut copy = entry.clone();
        copy.path = format!("peligames:{}", uuid::Uuid::new_v4());
        copy.name = name.into();
        copy.executable = executable.to_string_lossy().into_owned();
        copy.directory = executable
            .parent()
            .ok_or("Executável inválido.")?
            .to_string_lossy()
            .into_owned();
        copy.prefix = prefix.to_string_lossy().into_owned();
        copy.profile = profile.clone();
        copy.shortcut = None;
        copy.discovery_source = None;
        // Do not retain a launch line or wrapper bound to the original PeliRun/game profile.
        copy.advanced.launch_options = None;
        copy.advanced.wrappers.clear();
        copy.launch_arguments = copy
            .launch_arguments
            .iter()
            .map(|arg| {
                Path::new(arg)
                    .strip_prefix(&base)
                    .map(|rest| prefix.join(rest).to_string_lossy().into_owned())
                    .unwrap_or_else(|_| arg.clone())
            })
            .collect();
        let metadata = Metadata {
            version: 2,
            source: base.to_string_lossy().into_owned(),
            kind: request.kind,
            shared_game_directory: options.shared_game_directory.clone(),
        };
        fs::write(
            destination.join("copy.json"),
            serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        check_cancel(&cancel)?;
        progress("Registrando a cópia na biblioteca…");
        installed_library::register_copy(copy, &destination)
    })();
    if result.is_err() {
        if let Some(profile) = &profile {
            if let Err(e) = game_profiles::remove(profile) {
                return Err(format!("A cópia falhou; não foi possível desmontar a pasta temporária: {e}. A origem foi preservada."));
            }
        }
        if has_nested_mounts(&destination) {
            return Err("A cópia falhou e há uma montagem ativa no destino. A pasta foi preservada para desmontagem; a origem não foi removida.".into());
        }
        let _ = fs::remove_dir_all(&destination);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_copy_does_not_nest_the_source_or_retain_overlay_storage() {
        let root = std::env::temp_dir().join(format!("peligames-flat-copy-{}", uuid::Uuid::new_v4()));
        let source = root.join("Source");
        let target = root.join("Copy");
        fs::create_dir_all(source.join("drive_c")).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(source.join("drive_c/game.exe"), b"MZ").unwrap();
        copy_tree(&source, &target, false, &AtomicBool::new(false)).unwrap();
        assert!(target.join("drive_c/game.exe").is_file());
        assert!(!target.join("Source").exists());
        let profile_root = source.join(".peligames-overlay/profiles").join(uuid::Uuid::new_v4().to_string());
        let merged = profile_root.join("prefix");
        fs::create_dir_all(merged.join("drive_c")).unwrap();
        fs::write(merged.join("drive_c/game.exe"), b"overlay").unwrap();
        fs::remove_dir_all(source.join("drive_c")).unwrap();
        std::os::unix::fs::symlink(merged.strip_prefix(&source).unwrap().join("drive_c"), source.join("drive_c")).unwrap();
        let profile = GameProfile { kind: ProfileKind::Overlay, base: root.join("Base").to_string_lossy().into_owned(), root: profile_root.to_string_lossy().into_owned(), flat_copy: true };
        let physical_executable = source.join("drive_c/game.exe").canonicalize().unwrap();
        let logical_executable = copied_executable(&physical_executable, &merged, &source);
        assert_eq!(copied_executable(&logical_executable, &source, &target), target.join("drive_c/game.exe"));
        copy_flat_overlay(&profile, &target, false, &AtomicBool::new(false)).unwrap();
        assert_eq!(fs::read(target.join("drive_c/game.exe")).unwrap(), b"overlay");
        assert!(!fs::symlink_metadata(target.join("drive_c")).unwrap().file_type().is_symlink());
        assert!(!target.join(".peligames-overlay").exists());
        fs::remove_dir_all(source).unwrap();
        assert_eq!(fs::read(target.join("drive_c/game.exe")).unwrap(), b"overlay");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn external_executable_copy_keeps_game_files_shared_and_prefix_writes_separate() {
        let root=std::env::temp_dir().join(format!("peligames-external-copy-{}",uuid::Uuid::new_v4()));
        let base=root.join("prefix-source");let target=root.join("prefix-copy");
        let game=root.join("external-game");fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(base.join("drive_c")).unwrap();
        fs::write(base.join("user.reg"),b"original registry").unwrap();
        let executable=game.join("game.exe");fs::write(&executable,b"MZgame").unwrap();
        copy_tree(&base,&target,false,&AtomicBool::new(false)).unwrap();
        assert_eq!(copied_executable(&executable,&base,&target),executable);
        assert_eq!(copied_executable(&base.join("drive_c/game.exe"),&base,&target),target.join("drive_c/game.exe"));
        assert_eq!(shared_game_directory(executable.to_str().unwrap(),&base).unwrap(),Some(game.to_string_lossy().into_owned()));
        fs::write(target.join("user.reg"),b"copy registry").unwrap();
        assert_eq!(fs::read(base.join("user.reg")).unwrap(),b"original registry");
        fs::remove_dir_all(&target).unwrap();
        assert_eq!(fs::read(&executable).unwrap(),b"MZgame");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn names_cannot_escape_the_destination() {
        for name in ["", ".", "..", "../game", "x/y", "x\\y", "a\nb"] {
            assert!(!valid_name(name));
        }
        assert!(valid_name("Real MU - cópia 2"));
    }
    #[test]
    fn full_copy_preserves_links_but_not_shared_writes() {
        let root = std::env::temp_dir().join(format!("peligames-copy-{}", uuid::Uuid::new_v4()));
        let base = root.join("original");
        let target = root.join("copy");
        fs::create_dir_all(base.join("drive_c")).unwrap();
        fs::write(base.join("drive_c/game.exe"), b"original").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join("drive_c"), base.join("c-link")).unwrap();
        copy_tree(&base, &target, false, &AtomicBool::new(false)).unwrap();
        rebase_links(&base, &target).unwrap();
        fs::write(target.join("drive_c/game.exe"), b"changed").unwrap();
        assert_eq!(
            fs::read(base.join("drive_c/game.exe")).unwrap(),
            b"original"
        );
        #[cfg(unix)]
        assert_eq!(
            fs::read_link(target.join("c-link")).unwrap(),
            target.join("drive_c")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
