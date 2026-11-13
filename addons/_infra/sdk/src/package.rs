//! Immutable version directories. Installation never touches games or their INIs.
use crate::{
    catalog::Entry,
    manifest::{relative_file, Manifest},
    Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

pub const MAX_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 2048;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const PACKAGE_MANIFEST: &str = "addon.json";
const RECEIPT: &str = "receipt.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contents {
    pub manifest: Manifest,
    pub files: BTreeMap<String, File>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    #[serde(default)]
    pub local: bool,
    pub archive_sha256: String,
    pub contents: Contents,
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Read a regular managed file without allowing its contents to exceed a limit.
pub fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    regular_file(path)?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Add-on file exceeds size limit".into());
    }
    Ok(bytes)
}
fn regular_file(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err("Expected a regular add-on file".into());
    }
    Ok(())
}
/// Check every ancestor, including the profile directory. A redirected managed
/// directory must never allow removal/installation outside the add-on root.
pub(crate) use crate::files::safe_directory;
fn validate_contents(contents: &Contents) -> Result<()> {
    contents.manifest.validate()?;
    if contents.files.is_empty()
        || contents.files.len() > MAX_FILES
        || !contents.files.contains_key("LICENSE")
    {
        return Err("Add-on package needs payload files and a LICENSE".into());
    }
    let mut names = BTreeSet::new();
    let mut size = 0_u64;
    for (path, file) in &contents.files {
        relative_file(path)?;
        if [PACKAGE_MANIFEST, RECEIPT].contains(&path.to_ascii_lowercase().as_str())
            || !names.insert(path.to_ascii_lowercase())
        {
            return Err("Reserved or duplicate add-on file".into());
        }
        size = size
            .checked_add(file.bytes)
            .ok_or("Package size overflow")?;
        if size > MAX_EXPANDED_BYTES
            || file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("Invalid add-on file size or checksum".into());
        }
    }
    for entry in [
        &contents.manifest.entrypoints.ui,
        &contents.manifest.entrypoints.service,
    ]
    .into_iter()
    .flatten()
    {
        if !contents.files.contains_key(entry) {
            return Err(format!("Missing entrypoint: {entry}"));
        }
    }
    Ok(())
}

