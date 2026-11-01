//! Versioned, explicitly imported runtime packages. No proprietary DLL is bundled with the launcher.
use super::*;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt};
pub const HUB: &str = "AMDNR-NeuralHub-080";
const MANIFEST: &str = "peligames-runtime.json";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Runtime {
    #[serde(rename = "0.6.0")]
    V06,
    #[serde(rename = "0.8.0")]
    V08,
}
impl Runtime {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "0.6.0" => Ok(Self::V06),
            "0.8.0" => Ok(Self::V08),
            _ => Err("Versão Daniel NR não suportada.".into()),
        }
    }
    pub fn version(self) -> &'static str {
        match self {
            Self::V06 => "0.6.0",
            Self::V08 => "0.8.0",
        }
    }
    pub fn package(self, base: &Path) -> PathBuf {
        base.join("prepared")
            .join(format!("daniel-{}", self.version()))
    }
}
#[derive(Serialize)]
pub struct RuntimeState {
    pub version: &'static str,
    pub ready: bool,
    pub present: bool,
}
pub fn states(base: &Path) -> Vec<RuntimeState> {
    [Runtime::V06, Runtime::V08]
        .into_iter()
        .map(|r| RuntimeState {
            version: r.version(),
            ready: ready(base, r),
            present: fs::symlink_metadata(r.package(base)).is_ok(),
        })
        .collect()
}
pub(super) fn installed(directory: &Path, executable: &Path) -> Option<Runtime> {
    let receipt: Receipt = serde_json::from_str(&text(&directory.join(RECEIPT))).ok()?;
    (receipt.version == 1 && receipt.executable == string(executable))
        .then(|| Runtime::parse(&receipt.backend).ok())
        .flatten()
}
pub fn ready(base: &Path, runtime: Runtime) -> bool {
    if runtime == Runtime::V06 {
        return super::package_ready(base);
    }
    // Status reads only the small manifest and required files. Full hashes are checked before installation.
    let folder = runtime.package(base);
    manifest(&folder).is_ok_and(|m| {
        required()
            .iter()
            .all(|p| m.files.contains_key(*p) && folder.join(p).is_file())
    })
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    runtime: Runtime,
    bridge: String,
    files: BTreeMap<String, String>,
}
fn manifest(folder: &Path) -> Result<Manifest, String> {
    let path = folder.join(MANIFEST);
    let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 2 * 1024 * 1024 || meta.file_type().is_symlink() {
        return Err("Manifesto do runtime inválido.".into());
    }
    let m: Manifest = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if m.schema != 1 || m.runtime != Runtime::V08 || m.bridge != "peligames-hip-080-v3" {
        return Err("Pacote não usa o runtime/ponte experimental esperado.".into());
    }
    Ok(m)
}
fn required() -> &'static [&'static str] {
    &[
        "winmm.dll",
        "d3dcompiler_47.dll",
        "OptiScaler.ini",
        "amdnr-linux/run.sh",
        "amdnr-linux/launch.py",
        "amdnr-linux/proxies.py",
        "amdnr-linux/rocm/lib/libamdhip64.so.7",
        "amdnr-linux/x86_64-unix/amdhip64_7.so",
        "amdnr-linux/x86_64-windows/amdhip64_7.dll",
        "amdnr-linux/x86_64-unix/libamdnr_signal_guard.so",
        "AMDNR-NeuralHub-080/neuralhub_host.dll",
        "AMDNR-NeuralHub-080/settings.ini",
        "AMDNR-NeuralHub-080/neuralhub.ini",
        "AMDNR-NeuralHub-080/data/nvngx_dlssnr.dll",
        "AMDNR-NeuralHub-080/data/index.ini",
    ]
}
fn relative(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && value.len() < 1024
        && Path::new(value)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}
