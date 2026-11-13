use super::{config_path, game_directory, settings, WRITE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path},
};
const ROOT_FILES: &[&str] = &[
    "amd_fidelityfx_dx12.dll",
    "amd_fidelityfx_framegeneration_dx12.dll",
    "amd_fidelityfx_upscaler_dx12.dll",
    "amd_fidelityfx_vk.dll",
    "dlssg_to_fsr3_amd_is_better.dll",
    "fakenvapi.dll",
    "fakenvapi.ini",
    "libxell.dll",
    "libxess.dll",
    "libxess_dx11.dll",
    "libxess_fg.dll",
];
pub const JOURNAL: &str = "peligames-optiscaler-install.json";
#[derive(Serialize, Deserialize)]
pub(super) struct Entry {
    pub path: String,
    pub sha256: String,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Journal {
    pub schema: u8,
    pub proxy: String,
    pub files: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supported_settings: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_version: Option<String>,
}
pub(super) fn package_version(game: &Path) -> Option<String> {
    read_journal(game).ok()?.package_version
}
pub(super) fn supported_settings(game: &Path) -> Option<Vec<String>> {
    read_journal(game).ok()?.supported_settings
}
pub fn managed(game: &Path) -> bool {
    peligames_mod_config::ownership::installed(game,"optiscaler")
}
pub(super) fn managed_proxy(game: &Path) -> Option<String> {
    let text = super::read_file(&game.join(JOURNAL)).ok()?;
    let journal: Journal = serde_json::from_str(&text).ok()?;
    super::supported_proxy(&journal.proxy).then_some(journal.proxy)
}
pub(super) fn hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
fn relative(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
pub(super) fn safe_target(game: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    if !relative(name) {
        return Err("Invalid OptiScaler manifest path".into());
    }
    let mut path = game.to_path_buf();
    for c in Path::new(name).components() {
        path.push(c);
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("Redirected component paths are not supported".into());
        }
    }
    Ok(path)
}
/// Read only the PE headers; game executables can be several gigabytes.
pub(super) fn machine(path: &Path) -> Option<u16> {
    let mut file = fs::File::open(path).ok()?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header).ok()?;
    if &header[..2] != b"MZ" {
        return None;
    }
    let offset = u32::from_le_bytes(header[0x3c..0x40].try_into().ok()?) as u64;
    file.seek(SeekFrom::Start(offset)).ok()?;
    let mut pe = [0; 6];
    file.read_exact(&mut pe).ok()?;
    if &pe[..4] != b"PE\0\0" {
        return None;
    }
    Some(u16::from_le_bytes([pe[4], pe[5]]))
}

pub fn install(
    component: &Path,
    game: &Path,
    proxy: &str,
    choices: &settings::Settings,
) -> Result<String, String> {
    let _lock = WRITE.lock().map_err(|e| e.to_string())?;
    let game = game_directory(game)?;
    let _directory = peligames_mod_config::DirectoryLock::acquire(&game)?;
    install_locked(component, &game, proxy, choices)
}

