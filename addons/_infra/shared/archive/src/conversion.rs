//! Bounded archive conversion shared by integrations and the host.
use std::{
    ffi::{c_char, c_int, c_void, CStr, CString},
    fs,
    io::Write,
    os::unix::ffi::OsStrExt,
    path::Path,
};
pub const FILE_LIMIT: u64 = 512 * 1024 * 1024;
pub const TOTAL_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
pub fn safe_name(name: &str) -> Result<String, String> {
    let name = name.replace('\\', "/");
    let path = name.trim_end_matches('/');
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == ".." || part.contains(':'))
    {
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
pub fn normalize(source: &Path, destination: &Path) -> Result<(), String> {
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
