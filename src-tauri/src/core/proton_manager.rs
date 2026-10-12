//! Official release catalog and transactional Latest runner installation, independent of Tauri.
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    GeProton,
    CachyosProton,
}
impl Family {
    pub fn id(self) -> &'static str {
        match self {
            Self::GeProton => "ge-proton",
            Self::CachyosProton => "cachyos-proton",
        }
    }
    pub fn latest_name(self) -> &'static str {
        match self {
            Self::GeProton => "Proton-GE Latest",
            Self::CachyosProton => "Proton-CachyOS Latest",
        }
    }
    fn repo(self) -> &'static str {
        match self {
            Self::GeProton => "GloriousEggroll/proton-ge-custom",
            Self::CachyosProton => "CachyOS/proton-cachyos",
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
    pub digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    assets: Vec<Asset>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct RunnerRelease {
    pub family: Family,
    pub latest: String,
    pub release_url: String,
    pub asset: Asset,
    pub checksum: Option<Asset>,
    pub current: Option<String>,
    pub installed_path: Option<String>,
    pub needs_update: bool,
}
#[derive(Clone, Serialize)]
pub struct Progress {
    pub family: Family,
    pub phase: &'static str,
    pub downloaded: u64,
    pub total: u64,
    pub speed_bytes_per_sec: f64,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: String,
    directory: String,
}

pub fn runners_dir() -> Result<PathBuf, String> {
    Ok(super::paths::app_root()?.join("runners/proton"))
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}
fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent("PeliGames/0.0.1")
        .connect_timeout(Duration::from_secs(20))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())
}
fn select_asset(assets: &[Asset], arch: &str) -> Result<Asset, String> {
    let suffixes: &[&str] = match arch {
        "x86_64" => &["-x86_64.tar.gz", "-x86_64.tar.xz"],
        "aarch64" => &["-aarch64.tar.gz", "-arm64.tar.xz"],
        _ => return Err("Arquitetura não suportada para este runner.".into()),
    };
    let matches: Vec<_> = assets
        .iter()
        .filter(|a| suffixes.iter().any(|s| a.name.ends_with(s)))
        .collect();
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }
    // Older GE releases use a generic x86_64 archive without an architecture suffix.
    if arch == "x86_64" {
        if let Some(a) = assets.iter().find(|a| {
            a.name.starts_with("GE-Proton")
                && a.name.ends_with(".tar.gz")
                && !a.name.contains("aarch64")
                && !a.name.contains("x86_64")
        }) {
            return Ok(a.clone());
        }
    }
    Err("O release não contém um pacote compatível com esta arquitetura.".into())
}
fn validate_url(family: Family, url: &str) -> Result<(), String> {
    if url.starts_with(&format!(
        "https://github.com/{}/releases/download/",
        family.repo()
    )) {
        Ok(())
    } else {
        Err("URL do pacote fora do repositório oficial.".into())
    }
}
pub async fn check_at(family: Family, root: &Path) -> Result<RunnerRelease, String> {
    let base = root.join(family.id());
    let recovery_base = base.clone();
    tokio::task::spawn_blocking(move || {
        let _lock = PUBLICATION_LOCK.lock().map_err(|e| e.to_string())?;
        recover_latest(family, &recovery_base)
    })
    .await
    .map_err(|e| e.to_string())??;
    let response = client()?
        .get(format!(
            "https://api.github.com/repos/{}/releases/latest",
            family.repo()
        ))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Falha ao consultar GitHub: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub retornou {}. Tente novamente mais tarde (limite de API ou rede).",
            response.status()
        ));
    }
    let release: Release = response.json().await.map_err(|e| e.to_string())?;
    if !safe_name(&release.tag_name) {
        return Err("Nome de versão inválido.".into());
    }
    let asset = select_asset(&release.assets, std::env::consts::ARCH)?;
    validate_url(family, &asset.browser_download_url)?;
    let stem = asset
        .name
        .trim_end_matches(".tar.gz")
        .trim_end_matches(".tar.xz");
    let checksum = release
        .assets
        .iter()
        .find(|a| a.name == format!("{stem}.sha512sum"))
        .cloned();
    if let Some(a) = &checksum {
        validate_url(family, &a.browser_download_url)?;
    }
    let installed = read_manifest(&base);
    let current = installed.as_ref().map(|m| m.version.clone());
    let legacy_directory = installed
        .as_ref()
        .is_some_and(|m| m.directory != family.latest_name());
    let installed_path = installed.map(|m| base.join(m.directory).to_string_lossy().into_owned());
    Ok(RunnerRelease {
        family,
        needs_update: legacy_directory || current.as_deref() != Some(&release.tag_name),
        latest: release.tag_name,
        release_url: release.html_url,
        asset,
        checksum,
        current,
        installed_path,
    })
}
static PUBLICATION_LOCK: Mutex<()> = Mutex::new(());