pub(super) fn install_locked(
    component: &Path,
    game: &Path,
    proxy: &str,
    choices: &settings::Settings,
) -> Result<String, String> {
    let game = game_directory(game)?;
    if !super::supported_proxy(proxy) {
        return Err("Invalid OptiScaler proxy DLL".into());
    }
    if super::nr_route(&game)?.is_some() {
        return Err(
            "DLSSNR files were detected. Manage or remove that installation through DLSSNR before installing standalone OptiScaler"
                .into(),
        );
    }
    if managed(&game) || config_path(&game)?.exists() {
        return Err("OptiScaler is already present; configure it instead of overwriting it".into());
    }
    let dll = component.join("OptiScaler.dll");
    if machine(&dll) != Some(0x8664) {
        return Err(
            "OptiScaler requires a 64-bit Windows DLL; prepare the component in Add-ons".into(),
        );
    }
    for entry in fs::read_dir(&game).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("exe"))
        {
            if machine(&path) == Some(0x014c) {
                return Err("OptiScaler requires a 64-bit game".into());
            }
        }
    }
    let ini = fs::read_to_string(component.join("OptiScaler.ini"))
        .map_err(|_| "OptiScaler component settings are missing")?;
    let separate_latency = component.join("fakenvapi.ini").is_file();
    let ini_choices = choices
        .iter()
        .filter(|(k, _)| !separate_latency || !k.starts_with("fakenvapi."))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let ini = settings::apply(&ini, &ini_choices)?;
    let latency_ini = if separate_latency {
        let mut text = super::read_file(&component.join("fakenvapi.ini"))?;
        for (key, value) in choices.iter().filter(|(k, _)| k.starts_with("fakenvapi.")) {
            text = super::latency::patch(&text, key, value)?;
        }
        Some(text)
    } else {
        None
    };
    let mut files = vec![
        (dll, proxy.to_string()),
        (component.join("OptiScaler.ini"), "OptiScaler.ini".into()),
    ];
    let license = component.join("OptiScaler_LICENSE.txt");
    if !license.is_file() {
        return Err("OptiScaler license is missing from the component".into());
    }
    files.push((license, "OptiScaler/Licenses/OptiScaler_LICENSE.txt".into()));
    for &name in ROOT_FILES {
        let source = component.join(name);
        if source.is_file() {
            files.push((source, name.into()));
        }
    }
    for dir in ["OptiScaler", "Licenses", "D3D12_Optiscaler"] {
        let source = component.join(dir);
        if source.is_dir() {
            for entry in walkdir::WalkDir::new(&source).follow_links(false) {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_type().is_symlink() {
                    return Err("Redirected package files are not supported".into());
                }
                if entry.file_type().is_file() {
                    let suffix = entry
                        .path()
                        .strip_prefix(&source)
                        .map_err(|e| e.to_string())?;
                    let target = if dir == "Licenses" {
                        Path::new("OptiScaler/Licenses").join(suffix)
                    } else {
                        Path::new(dir).join(suffix)
                    };
                    files.push((entry.path().to_path_buf(), target.to_string_lossy().into()));
                }
            }
        }
    }
    files.sort_by(|a, b| a.1.cmp(&b.1));
    files.dedup_by(|a, b| a.1 == b.1);
    // Validate every destination before writing any game file. Original DLLs are never replaced.
    for (_, name) in &files {
        if safe_target(&game, name)?.exists() {
            return Err(format!(
                "File already exists; it will not be replaced: {name}"
            ));
        }
    }
    let mut written = Vec::new();
    let result = (|| {
        let mut journal = Journal {
            schema: 1,
            proxy: proxy.into(),
            files: Vec::new(),
            supported_settings: Some(settings::read(&ini).into_keys().collect()),
            package_version: super::read_file(&component.join("version.txt"))
                .ok()
                .map(|version| version.trim().to_string())
                .filter(|version| !version.is_empty() && version.len() <= 160),
        };
        for (source, name) in files {
            let target = safe_target(&game, &name)?;
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            let mut output = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&target)
                .map_err(|e| e.to_string())?;
            written.push(target.clone());
            use std::io::Write;
            if name == "OptiScaler.ini" {
                output
                    .write_all(ini.as_bytes())
                    .map_err(|e| e.to_string())?;
            } else if name == "fakenvapi.ini" && latency_ini.is_some() {
                output
                    .write_all(latency_ini.as_ref().unwrap().as_bytes())
                    .map_err(|e| e.to_string())?;
            } else {
                let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
                std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
            }
            output.sync_all().map_err(|e| e.to_string())?;
            journal.files.push(Entry {
                path: name,
                sha256: hash(&target)?,
            });
        }
        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(game.join(JOURNAL))
            .map_err(|e| e.to_string())?;
        written.push(game.join(JOURNAL));
        use std::io::Write;
        f.write_all(&serde_json::to_vec_pretty(&journal).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        for path in written {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    Ok(format!(
        "WINEDLLOVERRIDES=\"{}=n,b\" %command%",
        proxy.trim_end_matches(".dll")
    ))
}
pub fn remove(game: &Path) -> Result<(), String> {
    let _lock = WRITE.lock().map_err(|e| e.to_string())?;
    let game = game_directory(game)?;
    let _directory = peligames_mod_config::DirectoryLock::acquire(&game)?;
    if super::nr_route(&game)?.is_some() {
        return Err(
            "DLSSNR files were detected; manage that installation through its own menu".into(),
        );
    }
    let journal = read_journal(&game)?;
    let paths = owned_files(&game, &journal)?;
    let journal_path = safe_target(&game, JOURNAL)?;
    super::patcher::remove_owned(&game)?;
    for path in paths {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    fs::remove_file(journal_path).map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn read_journal(game: &Path) -> Result<Journal, String> {
    let journal_path = safe_target(game, JOURNAL)?;
    let text = super::read_file(&journal_path)?;
    let journal: Journal = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if journal.schema != 1
        || !super::supported_proxy(&journal.proxy)
        || journal.files.len() > 4096
        || journal
            .files
            .iter()
            .map(|entry| &entry.path)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != journal.files.len()
        || journal.files.iter().any(|e| {
            e.path != journal.proxy
                && e.path != "OptiScaler.ini"
                && !e.path.starts_with("OptiScaler/")
                && !e.path.starts_with("D3D12_Optiscaler/")
                && !ROOT_FILES.contains(&e.path.as_str())
        })
        || !journal.files.iter().any(|e| e.path == journal.proxy)
    {
        return Err("Invalid OptiScaler manifest".into());
    }
    Ok(journal)
}

pub(super) fn owned_files(
    game: &Path,
    journal: &Journal,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut paths = Vec::new();
    for entry in &journal.files {
        let path = safe_target(game, &entry.path)?;
        if path.is_file() {
            if !["OptiScaler.ini", "fakenvapi.ini"].contains(&entry.path.as_str())
                && hash(&path)? != entry.sha256
            {
                return Err(format!(
                    "Component changed since installation; operation stopped: {}",
                    entry.path
                ));
            }
            paths.push(path);
        }
    }
    Ok(paths)
}
