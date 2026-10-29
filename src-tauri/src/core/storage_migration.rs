//! One-way, restartable storage organization. No downloads or game files move.
use super::layout;
use std::{
    fs,
    io::Write,
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::Path,
};

fn regular(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}
fn private_directory(path: &Path) -> Result<(), String> {
    for parent in path.ancestors() {
        if fs::symlink_metadata(parent).is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()) {
            return Err(format!("Diretório inválido: {}", parent.display()));
        }
    }
    fs::create_dir_all(path).map_err(|e| e.to_string())
}
fn atomic_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    private_directory(path.parent().ok_or("Arquivo sem pasta")?)?;
    if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink() || !m.is_file()) {
        return Err("Arquivo da migração inválido.".into());
    }
    let temp = path.with_file_name(format!(
        ".{}-{}-{}.tmp",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        f.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temp, path).map_err(|e| e.to_string())?;
        fs::File::open(path.parent().unwrap())
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn completed(root: &Path) -> bool {
    fs::read(root.join("settings/storage-layout-v2.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .is_some_and(|v| v["version"] == 2)
}
fn aliases(root: &Path) -> Result<Vec<(String, String)>, String> {
    match fs::read(root.join("settings/path-aliases.json")) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| format!("Mapa de migração inválido: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}
fn remember_move(
    root: &Path,
    pairs: &mut Vec<(String, String)>,
    from: &Path,
    to: &Path,
) -> Result<(), String> {
    let pair = (
        from.to_string_lossy().into_owned(),
        to.to_string_lossy().into_owned(),
    );
    if !pairs.contains(&pair) {
        pairs.push(pair);
        atomic_json(&root.join("settings/path-aliases.json"), pairs)?;
    }
    Ok(())
}
fn move_missing(source: &Path, target: &Path) -> Result<(), String> {
    let meta = match fs::symlink_metadata(source) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if meta.file_type().is_symlink() {
        return Err(format!("A migração não segue links: {}", source.display()));
    }
    if let Some(parent) = target.parent() {
        private_directory(parent)?;
    }
    match fs::symlink_metadata(target){
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>fs::rename(source,target).map_err(|e|e.to_string()),
        Ok(m) if m.is_dir()&&meta.is_dir()&&!m.file_type().is_symlink()=>{
            for e in fs::read_dir(source).map_err(|e|e.to_string())?{let e=e.map_err(|e|e.to_string())?;move_missing(&e.path(),&target.join(e.file_name()))?;}
            fs::remove_dir(source).map_err(|e|e.to_string())
        },
        // Leftover duplicates (an empty lock recreated by an older launcher) hold no data.
        Ok(m) if m.is_file()&&meta.is_file()&&m.len()==meta.len()&&fs::read(source).ok()==fs::read(target).ok()=>fs::remove_file(source).map_err(|e|e.to_string()),
        Ok(_) =>Err(format!("Existem duas cópias de {}. A migração preservou ambas; resolva o conflito antes de continuar.",source.display())),
        Err(e)=>Err(e.to_string()),
    }
}
fn replace_prefix(text: &str, from: &str, to: &str) -> String {
    let mut result = String::new();
    let mut at = 0;
    for (pos, _) in text.match_indices(from) {
        let end = pos + from.len();
        if text[end..].chars().next().is_some_and(|c| {
            !matches!(
                c,
                '/' | '\'' | '"' | '\n' | '\r' | '\t' | ' ' | ';' | ':' | ')' | '\\'
            )
        }) {
            continue;
        }
        result.push_str(&text[at..pos]);
        result.push_str(to);
        at = end;
    }
    result.push_str(&text[at..]);
    result
}
fn rewrite(
    root: &Path,
    folder: &Path,
    pairs: &[(String, String)],
    depth: usize,
) -> Result<(), String> {
    if depth > 12 {
        return Ok(());
    }
    let entries = match fs::read_dir(folder) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            rewrite(root, &path, pairs, depth + 1)?;
            continue;
        }
        if !kind.is_file()
            || !matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("json" | "toml" | "sh" | "conf")
            )
            || entry.metadata().map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024
        {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let updated = pairs
            .iter()
            .fold(text.clone(), |s, (old, new)| replace_prefix(&s, old, new));
        if text == updated {
            continue;
        }
        let backup = root
            .join("migration-backup/layout-v2")
            .join(path.strip_prefix(root).map_err(|e| e.to_string())?);
        if !backup.exists() {
            private_directory(backup.parent().unwrap())?;
            fs::copy(&path, &backup).map_err(|e| e.to_string())?;
        }
        let temp = path.with_file_name(format!(
            ".{}-migration-{}.tmp",
            path.file_name().unwrap().to_string_lossy(),
            std::process::id()
        ));
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        if let Err(e) = output
            .write_all(updated.as_bytes())
            .and_then(|_| output.sync_all())
        {
            let _ = fs::remove_file(&temp);
            return Err(e.to_string());
        }
        fs::set_permissions(
            &temp,
            entry.metadata().map_err(|e| e.to_string())?.permissions(),
        )
        .map_err(|e| e.to_string())?;
        fs::rename(temp, path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn rename_profiles(root: &Path) -> Result<(), String> {
    for (category, key_field, label_field) in [
        ("CaptureSettings", "game_key", "game_name"),
        ("GameModules", "", "root"),
    ] {
        let folder = layout::join(root, category);
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let old = entry.path();
            let stem = old.file_stem().and_then(|v| v.to_str()).unwrap_or("");
            if !entry.file_type().is_ok_and(|t| t.is_file())
                || old.extension().is_none_or(|e| e != "json")
                || stem.len() != 64
                || !stem.bytes().all(|b| b.is_ascii_hexdigit())
            {
                continue;
            }
            let Ok(value) = fs::read(&old).map_err(|e| e.to_string()).and_then(|v| {
                serde_json::from_slice::<serde_json::Value>(&v).map_err(|e| e.to_string())
            }) else {
                continue;
            };
            let Some(label) = value[label_field].as_str() else {
                continue;
            };
            let label = if category == "GameModules" {
                Path::new(label)
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("game-modules")
            } else {
                label
            };
            let identity = if key_field.is_empty() {
                stem
            } else {
                let Some(key) = value[key_field].as_str() else {
                    continue;
                };
                key
            };
            let target = super::names::allocate(&folder, label, identity)?.with_extension("json");
            move_missing(&old, &target)?;
        }
    }
    Ok(())
}
fn rename_saved_selections(root: &Path) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let mut labels = std::collections::BTreeMap::new();
    if let Ok(entries) = fs::read_dir(layout::join(root, "CaptureSettings")) {
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_file())
                || entry.path().extension().is_none_or(|e| e != "json")
            {
                continue;
            }
            if let Ok(value) = fs::read(entry.path())
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                .ok_or(())
            {
                if let (Some(key), Some(name)) =
                    (value["game_key"].as_str(), value["game_name"].as_str())
                {
                    labels.insert(
                        format!("{:x}", Sha256::digest(key.as_bytes())),
                        name.to_string(),
                    );
                }
            }
        }
    }
    let profiles = layout::join(root, "FrameGeneration/profiles");
    if let Ok(entries) = fs::read_dir(&profiles) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(identity) = name.strip_suffix(".selection.json") else {
                continue;
            };
            if identity.len() != 64
                || !identity.bytes().all(|b| b.is_ascii_hexdigit())
                || !entry.file_type().is_ok_and(|t| t.is_file())
            {
                continue;
            }
            let base = super::names::allocate(
                &profiles.join("selections"),
                labels
                    .get(identity)
                    .map(String::as_str)
                    .unwrap_or("game-settings"),
                identity,
            )?;
            move_missing(&entry.path(), &base.with_extension("json"))?;
        }
    }
    let sessions = layout::join(root, "executions");
    if let Ok(entries) = fs::read_dir(&sessions) {
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.len() != 36
                || path.extension().is_none_or(|e| e != "json")
                || !entry.file_type().is_ok_and(|t| t.is_file())
            {
                continue;
            }
            let Some(value) = fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            else {
                continue;
            };
            if value["status"]["id"] != stem {
                continue;
            }
            let label = Path::new(value["status"]["path"].as_str().unwrap_or("game"))
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("game");
            let base = super::names::allocate(&sessions, &format!("{label}-session"), stem)?;
            move_missing(&path, &base.with_extension("json"))?;
            let cancel = path.with_extension("cancel");
            if regular(&cancel) {
                move_missing(&cancel, &base.with_extension("cancel"))?;
            }
        }
    }
    Ok(())
}
fn readable_assets(root: &Path, pairs: &mut Vec<(String, String)>) -> Result<(), String> {
    for (relative, label, extensions) in [
        (
            "covers",
            "imported-cover",
            &["png", "jpg", "jpeg", "webp"][..],
        ),
        ("icons/executables", "executable-icon", &["png", "svg"][..]),
        ("CaptureLibrary", "saved-clip", &["json"][..]),
    ] {
        let folder = layout::join(root, relative);
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        let files: Vec<_> = entries.flatten().map(|e| e.path()).collect();
        for source in files {
            let Some(extension) = source.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if !extensions.contains(&extension) || !regular(&source) {
                continue;
            }
            let stem = source.file_stem().and_then(|v| v.to_str()).unwrap_or("");
            let opaque = stem.len() == 64 && stem.bytes().all(|b| b.is_ascii_hexdigit());
            if !opaque
                && !stem.starts_with("automatic-")
                && !stem.starts_with("heroic-")
                && !(stem.len() == 36 && stem.chars().filter(|c| *c == '-').count() == 4)
            {
                continue;
            }
            let base = super::names::allocate(&folder, label, stem)?;
            let target = base.with_extension(extension);
            remember_move(root, pairs, &source, &target)?;
            let image = source.with_extension("jpg");
            let target_image = base.with_extension("jpg");
            if relative == "CaptureLibrary" && regular(&image) {
                remember_move(root, pairs, &image, &target_image)?;
            }
            move_missing(&source, &target)?;
            if relative == "CaptureLibrary" && regular(&image) {
                move_missing(&image, &target_image)?;
            }
        }
    }
    for relative in ["Overlay/overlay", "FrameGeneration/overlay"] {
        let folder = layout::join(root, relative);
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let stem = entry.file_name().to_string_lossy().into_owned();
            if stem.len() != 64
                || !stem.bytes().all(|b| b.is_ascii_hexdigit())
                || !entry.file_type().is_ok_and(|t| t.is_dir())
            {
                continue;
            }
            let base = super::names::allocate(&folder, "peligames-previous", &stem)?;
            remember_move(root, pairs, &entry.path(), &base)?;
            move_missing(&entry.path(), &base)?;
        }
    }
    Ok(())
}
fn rewrite_links(
    root: &Path,
    folder: &Path,
    pairs: &[(String, String)],
    depth: usize,
    backups: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if depth > 24 {
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(folder) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_dir() {
            rewrite_links(root, &path, pairs, depth + 1, backups)?;
        } else if kind.is_symlink() {
            let target = fs::read_link(&path).map_err(|e| e.to_string())?;
            if !target.is_absolute() {
                continue;
            }
            let old = target.to_string_lossy();
            let new = pairs.iter().fold(old.to_string(), |s, (from, to)| {
                replace_prefix(&s, from, to)
            });
            if old == new {
                continue;
            }
            backups.push((path.to_string_lossy().into_owned(), old.into_owned()));
            atomic_json(
                &root.join("migration-backup/layout-v2/symlinks.json"),
                backups,
            )?;
            super::replace_symlink(&path, Path::new(&new))?;
        }
    }
    Ok(())
}
fn rewrite_shortcuts(
    root: &Path,
    applications: &Path,
    pairs: &[(String, String)],
) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(applications) else {
        return Ok(());
    };
    for entry in entries.take(16384).flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("peligames-")
            || !name.ends_with(".desktop")
            || !entry.file_type().is_ok_and(|t| t.is_file())
            || entry.metadata().map_or(true, |m| m.len() > 65536)
        {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let updated = pairs
            .iter()
            .fold(text.clone(), |s, (old, new)| replace_prefix(&s, old, new));
        if updated == text {
            continue;
        }
        let backup = root
            .join("migration-backup/layout-v2/desktop-shortcuts")
            .join(&name);
        if !backup.exists() {
            private_directory(backup.parent().unwrap())?;
            fs::copy(&path, backup).map_err(|e| e.to_string())?;
        }
        let temp = applications.join(format!(".{name}-migration-{}.tmp", std::process::id()));
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        out.write_all(updated.as_bytes())
            .and_then(|_| out.sync_all())
            .map_err(|e| e.to_string())?;
        fs::set_permissions(
            &temp,
            entry.metadata().map_err(|e| e.to_string())?.permissions(),
        )
        .map_err(|e| e.to_string())?;
        fs::rename(temp, path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn rewrite_flatpak_grants(
    root: &Path,
    overrides: &Path,
    pairs: &[(String, String)],
) -> Result<(), String> {
    // Repoint only existing per-user Steam grants to the same managed files.
    // No new filesystem grant or other application's override is introduced.
    let path = overrides.join("com.valvesoftware.Steam");
    if !regular(&path) {
        return Ok(());
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut grants = pairs.to_vec();
    for &(old, new) in layout::MOVES {
        grants.push((
            format!("xdg-config/PeliGames/{old}"),
            format!("xdg-config/PeliGames/{new}"),
        ));
    }
    let updated = grants
        .iter()
        .fold(text.clone(), |s, (from, to)| replace_prefix(&s, from, to));
    if updated == text {
        return Ok(());
    }
    let backup = root.join("migration-backup/layout-v2/flatpak/com.valvesoftware.Steam");
    private_directory(backup.parent().unwrap())?;
    if !backup.exists() {
        fs::copy(&path, backup).map_err(|e| e.to_string())?;
    }
    let temporary = overrides.join(format!(".peligames-storage-{}.tmp", std::process::id()));
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    out.write_all(updated.as_bytes())
        .and_then(|_| out.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(temporary, path).map_err(|e| e.to_string())
}
fn running_component(root: &Path) -> bool {
    use std::io::Read;
    for folder in [root.join("executions"), layout::join(root, "executions")] {
        let Ok(entries) = fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|t| t.is_file())
                || entry.path().extension().is_none_or(|e| e != "json")
            {
                continue;
            }
            let Some(record) = fs::read(entry.path())
                .ok()
                .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            else {
                continue;
            };
            let Some(pid) = record["owner"].as_u64() else {
                continue;
            };
            let Some(birth) = record["birth"].as_u64() else {
                continue;
            };
            if let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) {
                if stat
                    .rsplit_once(") ")
                    .and_then(|(_, s)| s.split_whitespace().nth(19))
                    .and_then(|s| s.parse::<u64>().ok())
                    == Some(birth)
                {
                    return true;
                }
            }
        }
    }
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    let prefixes: Vec<_> = layout::MOVES
        .iter()
        .filter(|(_, new)| new.starts_with("components/"))
        .flat_map(|(old, new)| [*old, *new])
        .map(|name| root.join(name).to_string_lossy().into_owned() + "/")
        .collect();
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let mut maps = String::new();
        if fs::File::open(entry.path().join("maps"))
            .and_then(|f| f.take(8 * 1024 * 1024).read_to_string(&mut maps))
            .is_ok()
            && prefixes.iter().any(|path| maps.contains(path))
        {
            return true;
        }
    }
    false
}
pub fn organize(root: &Path, owns_launcher: bool) -> Result<(), String> {
    if completed(root) {
        return Ok(());
    }
    // Old launchers keep using the root lock too. Never move their live settings.
    let launcher = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join("launcher.lock"))
        .map_err(|e| e.to_string())?;
    if !owns_launcher
        && unsafe { libc::flock(launcher.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0
    {
        return Err("Feche o PeliGames antes de reorganizar seus arquivos.".into());
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join(".storage-migration.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    if completed(root) {
        return Ok(());
    }
    if running_component(root) {
        return Err(
            "Feche os jogos que usam os componentes do PeliGames antes de organizar as pastas."
                .into(),
        );
    }
    let applications = dirs::data_local_dir().map(|d| d.join("applications"));
    organize_at(root, applications.as_deref())?;
    Ok(())
}
/// Settings, library and profile data copied before the first move, in their
/// original places below `migration-backup/layout-v2/before`. Components,
/// artwork, logs and downloads only change folders and are left out.
fn snapshot(root: &Path) -> Result<(), String> {
    let target = root.join("migration-backup/layout-v2/before");
    if fs::symlink_metadata(&target).is_ok() {
        return Ok(());
    }
    const SKIPPED: &[&str] = &[
        "runners", "tools", "CaptureLibrary", "covers", "icons", "updates", "Logs", "game-logs",
        "overlay-logs", "launch-reports", "nexus-browser", "CaptureRuntime", "Overlay", "executions",
    ];
    let stage = target.with_file_name(format!(".before-{}.tmp", std::process::id()));
    let _ = fs::remove_dir_all(&stage);
    private_directory(&stage)?;
    let wanted = |path: &Path| {
        matches!(path.extension().and_then(|e| e.to_str()), Some("json" | "toml" | "conf"))
            && fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && m.len() <= 4 * 1024 * 1024)
    };
    let mut sources: Vec<String> = Vec::new();
    for &(old, new) in layout::MOVES {
        if !SKIPPED.contains(&old) {
            sources.push(old.into());
            sources.push(new.into());
        }
    }
    for relative in sources {
        let source = root.join(&relative);
        if wanted(&source) {
            let copy = stage.join(&relative);
            private_directory(copy.parent().unwrap())?;
            fs::copy(&source, copy).map_err(|e| e.to_string())?;
            continue;
        }
        if !fs::symlink_metadata(&source).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        for entry in walkdir::WalkDir::new(&source).follow_links(false).max_depth(6) {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().is_file() || !wanted(entry.path()) {
                continue;
            }
            let copy = stage.join(&relative).join(entry.path().strip_prefix(&source).map_err(|e| e.to_string())?);
            private_directory(copy.parent().unwrap())?;
            fs::copy(entry.path(), copy).map_err(|e| e.to_string())?;
        }
    }
    fs::rename(&stage, &target).map_err(|e| e.to_string())
}
fn organize_at(root: &Path, applications: Option<&Path>) -> Result<(), String> {
    snapshot(root)?;
    let old_aliases = aliases(root)?;
    let mut pairs = Vec::new();
    for &(old, new) in layout::MOVES {
        let source = root.join(old);
        let destination = root.join(new);
        move_missing(&source, &destination)?;
        let pair = (
            source.to_string_lossy().into_owned(),
            destination.to_string_lossy().into_owned(),
        );
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
    for pair in old_aliases {
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
    // A recorded rename can outlive the process that requested it.
    for (from, to) in &pairs {
        let from = Path::new(from);
        let to = Path::new(to);
        if !from.starts_with(root) || !to.starts_with(root) || from == root || to == root {
            return Err("Mapa de migração contém caminhos externos.".into());
        }
        if fs::symlink_metadata(from).is_ok() {
            move_missing(from, to)?;
        }
    }
    // Old capture sockets are session state; keep them out of the profile root.
    for entry in fs::read_dir(root).map_err(|e| e.to_string())?.flatten() {
        if entry.file_name().to_string_lossy().starts_with("cap-")
            && entry.file_type().is_ok_and(|t| t.is_dir())
        {
            move_missing(
                &entry.path(),
                &root
                    .join("runtime/previous-capture-sessions")
                    .join(entry.file_name()),
            )?;
        }
    }
    rename_profiles(root)?;
    rename_saved_selections(root)?;
    readable_assets(root, &mut pairs)?;
    let mut links = Vec::new();
    rewrite_links(root, &root.join("components"), &pairs, 0, &mut links)?;
    atomic_json(&root.join("settings/path-aliases.json"), &pairs)?;
    // Rewrite only managed settings/manifests. Browser databases and downloaded
    // executables are moved intact, never parsed or modified by this migration.
    for name in [
        "settings",
        "profiles",
        "library",
        "components/FrameGeneration/profiles",
        "components/MangoHUD",
        "components/Mods",
        "cache/video-library",
    ] {
        rewrite(root, &root.join(name), &pairs, 0)?;
    }
    private_directory(&root.join("settings"))?;
    atomic_json(&root.join("settings/path-aliases.json"), &pairs)?;
    if let Some(applications) = applications {
        rewrite_shortcuts(root, applications, &pairs)?;
        if let Some(data) = applications.parent() {
            rewrite_flatpak_grants(root, &data.join("flatpak/overrides"), &pairs)?;
        }
    }
    atomic_json(
        &root.join("settings/storage-layout-v2.json"),
        &serde_json::json!({"version":2,"paths":layout::MOVES}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn organization_keeps_settings_components_and_backups_and_is_repeatable() {
        let root = std::env::temp_dir().join(format!("peli-organize-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("Mod/DLSSNR")).unwrap();
        let old = root.join("Mod/DLSSNR/backend-files");
        fs::write(
            root.join("config.json"),
            serde_json::to_vec(&serde_json::json!({"backend_dir":old})).unwrap(),
        )
        .unwrap();
        fs::create_dir_all(root.join("CaptureSettings")).unwrap();
        let filename = format!("{}.json", "a".repeat(64));
        fs::write(
            root.join("CaptureSettings").join(filename),
            br#"{"game_key":"Steam:123","game_name":"Tomb Raider"}"#,
        )
        .unwrap();
        use sha2::{Digest, Sha256};
        let profiles = root.join("FrameGeneration/profiles");
        fs::create_dir_all(&profiles).unwrap();
        let selection = profiles.join(format!("{:x}.selection.json", Sha256::digest(b"Steam:123")));
        fs::write(selection, b"{\"enabled\":true}").unwrap();
        fs::create_dir_all(root.join("webview/localstorage")).unwrap();
        fs::write(
            root.join("webview/localstorage/database.sqlite"),
            b"browser-session",
        )
        .unwrap();
        organize_at(&root, None).unwrap();
        assert!(!root.join("config.json").exists());
        assert!(root.join("components/Mods/DLSSNR").is_dir());
        assert!(root.join("profiles/capture/tomb-raider.json").is_file());
        assert!(root
            .join("components/FrameGeneration/profiles/selections/tomb-raider.json")
            .is_file());
        let config = fs::read_to_string(root.join("settings/config.json")).unwrap();
        assert!(config.contains("components/Mods/DLSSNR/backend-files"));
        assert!(root
            .join("migration-backup/layout-v2/settings/config.json")
            .is_file());
        // Copy of the data as it was before anything moved.
        let before = root.join("migration-backup/layout-v2/before");
        assert!(fs::read_to_string(before.join("config.json")).unwrap().contains("Mod/DLSSNR/backend-files"));
        assert!(before.join("CaptureSettings").join(format!("{}.json", "a".repeat(64))).is_file());
        assert!(!before.join("webview").exists());
        assert_eq!(
            fs::read(root.join("webview/localstorage/database.sqlite")).unwrap(),
            b"browser-session"
        );
        assert!(organize(&root, false).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn identical_leftovers_do_not_block_the_organization() {
        let root = std::env::temp_dir().join(format!("peli-organize-leftover-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("library")).unwrap();
        fs::create_dir_all(root.join("components/Mods/DLSSNR")).unwrap();
        fs::create_dir_all(root.join("Mod/DLSSNR")).unwrap();
        fs::write(root.join("Mod/DLSSNR/config.json"), b"{}").unwrap();
        fs::write(root.join("installed-destinations.lock"), b"").unwrap();
        fs::write(root.join("library/installed-destinations.lock"), b"").unwrap();
        fs::write(root.join("config.json"), b"{}").unwrap();
        organize_at(&root, None).unwrap();
        assert!(!root.join("installed-destinations.lock").exists());
        assert!(root.join("library/installed-destinations.lock").is_file());
        assert!(root.join("components/Mods/DLSSNR/config.json").is_file());
        assert!(root.join("settings/config.json").is_file());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn conflicts_and_links_never_overwrite_or_follow_user_files() {
        let root =
            std::env::temp_dir().join(format!("peli-organize-conflict-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("settings")).unwrap();
        fs::write(root.join("config.json"), b"old").unwrap();
        fs::write(root.join("settings/config.json"), b"new").unwrap();
        assert!(organize_at(&root, None).is_err());
        assert_eq!(fs::read(root.join("config.json")).unwrap(), b"old");
        assert_eq!(fs::read(root.join("settings/config.json")).unwrap(), b"new");
        assert_eq!(
            replace_prefix(
                "/profile/Models/data",
                "/profile/Mod",
                "/profile/components/Mods"
            ),
            "/profile/Models/data"
        );
        fs::remove_file(root.join("config.json")).unwrap();
        std::os::unix::fs::symlink(root.join("settings/config.json"), root.join("config.json"))
            .unwrap();
        assert!(organize_at(&root, None).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn artwork_shortcuts_and_absolute_component_links_keep_working_after_moving() {
        let root =
            std::env::temp_dir().join(format!("peli-artwork-migration-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let covers = root.join("covers");
        fs::create_dir_all(&covers).unwrap();
        let image = covers.join(format!("automatic-{}.jpg", "b".repeat(64)));
        fs::write(&image, b"image").unwrap();
        fs::create_dir_all(root.join("CaptureLibrary")).unwrap();
        let key = "c".repeat(64);
        let thumb = root.join("CaptureLibrary").join(format!("{key}.jpg"));
        fs::write(&thumb, b"thumbnail").unwrap();
        fs::write(
            root.join("CaptureLibrary").join(format!("{key}.json")),
            serde_json::to_vec(&serde_json::json!({"thumbnail":thumb})).unwrap(),
        )
        .unwrap();
        fs::create_dir_all(root.join("runners/proton/test")).unwrap();
        fs::write(root.join("runners/proton/test/proton"), b"runner").unwrap();
        std::os::unix::fs::symlink(
            root.join("runners/proton/test/proton"),
            root.join("runners/proton/linked"),
        )
        .unwrap();
        let apps = root.join("applications-fixture");
        fs::create_dir(&apps).unwrap();
        fs::write(
            apps.join("peligames-test.desktop"),
            format!("[Desktop Entry]\nIcon={}\n", image.display()),
        )
        .unwrap();
        organize_at(&root, Some(&apps)).unwrap();
        assert!(root.join("cache/covers/imported-cover.jpg").is_file());
        let details: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("cache/video-library/saved-clip.json")).unwrap(),
        )
        .unwrap();
        assert!(Path::new(details["thumbnail"].as_str().unwrap()).is_file());
        assert!(fs::read_to_string(apps.join("peligames-test.desktop"))
            .unwrap()
            .contains("cache/covers/imported-cover.jpg"));
        assert!(root.join("components/runners/proton/linked").exists());
        let mappings = aliases(&root).unwrap();
        assert!(mappings
            .iter()
            .any(|(_, new)| new.ends_with("imported-cover.jpg")));
        assert!(completed(&root));
        assert!(organize(&root, false).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn incomplete_rename_journal_is_resumed_and_external_entries_are_rejected() {
        let root =
            std::env::temp_dir().join(format!("peli-resume-migration-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("cache/covers")).unwrap();
        let source = root.join("cache/covers/old.jpg");
        let target = root.join("cache/covers/readable.jpg");
        fs::write(&source, b"keep").unwrap();
        atomic_json(
            &root.join("settings/path-aliases.json"),
            &vec![(
                source.to_string_lossy().into_owned(),
                target.to_string_lossy().into_owned(),
            )],
        )
        .unwrap();
        organize_at(&root, None).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        assert!(!source.exists());
        fs::remove_file(root.join("settings/storage-layout-v2.json")).unwrap();
        atomic_json(
            &root.join("settings/path-aliases.json"),
            &vec![(
                "/outside/private".to_string(),
                target.to_string_lossy().into_owned(),
            )],
        )
        .unwrap();
        assert!(organize_at(&root, None).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
