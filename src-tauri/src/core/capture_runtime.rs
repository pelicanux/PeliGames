//! Pinned portable recorder runtime, isolated from the launcher and host installation.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
pub const VERSION: &str = "6.1.4";
pub const URL:&str="https://github.com/pkgforge-dev/gpu-screen-recorder-AppImage/releases/download/6.1.4%402026-10-17_1792236768/gpu-screen-recorder-6.1.4-anylinux-x86_64.AppImage";
const SHA256: &str = "bfa97075e9bb80888d519d82e40312a445529aa349f480251687e333e71d2f4d";
pub(crate) static OPERATION: Mutex<()> = Mutex::new(());
#[derive(Serialize)]
pub struct RuntimeStatus {
    pub installed: bool,
    pub version: &'static str,
    pub source: &'static str,
    pub bytes: u64,
    pub directory: String,
}
#[derive(Clone, Serialize)]
pub struct Progress {
    pub phase: &'static str,
    pub downloaded: u64,
    pub total: Option<u64>,
}
fn base() -> Result<PathBuf, String> {
    Ok(super::paths::app_path("CaptureRuntime")?)
}
fn location() -> Result<PathBuf, String> {
    Ok(base()?.join(VERSION))
}
fn private(path: &Path) -> Result<(), String> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.to_string()),
    }
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return Err("Pasta do componente de captura sem permissões privadas.".into());
    }
    Ok(())
}
fn valid_at(path: &Path) -> bool {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return false;
    };
    if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
        return false;
    }
    fs::read_to_string(path.join("verified.sha256")).is_ok_and(|v| v == SHA256)
        && [
            "bin/gpu-screen-recorder",
            "bin/gsr-kms-server",
            "shared/bin/gpu-screen-recorder",
            "shared/bin/gsr-kms-server",
        ]
        .iter()
        .all(|name| {
            fs::symlink_metadata(path.join(name)).is_ok_and(|m| {
                m.is_file()
                    && m.uid() == unsafe { libc::geteuid() }
                    && m.mode() & 0o022 == 0
                    && m.mode() & 0o100 != 0
            })
        })
}
pub fn executable() -> Option<PathBuf> {
    let path = location().ok()?;
    valid_at(&path).then(|| path.join("bin/gpu-screen-recorder"))
}
pub fn status() -> Result<RuntimeStatus, String> {
    let path = location()?;
    let installed = valid_at(&path);
    Ok(RuntimeStatus {
        installed,
        version: VERSION,
        source: "pkgforge-dev · pacote portátil do GPU Screen Recorder",
        bytes: if installed { tree_size(&path) } else { 0 },
        directory: path.to_string_lossy().into_owned(),
    })
}
fn tree_size(path: &Path) -> u64 {
    fs::read_dir(path)
        .map(|items| {
            items
                .flatten()
                .map(|e| {
                    let Ok(m) = fs::symlink_metadata(e.path()) else {
                        return 0;
                    };
                    if m.is_dir() {
                        tree_size(&e.path())
                    } else if m.is_file() {
                        m.len()
                    } else {
                        0
                    }
                })
                .sum()
        })
        .unwrap_or(0)
}
fn verify(bytes: &[u8]) -> Result<(), String> {
    if format!("{:x}", Sha256::digest(bytes)) != SHA256 {
        Err("A verificação SHA-256 falhou. O componente anterior foi preservado.".into())
    } else {
        Ok(())
    }
}
fn run_extractor(package: &Path, directory: &Path) -> Result<(), String> {
    let mut command = Command::new(package);
    super::env_utils::host_environment(&mut command);
    let mut child = command
        .arg("--appimage-extract")
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err("Não foi possível extrair o componente de captura.".into())
                }
            }
            None if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("A extração do componente excedeu o tempo limite.".into());
            }
            _ => std::thread::sleep(Duration::from_millis(80)),
        }
    }
}
/// Synchronous extraction/publication is run on a blocking worker by the command.
pub fn install_bytes(
    bytes: &[u8],
    progress: impl Fn(Progress),
    is_active: impl Fn() -> bool,
) -> Result<RuntimeStatus, String> {
    let _guard = OPERATION
        .try_lock()
        .map_err(|_| "Já existe uma operação no componente de captura.")?;
    if is_active() {
        return Err("Encerre a captura antes de alterar o componente.".into());
    }
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("Este componente de captura requer Linux x86_64.".into());
    }
    verify(bytes)?;
    let root = base()?;
    private(&root)?;
    let staging = root.join(format!(".install-{}", uuid::Uuid::new_v4()));
    private(&staging)?;
    let result = (|| {
        let package = staging.join("backend.AppImage");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&package)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        drop(file);
        progress(Progress {
            phase: "extracting",
            downloaded: bytes.len() as u64,
            total: Some(bytes.len() as u64),
        });
        run_extractor(&package, &staging)?;
        let extracted = staging
            .join("squashfs-root")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !extracted.starts_with(&staging) {
            return Err("Destino de extração inválido.".into());
        }
        fs::set_permissions(&extracted, fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
        // Keep only CLI/helper entry points. Never run AppRun, GUI or updater hooks.
        for folder in ["bin", "shared/bin"] {
            for entry in fs::read_dir(extracted.join(folder))
                .map_err(|e| e.to_string())?
                .flatten()
            {
                if !matches!(
                    entry.file_name().to_str(),
                    Some("gpu-screen-recorder" | "gsr-kms-server")
                ) {
                    let path = entry.path();
                    if path.is_dir() {
                        fs::remove_dir_all(path)
                    } else {
                        fs::remove_file(path)
                    }
                    .map_err(|e| e.to_string())?;
                }
            }
        }
        for name in [
            "AppRun",
            "AppRun.sh",
            "AppRun.lib",
            "com.dec05eba.gpu_screen_recorder.desktop",
            "com.dec05eba.gpu_screen_recorder.png",
        ] {
            let _ = fs::remove_file(extracted.join(name));
        }
        fs::write(extracted.join("verified.sha256"), SHA256).map_err(|e| e.to_string())?;
        fs::write(extracted.join("PELIGAMES-SOURCE.txt"),format!("GPU Screen Recorder {VERSION} · GPL-3.0-only\nSource: https://git.dec05eba.com/gpu-screen-recorder/\nPortable packaging: https://github.com/pkgforge-dev/gpu-screen-recorder-AppImage\nPackage: {URL}\nSHA-256: {SHA256}\nThe launcher executes only the CLI and its KMS helper.\n")).map_err(|e|e.to_string())?;
        fs::write(
            extracted.join("COPYING-GPU-SCREEN-RECORDER"),
            include_str!("../../licenses/gpu-screen-recorder-GPL-3.0.txt"),
        )
        .map_err(|e| e.to_string())?;
        if !valid_at(&extracted) {
            return Err("O pacote extraído não contém o backend esperado.".into());
        }
        super::capture::check_runtime(&extracted.join("bin/gpu-screen-recorder"))?;
        let target = location()?;
        let previous = root.join(format!(".previous-{}", uuid::Uuid::new_v4()));
        let had_previous = target.exists();
        if had_previous {
            let m = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
            if !m.is_dir() {
                return Err("Destino do componente inválido.".into());
            }
            fs::rename(&target, &previous).map_err(|e| e.to_string())?;
        }
        if let Err(e) = fs::rename(&extracted, &target) {
            if had_previous {
                let _ = fs::rename(&previous, &target);
            }
            return Err(e.to_string());
        }
        if had_previous {
            let _ = fs::remove_dir_all(previous);
        }
        progress(Progress {
            phase: "installed",
            downloaded: bytes.len() as u64,
            total: Some(bytes.len() as u64),
        });
        status()
    })();
    let _ = fs::remove_dir_all(staging);
    result
}
pub fn remove(is_active: impl Fn() -> bool) -> Result<RuntimeStatus, String> {
    let _guard = OPERATION
        .try_lock()
        .map_err(|_| "O componente de captura está ocupado.")?;
    if is_active() {
        return Err("Encerre a captura antes de remover o componente.".into());
    }
    let root = base()?;
    if !root.exists() {
        return status();
    }
    private(&root)?;
    let path = location()?;
    match fs::symlink_metadata(&path) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {
            fs::remove_dir_all(path).map_err(|e| e.to_string())?
        }
        Ok(_) => return Err("Destino do componente inválido.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(e.to_string()),
    };
    status()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_corrupt_download_before_touching_installation() {
        assert!(verify(b"invalid package").is_err());
    }
    #[test]
    fn runtime_validation_rejects_links_and_missing_payload() {
        let root =
            std::env::temp_dir().join(format!("capture-runtime-test-{}", uuid::Uuid::new_v4()));
        private(&root).unwrap();
        assert!(!valid_at(&root));
        let link = root.with_extension("link");
        std::os::unix::fs::symlink(&root, &link).unwrap();
        assert!(!valid_at(&link));
        fs::remove_file(link).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
