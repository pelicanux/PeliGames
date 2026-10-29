//! Bounded reads of fixed regular files; never follow a log outside its root.
use crate::core::report_privacy::sanitize;
use serde_json::Value;
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
pub(super) const FILE_LIMIT: usize = 64 * 1024;
pub(super) fn tail(text: &str, limit: usize) -> &str {
    let mut start = text.len().saturating_sub(limit);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}
/// Read fixed, regular files only. A mod log symlink cannot disclose another file.
pub(super) fn read_at(root: &Path, relative: &str) -> Option<String> {
    let root = root.canonicalize().ok()?;
    let path = root.join(relative);
    if !path.canonicalize().ok()?.starts_with(&root) {
        return None;
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() {
        return None;
    }
    file.seek(SeekFrom::Start(
        meta.len().saturating_sub(FILE_LIMIT as u64),
    ))
    .ok()?;
    let mut bytes = Vec::new();
    file.take(FILE_LIMIT as u64).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}
pub(super) fn section(out: &mut String, title: &str, text: &str) {
    out.push_str(&format!(
        "\n--- {title} ---\n{}\n",
        sanitize(tail(text, FILE_LIMIT))
    ));
}
pub(super) fn technical(out: &mut String, title: &str, value: &Value) {
    section(
        out,
        title,
        &serde_json::to_string_pretty(value).unwrap_or_default(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_log_reader_rejects_symlinks_and_special_files_and_bounds_tail() {
        let root =
            std::env::temp_dir().join(format!("peli-report-fixture-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("normal.log"), "x".repeat(FILE_LIMIT + 100)).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", root.join("secret.log")).unwrap();
        let fifo = root.join("pipe.log");
        let name = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(read_at(&root, "pipe.log").is_none());
        assert!(read_at(&root, "secret.log").is_none());
        assert_eq!(read_at(&root, "normal.log").unwrap().len(), FILE_LIMIT);
        assert!(read_at(&root, "../passwd").is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
