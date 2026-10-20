//! Cross-process discovery and cancellation requests for PeliGames-owned sessions.
use super::game_execution::ExecutionStatus;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Serialize, Deserialize)]
struct Record {
    owner: u32,
    birth: u64,
    status: ExecutionStatus,
}
pub(crate) fn directory() -> Result<PathBuf, String> {
    Ok(super::paths::app_root()?.join("executions"))
}
fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok()
}
fn birth(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields: Vec<_> = stat.rsplit_once(") ")?.1.split_whitespace().collect();
    if fields.first()? == &"Z" {
        return None;
    }
    fields.get(19)?.parse().ok()
}
fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    if let Ok(meta) = fs::symlink_metadata(dir) {
        if !meta.is_dir() {
            return Err("Registro de execução inválido.".into());
        }
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    let temp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        fs::rename(&temp, dir.join(name)).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
pub(crate) fn publish(dir: &Path, status: &ExecutionStatus) -> Result<(), String> {
    let record = Record {
        owner: std::process::id(),
        birth: birth(std::process::id()).ok_or("Monitor de execução indisponível.")?,
        status: status.clone(),
    };
    write(
        dir,
        &format!("{}.json", status.id),
        &serde_json::to_vec(&record).map_err(|e| e.to_string())?,
    )
}
fn record(dir: &Path, id: &str) -> Option<Record> {
    use std::os::unix::fs::MetadataExt;
    if !valid_id(id) {
        return None;
    }
    let path = dir.join(format!("{id}.json"));
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > 65536 || meta.uid() != unsafe { libc::geteuid() } {
        return None;
    }
    let record: Record = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    (record.status.id == id && record.status.active() && birth(record.owner) == Some(record.birth))
        .then_some(record)
}
pub(crate) fn statuses_at(dir:&Path)->Vec<ExecutionStatus>{
    let Ok(entries)=fs::read_dir(dir) else{return vec![];};
    entries.take(1024).flatten().filter_map(|entry|{
        let path=entry.path();
        if path.extension().is_none_or(|e|e!="json"){return None;}
        record(dir,path.file_stem()?.to_str()?).map(|r|r.status)
    }).collect()
}
pub(crate) fn status_at(dir:&Path)->Option<ExecutionStatus>{statuses_at(dir).into_iter().next()}
pub(crate) fn cancel_at(dir: &Path, id: &str) -> Result<ExecutionStatus, String> {
    let mut record =
        record(dir, id).ok_or("Esta execução já terminou ou o monitor está indisponível.")?;
    write(dir, &format!("{id}.cancel"), id.as_bytes())?;
    record.status.state = "stopping".into();
    Ok(record.status)
}
pub(crate) fn cancellation_requested(dir: &Path, id: &str) -> bool {
    let path = dir.join(format!("{id}.cancel"));
    fs::symlink_metadata(&path).is_ok_and(|m| m.is_file() && m.len() == id.len() as u64)
        && fs::read(path).is_ok_and(|b| b == id.as_bytes())
}
pub(crate) fn finish(dir: &Path, id: &str) {
    let _ = fs::remove_file(dir.join(format!("{id}.json")));
    let _ = fs::remove_file(dir.join(format!("{id}.cancel")));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry_tracks_each_session_and_cancels_only_the_selected_one() {
        let root=std::env::temp_dir().join(format!("peligames-session-registry-{}",uuid::Uuid::new_v4()));
        let sessions:Vec<_>=(1..=3).map(|number|ExecutionStatus{
            id:uuid::Uuid::new_v4().to_string(),path:format!("copy-{number}"),state:"running".into(),log:String::new(),error:None,
        }).collect();
        for session in &sessions {publish(&root,session).unwrap();}
        assert_eq!(statuses_at(&root).len(),3);
        cancel_at(&root,&sessions[1].id).unwrap();
        assert!(!cancellation_requested(&root,&sessions[0].id));
        assert!(cancellation_requested(&root,&sessions[1].id));
        assert!(!cancellation_requested(&root,&sessions[2].id));
        finish(&root,&sessions[1].id);
        assert_eq!(statuses_at(&root).len(),2);
        fs::remove_dir_all(root).unwrap();
    }
}
