//! Official release catalog and versioned runner installation, independent of Tauri.
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
        Arc,
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
    let base = root.join(family.id());
    let installed = read_manifest(&base);
    let current = installed.as_ref().map(|m| m.version.clone());
    let installed_path = installed.map(|m| base.join(m.directory).to_string_lossy().into_owned());
    Ok(RunnerRelease {
        family,
        needs_update: current.as_deref() != Some(&release.tag_name),
        latest: release.tag_name,
        release_url: release.html_url,
        asset,
        checksum,
        current,
        installed_path,
    })
}
fn read_manifest(base: &Path) -> Option<Manifest> {
    let m: Manifest = serde_json::from_slice(&fs::read(base.join("current.json")).ok()?).ok()?;
    if !safe_name(&m.directory) || !base.join(&m.directory).join("proton").is_file() {
        return None;
    }
    Some(m)
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
    if !release.needs_update {
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
    let target = base.join(&release.latest);
    if target.exists() {
        return Err(
            "A pasta desta versão já existe. Verifique sua instalação antes de repetir.".into(),
        );
    }
    // Publish only a completely verified and extracted runner. Older versions remain intact.
    fs::rename(runner, &target).map_err(|e| e.to_string())?;
    let manifest = Manifest {
        version: release.latest.clone(),
        directory: release.latest,
    };
    let pending = staging.0.join("current.json");
    let publish = (|| {
        fs::write(
            &pending,
            serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        fs::rename(pending, base.join("current.json")).map_err(|e| e.to_string())
    })();
    if let Err(e) = publish {
        let _ = fs::remove_dir_all(&target);
        return Err(e);
    }
    emit(Progress {
        family,
        phase: "complete",
        downloaded,
        total,
        speed_bytes_per_sec: 0.0,
    });
    Ok(target.to_string_lossy().into_owned())
}
