//! Free accounts: the site's "Slow download" saves through the browser with no
//! notification. While a queued file waits for authorization, watch the user's
//! Downloads folder for that exact file, confirm it by MD5 and import a copy.
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

static RUNNING: AtomicBool = AtomicBool::new(false);
const POLL: Duration = Duration::from_secs(2);
/// Browser in-progress suffixes (Firefox, Chromium, Safari/WebKit).
const PARTIAL: [&str; 3] = [".part", ".crdownload", ".download"];

/// Folders watched for browser downloads.
pub(super) fn download_dirs() -> Vec<PathBuf> {
    dirs::download_dir().into_iter().filter(|dir| dir.is_dir()).collect()
}

/// The completed file named exactly `file_name` (case-insensitive) with the
/// expected size, while no in-progress copy of it exists. Symlinks are ignored.
pub(super) fn find_candidate(dirs: &[PathBuf], file_name: &str, size: Option<u64>) -> Option<PathBuf> {
    for dir in dirs {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        let names: Vec<(String, PathBuf)> = entries.flatten().take(5000)
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path())).collect();
        let partial = names.iter().any(|(name, _)| PARTIAL.iter().any(|suffix| name.eq_ignore_ascii_case(&format!("{file_name}{suffix}"))));
        if partial { continue; }
        for (name, path) in &names {
            if !name.eq_ignore_ascii_case(file_name) { continue; }
            let Ok(meta) = fs::symlink_metadata(path) else { continue };
            if !meta.file_type().is_file() || meta.len() == 0 { continue; }
            if size.is_some_and(|expected| expected != meta.len()) { continue; }
            return Some(path.clone());
        }
    }
    None
}

#[derive(Clone)]
struct Expected { file_name: String, size: Option<u64> }

/// Starts the watcher if it is not running; it stops by itself when no file
/// waits for authorization.
pub(super) fn ensure(app: &tauri::AppHandle) {
    if RUNNING.swap(true, Ordering::AcqRel) { return; }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut expected: HashMap<String, Expected> = HashMap::new();
        // A file must keep the same size across two polls before it is used.
        let mut seen: HashMap<String, (PathBuf, u64)> = HashMap::new();
        let started = Instant::now();
        loop {
            let waiting = super::nexus_downloads::authorizing_jobs(&app);
            if waiting.is_empty() || started.elapsed() > Duration::from_secs(6 * 60 * 60) { break; }
            let dirs = super::nexus_collections::blocking(|| Ok(download_dirs())).await.unwrap_or_default();
            for job in waiting {
                if !expected.contains_key(&job.id) {
                    let path = format!("games/{}/mods/{}/files/{}.json", job.domain, job.mod_id, job.file_id);
                    let Ok(file) = super::nexus_account::metadata(&path).await else { continue };
                    let Some(file_name) = file["file_name"].as_str().filter(|n| super::nexus_archive::supported_filename(n) && !n.contains(['/', '\\'])) else { continue };
                    expected.insert(job.id.clone(), Expected { file_name: file_name.to_string(), size: file["size_in_bytes"].as_u64().filter(|s| *s > 0) });
                }
                let Some(want) = expected.get(&job.id).cloned() else { continue };
                // Directory listing runs on the blocking pool, not on the async threads.
                let found = { let dirs = dirs.clone(); let want = want.clone();
                    super::nexus_collections::blocking(move || Ok(find_candidate(&dirs, &want.file_name, want.size).map(|path| { let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0); (path, size) }))).await.ok().flatten() };
                let Some((path, size)) = found else { seen.remove(&job.id); continue };
                match seen.get(&job.id) {
                    Some((previous, previous_size)) if *previous == path && *previous_size == size => {}
                    _ => { seen.insert(job.id.clone(), (path, size)); continue; }
                }
                seen.remove(&job.id);
                if let Err(error) = super::nexus_downloads::import_browser_download(&app, &job.id, &path).await {
                    super::nexus_downloads::fail(&app, &job.id, error);
                }
            }
            tokio::time::sleep(POLL).await;
        }
        RUNNING.store(false, Ordering::Release);
        // A job queued while the loop was exiting restarts the watcher.
        if !super::nexus_downloads::authorizing_jobs(&app).is_empty() { ensure(&app); }
    });
}

/// Confirms by content that `path` is the Nexus file `(mod_id, file_id)`.
pub(super) async fn confirm_md5(path: &Path, domain: &str, mod_id: u64, file_id: u64) -> Result<(), String> {
    let owned = path.to_path_buf();
    let hash = tauri::async_runtime::spawn_blocking(move || super::nexus_local::archive_md5(&owned)).await.map_err(|e| e.to_string())??;
    let result = super::nexus_account::metadata(&format!("games/{domain}/mods/md5_search/{hash}.json")).await
        .map_err(|_| "Não foi possível confirmar o arquivo baixado no Nexus.".to_string())?;
    if super::nexus_local::md5_lists_file(&result, domain, mod_id, file_id) { Ok(()) }
    else { Err("O arquivo da pasta Downloads tem o nome esperado, mas o conteúdo não corresponde ao arquivo do Nexus. Nada foi importado.".into()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_needs_exact_name_size_and_no_partial_copy() {
        let dir = std::env::temp_dir().join(format!("nexus-watch-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let dirs = vec![dir.clone()];
        let name = "Better Menu-577-1-0-1700000000.zip";
        assert_eq!(find_candidate(&dirs, name, Some(4)), None);
        fs::write(dir.join(format!("{name}.part")), b"ab").unwrap();
        fs::write(dir.join(name), b"abcd").unwrap();
        assert_eq!(find_candidate(&dirs, name, Some(4)), None, "download ainda em andamento");
        fs::remove_file(dir.join(format!("{name}.part"))).unwrap();
        assert_eq!(find_candidate(&dirs, name, Some(4)), Some(dir.join(name)));
        assert_eq!(find_candidate(&dirs, &name.to_uppercase(), Some(4)), Some(dir.join(name)));
        assert_eq!(find_candidate(&dirs, name, Some(5)), None, "tamanho diferente");
        assert_eq!(find_candidate(&dirs, "Other-1-1.zip", None), None);
        #[cfg(unix)] {
            let other = std::env::temp_dir().join(format!("nexus-watch-target-{}", uuid::Uuid::new_v4()));
            fs::write(&other, b"abcd").unwrap();
            fs::remove_file(dir.join(name)).unwrap();
            std::os::unix::fs::symlink(&other, dir.join(name)).unwrap();
            assert_eq!(find_candidate(&dirs, name, Some(4)), None, "links simbólicos não são seguidos");
            fs::remove_file(other).unwrap();
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
