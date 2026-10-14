//! Read-only preparation feedback for UMU processes launched by PeliGames/Pelinstall.
use serde::Serialize;
use std::{fs, io::{Read, Seek, SeekFrom}, path::Path};
#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct RuntimeActivity { pub id:String, pub runtime:String, pub phase:String, pub downloaded_bytes:Option<u64> }
fn phase_from_log(text: &str) -> Option<(String,String)> {
    let mut activity=None;
    for line in text.lines() {
        let runtime=line.split_whitespace().find(|part| part.starts_with("steamrt") || *part=="sniper" || *part=="soldier")
            .map(|part|part.trim_matches(|c:char| !c.is_ascii_alphanumeric()).to_string());
        if line.contains("umu-launcher version") {
            activity=Some(("Steam Runtime".into(),"preparing".into()));
        } else if line.contains("Downloading ") {
            if let Some(runtime)=runtime { activity=Some((runtime,"downloading".into())); }
        } else if line.contains("Extracting ") || line.contains("Unpacking ") {
            if let Some((name,_))=&activity { activity=Some((name.clone(),"extracting".into())); }
        } else if line.contains("Updating steamrt") || line.contains("Restoring Runtime Platform") {
            activity=Some((runtime.unwrap_or_else(||"Steam Runtime".into()),"preparing".into()));
        } else if line.contains("is up to date") || line.contains("Running '") || line.contains("ProtonFixes[") {
            activity=None;
        } else if activity.is_some() && ["Traceback", "ReadTimeoutError", "NameResolutionError", "MaxRetryError", "HTTPError", " returned the status:"].iter().any(|needle|line.contains(needle)) {
            if let Some((name,_))=&activity { activity=Some((name.clone(),"failed".into())); }
        }
    }
    activity
}
pub(crate) fn failure_hint(path: &Path) -> Option<String> {
    let text=log_tail(path)?;
    let (runtime,phase)=phase_from_log(&text)?;
    (phase=="failed").then(||format!("Não foi possível preparar {runtime}. Verifique a conexão e tente novamente. Se o runtime ainda não estiver instalado, conecte-se à internet para concluir o primeiro download."))
}
fn log_tail(path: &Path) -> Option<String> {
    let mut file=fs::File::open(path).ok()?;
    let len=file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(32768))).ok()?;
    let mut data=Vec::new(); file.take(32768).read_to_end(&mut data).ok()?;
    Some(String::from_utf8_lossy(&data).into_owned())
}
#[cfg(target_os="linux")]
fn parent(pid:u32) -> Option<u32> {
    let stat=fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()
}
#[cfg(target_os="linux")]
fn owned_by_launcher(mut pid:u32) -> bool {
    for _ in 0..16 {
        let Some(next)=parent(pid) else {return false;};
        if next<=1 || next==pid {return false;}
        let name=fs::read_to_string(format!("/proc/{next}/comm")).unwrap_or_default();
        if ["peligames","pelinstall"].contains(&name.trim()) {return true;}
        pid=next;
    }
    false
}
#[cfg(target_os="linux")]
fn activities() -> Vec<RuntimeActivity> {
    let Ok(entries)=fs::read_dir("/proc") else {return vec![];};
    let mut result=Vec::new();
    for entry in entries.flatten() {
        let Some(pid)=entry.file_name().to_str().and_then(|s|s.parse::<u32>().ok()) else {continue;};
        let dir=entry.path();
        let name=fs::read_to_string(dir.join("comm")).unwrap_or_default();
        if name.trim()!="umu-run" && !name.trim().starts_with("python") {continue;}
        // Only inspect the two executable arguments, never dump environment or logs.
        let Ok(cmd)=fs::read(dir.join("cmdline")) else {continue;};
        let is_umu=cmd.split(|b|*b==0).take(2).any(|arg| {
            let path=String::from_utf8_lossy(arg);
            Path::new(path.as_ref()).file_name().is_some_and(|name|name=="umu-run" || name=="umu_run.py")
        });
        if !is_umu || !owned_by_launcher(pid) {continue;}
        let Ok(log)=fs::read_link(dir.join("fd/1")) else {continue;};
        if !log.is_absolute() || !log.is_file() {continue;}
        let Some((runtime,mut phase))=log_tail(&log).and_then(|text|phase_from_log(&text)) else {continue;};
        let mut bytes=None;
        if let Ok(fds)=fs::read_dir(dir.join("fd")) {
            for fd in fds.flatten() {
                let Ok(path)=fs::read_link(fd.path()) else {continue;};
                let filename=path.file_name().unwrap_or_default().to_string_lossy();
                if filename.starts_with("SteamLinuxRuntime_") && filename.contains(".tar.") && filename.ends_with(".parts") {
                    if let Ok(meta)=fs::metadata(&path) { bytes=Some(bytes.unwrap_or(0u64).max(meta.len())); }
                }
                if filename.starts_with("SteamLinuxRuntime_") && filename.ends_with(".tar.xz") && phase=="downloading" {phase="extracting".into();}
            }
        }
        result.push(RuntimeActivity {id:format!("umu:{pid}"),runtime,phase,downloaded_bytes:bytes});
    }
    result.sort_by(|a,b|a.id.cmp(&b.id));result
}
#[cfg(not(target_os="linux"))] fn activities() -> Vec<RuntimeActivity> {vec![]}
#[tauri::command]
pub async fn get_runtime_activity() -> Result<Vec<RuntimeActivity>,String> {
    tauri::async_runtime::spawn_blocking(activities).await.map_err(|e|e.to_string())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn reports_download_without_claiming_game_is_running() {
        let log="INFO: Updating steamrt4 to 4.0.20261007\nINFO: Downloading steamrt4 (4.0.20261007), please wait...";
        assert_eq!(phase_from_log(log),Some(("steamrt4".into(),"downloading".into())));
        assert_eq!(phase_from_log(&format!("{log}\nINFO: Running 'Proton' using runtime 'steamrt4'")),None);
    }
    #[test] fn distinguishes_extraction_and_connection_errors() {
        let log="INFO: Downloading steamrt4, please wait...";
        assert_eq!(phase_from_log(&format!("{log}\nINFO: Extracting archive...")),Some(("steamrt4".into(),"extracting".into())));
        assert_eq!(phase_from_log(&format!("{log}\nNameResolutionError: unavailable")),Some(("steamrt4".into(),"failed".into())));
        assert_eq!(phase_from_log("INFO: steamrt4 is up to date"),None);
        assert_eq!(phase_from_log("INFO: umu-launcher version 1.4.4\nNameResolutionError: offline"),Some(("Steam Runtime".into(),"failed".into())));
    }
}