fn managed_directory(name: &str) -> bool {
    safe_name(name) || name == "Proton-GE Latest" || name == "Proton-CachyOS Latest"
}

fn validate_managed_folder(base: &Path, target: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(target).map_err(|e| e.to_string())?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || target.canonicalize().map_err(|e| e.to_string())?.parent()
            != Some(base.canonicalize().map_err(|e| e.to_string())?.as_path())
    {
        return Err("Pasta de Proton fora do diretório gerenciado.".into());
    }
    Ok(())
}

// The persisted manifest is the commit point. Recover an interrupted folder swap.
fn recover_latest(family: Family, base: &Path) -> Result<(), String> {
    let backup = base.join(".latest-previous");
    if !backup.exists() {
        return Ok(());
    }
    validate_managed_folder(base, &backup)?;
    let old_manifest =
        fs::read(backup.join(".peligames-previous.json")).map_err(|e| e.to_string())?;
    let previous: Manifest = serde_json::from_slice(&old_manifest).map_err(|e| e.to_string())?;
    let target = base.join(family.latest_name());
    let committed = read_manifest(base)
        .map(|m| m.directory == family.latest_name() && m.version != previous.version)
        .unwrap_or(false);
    if committed {
        fs::remove_dir_all(backup).map_err(|e| e.to_string())?;
    } else {
        if target.exists() {
            validate_managed_folder(base, &target)?;
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        fs::rename(&backup, &target).map_err(|e| e.to_string())?;
        fs::write(base.join("current.json"), old_manifest).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(target.join(".peligames-previous.json"));
    }
    Ok(())
}

fn rewrite_latest_identity(runner: &Path, name: &str) -> Result<(), String> {
    let path = runner.join("compatibilitytool.vdf");
    if !path.exists() {
        return Ok(());
    }
    if fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Manifesto do Proton não pode ser um link simbólico.".into());
    }
    let mut text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // Collect quoted token spans without altering unrelated manifest fields.
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let start = i + 1;
        i = start;
        while i < bytes.len() && bytes[i] != b'"' {
            if bytes[i] == b'\\' {
                i += 1;
            }
            i += 1;
        }
        if i >= bytes.len() {
            return Err("Manifesto do Proton inválido.".into());
        }
        spans.push((start, i));
        i += 1;
    }
    let mut edits = Vec::new();
    for (index, &(start, end)) in spans.iter().enumerate() {
        if &text[start..end] == "compat_tools" || &text[start..end] == "display_name" {
            let span = *spans
                .get(index + 1)
                .ok_or("Manifesto do Proton incompleto.")?;
            edits.push(span);
        }
    }
    if edits.len() != 2 {
        return Err("Manifesto do Proton não contém uma ferramenta única.".into());
    }
    for (start, end) in edits.into_iter().rev() {
        text.replace_range(start..end, name);
    }
    fs::write(path, text).map_err(|e| e.to_string())
}

fn ensure_runner_idle(target: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if target.exists() {
        let prefix = format!(
            "{}/",
            target.canonicalize().map_err(|e| e.to_string())?.display()
        );
        for entry in fs::read_dir("/proc").map_err(|e| e.to_string())?.flatten() {
            if !entry
                .file_name()
                .to_string_lossy()
                .bytes()
                .all(|c| c.is_ascii_digit())
            {
                continue;
            }
            for file in ["cmdline", "maps"] {
                if fs::read(entry.path().join(file))
                    .ok()
                    .is_some_and(|data| String::from_utf8_lossy(&data).contains(&prefix))
                {
                    return Err("Este Proton está em uso. Feche os jogos antes de atualizar ou desinstalar.".into());
                }
            }
        }
    }
    Ok(())
}

