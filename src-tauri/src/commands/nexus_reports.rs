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
// Record only selected launch settings: never dump the parent environment or registry.
pub(super) fn launch_context(game: &NexusGame, command: &std::process::Command) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t|t.as_secs()).unwrap_or(0);
    let local = std::process::Command::new("date").arg("--iso-8601=seconds").output().ok().filter(|o|o.status.success()).map(|o|String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(||"horário local indisponível".into());
    let _ = writeln!(out, "PELIGAMES — INÍCIO DA EXECUÇÃO {local} | Unix {stamp}");
    let _ = writeln!(out, "Launcher={} plataforma={} deploy={}\nPrograma={}\nDiretório de trabalho={}\nProton={}\nPrefixo={}",
        env!("CARGO_PKG_VERSION"), game.platform, serde_json::to_string(&game.deploy_method).unwrap_or_default(),
        command.get_program().to_string_lossy(), command.get_current_dir().unwrap_or(Path::new(".")).display(), game.proton, game.compat_data);
    for key in ["WINEDLLOVERRIDES", "WINEDEBUG", "SteamAppId", "STEAM_COMPAT_DATA_PATH", "WINEPREFIX", "PROTONPATH"] {
        let value = command.get_envs().find(|(k, _)| *k == key).map(|(_, v)|v.map(|v|v.to_string_lossy().into_owned()).unwrap_or_else(||"[removido]".into()))
            .unwrap_or_else(||std::env::var(key).unwrap_or_else(|_|"[não definido]".into()));
        let _ = writeln!(out, "{key}={value}");
    }
    let root = Path::new(game.game["directory"].as_str().unwrap_or(""));
    if let Some(module) = super::nexus_deployment::definition(game) {
        let _ = writeln!(out, "Módulo={} versão={}", module.domain, module.version);
        for framework in &module.frameworks {
            let _ = writeln!(out, "Framework {} — presença de arquivos (não comprova carregamento):", framework.name);
            for path in framework.required_files.iter().chain(&framework.any_files) {
                let file = root.join(path);
                let _ = writeln!(out, "  {}: existe={} link={} destino={}",path,file.is_file(),file.is_symlink(),
                    fs::canonicalize(&file).map(|p|p.display().to_string()).unwrap_or_else(|e|e.to_string()));
            }
        }
        for path in &module.allowed_files {
            let _ = writeln!(out, "Arquivo do módulo {path}: existe={}", root.join(path).is_file());
        }
    }
    let mut observations = 0;
    let ue4ss_mods = super::nexus_deployment::definition(game).and_then(|module| module.ue4ss()).map(|layout| layout.mod_folders()[0].clone());
    for folder in ["r6/tweaks", "r6/scripts", "BepInEx/plugins", "Mods"].into_iter().chain(ue4ss_mods.as_deref()) {
        for entry in walkdir::WalkDir::new(root.join(folder)).max_depth(4).follow_links(false).into_iter().take(2000).filter_map(Result::ok) {
            let path = entry.path();
            let special = !path.strip_prefix(root).unwrap_or(path).to_string_lossy().is_ascii();
            let broken = entry.file_type().is_symlink() && !path.exists();
            if (special || broken) && observations < 40 {
                let _ = writeln!(out,"Verificar caminho {}: Unicode={} link_quebrado={} (observação, não comprova falha)",path.strip_prefix(root).unwrap_or(path).display(),special,broken);
                observations += 1;
            }
        }
    }
    let runner_version = Path::new(&game.proton).join("version");
    if fs::metadata(&runner_version).is_ok_and(|m|m.len() <= 4096) {
        if let Ok(version) = fs::read_to_string(runner_version) {let _ = writeln!(out,"Versão informada pelo Proton: {}",version.trim());}
    }
    let prefix = Path::new(&game.compat_data);
    let prefix = if prefix.join("pfx").is_dir() {prefix.join("pfx")} else {prefix.to_path_buf()};
    for name in ["msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"] {
        let path = prefix.join("drive_c/windows/system32").join(name);
        let version = pe_version(&path).map(|v|format!("{}.{}.{}.{}",v[0],v[1],v[2],v[3])).unwrap_or_else(||"indisponível".into());
        let _ = writeln!(out, "Runtime x64 {name}: presente={} versão={version}",path.is_file());
    }
    // Only DLL override sections, with strict size/line limits. No credentials/settings dump.
    if let Ok(meta) = fs::metadata(prefix.join("user.reg")) {
        if meta.len() <= 8*1024*1024 {
            if let Ok(reg) = fs::read_to_string(prefix.join("user.reg")) {
                out.push_str("Overrides registrados no prefixo (podem ser sobrepostos pelo ambiente):\n");
                let exe = root.join(game.game["executable"].as_str().unwrap_or(""));
                let module = super::nexus_deployment::definition(game);
                let exe = module.as_ref().map(|m|m.executable.as_str()).unwrap_or(exe.to_str().unwrap_or(""));
                let exe = Path::new(exe).file_name().and_then(|p|p.to_str()).unwrap_or("");
                let mut selected = false;
                for line in reg.lines().take(100_000) {
                    if line.starts_with('[') {
                        selected = line.starts_with("[Software\\\\Wine\\\\DllOverrides]") || (!exe.is_empty() && line.starts_with(&format!("[Software\\\\Wine\\\\AppDefaults\\\\{exe}\\\\DllOverrides]")));
                        if selected { let _ = writeln!(out,"{}",line); }
                    } else if selected && line.starts_with('"') && line.len() < 256 {
                        let _ = writeln!(out,"{line}");
                    }
                }
            }
        }
    }
    sanitize(&out)
}
fn fixed_version(bytes: &[u8]) -> Option<[u16;4]> {
    let signature = [0xbd,0x04,0xef,0xfe,0x00,0x00,0x01,0x00];
    let index = bytes.windows(8).position(|part|part==signature)?;
    let v = bytes.get(index+8..index+16)?;
    let ms = u32::from_le_bytes(v[..4].try_into().ok()?);
    let ls = u32::from_le_bytes(v[4..].try_into().ok()?);
    Some([(ms>>16) as u16,ms as u16,(ls>>16) as u16,ls as u16])
}
fn pe_version(path: &Path) -> Option<[u16;4]> {
    let file = fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 16*1024*1024 {return None;}
    use std::io::Read;
    let mut bytes = Vec::new();file.take(16*1024*1024).read_to_end(&mut bytes).ok()?;
    if !bytes.starts_with(b"MZ") {return None;}
    fixed_version(&bytes)
}
fn framework_logs(game: &NexusGame) -> String {
    sanitize(&super::nexus_loader_logs::read(game).unwrap_or_else(|e| format!("Falha ao ler logs dos mods: {e}")))
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
    let mut content = format!("PELIGAMES — RELATÓRIO NEXUS\nData Unix: {}\nJogo: {}\nLauncher: {} (backend {})\nVersão: {}\nSistema: {} / {}\n{}\nKernel: {}\nPlataforma: {}\nProton: {}\nDeploy: {}\nMódulo: {}\nPasta do jogo: {}\nPrefixo: {}\n\nDescrição:\n{}\n\nMODS\n",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t|t.as_secs()).unwrap_or(0),game.game["name"].as_str().unwrap_or(""),build_label.chars().take(64).collect::<String>(),env!("CARGO_PKG_VERSION"),super::official_build::report_label(),std::env::consts::OS,std::env::consts::ARCH,distro,kernel,game.platform,game.proton,serde_json::to_string(&game.deploy_method).unwrap_or_default(),module.map(|m|format!("{} {}",m.domain,m.version)).unwrap_or_else(||game.adapter.clone()),game.game["directory"].as_str().unwrap_or(""),game.compat_data,description.chars().take(4000).collect::<String>());
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
    content.push_str("\n--- LOGS DOS FRAMEWORKS (data comparada com a última execução) ---\n");
    content.push_str(&framework_logs(&game));
    content.push_str(&super::nexus_crash_logs::read(&game));
    content = sanitize(&content);let truncated = limited(&mut content,REPORT_LIMIT);
    let temp=dir.join(format!("report-{}.tmp",uuid::Uuid::new_v4()));
    private_file(&temp,false)?.write_all(content.as_bytes()).map_err(|e|e.to_string())?;
    let path=dir.join("relatorio-nexus.txt");fs::rename(&temp,&path).map_err(|e|e.to_string())?;
    Ok(Report {path:path.to_string_lossy().into(),bytes:content.len(),content,truncated})
}
/// Report forum bridge (tools/report-relay): a public address, not a secret. Set
/// it here or with PELIGAMES_REPORT_RELAY at build time.
const DEFAULT_REPORT_RELAY: Option<&str> = Some("https://peligames-report.nairomarttins97.workers.dev");
/// Discord accepts webhook attachments of up to 10 MB.
const SEND_LIMIT: usize = 9_000_000;
fn report_relay() -> Option<&'static str> {
    option_env!("PELIGAMES_REPORT_RELAY").or(DEFAULT_REPORT_RELAY).filter(|url| url.starts_with("https://"))
}
#[tauri::command]
pub fn nexus_report_relay_configured() -> bool { report_relay().is_some() }
#[derive(serde::Deserialize)]
struct RelayReply { #[serde(default)] url: Option<String>, #[serde(default)] error: Option<String> }
/// Publishes the report as a new post of the Discord report forum, through the
/// bridge that keeps the webhook. Returns the post address when known.
#[tauri::command]
pub async fn send_nexus_report(game_id: String, build_label: String, title: String, description: String, discord: String) -> Result<Option<String>, String> {
    let relay = report_relay().ok_or("O envio direto não está configurado nesta versão. Use o canal do Discord.")?;
    let label = build_label.clone();
    let report = tauri::async_runtime::spawn_blocking(move || export_nexus_report(game_id, label, description.clone()).map(|report| (report, description)))
        .await.map_err(|e| e.to_string())??;
    let (report, description) = report;
    let mut content = report.content;
    limited(&mut content, SEND_LIMIT);
    let client = reqwest::Client::builder().user_agent("PeliGames").timeout(std::time::Duration::from_secs(90)).build().map_err(|e| e.to_string())?;
    let response = client.post(format!("{}/report", relay.trim_end_matches('/')))
        .json(&serde_json::json!({"title": title.chars().take(100).collect::<String>(), "description": description.chars().take(1500).collect::<String>(),
            "discord": discord.chars().take(40).collect::<String>(), "version": build_label.chars().take(64).collect::<String>(), "report": content}))
        .send().await.map_err(|_| "Não foi possível falar com o servidor de relatórios. Verifique a conexão.".to_string())?;
    let status = response.status();
    let reply: RelayReply = response.json().await.unwrap_or(RelayReply { url: None, error: None });
    if !status.is_success() { return Err(reply.error.unwrap_or_else(|| format!("O envio falhou ({status})."))); }
    Ok(reply.url.filter(|url| url.starts_with("https://discord.com/")))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn redacts_credentials_usernames_email_and_signed_urls() {
        let text=sanitize("api_key=private\nnxm://repo/mods/1?key=private\n/home/alice/Games/REPO\nuser@example.com\nhttps://cdn.example/file?token=private");
        assert!(!text.contains("private"));assert!(!text.contains("alice"));assert!(!text.contains("user@example.com"));assert!(text.contains("REPO"));
    }
    #[test] fn extracts_runtime_version_and_rejects_truncated_resource() {
        let mut resource=vec![0xbd,0x04,0xef,0xfe,0,0,1,0];
        resource.extend_from_slice(&((14u32<<16)|28).to_le_bytes());
        resource.extend_from_slice(&((29334u32<<16)|0).to_le_bytes());
        assert_eq!(fixed_version(&resource),Some([14,28,29334,0]));
        assert_eq!(fixed_version(&resource[..12]),None);
        assert_eq!(fixed_version(b"unrelated DLL bytes"),None);
    }
    #[test] fn bounds_report_on_utf8_boundary() {let mut text="á".repeat(1000);assert!(limited(&mut text,500));assert!(text.len()<=500);}
}
