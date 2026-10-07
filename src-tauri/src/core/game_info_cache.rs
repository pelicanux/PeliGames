use std::sync::{Mutex, MutexGuard};

// Serializes cache writes/deletion and invalidates queries started before a clear.
static GENERATION: Mutex<u64> = Mutex::new(0);

pub fn lock_generation() -> MutexGuard<'static, u64> {
    GENERATION.lock().unwrap_or_else(|error| error.into_inner())
}

pub fn clear_files(cache_dir: &std::path::Path) -> Result<(), String> {
    let mut generation = lock_generation();
    *generation = generation.wrapping_add(1);
    for name in ["analyzer_cache.json", "release_date_cache.json"] {
        match std::fs::remove_file(cache_dir.join(name)) {
            Ok(()) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(format!("{}: {}", name, error)),
        }
    }
    Ok(())
}