fn publish_latest(
    family: Family,
    base: &Path,
    runner: &Path,
    staging: &Path,
    version: &str,
) -> Result<PathBuf, String> {
    let _lock = PUBLICATION_LOCK.lock().map_err(|e| e.to_string())?;
    recover_latest(family, base)?;
    let target = base.join(family.latest_name());
    ensure_runner_idle(&target)?;
    let previous = read_manifest(base);
    if target.exists() {
        validate_managed_folder(base, &target)?;
        if previous.as_ref().map(|m| m.directory.as_str()) != Some(family.latest_name()) {
            return Err("Pasta Latest existente sem registro válido.".into());
        }
    }
    // Preserve user settings before touching the installed tree.
    if let Some(old) = &previous {
        let settings = base.join(&old.directory).join("user_settings.py");
        if settings.exists() {
            validate_managed_folder(base, &base.join(&old.directory))?;
            let destination = runner.join("user_settings.py");
            if fs::symlink_metadata(&destination).is_ok() {
                fs::remove_file(&destination).map_err(|e| e.to_string())?;
            }
            if fs::symlink_metadata(&settings)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
            {
                #[cfg(unix)]
                std::os::unix::fs::symlink(
                    fs::read_link(&settings).map_err(|e| e.to_string())?,
                    &destination,
                )
                .map_err(|e| e.to_string())?;
            } else {
                fs::copy(settings, &destination).map_err(|e| e.to_string())?;
            }
        }
    }
    rewrite_latest_identity(runner, family.latest_name())?;
    let pending = staging.join("current.json");
    let manifest = Manifest {
        version: version.into(),
        directory: family.latest_name().into(),
    };
    fs::write(
        &pending,
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let backup = base.join(".latest-previous");
    let replacing = target.exists();
    if replacing {
        let metadata = fs::read(base.join("current.json")).map_err(|e| e.to_string())?;
        fs::write(target.join(".peligames-previous.json"), metadata).map_err(|e| e.to_string())?;
        fs::rename(&target, &backup).map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::rename(runner, &target) {
        if replacing {
            recover_latest(family, base)?;
        }
        return Err(error.to_string());
    }
    if let Err(error) = fs::rename(pending, base.join("current.json")) {
        if replacing {
            recover_latest(family, base)?;
        } else {
            fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
        }
        return Err(error.to_string());
    }
    // A failed cleanup remains recoverable on the next operation.
    if replacing {
        let _ = fs::remove_dir_all(backup);
    }
    Ok(target)
}

fn read_manifest(base: &Path) -> Option<Manifest> {
    let m: Manifest = serde_json::from_slice(&fs::read(base.join("current.json")).ok()?).ok()?;
    if !managed_directory(&m.directory) || !base.join(&m.directory).join("proton").is_file() {
        return None;
    }
    Some(m)
}
/// Remove only the confirmed version managed by PeliGames, preserving older runners.
pub fn uninstall_at(family: Family, root: &Path, version: &str) -> Result<(), String> {
    let _lock = PUBLICATION_LOCK.lock().map_err(|e| e.to_string())?;
    let base = root.join(family.id());
    recover_latest(family, &base)?;
    let installed = read_manifest(&base).ok_or("Nenhuma versão gerenciada instalada.")?;
    if installed.version != version {
        return Err("A versão instalada mudou. Verifique novamente antes de desinstalar.".into());
    }
    let target = base.join(&installed.directory);
    ensure_runner_idle(&target)?;
    let metadata = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
    let canonical_base = base.canonicalize().map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink()
        || target.canonicalize().map_err(|e| e.to_string())?.parent()
            != Some(canonical_base.as_path())
    {
        return Err("A pasta do runner está fora do diretório gerenciado.".into());
    }
    fs::remove_dir_all(target).map_err(|e| format!("Falha ao remover Proton: {e}"))?;
    fs::remove_file(base.join("current.json")).map_err(|e| e.to_string())
}

fn cancelled(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Download cancelado.".into())
    } else {
        Ok(())
    }
}
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn digest_matches(file: &Path, expected: &str, sha512: bool) -> Result<(), String> {
    let mut reader = fs::File::open(file).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 65536];
    let mut h256 = Sha256::new();
    let mut h512 = Sha512::new();
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if sha512 {
            h512.update(&buf[..n]);
        } else {
            h256.update(&buf[..n]);
        }
    }
    let actual = if sha512 {
        format!("{:x}", h512.finalize())
    } else {
        format!("{:x}", h256.finalize())
    };
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err("A verificação de integridade do pacote falhou.".into())
    }
}
fn extract(file: &Path, name: &str, dest: &Path, cancel: &AtomicBool) -> Result<PathBuf, String> {
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let file = fs::File::open(file).map_err(|e| e.to_string())?;
    let decoder: Box<dyn Read> = if name.ends_with(".tar.gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(xz2::read::XzDecoder::new(file))
    };
    let mut archive = tar::Archive::new(decoder);
    let mut root: Option<PathBuf> = None;
    for entry in archive.entries().map_err(|e| e.to_string())? {
        cancelled(cancel)?;
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err("Caminho inválido no pacote.".into());
        }
        let mut components = path
            .components()
            .filter(|c| !matches!(c, Component::CurDir));
        let Some(first) = components.next() else {
            continue;
        };
        let first = PathBuf::from(first.as_os_str());
        if root.as_ref().is_some_and(|r| r != &first) {
            return Err("Pacote com múltiplas raízes inesperadas.".into());
        }
        root = Some(first);
        if !entry
            .unpack_in(dest)
            .map_err(|e| format!("Falha ao extrair runner: {e}"))?
        {
            return Err("Entrada fora da pasta de instalação.".into());
        }
    }
    let runner = dest.join(root.ok_or("Pacote vazio.")?);
    if !runner.join("proton").is_file() {
        return Err("O pacote não contém o executável proton.".into());
    }
    Ok(runner)
}

