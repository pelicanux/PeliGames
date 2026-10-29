//! Mod ownership belongs to the game, not the resettable launcher profile.
use super::nexus_local::{NexusGame, Workspace};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
struct Record {
    format: u32,
    game: NexusGame,
    storage: PathBuf,
    device: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: String,
    pub name: String,
    pub directory: String,
    pub device: String,
    pub available: bool,
    pub portable: bool,
    pub mods: usize,
}
fn catalog() -> Result<PathBuf, String> {
    Ok(dirs::data_dir()
        .ok_or("Pasta de dados indisponível.")?
        .join(".peligames-mod-library/catalog.json"))
}
fn real(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || path.parent().is_none() {
        return Err("Pasta de mods inválida.".into());
    }
    for p in path.ancestors() {
        if fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "Armazenamento de mods redirecionado: {}",
                p.display()
            ));
        }
    }
    Ok(())
}
fn json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    real(path)?;
    fs::create_dir_all(path.parent().ok_or("Pasta de mods inválida.")?)
        .map_err(|e| e.to_string())?;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    // Persist newly created marker directories as well as the JSON itself.
    for parent in path
        .parent()
        .into_iter()
        .flat_map(|p| p.ancestors())
        .take(4)
    {
        fs::File::open(parent)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    real(path)?;
    let m = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > 32 * 1024 * 1024 {
        return Err("Registro persistente de mods inválido.".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn records() -> Result<Vec<Record>, String> {
    let path = catalog()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    read(&path)
}
pub(super) fn offline_storage(game: &NexusGame) -> Result<Option<PathBuf>, String> {
    let target = portable(game)?;
    Ok(records()?
        .into_iter()
        .find(|r| r.game.id == game.id && r.storage == target)
        .map(|r| r.storage))
}
fn root(game: &NexusGame) -> Result<PathBuf, String> {
    let path = PathBuf::from(
        game.game["directory"]
            .as_str()
            .ok_or("Jogo sem pasta registrada.")?,
    );
    real(&path)?;
    Ok(path)
}
fn mods_folder(game: &NexusGame) -> Result<PathBuf, String> {
    uuid::Uuid::parse_str(&game.id).map_err(|_| "Identificador de mods inválido.")?;
    Ok(root(game)?.join(".peligames/mods"))
}
/// `.peligames/mods/<game>` beside the game, named after it; the game id lives
/// in `<game>.identity`. Storage created before readable names keeps `<id>`
/// until `readable_storage` renames it.
pub(super) fn portable(game: &NexusGame) -> Result<PathBuf, String> {
    let folder = mods_folder(game)?;
    let legacy = folder.join(&game.id);
    Ok(match crate::core::paths::names::find(&folder, &game.id) {
        Some(path) if path.exists() || !legacy.exists() => path,
        _ => legacy,
    })
}
/// The readable folder for a game that has no storage yet.
pub(super) fn reserve(game: &NexusGame) -> Result<PathBuf, String> {
    let current = portable(game)?;
    if current.exists() || current.file_name().is_some_and(|name| name != game.id.as_str()) {
        return Ok(current);
    }
    let folder = mods_folder(game)?;
    real(&folder)?;
    crate::core::paths::names::allocate(&folder, game.game["name"].as_str().unwrap_or("jogo"), &game.id)
}
/// Marker carried inside a renamed storage until every reference follows it.
const RENAME_MARKER: &str = "storage-rename.json";
fn regular_folder(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}
/// Gives id-named mod storage its readable name: `.peligames/mods/<id>` beside
/// the game and the older `games/<name>--<id>` (plus its `games/<id>` link) in
/// the launcher folder. Paths recorded inside it, the game's symlinks into it
/// and the workspace entry follow. A marker travels with the folder, so an
/// interrupted rename resumes on the next load; a running game keeps its folder.
pub(super) fn readable_storage(game: &mut NexusGame) -> Result<bool, String> {
    let mut changed = false;
    if let Ok(parent) = super::nexus_local::root().map(|root| root.join("games")) {
        if regular_folder(&parent) {
            let suffix = format!("--{}", game.id);
            let named: Vec<PathBuf> = fs::read_dir(&parent).map_err(|e| e.to_string())?.flatten().map(|e| e.path())
                .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(&suffix)) && regular_folder(p)).collect();
            if let [old] = named.as_slice() {
                let link = parent.join(&game.id);
                let linked = fs::read_link(&link).ok().is_some_and(|target| Some(target.as_os_str()) == old.file_name());
                let from = if linked { vec![old.clone(), link.clone()] } else { vec![old.clone()] };
                if begin_rename(game, &parent, old, &from)? && linked {
                    fs::remove_file(&link).map_err(|e| e.to_string())?;
                }
            }
            changed |= finish_rename(game, &parent)?;
            // The old `games/<id>` link, left behind by an interrupted rename.
            let link = parent.join(&game.id);
            if fs::read_link(&link).is_ok_and(|target| target.to_string_lossy().ends_with(&suffix)) && !link.exists() {
                fs::remove_file(&link).map_err(|e| e.to_string())?;
            }
        }
    }
    if let Ok(folder) = mods_folder(game) {
        if regular_folder(&folder) {
            let old = folder.join(&game.id);
            if regular_folder(&old) {
                begin_rename(game, &folder, &old, &[old.clone()])?;
            }
            changed |= finish_rename(game, &folder)?;
        }
    }
    Ok(changed)
}
/// Renames `old` to the readable name reserved for the game in `parent`, leaving
/// the marker inside. `false` while the game (or its virtual file system) uses it.
fn begin_rename(game: &NexusGame, parent: &Path, old: &Path, from: &[PathBuf]) -> Result<bool, String> {
    real(old)?;
    if super::nexus_storage::in_use(old) {
        return Ok(false);
    }
    let target = crate::core::paths::names::allocate(parent, game.game["name"].as_str().unwrap_or("jogo"), &game.id)?;
    if fs::symlink_metadata(&target).is_ok() {
        return Err(format!("Pastas antiga e nova de mods coexistem em {}; nenhuma foi alterada.", parent.display()));
    }
    json(&old.join(RENAME_MARKER), &serde_json::json!({"from": from}))?;
    if let Err(error) = fs::rename(old, &target) {
        let _ = fs::remove_file(old.join(RENAME_MARKER));
        let _ = fs::remove_file(target.with_extension("identity"));
        return Err(error.to_string());
    }
    Ok(true)
}
fn finish_rename(game: &mut NexusGame, parent: &Path) -> Result<bool, String> {
    let Some(target) = crate::core::paths::names::find(parent, &game.id).filter(|t| regular_folder(t)) else { return Ok(false) };
    let marker = target.join(RENAME_MARKER);
    if !marker.is_file() {
        return Ok(false);
    }
    let record: serde_json::Value = read(&marker)?;
    let from: Vec<PathBuf> = serde_json::from_value(record["from"].clone()).map_err(|_| "Registro de renomeação inválido.")?;
    if from.is_empty() || from.iter().any(|path| path.parent() != Some(parent)) {
        return Err("Registro de renomeação fora da pasta de mods.".into());
    }
    let repoint = |link: &Path| -> Option<PathBuf> {
        from.iter().find_map(|old| link.strip_prefix(old).ok().map(|rest| target.join(rest)))
    };
    let relink = |path: &Path, next: PathBuf| crate::core::paths::replace_symlink(path, &next);
    // The game's symlinks into the folder, found through the deployment record.
    let manifest = target.join("deployment.json");
    if manifest.is_file() {
        let value: serde_json::Value = read(&manifest)?;
        let destination = super::nexus_deployment::destinations(game).ok();
        let mut deployed: Vec<PathBuf> = value["targets"].as_object().into_iter().flatten().filter_map(|(_, v)| v.as_str().map(PathBuf::from)).collect();
        if let Some(destination) = &destination {
            deployed.extend(value["files"].as_object().into_iter().flatten().filter_map(|(key, _)| destination(key).ok()));
        }
        for path in deployed.iter().filter(|p| p.is_absolute()) {
            let Ok(link) = fs::read_link(path) else { continue };
            if let Some(next) = repoint(&link) { relink(path, next)?; }
        }
    }
    // Absolute paths recorded inside the folder: manifests and internal links.
    for entry in fs::read_dir(&target).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if entry.file_name() == RENAME_MARKER || path.extension().is_none_or(|e| e != "json")
            || !entry.file_type().is_ok_and(|t| t.is_file()) || entry.metadata().map_or(true, |m| m.len() > 32 * 1024 * 1024) {
            continue;
        }
        let mut value: serde_json::Value = read(&path)?;
        let before = value.clone();
        for old in &from { rebase(&mut value, old, &target); }
        if value != before { json(&path, &value)?; }
    }
    for entry in walkdir::WalkDir::new(&target).follow_links(false).min_depth(1) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_symlink() { continue; }
        let link = fs::read_link(entry.path()).map_err(|e| e.to_string())?;
        if let Some(next) = repoint(&link) { relink(entry.path(), next)?; }
    }
    for old in &from { *game = rebased(game, old, &target)?; }
    fs::remove_file(&marker).map_err(|e| e.to_string())?;
    Ok(true)
}
fn device(path: &Path) -> String {
    // Only on save/review, never a recursive scan of disks or games.
    std::process::Command::new("findmnt")
        .args(["--noheadings", "--output", "UUID,SOURCE,TARGET", "--target"])
        .arg(path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "Dispositivo não identificado".into())
}
fn rebase(value: &mut serde_json::Value, old: &Path, new: &Path) {
    match value {
        serde_json::Value::String(s) => {
            if let Ok(rel) = Path::new(s.as_str()).strip_prefix(old) {
                *s = new.join(rel).to_string_lossy().into_owned()
            }
        }
        serde_json::Value::Array(a) => {
            for v in a {
                rebase(v, old, new)
            }
        }
        serde_json::Value::Object(o) => {
            for v in o.values_mut() {
                rebase(v, old, new)
            }
        }
        _ => {}
    }
}
fn rebased(game: &NexusGame, old: &Path, new: &Path) -> Result<NexusGame, String> {
    let mut value = serde_json::to_value(game).map_err(|e| e.to_string())?;
    rebase(&mut value, old, new);
    serde_json::from_value(value).map_err(|e| e.to_string())
}
pub(super) fn stage_metadata(data: &Workspace) -> Result<MetadataTransaction, String> {
    let previous = records()?;
    let mut next = vec![];
    let mut writes = vec![];
    for game in &data.games {
        if game.mods.is_empty() && !portable(game)?.join("game.json").is_file() {
            continue;
        }
        let folder = root(game)?;
        let storage = super::nexus_profile::directory(game)?;
        let old = previous.iter().find(|r| r.game.id == game.id);
        let record = Record {
            format: 1,
            game: game.clone(),
            storage: storage.clone(),
            device: old
                .map(|r| r.device.clone())
                .unwrap_or_else(|| device(&folder)),
        };
        if storage == portable(game)?
            && folder.is_dir()
            && (!game.mods.is_empty() || storage.join("game.json").is_file())
        {
            writes.push((
                storage.join("game.json"),
                serde_json::to_value(&record).map_err(|e| e.to_string())?,
            ));
        }
        if !game.mods.is_empty() {
            next.push(record);
        }
    }
    if !next.is_empty() || catalog()?.exists() {
        writes.push((
            catalog()?,
            serde_json::to_value(&next).map_err(|e| e.to_string())?,
        ));
    }
    let mut tx = MetadataTransaction {
        previous: vec![],
        committed: false,
    };
    for (path, value) in writes {
        real(&path)?;
        let previous = match fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => {
                let _: serde_json::Value = read(&path)?;
                let backup =
                    path.with_extension(format!("{}.metadata-backup", uuid::Uuid::new_v4()));
                if fs::hard_link(&path, &backup).is_err() {
                    fs::copy(&path, &backup).map_err(|e| e.to_string())?;
                    fs::File::open(&backup)
                        .and_then(|f| f.sync_all())
                        .map_err(|e| e.to_string())?;
                }
                Some(backup)
            }
            Ok(_) => return Err("Registro de mods redirecionado; operação interrompida.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        tx.previous.push((path.clone(), previous));
        json(&path, &value)?;
    }
    Ok(tx)
}
pub(super) struct MetadataTransaction {
    previous: Vec<(PathBuf, Option<PathBuf>)>,
    committed: bool,
}
impl MetadataTransaction {
    pub(super) fn commit(mut self) {
        self.committed = true;
        for (_, old) in &self.previous {
            if let Some(backup) = old {
                let _ = fs::remove_file(backup);
            }
        }
    }
}
impl Drop for MetadataTransaction {
    fn drop(&mut self) {
        if !self.committed {
            for (path, old) in self.previous.iter().rev() {
                if let Some(backup) = old {
                    let _ = fs::rename(backup, path);
                } else {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}
fn persist(data: &Workspace) -> Result<(), String> {
    stage_metadata(data)?.commit();
    Ok(())
}
/// Read only the selected game's own marker folder; never walk a disk looking for mods.
pub(super) fn recover_game(directory: &Path, data: &mut Workspace) -> Result<bool, String> {
    real(directory)?;
    let markers = directory.join(".peligames/mods");
    if !markers.exists() {
        return Ok(false);
    }
    real(&markers)?;
    for entry in fs::read_dir(&markers).map_err(|e| e.to_string())?.take(256) {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        if !entry.path().join("game.json").is_file() {
            continue;
        }
        let saved: Record = read(&entry.path().join("game.json"))?;
        let id = saved.game.id.clone();
        // The folder is the game id (older layout) or a readable name whose
        // `.identity` holds it; the record was saved for this same folder name.
        let owned = entry.file_name() == id.as_str()
            || crate::core::paths::names::find(&markers, &id).is_some_and(|path| path == entry.path());
        if saved.format != 1 || uuid::Uuid::parse_str(&id).is_err() || !owned
            || saved.storage.file_name() != Some(entry.file_name().as_os_str())
            || saved.storage.parent() != Some(root(&saved.game)?.join(".peligames/mods").as_path()) {
            return Err("Registro de mods inválido junto ao jogo; arquivos preservados.".into());
        }
        if saved.game.mods.is_empty() {
            continue;
        }
        if data.games.iter().any(|g| g.id == id) {
            return Ok(false);
        }
        let old_root = root(&saved.game)?;
        let new_storage = entry.path();
        let mut value = serde_json::to_value(&saved.game).map_err(|e| e.to_string())?;
        rebase(&mut value, &old_root, directory);
        rebase(&mut value, &saved.storage, &new_storage);
        let game: NexusGame = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if old_root != directory {
            let manifest = new_storage.join("deployment.json");
            if manifest.is_file() {
                fs::copy(&manifest, new_storage.join("recovery-deployment.json"))
                    .map_err(|e| e.to_string())?;
                super::mod_backup::repair_links(
                    &game,
                    &new_storage,
                    &saved.storage,
                    &new_storage,
                    &old_root,
                    directory,
                )?;
                let mut value: serde_json::Value = read(&manifest)?;
                rebase(&mut value, &old_root, directory);
                json(&manifest, &value)?;
            }
        }
        data.games.push(game);
        return Ok(true);
    }
    Ok(false)
}
pub(super) fn recover(data: &mut Workspace) -> Result<(), String> {
    for record in records()? {
        if record.format != 1 {
            return Err("Versão desconhecida do índice de mods.".into());
        }
        let target = portable(&record.game)?;
        if data
            .games
            .iter()
            .any(|g| g.id == record.game.id || g.game["directory"] == record.game.game["directory"])
        {
            continue;
        }
        // Existing offline records remain mapped, so reconnecting a disk needs no rescan.
        let game = if target.join("game.json").is_file() {
            let saved: Record = read(&target.join("game.json"))?;
            if saved.format != 1
                || saved.game.id != record.game.id
                || saved.game.game["directory"] != record.game.game["directory"]
            {
                return Err("O registro de mods não corresponde à instalação do jogo.".into());
            }
            rebased(&saved.game, &saved.storage, &target)?
        } else {
            record.game
        };
        data.games.push(game);
    }
    Ok(())
}
pub(super) fn review() -> Result<Vec<Review>, String> {
    let _lock = super::nexus_local::STORAGE
        .lock()
        .map_err(|e| e.to_string())?;
    let mut data = super::nexus_local::load()?;
    recover(&mut data)?;
    let saved = records()?;
    data.games
        .iter()
        .filter(|g| !g.mods.is_empty())
        .map(|g| {
            let path = root(g)?;
            let portable = super::nexus_profile::directory(g)? == self::portable(g)?;
            let current_device = path.is_dir().then(|| device(&path));
            let device_matches = saved
                .iter()
                .find(|r| r.game.id == g.id)
                .is_none_or(|record| {
                    let known = record.device.split_whitespace().next().unwrap_or("");
                    let current = current_device
                        .as_deref()
                        .unwrap_or("")
                        .split_whitespace()
                        .next()
                        .unwrap_or("");
                    !known.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
                        || known.len() < 8
                        || known == current
                });
            let available = device_matches
                && path.is_dir()
                && super::nexus_discovery::resolve(&path).is_ok_and(|(_, module)| module.is_some());
            Ok(Review {
                id: g.id.clone(),
                name: g.game["name"].as_str().unwrap_or("Jogo").into(),
                directory: path.to_string_lossy().into_owned(),
                device: saved
                    .iter()
                    .find(|r| r.game.id == g.id)
                    .map(|r| r.device.clone())
                    .unwrap_or_else(|| device(&path)),
                available,
                portable,
                mods: g.mods.len(),
            })
        })
        .collect()
}
pub(super) fn reset_revision() -> Result<String, String> {
    let data = super::nexus_local::workspace()?;
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&data).map_err(|e| e.to_string())?)
    ))
}
fn digest(path: &Path) -> Result<Vec<u8>, String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = f.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(h.finalize().to_vec())
}
fn copy_tree(
    old: &Path,
    target: &Path,
    final_target: &Path,
    game_root: &Path,
) -> Result<(), String> {
    real(old)?;
    real(target)?;
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in walkdir::WalkDir::new(old).follow_links(false).min_depth(1) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let dest = target.join(path.strip_prefix(old).map_err(|e| e.to_string())?);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        } else if entry.file_type().is_file() {
            if fs::hard_link(path, &dest).is_err() {
                fs::copy(path, &dest).map_err(|e| e.to_string())?;
            }
            if digest(path)? != digest(&dest)? {
                return Err("Falha ao verificar a migração dos mods; origem preservada.".into());
            }
        } else if entry.file_type().is_symlink() {
            let link = fs::read_link(path).map_err(|e| e.to_string())?;
            let resolved = path.canonicalize().map_err(|e| e.to_string())?;
            if !resolved.starts_with(old) && !resolved.starts_with(game_root) {
                return Err("Link externo no armazenamento de mods; origem preservada.".into());
            }
            let link = if link.is_absolute() {
                link.strip_prefix(old)
                    .map(|relative| final_target.join(relative))
                    .unwrap_or(link)
            } else {
                link
            };
            #[cfg(unix)]
            std::os::unix::fs::symlink(link, &dest).map_err(|e| e.to_string())?;
        } else {
            return Err("Tipo de arquivo não suportado na migração dos mods.".into());
        }
    }
    Ok(())
}
fn migrate(game: &mut NexusGame) -> Result<(), String> {
    let old = super::nexus_storage::legacy_directory(game)?;
    let target = reserve(game)?;
    if !old.exists() && target.join("game.json").is_file() {
        return Ok(());
    }
    if !old.is_dir() && !target.join("game.json").is_file() {
        return Err(format!(
            "O armazenamento dos mods de {} está indisponível. A redefinição foi interrompida.",
            game.game["name"].as_str().unwrap_or("jogo")
        ));
    }
    let folder = root(game)?;
    real(&target)?;
    if target.exists() {
        let saved: Record = read(&target.join("game.json"))?;
        if saved.format != 1
            || saved.game.id != game.id
            || saved.storage != target
            || root(&saved.game)? != folder
        {
            return Err(
                "Já existe armazenamento de outra instalação; nenhuma pasta foi sobrescrita."
                    .into(),
            );
        }
        let migrated = rebased(game, &old, &target)?;
        super::mod_backup::repair_links(&migrated, &target, &old, &target, &folder, &folder)?;
        *game = migrated;
        return Ok(());
    }
    fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    let stage = target.with_file_name(format!(".migrate-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&stage).map_err(|e| e.to_string())?;
    struct Stage(PathBuf);
    impl Drop for Stage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _stage = Stage(stage.clone());
    copy_tree(&old, &stage, &target, &folder)?;
    let migrated = rebased(game, &old, &target)?;
    // Keep old deployment sources until validated game symlinks are repaired.
    if stage.join("deployment.json").is_file() {
        fs::copy(
            stage.join("deployment.json"),
            stage.join("recovery-deployment.json"),
        )
        .map_err(|e| e.to_string())?;
    }
    if stage.join("deployment.json").is_file() {
        let path = stage.join("deployment.json");
        let mut value: serde_json::Value = read(&path)?;
        rebase(&mut value, &old, &target);
        json(&path, &value)?;
    }
    // A durable marker must never get ahead of the archive/original bytes.
    for entry in walkdir::WalkDir::new(&stage)
        .follow_links(false)
        .contents_first(true)
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().is_symlink() {
            fs::File::open(entry.path())
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
    }
    // The marker is committed with the directory, so a crash before saving the
    // central index can still recover the complete per-game registration.
    json(
        &stage.join("game.json"),
        &Record {
            format: 1,
            game: migrated.clone(),
            storage: target.clone(),
            device: device(&folder),
        },
    )?;
    fs::rename(&stage, &target).map_err(|e| e.to_string())?;
    fs::File::open(target.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())?;
    super::mod_backup::repair_links(&migrated, &target, &old, &target, &folder, &folder)?;
    *game = migrated;
    Ok(())
}
/// All games are preflighted before any uninstall. Transactions stay armed
/// across the entire batch, so an error rolls previously applied games back.
pub(super) fn prepare_reset(remove: bool) -> Result<(), String> {
    let review = review()?;
    if let Some(item) = review
        .iter()
        .find(|r| !r.available && (remove || !r.portable))
    {
        return Err(format!("Não foi possível acessar “{}”, em {}. O dispositivo registrado é {}. Verifique se ele está conectado e montado no caminho original e tente novamente. Nenhum mod foi removido e a redefinição não foi iniciada.",item.name,item.directory,item.device));
    }
    let _lock = super::nexus_local::STORAGE
        .lock()
        .map_err(|e| e.to_string())?;
    let _backup = super::mod_backup::migration_lock()?;
    let mut data = super::nexus_local::load()?;
    recover(&mut data)?;
    for game in &data.games {
        if !game.mods.is_empty() && root(game)?.is_dir() {
            super::nexus_deployment::idle(game)?;
        }
    }
    // Index first: if a copy fails, legacy data remains discoverable and reset aborts.
    persist(&data)?;
    for game in &mut data.games {
        if !game.mods.is_empty() && root(game)?.is_dir() {
            migrate(game)?;
        }
    }
    super::nexus_local::save(&data)?;
    if !remove {
        return Ok(());
    }
    let mut transactions = vec![];
    for game in &mut data.games {
        if game.mods.is_empty() {
            continue;
        }
        game.mods.clear();
        game.profile = Some(if super::nexus_deployment::supports(game) {
            super::nexus_deployment::build_after_removal(game)?
        } else {
            super::nexus_profile::build(game)?
        });
        if super::nexus_deployment::supports(game) {
            transactions.push(super::nexus_deployment::apply(game)?);
        }
    }
    super::nexus_local::save(&data)?;
    for tx in transactions {
        tx.commit();
    }
    // Keep empty per-game records (no automatic re-import of removed mods).
    for game in &data.games {
        let path = portable(game)?;
        if path.join("game.json").is_file() {
            real(&path)?;
            let saved: Record = read(&path.join("game.json"))?;
            if saved.game.id != game.id {
                return Err("Registro de armazenamento divergente; pacotes preservados.".into());
            }
            fs::remove_dir_all(&path).map_err(|e|format!("Mods removidos do jogo, mas não foi possível limpar os pacotes preservados: {e}"))?;
            json(
                &path.join("game.json"),
                &Record {
                    format: 1,
                    game: game.clone(),
                    storage: path.clone(),
                    device: device(&root(game)?),
                },
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn isolated() {
        for key in ["XDG_CONFIG_HOME", "XDG_DATA_HOME"] {
            let path = PathBuf::from(std::env::var(key).expect("isolated XDG required"));
            assert!(
                path.starts_with("/tmp")
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("peligames-")
            );
        }
    }
    fn fixture(base: &Path, method: &str) -> NexusGame {
        let root = base.join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("bin/bg3_dx11.exe"), b"MZ fixture").unwrap();
        fs::write(root.join("bin/bink2w64.dll"), b"original").unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let mut game:NexusGame=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"game":{"directory":root,"name":"Baldur's Gate 3","path":root.join("bin/bg3_dx11.exe")},"platform":"proton","deploy_method":method,"mods":[{"id":id,"name":"Loader","archive":"placeholder.zip","size":1,"installed":true,"enabled":true,"replace_originals":true,"installed_files":[["bin/bink2w64.dll","bin/bink2w64.dll"],["bin/NativeMods/Fix.dll","bin/NativeMods/Fix.dll"]]}]})).unwrap();
        // Deliberately create the previous central layout to exercise migration.
        let old = super::super::nexus_storage::legacy_directory(&game).unwrap();
        fs::create_dir_all(old.join("archives")).unwrap();
        let archive = old.join("archives").join(format!("{id}.zip"));
        let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        for (name, bytes) in [
            ("bin/bink2w64.dll", b"loader".as_slice()),
            ("bin/NativeMods/Fix.dll", b"fix".as_slice()),
        ] {
            zip.start_file(name, zip::write::FileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        game.mods[0].archive = archive.to_string_lossy().into_owned();
        game.profile = Some(super::super::nexus_deployment::build(&game).unwrap());
        super::super::nexus_deployment::apply(&game)
            .unwrap()
            .commit();
        game
    }
    #[test]
    #[ignore = "Requires isolated XDG_CONFIG_HOME and XDG_DATA_HOME (/tmp/peligames-*)"]
    fn reset_preserves_mods_recovers_after_config_deletion_and_purges_transactionally() {
        isolated();
        let base = std::env::temp_dir().join(format!(
            "peligames-durable-fixture-{}",
            uuid::Uuid::new_v4()
        ));
        let first = fixture(&base, "copy");
        let second = fixture(&base, "symlink");
        super::super::nexus_local::save(&Workspace {
            games: vec![first.clone(), second.clone()],
        })
        .unwrap();
        prepare_reset(false).unwrap();
        for game in [&first, &second] {
            let storage = portable(game).unwrap();
            assert!(storage.join("game.json").is_file());
            assert_eq!(
                fs::read(storage.join("originals/bin/bink2w64.dll")).unwrap(),
                b"original"
            );
            assert_eq!(
                fs::read(root(game).unwrap().join("bin/bink2w64.dll")).unwrap(),
                b"loader"
            );
        }
        let old_config = super::super::nexus_local::root().unwrap();
        fs::remove_dir_all(&old_config).unwrap();
        let recovered = super::super::nexus_local::load().unwrap();
        assert_eq!(recovered.games.len(), 2);
        // Detached monitors read the central snapshot, not the recovery catalog.
        for game in &recovered.games {
            assert_eq!(super::super::nexus_local::log_game(&game.id).unwrap().id, game.id);
        }
        let persisted = fs::read(old_config.join("library.json")).unwrap();
        super::super::nexus_local::load().unwrap();
        assert_eq!(fs::read(old_config.join("library.json")).unwrap(), persisted);
        let before = fs::read(catalog().unwrap()).unwrap();
        {
            let mut draft = recovered.clone();
            draft.games[0].mods[0].name = "Uncommitted".into();
            let _rollback = stage_metadata(&draft).unwrap();
        }
        assert_eq!(
            fs::read(catalog().unwrap()).unwrap(),
            before,
            "metadata rollback restores the previous catalog without rewriting it"
        );
        let marker = portable(&second).unwrap().join("game.json");
        let backup = marker.with_extension("saved");
        fs::rename(&marker, &backup).unwrap();
        fs::create_dir(&marker).unwrap();
        assert!(stage_metadata(&recovered).is_err());
        assert_eq!(
            fs::read(catalog().unwrap()).unwrap(),
            before,
            "a later marker error must not commit earlier metadata"
        );
        fs::remove_dir(&marker).unwrap();
        fs::rename(&backup, &marker).unwrap();
        for game in &recovered.games {
            assert!(Path::new(&game.mods[0].archive).is_file());
            assert!(game.mods[0].enabled);
        }
        // Neither deletion of settings nor a missing catalog prevents recovery from the selected game.
        let mut standalone = Workspace::default();
        assert!(recover_game(&root(&first).unwrap(), &mut standalone).unwrap());
        assert_eq!(standalone.games[0].mods[0].name, "Loader");
        let offline = root(&first).unwrap().with_extension("offline");
        fs::rename(root(&first).unwrap(), &offline).unwrap();
        assert!(prepare_reset(true).unwrap_err().contains("montado"));
        super::super::nexus_local::save(&recovered).unwrap();
        assert!(review()
            .unwrap()
            .iter()
            .any(|g| g.id == first.id && g.portable && !g.available));
        prepare_reset(false).unwrap();
        assert_eq!(
            fs::read(offline.join("bin/bink2w64.dll")).unwrap(),
            b"loader"
        );
        fs::rename(&offline, root(&first).unwrap()).unwrap();
        // A failure in the second game rolls the first game's uninstall back.
        let original = portable(&second)
            .unwrap()
            .join("originals/bin/bink2w64.dll");
        fs::write(&original, b"corrupted backup").unwrap();
        assert!(prepare_reset(true).is_err());
        assert_eq!(
            fs::read(root(&first).unwrap().join("bin/bink2w64.dll")).unwrap(),
            b"loader"
        );
        assert!(super::super::nexus_local::load()
            .unwrap()
            .games
            .iter()
            .all(|g| g.mods[0].enabled));
        fs::write(&original, b"original").unwrap();
        prepare_reset(true).unwrap();
        for game in [&first, &second] {
            assert_eq!(
                fs::read(root(game).unwrap().join("bin/bink2w64.dll")).unwrap(),
                b"original"
            );
            assert!(!root(game).unwrap().join("bin/NativeMods/Fix.dll").exists());
            assert!(!portable(game).unwrap().join("archives").exists());
        }
        assert!(records().unwrap().is_empty());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn refuses_symlinked_storage_and_rebases_only_path_boundaries() {
        let base =
            std::env::temp_dir().join(format!("peligames-durable-safety-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        std::os::unix::fs::symlink("/tmp", base.join("redirect")).unwrap();
        assert!(json(&base.join("redirect/game.json"), &serde_json::json!({})).is_err());
        let mut value = serde_json::json!(["/old/mod/file", "/old/mod-extra/file"]);
        rebase(&mut value, Path::new("/old/mod"), Path::new("/new"));
        assert_eq!(
            value,
            serde_json::json!(["/new/file", "/old/mod-extra/file"])
        );
        fs::remove_dir_all(base).unwrap();
    }
    /// A symlink-deployed game whose storage and package still use ids as names.
    fn id_named_fixture(base: &Path) -> (NexusGame, PathBuf, PathBuf) {
        let root = base.join("Baldurs Gate 3");
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("bin/bg3_dx11.exe"), b"MZ fixture").unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let mut game: NexusGame = serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"game":{"directory":root,"name":"Baldur's Gate 3","path":root.join("bin/bg3_dx11.exe")},"platform":"proton","deploy_method":"symlink","mods":[{"id":id,"name":"Native Fix","version":"1.2","archive":"placeholder.zip","size":1,"installed":true,"enabled":true,"installed_files":[["bin/NativeMods/Fix.dll","bin/NativeMods/Fix.dll"]]}]})).unwrap();
        let readable = super::super::nexus_storage::directory(&game).unwrap();
        fs::create_dir_all(readable.join("archives")).unwrap();
        let archive = readable.join("archives").join(format!("{id}.zip"));
        let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        zip.start_file("bin/NativeMods/Fix.dll", zip::write::FileOptions::default()).unwrap();
        zip.write_all(b"fix").unwrap();
        zip.finish().unwrap();
        game.mods[0].archive = archive.to_string_lossy().into_owned();
        game.profile = Some(super::super::nexus_deployment::build(&game).unwrap());
        super::super::nexus_deployment::apply(&game).unwrap().commit();
        // Put the storage back under the game id, as older versions saved it.
        let old = readable.with_file_name(&game.id);
        fs::rename(&readable, &old).unwrap();
        fs::remove_file(readable.with_extension("identity")).unwrap();
        let mut manifest: serde_json::Value = read(&old.join("deployment.json")).unwrap();
        rebase(&mut manifest, &readable, &old);
        json(&old.join("deployment.json"), &manifest).unwrap();
        let link = root.join("bin/NativeMods/Fix.dll");
        let target = fs::read_link(&link).unwrap();
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(old.join(target.strip_prefix(&readable).unwrap()), &link).unwrap();
        (rebased(&game, &readable, &old).unwrap(), readable, old)
    }
    #[test]
    fn id_named_storage_and_packages_get_readable_names_and_links_follow() {
        let base = std::env::temp_dir().join(format!("peligames-readable-storage-{}", uuid::Uuid::new_v4()));
        let (mut game, readable, old) = id_named_fixture(&base);
        assert_eq!(portable(&game).unwrap(), old, "the old folder stays in use until renamed");
        assert!(readable_storage(&mut game).unwrap());
        assert_eq!(readable.file_name().unwrap(), "baldur-s-gate-3");
        assert!(readable.is_dir() && !old.exists());
        assert_eq!(portable(&game).unwrap(), readable);
        let link = PathBuf::from(game.game["directory"].as_str().unwrap()).join("bin/NativeMods/Fix.dll");
        assert!(fs::read_link(&link).unwrap().starts_with(&readable));
        assert_eq!(fs::read(&link).unwrap(), b"fix");
        let manifest = fs::read_to_string(readable.join("deployment.json")).unwrap();
        assert!(!manifest.contains(&game.id) && !readable.join(RENAME_MARKER).exists() && !readable.join("recovery-deployment.json").exists());
        assert!(Path::new(&game.mods[0].archive).starts_with(&readable));
        // The package itself is renamed after the mod and still opens.
        let mut item = game.mods[0].clone();
        assert!(super::super::nexus_archive::readable_package(&game, &mut item).unwrap());
        assert_eq!(Path::new(&item.archive).file_name().unwrap(), "native-fix-1-2.zip");
        assert!(super::super::nexus_archive::open(&game, &item).is_ok());
        assert!(!super::super::nexus_archive::readable_package(&game, &mut item).unwrap(), "nothing left to rename");
        assert!(!readable_storage(&mut game).unwrap(), "repeatable");
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn interrupted_rename_resumes_and_a_running_game_keeps_its_folder() {
        let base = std::env::temp_dir().join(format!("peligames-readable-resume-{}", uuid::Uuid::new_v4()));
        let (mut game, readable, old) = id_named_fixture(&base);
        fs::write(old.join("session.pid"), std::process::id().to_string()).unwrap();
        assert!(!readable_storage(&mut game).unwrap());
        assert!(old.is_dir());
        fs::remove_file(old.join("session.pid")).unwrap();
        // Crash right after the rename: only the marker knows the old name.
        json(&old.join(RENAME_MARKER), &serde_json::json!({"from": [old]})).unwrap();
        crate::core::paths::names::allocate(old.parent().unwrap(), "Baldur's Gate 3", &game.id).unwrap();
        fs::rename(&old, &readable).unwrap();
        assert_eq!(portable(&game).unwrap(), readable);
        assert!(readable_storage(&mut game).unwrap());
        let link = PathBuf::from(game.game["directory"].as_str().unwrap()).join("bin/NativeMods/Fix.dll");
        assert_eq!(fs::read(&link).unwrap(), b"fix");
        assert!(!readable.join(RENAME_MARKER).exists());
        // A readable folder found beside the game is recovered as its own.
        json(&readable.join("game.json"), &Record { format: 1, game: game.clone(), storage: readable.clone(), device: String::new() }).unwrap();
        let mut workspace = Workspace::default();
        assert!(recover_game(Path::new(game.game["directory"].as_str().unwrap()), &mut workspace).unwrap());
        assert_eq!(workspace.games[0].id, game.id);
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn launcher_folder_storage_named_with_the_id_gets_the_game_name() {
        let base = std::env::temp_dir().join(format!("peligames-readable-legacy-{}", uuid::Uuid::new_v4()));
        let mut game = fixture(&base, "symlink");
        let old = super::super::nexus_storage::legacy_directory(&game).unwrap();
        assert!(old.file_name().unwrap().to_string_lossy().ends_with(&format!("--{}", game.id)));
        // The link older versions kept for paths saved as `games/<id>`.
        let link = old.with_file_name(&game.id);
        std::os::unix::fs::symlink(old.file_name().unwrap(), &link).unwrap();
        assert!(readable_storage(&mut game).unwrap());
        let readable = super::super::nexus_storage::legacy_directory(&game).unwrap();
        assert_eq!(readable.file_name().unwrap(), "baldur-s-gate-3");
        assert!(!old.exists() && fs::symlink_metadata(&link).is_err());
        let fix = PathBuf::from(game.game["directory"].as_str().unwrap()).join("bin/NativeMods/Fix.dll");
        assert!(fs::read_link(&fix).unwrap().starts_with(&readable));
        assert_eq!(fs::read(&fix).unwrap(), b"fix");
        assert!(Path::new(&game.mods[0].archive).starts_with(&readable));
        assert!(super::super::nexus_archive::open(&game, &game.mods[0]).is_ok());
        assert!(!readable_storage(&mut game).unwrap());
        fs::remove_dir_all(base).unwrap();
        fs::remove_dir_all(readable).unwrap();
    }
}