fn safe_files(folder: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for (count, entry) in walkdir::WalkDir::new(folder)
        .follow_links(false)
        .into_iter()
        .enumerate()
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if count > 10000 || (!entry.file_type().is_file() && !entry.file_type().is_dir()) {
            return Err("Pacote contém links, arquivos especiais ou entradas demais.".into());
        }
        if entry.file_type().is_dir() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(folder)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("Nome de arquivo inválido.")?
            .to_owned();
        if !relative(&rel) {
            return Err("Caminho inválido no pacote.".into());
        }
        if rel == MANIFEST {
            continue;
        }
        if rel.starts_with('.')
            || rel.contains("/prefix-backups/")
            || rel.starts_with("amdnr-linux/prefixes.")
            || rel.ends_with(".log")
            || rel.contains("/cache/")
        {
            return Err(format!(
                "Pacote contém estado de jogo ou diagnóstico: {rel}"
            ));
        }
        total = total
            .checked_add(entry.metadata().map_err(|e| e.to_string())?.len())
            .ok_or("Pacote grande demais.")?;
        if total > 8 * 1024 * 1024 * 1024 {
            return Err("Pacote excede 8 GB.".into());
        }
        files.insert(rel, entry.into_path());
    }
    Ok(files)
}
pub(super) fn verify(folder: &Path) -> Result<(), String> {
    let m = manifest(folder)?;
    let files = safe_files(folder)?;
    if files.len() != m.files.len() || !required().iter().all(|key| files.contains_key(*key)) {
        return Err("Arquivos do pacote não correspondem ao manifesto.".into());
    }
    for (rel, path) in &files {
        let expected = m.files.get(rel).ok_or("Arquivo extra no pacote.")?;
        if expected.len() != 64
            || !expected.bytes().all(|c| c.is_ascii_hexdigit())
            || operations::hash(path)? != *expected
        {
            return Err(format!("Integridade do runtime não confere: {rel}"));
        }
    }
    // These are the paired artifacts validated with the 0.8 bridge; never mix Unix-call dispatch tables.
    for (file, sha) in [
        (
            "AMDNR-NeuralHub-080/neuralhub_host.dll",
            "be9172b68d0e77f98f04f941f4e2abe2c73eb01b8aafe94b26ee31eefefb5eb8",
        ),
        (
            "amdnr-linux/x86_64-windows/amdhip64_7.dll",
            "181c8fbe1a63976a51c39684ee0ce8b064d7c83ebf8a84eec19ef06c87e8038a",
        ),
        (
            "amdnr-linux/x86_64-unix/amdhip64_7.so",
            "40e364d2403b58690158bbf533550b90cf79f75ff559571e276423e5f3f73b7e",
        ),
        (
            "amdnr-linux/x86_64-unix/libamdnr_signal_guard.so",
            "81694316d032c4242c6f1419822634563fc818fd3a706ca183c520ecc292a21d",
        ),
    ] {
        if m.files.get(file).map(String::as_str) != Some(sha) {
            return Err(format!("Versão não validada do host/ponte: {file}"));
        }
    }
    let settings =
        super::super::neural_config::sections(&text(&folder.join(HUB).join("settings.ini")));
    if super::super::neural_config::get(&settings, "Compute", "Backend") != Some("HIP")
        || super::super::neural_config::get(&settings, "Debug", "HipPredicatedWait") != Some("0")
    {
        return Err("Pacote requer Backend=HIP e HipPredicatedWait=0.".into());
    }
    Ok(())
}
pub fn import(
    base: &Path,
    source: &Path,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    if !source.is_absolute()
        || fs::symlink_metadata(source)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
    {
        return Err("Escolha uma pasta absoluta de runtime preparada.".into());
    }
    verify(source)?;
    let destination = Runtime::V08.package(base);
    if destination.exists() {
        verify(&destination)?;
        return Ok(());
    }
    fs::create_dir_all(destination.parent().unwrap()).map_err(|e| e.to_string())?;
    let stage = destination.with_file_name("importing-daniel-0.8.0");
    if stage.exists() {
        return Err("Há uma importação incompleta. Preserve a pasta importing-daniel-0.8.0 antes de tentar novamente.".into());
    }
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = (|| {
        progress("Importando Daniel 0.8.0 e a ponte HIP verificada…".into());
        for (rel, file) in safe_files(source)? {
            let target = stage.join(&rel);
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::copy(file, &target).map_err(|e| e.to_string())?;
            fs::set_permissions(
                &target,
                fs::Permissions::from_mode(if rel.ends_with(".sh") { 0o700 } else { 0o600 }),
            )
            .map_err(|e| e.to_string())?;
        }
        fs::copy(source.join(MANIFEST), stage.join(MANIFEST)).map_err(|e| e.to_string())?;
        verify(&stage)?;
        fs::rename(&stage, destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}