pub async fn install_at(
    family: Family,
    root: PathBuf,
    cancel: Arc<AtomicBool>,
    emit: Arc<dyn Fn(Progress) + Send + Sync>,
) -> Result<String, String> {
    let release = check_at(family, &root).await?;
    cancelled(&cancel)?;
    if !release.needs_update
        && release
            .installed_path
            .as_ref()
            .map(|p| {
                Path::new(p).file_name().and_then(|n| n.to_str()) == Some(family.latest_name())
            })
            .unwrap_or(false)
    {
        return release
            .installed_path
            .ok_or("Instalação não encontrada.".into());
    }
    let base = root.join(family.id());
    fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let staging = Staging(base.join(format!(".download-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&staging.0).map_err(|e| e.to_string())?;
    let archive_path = staging.0.join("package");
    let client = client()?;
    let mut response = client
        .get(&release.asset.browser_download_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let total = release.asset.size;
    let mut downloaded = 0u64;
    let start = Instant::now();
    let mut last = Instant::now();
    let mut file = fs::File::create(&archive_path).map_err(|e| e.to_string())?;
    emit(Progress {
        family,
        phase: "downloading",
        downloaded,
        total,
        speed_bytes_per_sec: 0.0,
    });
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        cancelled(&cancel)?;
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        if downloaded > total && total > 0 {
            return Err("Pacote maior que o tamanho informado pelo GitHub.".into());
        }
        if last.elapsed() >= Duration::from_millis(100) {
            emit(Progress {
                family,
                phase: "downloading",
                downloaded,
                total,
                speed_bytes_per_sec: downloaded as f64 / start.elapsed().as_secs_f64().max(0.001),
            });
            last = Instant::now();
        }
    }
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    if downloaded != total {
        return Err("Download incompleto. Tente novamente.".into());
    }
    cancelled(&cancel)?;
    emit(Progress {
        family,
        phase: "verifying",
        downloaded,
        total,
        speed_bytes_per_sec: 0.0,
    });
    let expected = if let Some(digest) = release
        .asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
    {
        (digest.to_string(), false)
    } else if let Some(checksum) = &release.checksum {
        let text = client
            .get(&checksum.browser_download_url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        (
            text.split_whitespace()
                .next()
                .ok_or("Checksum vazio.")?
                .to_string(),
            true,
        )
    } else {
        return Err("O release não fornece checksum para validar o pacote.".into());
    };
    let hash_path = archive_path.clone();
    tokio::task::spawn_blocking(move || digest_matches(&hash_path, &expected.0, expected.1))
        .await
        .map_err(|e| e.to_string())??;
    cancelled(&cancel)?;
    emit(Progress {
        family,
        phase: "extracting",
        downloaded,
        total,
        speed_bytes_per_sec: 0.0,
    });
    let unpack_path = staging.0.join("unpacked");
    let asset_name = release.asset.name.clone();
    let flag = cancel.clone();
    let runner = tokio::task::spawn_blocking(move || {
        extract(&archive_path, &asset_name, &unpack_path, &flag)
    })
    .await
    .map_err(|e| e.to_string())??;
    cancelled(&cancel)?;
    let publish_base = base.clone();
    let publish_stage = staging.0.clone();
    let version = release.latest.clone();
    let target = tokio::task::spawn_blocking(move || {
        publish_latest(family, &publish_base, &runner, &publish_stage, &version)
    })
    .await
    .map_err(|e| e.to_string())??;
    emit(Progress {
        family,
        phase: "complete",
        downloaded,
        total,
        speed_bytes_per_sec: 0.0,
    });
    Ok(target.to_string_lossy().into_owned())
}

#[cfg(test)]
mod uninstall_tests {
    use super::*;
    fn fixture() -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("peligames-proton-test-{}", uuid::Uuid::new_v4()));
        let base = root.join("ge-proton");
        fs::create_dir_all(base.join("new-version")).unwrap();
        fs::write(base.join("new-version/proton"), "runner").unwrap();
        fs::write(
            base.join("current.json"),
            r#"{"version":"new-version","directory":"new-version"}"#,
        )
        .unwrap();
        (root, base)
    }
    #[test]
    fn uninstall_preserves_other_versions_and_families() {
        let (root, base) = fixture();
        fs::create_dir_all(base.join("old-version")).unwrap();
        fs::write(base.join("old-version/proton"), "old").unwrap();
        fs::create_dir_all(root.join("cachyos-proton")).unwrap();
        uninstall_at(Family::GeProton, &root, "new-version").unwrap();
        assert!(!base.join("new-version").exists());
        assert!(!base.join("current.json").exists());
        assert!(base.join("old-version/proton").is_file());
        assert!(root.join("cachyos-proton").is_dir());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn uninstall_rejects_stale_confirmation_and_unsafe_paths() {
        let (root, base) = fixture();
        assert!(uninstall_at(Family::GeProton, &root, "wrong-version").is_err());
        assert!(base.join("new-version/proton").is_file());
        fs::write(
            base.join("current.json"),
            r#"{"version":"new-version","directory":".."}"#,
        )
        .unwrap();
        assert!(uninstall_at(Family::GeProton, &root, "new-version").is_err());
        #[cfg(unix)]
        {
            fs::write(
                base.join("current.json"),
                r#"{"version":"new-version","directory":"linked"}"#,
            )
            .unwrap();
            std::os::unix::fs::symlink(base.join("new-version"), base.join("linked")).unwrap();
            assert!(uninstall_at(Family::GeProton, &root, "new-version").is_err());
            assert!(base.join("new-version/proton").is_file());
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod latest_tests {
    use super::*;
    fn package(base: &Path, label: &str) -> (PathBuf, PathBuf) {
        let stage = base.join(format!(".test-{}", uuid::Uuid::new_v4()));
        let runner = stage.join("runner");
        fs::create_dir_all(&runner).unwrap();
        fs::write(runner.join("proton"), label).unwrap();
        fs::write(runner.join("compatibilitytool.vdf"), r#""compatibilitytools" { "compat_tools" { "old-name" { "display_name" "Old name" "install_path" "." } } }"#).unwrap();
        (stage, runner)
    }
    fn root() -> PathBuf {
        std::env::temp_dir().join(format!("peligames-latest-{}", uuid::Uuid::new_v4()))
    }
    #[test]
    fn replacement_preserves_settings_and_removes_obsolete_files() {
        let root = root();
        for family in [Family::GeProton, Family::CachyosProton] {
            let base = root.join(family.id());
            let (stage, runner) = package(&base, "v1");
            let first = publish_latest(family, &base, &runner, &stage, "v1").unwrap();
            fs::write(first.join("obsolete"), "old").unwrap();
            fs::write(first.join("user_settings.py"), "custom = True").unwrap();
            let (stage, runner) = package(&base, "v2");
            let second = publish_latest(family, &base, &runner, &stage, "v2").unwrap();
            assert_eq!(first, second);
            assert_eq!(
                fs::read_to_string(second.join("user_settings.py")).unwrap(),
                "custom = True"
            );
            assert!(!second.join("obsolete").exists());
            assert!(!base.join(".latest-previous").exists());
            assert_eq!(read_manifest(&base).unwrap().version, "v2");
            let vdf = fs::read_to_string(second.join("compatibilitytool.vdf")).unwrap();
            assert_eq!(vdf.matches(family.latest_name()).count(), 2);
            assert!(vdf.contains("install_path"));
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn invalid_package_leaves_installed_runner_unchanged() {
        let root = root();
        let base = root.join("ge-proton");
        let (stage, runner) = package(&base, "v1");
        let target = publish_latest(Family::GeProton, &base, &runner, &stage, "v1").unwrap();
        let (stage, runner) = package(&base, "v2");
        fs::write(runner.join("compatibilitytool.vdf"), "invalid").unwrap();
        assert!(publish_latest(Family::GeProton, &base, &runner, &stage, "v2").is_err());
        assert_eq!(fs::read_to_string(target.join("proton")).unwrap(), "v1");
        assert_eq!(read_manifest(&base).unwrap().version, "v1");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_promotion_restores_previous_folder() {
        let root = root();
        let base = root.join("ge-proton");
        let (stage, runner) = package(&base, "v1");
        let target = publish_latest(Family::GeProton, &base, &runner, &stage, "v1").unwrap();
        let stage = base.join(".missing-package");
        fs::create_dir(&stage).unwrap();
        assert!(publish_latest(
            Family::GeProton,
            &base,
            &stage.join("missing"),
            &stage,
            "v2"
        )
        .is_err());
        assert_eq!(fs::read_to_string(target.join("proton")).unwrap(), "v1");
        assert_eq!(read_manifest(&base).unwrap().version, "v1");
        assert!(!base.join(".latest-previous").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_swap_rolls_back_and_committed_swap_cleans_backup() {
        for committed in [false, true] {
            let root = root();
            let base = root.join("ge-proton");
            let (stage, runner) = package(&base, "v1");
            let target = publish_latest(Family::GeProton, &base, &runner, &stage, "v1").unwrap();
            let old = fs::read(base.join("current.json")).unwrap();
            fs::write(target.join(".peligames-previous.json"), old).unwrap();
            fs::rename(&target, base.join(".latest-previous")).unwrap();
            let (_, fresh) = package(&base, "v2");
            fs::rename(fresh, &target).unwrap();
            if committed {
                fs::write(
                    base.join("current.json"),
                    r#"{"version":"v2","directory":"Proton-GE Latest"}"#,
                )
                .unwrap();
            }
            recover_latest(Family::GeProton, &base).unwrap();
            assert_eq!(
                fs::read_to_string(target.join("proton")).unwrap(),
                if committed { "v2" } else { "v1" }
            );
            assert!(!base.join(".latest-previous").exists());
            uninstall_at(Family::GeProton, &root, if committed { "v2" } else { "v1" }).unwrap();
            assert!(!target.exists());
            fs::remove_dir_all(root).unwrap();
        }
    }
}
