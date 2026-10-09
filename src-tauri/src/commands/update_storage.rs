use std::{fs::{self, File, OpenOptions}, io, path::{Path, PathBuf}};

// Keep active downloads outside identifier directories moved by legacy migration.
pub fn cache_root(base: &Path) -> PathBuf { base.join("PeliGames") }

pub fn io_error(operation: &str, path: &Path, error: io::Error) -> String {
    format!("download|{operation}: {}: {error}", path.display())
}

fn directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(io_error("cache", path, io::Error::new(io::ErrorKind::InvalidInput, "Not a regular directory"))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(|error| io_error("create directory", path, error))
        }
        Err(error) => Err(io_error("open directory", path, error)),
    }
}

pub fn prepare(root: &Path) -> Result<PathBuf, String> {
    directory(root)?;
    let updates = root.join("updates");
    directory(&updates)?;
    Ok(updates)
}

pub fn create_partial(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|error| io_error("create file", path, error))
}