struct Stage(PathBuf);
impl Stage {
    fn new(root: &Path, id: &str) -> Result<Self> {
        safe_directory(root, true)?;
        // Temporary collisions get readable numeric suffixes, never persistent UUIDs.
        for suffix in 1..=1000 {
            let path = root.join(format!("install-{id}-{}-{suffix}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Too many unfinished add-on installs".into())
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Prepare a verified version without selecting or starting it. The trusted
/// entry must come from the host's catalog, never from inside the ZIP itself.
pub fn prepare(root: &Path, entry: &Entry, platform: &str, bytes: &[u8]) -> Result<Receipt> {
    entry.manifest.validate()?;
    let artifact = entry
        .artifacts
        .get(platform)
        .ok_or("No artifact for this platform")?;
    artifact.validate()?;
    if bytes.len() as u64 != artifact.bytes || digest(bytes) != artifact.sha256 {
        return Err("Add-on archive size or checksum mismatch".into());
    }
    prepare_contents(root, &entry.manifest, &artifact.sha256, false, bytes)
}

pub fn inspect_local(bytes: &[u8]) -> Result<Contents> {
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err("Add-on archive exceeds size limit".into());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if zip.len() > MAX_FILES + 1 {
        return Err("Too many files in add-on archive".into());
    }
    let contents: Contents = {
        let mut file = zip.by_name(PACKAGE_MANIFEST).map_err(|e| e.to_string())?;
        if file.size() > MAX_METADATA_BYTES {
            return Err("Add-on manifest is too large".into());
        }
        let mut data = Vec::new();
        file.by_ref()
            .take(MAX_METADATA_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&data).map_err(|e| e.to_string())?
    };
    validate_contents(&contents)?;
    if zip.len() != contents.files.len() + 1 {
        return Err("Add-on archive inventory does not match its manifest".into());
    }
    Ok(contents)
}

/// Explicit local import uses the same extraction and integrity checks as the catalog.
pub fn prepare_local(
    root: &Path,
    bytes: &[u8],
    expected_sha256: &str,
    host: &str,
    platform: &str,
) -> Result<Receipt> {
    if digest(bytes) != expected_sha256 {
        return Err("The local package changed; inspect it again before importing".into());
    }
    let contents = inspect_local(bytes)?;
    contents.manifest.compatible(host, platform)?;
    prepare_contents(root, &contents.manifest, expected_sha256, true, bytes)
}
fn prepare_contents(
    root: &Path,
    manifest: &Manifest,
    archive_sha256: &str,
    local: bool,
    bytes: &[u8],
) -> Result<Receipt> {
    let contents = inspect_local(bytes)?;
    if contents.manifest != *manifest {
        return Err("Add-on archive does not match its selected catalog or local manifest".into());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let stage = Stage::new(&root.join("staging"), &manifest.id)?;
    let mut seen = BTreeSet::new();
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = file.name().to_owned();
        relative_file(&name)?;
        if !seen.insert(name.to_ascii_lowercase())
            || file.is_dir()
            || file
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 != 0 && mode & 0o170000 != 0o100000)
        {
            return Err("Duplicate, linked or non-regular add-on archive entry".into());
        }
        if name == PACKAGE_MANIFEST {
            continue;
        }
        let expected = contents
            .files
            .get(&name)
            .ok_or("Undeclared add-on archive file")?;
        if file.size() != expected.bytes {
            return Err("Incorrect add-on file size".into());
        }
        let target = stage.0.join(&name);
        fs::create_dir_all(target.parent().ok_or("Invalid payload path")?)
            .map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut read = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            read += n as u64;
            if read > expected.bytes {
                return Err("Expanded add-on file exceeds declared size".into());
            }
            hash.update(&buffer[..n]);
            output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        }
        if read != expected.bytes || format!("{:x}", hash.finalize()) != expected.sha256 {
            return Err("Add-on file checksum mismatch".into());
        }
        output.sync_all().map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let executable =
                contents.manifest.entrypoints.service.as_deref() == Some(name.as_str());
            fs::set_permissions(
                &target,
                fs::Permissions::from_mode(if executable { 0o700 } else { 0o600 }),
            )
            .map_err(|e| e.to_string())?;
        }
    }
    let receipt = Receipt {
        local,
        archive_sha256: archive_sha256.into(),
        contents,
    };
    let receipt_bytes = serde_json::to_vec_pretty(&receipt).map_err(|e| e.to_string())?;
    fs::write(stage.0.join(RECEIPT), receipt_bytes).map_err(|e| e.to_string())?;
    fs::File::open(stage.0.join(RECEIPT))
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    let parent = root.join("installed").join(&manifest.id);
    safe_directory(&parent, true)?;
    let destination = parent.join(&manifest.version);
    if fs::symlink_metadata(&destination).is_ok() {
        let previous = verify(root, &manifest.id, &manifest.version)?;
        if previous.archive_sha256 != receipt.archive_sha256 {
            return Err("An add-on version cannot be replaced by different bytes".into());
        }
        return Ok(previous);
    } else {
        fs::rename(&stage.0, &destination).map_err(|e| e.to_string())?;
    }
    Ok(receipt)
}

/// Reuse a previously verified download, including a prepared package left by
/// an interrupted multi-add-on selection. A corrupt cache is never used silently.
pub fn cached(root: &Path, entry: &Entry, platform: &str) -> Result<Option<Receipt>> {
    entry.manifest.validate()?;
    let artifact = entry
        .artifacts
        .get(platform)
        .ok_or("No artifact for this platform")?;
    artifact.validate()?;
    let directory = version_directory(root, &entry.manifest.id, &entry.manifest.version)?;
    match fs::symlink_metadata(&directory) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
        Ok(_) => (),
    }
    let receipt = verify(root, &entry.manifest.id, &entry.manifest.version)?;
    if receipt.archive_sha256 != artifact.sha256 || receipt.contents.manifest != entry.manifest {
        return Err("Cached add-on does not match the reviewed catalog".into());
    }
    Ok(Some(receipt))
}

