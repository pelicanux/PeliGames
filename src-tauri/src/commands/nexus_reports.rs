//! Local per-game diagnostics. Reports are exported for manual sharing, never uploaded.
use super::nexus_local::NexusGame;
use std::{fs, io::Write, path::{Path, PathBuf}, sync::Mutex};
use serde::Serialize;
static LOCK: Mutex<()> = Mutex::new(());
const EVENT_LIMIT: u64 = 1024 * 1024;
const REPORT_LIMIT: usize = 19_000_000;

pub(super) fn sanitize(text: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    text.lines().map(|line| {
        let lower = line.to_ascii_lowercase();
        if ["api_key", "apikey", "authorization:", "authorization=", "access_token", "refresh_token", "password", "passwd", "secret=", "nxm://", "signature=", "key="].iter().any(|key| lower.contains(key)) {
            return "[linha com credenciais removida]".to_string();
        }
        let mut line = line.to_string();
        if !home.is_empty() { line = line.replace(&home, "~"); }
        for prefix in ["/home/", "/Users/", "drive_c/users/"] {
            let mut offset = 0;
            while let Some(index) = line[offset..].find(prefix) {
                let start = offset + index + prefix.len();
                let end = line[start..].find(|c: char| c == '/' || c.is_whitespace() || c == '"').map(|n| start+n).unwrap_or(line.len());
                line.replace_range(start..end, "[usuario]"); offset = start + "[usuario]".len();
            }
        }
        line.split_whitespace().map(|word| {
            if word.contains('@') && word.contains('.') { "[email removido]".to_string() }
            else if word.contains("http") && word.contains('?') { format!("{}?[parametros removidos]", word.split('?').next().unwrap_or("")) }
            else { word.to_string() }
        }).collect::<Vec<_>>().join(" ")
    }).collect::<Vec<_>>().join("\n")
}
fn directory(game: &NexusGame) -> Result<PathBuf,String> {
    uuid::Uuid::parse_str(&game.id).map_err(|_| "Identificador de jogo inválido.")?;
    let name = game.game["name"].as_str().unwrap_or("jogo");
    let slug: String = name.chars().take(48).map(|c| if c.is_ascii_alphanumeric() {c.to_ascii_lowercase()} else {'-'}).collect();
    let root = crate::core::paths::app_root()?;
    fs::create_dir_all(&root).map_err(|e|e.to_string())?;
    let root = fs::canonicalize(root).map_err(|e|e.to_string())?;
    let mut dir = root.clone();
    for component in ["Logs".to_string(),"Nexus".to_string(),format!("{slug}-{}",game.id),"erro".to_string()] {
        dir.push(component);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {},
            Ok(_) => return Err("Pasta de logs inválida ou simbólica.".into()),
            Err(error) if error.kind()==std::io::ErrorKind::NotFound => {fs::create_dir(&dir).map_err(|e|e.to_string())?;},
            Err(error) => return Err(error.to_string()),
        }
    }
    if !fs::canonicalize(&dir).map_err(|e|e.to_string())?.starts_with(&root) {return Err("Pasta de logs externa recusada.".into());}
    Ok(dir)
}
fn private_file(path: &Path, append: bool) -> Result<fs::File,String> {
    let mut options = fs::OpenOptions::new(); options.create(true).write(true).append(append).truncate(!append);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600).custom_flags(libc::O_NOFOLLOW);}
    options.open(path).map_err(|e|e.to_string())
}
pub(super) fn event(game: &NexusGame, level: &str, action: &str, detail: &str) {
    let _guard = LOCK.lock().unwrap_or_else(|e|e.into_inner());
    let _ = (|| -> Result<(),String> {
        let dir = directory(game)?;let path = dir.join("events.log");
        if fs::metadata(&path).is_ok_and(|m|m.len() >= EVENT_LIMIT) {fs::rename(&path,dir.join("events.previous.log")).map_err(|e|e.to_string())?;}
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t|t.as_secs()).unwrap_or(0);
        let detail = sanitize(detail).chars().take(4096).map(|c|if c.is_control(){' '}else{c}).collect::<String>();
        writeln!(private_file(&path,true)?,"[{stamp}] [{level}] {action}: {detail}").map_err(|e|e.to_string())
    })();
}
pub(super) fn outcome<T>(game: &NexusGame, action: &str, result: &Result<T,String>) {
    match result {Ok(_) => event(game,"INFO",action,"concluído"),Err(error) => event(game,"ERROR",action,error)}
}
#[derive(Serialize)]
pub struct Report {path: String, content: String, bytes: usize, truncated: bool}
fn limited(text: &mut String, limit: usize) -> bool {
    if text.len() <= limit {return false;}
    let mut end = limit.saturating_sub(100);while !text.is_char_boundary(end) {end-=1;}
    text.truncate(end);text.push_str("\n[Relatório reduzido para respeitar o limite de tamanho.]\n");true
}
#[tauri::command]
pub fn export_nexus_report(game_id: String, build_label: String, description: String) -> Result<Report,String> {
    let game = super::nexus_local::diagnostic_game(&game_id)?;
    let _guard = LOCK.lock().map_err(|e|e.to_string())?;
    let dir = directory(&game)?;
    let distro = fs::read_to_string("/etc/os-release").unwrap_or_default().lines().filter(|l|l.starts_with("PRETTY_NAME=") || l.starts_with("ID=") || l.starts_with("VERSION_ID=")).collect::<Vec<_>>().join("\n");
    let kernel = std::process::Command::new("uname").arg("-r").output().ok().filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let module = super::nexus_deployment::definition(&game);
    let mut content = format!("PELIGAMES — RELATÓRIO NEXUS\nData Unix: {}\nJogo: {}\nLauncher: {} (backend {})\nSistema: {} / {}\n{}\nKernel: {}\nPlataforma: {}\nProton: {}\nDeploy: {}\nMódulo: {}\nPasta do jogo: {}\nPrefixo: {}\n\nDescrição:\n{}\n\nMODS\n",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t|t.as_secs()).unwrap_or(0),game.game["name"].as_str().unwrap_or(""),build_label.chars().take(64).collect::<String>(),env!("CARGO_PKG_VERSION"),std::env::consts::OS,std::env::consts::ARCH,distro,kernel,game.platform,game.proton,serde_json::to_string(&game.deploy_method).unwrap_or_default(),module.map(|m|format!("{} {}",m.domain,m.version)).unwrap_or_else(||game.adapter.clone()),game.game["directory"].as_str().unwrap_or(""),game.compat_data,description.chars().take(4000).collect::<String>());
    for item in game.mods.iter().take(500) {
        content.push_str(&format!("{} | instalado={} | ativo={} | tamanho={} bytes",item.name,item.installed,item.enabled,item.size));
        if let Some(source)=&item.nexus_source {content.push_str(&format!(" | Nexus {}/mods/{} arquivo {}",source.domain,source.mod_id,source.file_id));}
        content.push('\n');
    }
    let game_root = Path::new(game.game["directory"].as_str().unwrap_or(""));
    for root in [game_root.join("Mods"),game_root.join("BepInEx/plugins")] {
        for entry in walkdir::WalkDir::new(&root).max_depth(5).into_iter().take(1000).filter_map(Result::ok).filter(|e|e.file_type().is_file() && e.file_name()=="manifest.json") {
            if entry.metadata().is_ok_and(|m|m.len() <= 64*1024) && fs::canonicalize(entry.path()).is_ok_and(|p|p.starts_with(&root)) {
                if let Ok(value)=fs::read(entry.path()).map_err(|e|e.to_string()).and_then(|b|serde_json::from_slice::<serde_json::Value>(&b).map_err(|e|e.to_string())) {
                    content.push_str(&format!("Manifesto: {} | versão: {}\n",value["Name"].as_str().or(value["name"].as_str()).unwrap_or("não informada"),value["Version"].as_str().or(value["version_number"].as_str()).unwrap_or("não informada")));
                }
            }
        }
    }
    for name in ["events.log","events.previous.log"] {
        content.push_str(&format!("\n--- {name} ---\n"));content.push_str(&super::nexus_profile::read_log_file(&dir.join(name)).unwrap_or_else(|e|format!("Log indisponível: {e}")));
    }
    for previous in [false,true] {
        content.push_str(if previous {"\n--- EXECUÇÃO ANTERIOR ---\n"} else {"\n--- EXECUÇÃO ATUAL ---\n"});
        content.push_str(&super::nexus_profile::logs(&game,previous).unwrap_or_else(|e|format!("Log indisponível: {e}")));
    }
    for path in ["BepInEx/LogOutput.log","reframework/log.txt"] {
        let file = game_root.join(path);
        if fs::canonicalize(&file).is_ok_and(|p|p.starts_with(game_root)) {content.push_str(&format!("\n--- {path} ---\n"));content.push_str(&super::nexus_profile::read_log_file(&file).unwrap_or_default());}
    }
    content = sanitize(&content);let truncated = limited(&mut content,REPORT_LIMIT);
    let temp=dir.join(format!("report-{}.tmp",uuid::Uuid::new_v4()));
    private_file(&temp,false)?.write_all(content.as_bytes()).map_err(|e|e.to_string())?;
    let path=dir.join("relatorio-nexus.txt");fs::rename(&temp,&path).map_err(|e|e.to_string())?;
    Ok(Report {path:path.to_string_lossy().into(),bytes:content.len(),content,truncated})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn redacts_credentials_usernames_email_and_signed_urls() {
        let text=sanitize("api_key=private\nnxm://repo/mods/1?key=private\n/home/alice/Games/REPO\nuser@example.com\nhttps://cdn.example/file?token=private");
        assert!(!text.contains("private"));assert!(!text.contains("alice"));assert!(!text.contains("user@example.com"));assert!(text.contains("REPO"));
    }
    #[test] fn bounds_report_on_utf8_boundary() {let mut text="á".repeat(1000);assert!(limited(&mut text,500));assert!(text.len()<=500);}
}
