//! Bounded archive and standalone mod reading into an owned cache, never into game paths.
use super::{
    nexus_local::{LocalMod, NexusGame},
    nexus_profile::directory,
};
use sha2::{Digest, Sha256};
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs,
    io::{Read, Write},
    os::unix::ffi::OsStrExt,
    path::Path,
};
pub(super) const FILE_LIMIT: u64 = 512 * 1024 * 1024;
pub(super) const TOTAL_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
pub(super) const FORMATS: &[&str] = &[
    "zip",
    "7z",
    "rar",
    "fbmod",
    "fbpack",
    "daimod",
    "package",
    "ts4script",
    "dll",
    "pak",
    "mem",
    "dl_bin",
    "stream",
    "gpu_resources",
];
pub(super) fn supported_filename(name: &str) -> bool {
    !name.contains(['/', '\\'])
        && (FORMATS.contains(
            &Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
        ) || super::nexus_patches::parts(name).is_some())
}
fn single(source: &Path, destination: &Path, name: &str) -> Result<(), String> {
    let name = safe_name(name)?;
    if name.contains('/') || name.len() > 240 {
        return Err("Nome de arquivo de mod inválido.".into());
    }
    let file = fs::File::open(source).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > FILE_LIMIT {
        return Err("Arquivo de mod excede 512 MiB.".into());
    }
    let mut zip = zip::ZipWriter::new(fs::File::create(destination).map_err(|e| e.to_string())?);
    zip.start_file(name, zip::write::FileOptions::default())
        .map_err(|e| e.to_string())?;
    let copied =
        std::io::copy(&mut file.take(metadata.len() + 1), &mut zip).map_err(|e| e.to_string())?;
    if copied != metadata.len() {
        return Err("Arquivo de mod foi alterado durante a leitura.".into());
    }
    zip.finish()
        .map_err(|e| e.to_string())?
        .sync_all()
        .map_err(|e| e.to_string())
}
pub(super) fn safe_name(name: &str) -> Result<String, String> {
    let name = name.replace('\\', "/");
    if !super::nexus_modules::relative(name.trim_end_matches('/')) {
        return Err(format!("Caminho inseguro no pacote: {name}"));
    }
    Ok(name)
}
// Only opaque handles cross the ABI; each entry stays owned by its reader until the next header.
#[link(name = "archive")]
extern "C" {
    fn archive_read_new() -> *mut c_void;
    fn archive_read_support_format_7zip(reader: *mut c_void) -> c_int;
    fn archive_read_support_format_rar(reader: *mut c_void) -> c_int;
    fn archive_read_support_format_rar5(reader: *mut c_void) -> c_int;
    fn archive_read_open_filename(
        reader: *mut c_void,
        path: *const c_char,
        block_size: usize,
    ) -> c_int;
    fn archive_read_next_header(reader: *mut c_void, entry: *mut *mut c_void) -> c_int;
    fn archive_read_data(reader: *mut c_void, buffer: *mut c_void, size: usize) -> isize;
    fn archive_read_free(reader: *mut c_void) -> c_int;
    fn archive_error_string(reader: *mut c_void) -> *const c_char;
    fn archive_entry_pathname_utf8(entry: *mut c_void) -> *const c_char;
    fn archive_entry_pathname(entry: *mut c_void) -> *const c_char;
    fn archive_entry_size(entry: *mut c_void) -> i64;
    fn archive_entry_size_is_set(entry: *mut c_void) -> c_int;
    fn archive_entry_filetype(entry: *mut c_void) -> u32;
    fn archive_entry_symlink(entry: *mut c_void) -> *const c_char;
    fn archive_entry_hardlink(entry: *mut c_void) -> *const c_char;
    fn archive_entry_is_encrypted(entry: *mut c_void) -> c_int;
}
struct Reader(*mut c_void);
impl Drop for Reader {
    fn drop(&mut self) {
        unsafe {
            archive_read_free(self.0);
        }
    }
}
impl Reader {
    fn error(&self) -> String {
        let error = unsafe { archive_error_string(self.0) };
        let message = if error.is_null() {
            "arquivo danificado ou formato incompatível".into()
        } else {
            unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .chars()
                .take(512)
                .collect::<String>()
        };
        format!("Não foi possível ler o pacote: {message}")
    }
}
fn normalize(source: &Path, destination: &Path) -> Result<(), String> {
    let path = CString::new(source.as_os_str().as_bytes()).map_err(|_| "Caminho inválido.")?;
    let handle = unsafe { archive_read_new() };
    if handle.is_null() {
        return Err("Não foi possível iniciar o leitor de arquivos.".into());
    }
    let reader = Reader(handle);
    unsafe {
        if archive_read_support_format_7zip(handle) != 0
            || archive_read_support_format_rar(handle) != 0
            || archive_read_support_format_rar5(handle) != 0
            || archive_read_open_filename(handle, path.as_ptr(), 65536) != 0
        {
            return Err(reader.error());
        }
    }
    let mut zip = zip::ZipWriter::new(fs::File::create(destination).map_err(|e| e.to_string())?);
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0u64;
    let mut count = 0usize;
    let mut buffer = [0u8; 65536];
    loop {
        let mut entry = std::ptr::null_mut();
        let status = unsafe { archive_read_next_header(handle, &mut entry) };
        if status == 1 {
            break;
        }
        if status != 0 || entry.is_null() {
            return Err(reader.error());
        }
        count += 1;
        if count > 10000 {
            return Err("O pacote contém mais de 10000 entradas.".into());
        }
        let mut pathname = unsafe { archive_entry_pathname_utf8(entry) };
        if pathname.is_null() {
            pathname = unsafe { archive_entry_pathname(entry) };
        }
        if pathname.is_null() {
            return Err("Nome ausente no pacote.".into());
        }
        let name = safe_name(
            unsafe { CStr::from_ptr(pathname) }
                .to_str()
                .map_err(|_| "Nome de arquivo inválido (UTF-8).")?,
        )?;
        let kind = unsafe { archive_entry_filetype(entry) };
        if !unsafe { archive_entry_symlink(entry) }.is_null()
            || !unsafe { archive_entry_hardlink(entry) }.is_null()
            || unsafe { archive_entry_is_encrypted(entry) } != 0
        {
            return Err("Links e arquivos criptografados não são aceitos.".into());
        }
        if kind == 0o040000 {
            continue;
        }
        if kind != 0o100000 {
            return Err("Somente arquivos regulares são aceitos no pacote.".into());
        }
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(format!("Arquivo duplicado no pacote: {name}"));
        }
        let size = unsafe { archive_entry_size(entry) };
        if unsafe { archive_entry_size_is_set(entry) } == 0 || size < 0 {
            return Err("Tamanho de arquivo inválido.".into());
        }
        let size = size as u64;
        total = total.checked_add(size).ok_or("Pacote grande demais.")?;
        if size > FILE_LIMIT || total > TOTAL_LIMIT {
            return Err(
                "O pacote excede os limites de extração (512 MB por arquivo, 2 GB no total)."
                    .into(),
            );
        }
        zip.start_file(
            &name,
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .map_err(|e| e.to_string())?;
        let mut copied = 0u64;
        loop {
            let read =
                unsafe { archive_read_data(handle, buffer.as_mut_ptr().cast(), buffer.len()) };
            if read < 0 {
                return Err(reader.error());
            }
            if read == 0 {
                break;
            }
            copied = copied
                .checked_add(read as u64)
                .ok_or("Arquivo grande demais.")?;
            if copied > size {
                return Err(format!("Tamanho inválido na extração: {name}"));
            }
            zip.write_all(&buffer[..read as usize])
                .map_err(|e| e.to_string())?;
        }
        if copied != size {
            return Err(format!("Tamanho inválido na extração: {name}"));
        }
    }
    if seen.is_empty() {
        return Err("O pacote está vazio.".into());
    }
    zip.finish()
        .map_err(|e| e.to_string())?
        .sync_all()
        .map_err(|e| e.to_string())
}
pub(super) fn open(game: &NexusGame, item: &LocalMod) -> Result<zip::ZipArchive<fs::File>, String> {
    uuid::Uuid::parse_str(&item.id).map_err(|_| "Mod inválido.")?;
    let extension = Path::new(&item.archive)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let original = if item
        .name
        .to_ascii_lowercase()
        .ends_with(&format!(".{extension}"))
    {
        item.name.clone()
    } else {
        format!("{}.{extension}", item.name)
    };
    let archive = ["zip", "7z", "rar"].contains(&extension.as_str());
    if !archive && !supported_filename(&original) {
        return Err("Formato de arquivo não suportado.".into());
    }
    let source = directory(game)?
        .join("archives")
        .join(format!("{}.{}", item.id, extension));
    let path = if extension == "zip" {
        source
    } else {
        let cache = directory(game)?.join("archive-cache");
        fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        if !archive {
            hash.update(original.as_bytes());
        }
        let mut file = fs::File::open(&source).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 65536];
        loop {
            let size = file.read(&mut buf).map_err(|e| e.to_string())?;
            if size == 0 {
                break;
            }
            hash.update(&buf[..size]);
        }
        let target = cache.join(format!("{}-{:x}.zip", item.id, hash.finalize()));
        if !target.is_file() {
            let temporary = cache.join(format!("{}.tmp", uuid::Uuid::new_v4()));
            let result = if ["7z", "rar"].contains(&extension.as_str()) {
                normalize(&source, &temporary)
            } else {
                single(&source, &temporary, &original)
            };
            let result =
                result.and_then(|_| fs::rename(&temporary, &target).map_err(|e| e.to_string()));
            if let Err(error) = result {
                let _ = fs::remove_file(&temporary);
                return Err(error);
            }
        }
        target
    };
    let mut zip = zip::ZipArchive::new(fs::File::open(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if zip.len() > 10000 {
        return Err("Entradas demais no pacote.".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut total = 0u64;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = safe_name(entry.name())?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Links não são aceitos no pacote.".into());
        }
        if entry.is_dir() {
            continue;
        }
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(format!("Arquivo duplicado no pacote: {name}"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Pacote grande demais.")?;
        if entry.size() > FILE_LIMIT || total > TOTAL_LIMIT {
            return Err("O pacote excede o limite de extração de 2 GB.".into());
        }
    }
    super::nexus_smapi::prepare(game, item, zip)
}