pub(crate) fn version_directory(root: &Path, id: &str, version: &str) -> Result<PathBuf> {
    crate::manifest::identifier(id)?;
    let parsed = semver::Version::parse(version).map_err(|e| e.to_string())?;
    if parsed.to_string() != version || !parsed.build.is_empty() {
        return Err("Invalid add-on version".into());
    }
    Ok(root.join("installed").join(id).join(version))
}
pub fn receipt(root: &Path, id: &str, version: &str) -> Result<Receipt> {
    let dir = version_directory(root, id, version)?;
    safe_directory(&dir, false)?;
    let receipt: Receipt =
        serde_json::from_slice(&bounded_read(&dir.join(RECEIPT), MAX_METADATA_BYTES)?)
            .map_err(|e| e.to_string())?;
    validate_contents(&receipt.contents)?;
    if receipt.contents.manifest.id != id || receipt.contents.manifest.version != version {
        return Err("Installed add-on identity does not match its directory".into());
    }
    Ok(receipt)
}
/// Run immediately before loading a UI or launching a helper, not only at download.
pub fn verify(root: &Path, id: &str, version: &str) -> Result<Receipt> {
    let receipt = receipt(root, id, version)?;
    let dir = version_directory(root, id, version)?;
    for (name, expected) in &receipt.contents.files {
        let path = dir.join(name);
        safe_directory(path.parent().ok_or("Invalid add-on file")?, false)?;
        let bytes = bounded_read(&path, expected.bytes)?;
        if bytes.len() as u64 != expected.bytes || digest(&bytes) != expected.sha256 {
            return Err(format!("Installed add-on file changed: {name}"));
        }
    }
    Ok(receipt)
}

/// Package an already-built payload. This function does not compile anything.
pub fn pack(manifest: Manifest, payload: &Path) -> Result<Vec<u8>> {
    manifest.validate()?;
    safe_directory(payload, false)?;
    let mut contents = Contents {
        manifest,
        files: BTreeMap::new(),
    };
    fn collect(root: &Path, dir: &Path, files: &mut BTreeMap<String, File>) -> Result<()> {
        for item in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = item.map_err(|e| e.to_string())?.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if meta.is_dir() {
                collect(root, &path, files)?;
            } else {
                let name = path
                    .strip_prefix(root)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .ok_or("Non-UTF8 add-on file")?
                    .replace('\\', "/");
                relative_file(&name)?;
                let bytes = bounded_read(&path, MAX_EXPANDED_BYTES)?;
                files.insert(
                    name,
                    File {
                        bytes: bytes.len() as u64,
                        sha256: digest(&bytes),
                    },
                );
                if files.len() > MAX_FILES {
                    return Err("Too many payload files".into());
                }
            }
        }
        Ok(())
    }
    collect(payload, payload, &mut contents.files)?;
    validate_contents(&contents)?;
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    archive
        .start_file(PACKAGE_MANIFEST, options)
        .map_err(|e| e.to_string())?;
    archive
        .write_all(&serde_json::to_vec_pretty(&contents).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    for (name, expected) in &contents.files {
        let bytes = bounded_read(&payload.join(name), expected.bytes)?;
        if digest(&bytes) != expected.sha256 {
            return Err("Payload changed while packaging".into());
        }
        archive
            .start_file(name, options)
            .map_err(|e| e.to_string())?;
        archive.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    let bytes = archive.finish().map_err(|e| e.to_string())?.into_inner();
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err("Add-on archive is too large".into());
    }
    Ok(bytes)
}
