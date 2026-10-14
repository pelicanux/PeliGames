//! Only bundled, reviewed recipes are executable. Requests never supply URLs or commands.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::{Path, PathBuf}, process::Command, sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}};
use tauri::Emitter;
static CANCEL: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
struct CancelGuard;
impl Drop for CancelGuard { fn drop(&mut self) { if let Ok(mut slot)=CANCEL.lock() { *slot=None; } } }
#[derive(Deserialize)]
pub struct Request { pub id: String, pub directory: String, pub proton: String, #[serde(default)] pub dependencies: Vec<String> }
fn recipe(id: &str) -> Result<serde_json::Value, String> {
    let raw = match id {
        "ubisoft-connect" => include_str!("../../../installers/recipes/ubisoft-connect.json"),
        "ea-app" => include_str!("../../../installers/recipes/ea-app.json"),
        "battle-net" => include_str!("../../../installers/recipes/battle-net.json"),
        _ => return Err("Launcher desconhecido.".into()),
    };
    let index: serde_json::Value = serde_json::from_str(include_str!("../../../installers/index.json")).map_err(|e| e.to_string())?;
    let expected = index["launchers"].as_array().and_then(|items| items.iter().find(|item| item["id"] == id)).and_then(|item| item["sha256"].as_str()).ok_or("Receita sem hash no catálogo.")?;
    if format!("{:x}", Sha256::digest(raw.as_bytes())) != expected { return Err("Receita inconsistente com o catálogo.".into()); }
    serde_json::from_str(raw).map_err(|e| e.to_string())
}
fn trusted_host(id: &str, url: &reqwest::Url) -> bool {
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.port().is_some_and(|p| p != 443) { return false; }
    let host=url.host_str().unwrap_or("");
    let under = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    match id {
        "ubisoft-connect" => host == "ubistatic3-a.akamaihd.net" || under("ubisoft.com"),
        "ea-app" => host == "origin-a.akamaihd.net" || under("ea.com") || under("origin.com"),
        "battle-net" => under("battle.net") || under("blizzard.com"),
        _ => false,
    }
}
fn normalized(value: &str) -> String { value.chars().filter(|c| c.is_ascii_alphanumeric()).flat_map(char::to_lowercase).collect() }
fn publisher_matches(id: &str, output: &str) -> bool {
    // -index 0 restricts verification to the same signature whose leaf is inspected.
    let section = match output.split("Signer's certificate:").nth(1) { Some(s) => s, None => return false };
    let subject = match section.lines().find_map(|line| line.trim().strip_prefix("Subject:")) { Some(s) => s, None => return false };
    let organization = if subject.contains("/O=") {
        subject.split("/O=").nth(1).and_then(|s|s.split('/').next()).map(normalized).unwrap_or_default()
    } else {
        // Current osslsigncode prints RFC2253 names; commas in O are escaped.
        let mut attributes=Vec::new(); let mut field=String::new(); let mut escaped=false;
        for c in subject.trim().chars() {
            if escaped { field.push(c); escaped=false; }
            else if c=='\\' { escaped=true; }
            else if c==',' || c=='+' { attributes.push(std::mem::take(&mut field)); }
            else { field.push(c); }
        }
        attributes.push(field);
        let organizations: Vec<_> = attributes.iter().filter_map(|field|field.trim().strip_prefix("O=")).collect();
        if organizations.len()!=1 { return false; }
        normalized(organizations[0])
    };
    let expected: &[&str] = match id {
        "ubisoft-connect" => &["Ubisoft Entertainment", "Ubisoft Entertainment SA"],
        "ea-app" => &["Electronic Arts Inc.", "Electronic Arts, Inc."],
        "battle-net" => &["Blizzard Entertainment, Inc.", "Blizzard Entertainment Inc."],
        _ => &[],
    };
    expected.iter().any(|name| organization == normalized(name))
}
fn verify_signature(id: &str, path: &Path) -> Result<(), String> {
    let ca = ["/etc/ssl/certs/ca-certificates.crt", "/etc/pki/tls/certs/ca-bundle.crt", "/etc/ssl/ca-bundle.pem"].into_iter().find(|p| Path::new(p).is_file()).ok_or("Pacote de certificados CA não encontrado no sistema.")?;
    let output = Command::new("osslsigncode").args(["verify", "-index", "0", "-CAfile", ca, "-TSA-CAfile", ca, "-in"]).arg(path).output().map_err(|e| format!("Instale osslsigncode para verificar a assinatura do instalador: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || !publisher_matches(id, &text) { return Err("Não foi possível confirmar a assinatura e o fornecedor do instalador. Nenhum executável foi iniciado.".into()); }
    Ok(())
}
#[tauri::command]
pub fn cancel_launcher_installation() -> Result<(), String> {
    let slot=CANCEL.lock().map_err(|e|e.to_string())?;
    let cancel=slot.as_ref().ok_or("Nenhuma instalação de launcher está ativa.")?;
    cancel.store(true, Ordering::Release);
    let _ = crate::core::game_installation::cancel_installation();
    Ok(())
}
#[tauri::command]
pub async fn install_launcher(app: tauri::AppHandle, request: Request) -> Result<crate::core::game_installation::InstallationResult, String> {
    let _busy = super::game_installation::acquire_installer_busy()?;
    let cancel=Arc::new(AtomicBool::new(false));
    *CANCEL.lock().map_err(|e|e.to_string())?=Some(cancel.clone());
    let _cancel_guard=CancelGuard;
    #[cfg(unix)] if unsafe { libc::geteuid() } == 0 { return Err("Execute a instalação como usuário comum, sem root.".into()); }
    if crate::core::game_execution::active() { return Err("Encerre o jogo ou programa antes de instalar.".into()); }
    let data = recipe(&request.id)?;
    if request.dependencies.len() > 8 || request.dependencies.iter().any(|verb| !data["dependencies"].as_array().is_some_and(|items| items.iter().any(|item| item["verb"] == *verb))) { return Err("Componente não autorizado pela receita.".into()); }
    crate::core::game_installation::validate_proton(&request.proton)?;
    // Preflight before downloading or creating the destination.
    Command::new("osslsigncode").arg("--version").output().map_err(|_| "Instale osslsigncode para verificar os instaladores oficiais.")?;
    // Reuse the game's automatic artwork lookup. Missing artwork/network never
    // prevents installation: each launcher also ships a local portrait cover.
    let cover_name=data["name"].as_str().ok_or("Receita inválida.")?.to_string();
    let cover=tokio::time::timeout(std::time::Duration::from_secs(12), super::scanner::fetch_steamgriddb_cover_command(cover_name, String::new())).await.ok().and_then(Result::ok).flatten()
        .unwrap_or_else(||format!("/launchers/{}-cover.svg",request.id));
    if cancel.load(Ordering::Acquire) { return Err("Instalação cancelada.".into()); }
    let directory = crate::core::game_installation::absolute(&request.directory)?;
    let parent = directory.parent().ok_or("Destino inválido.")?.canonicalize().map_err(|_| "Escolha uma pasta pai existente.")?;
    let directory = parent.join(directory.file_name().ok_or("Destino inválido.")?);
    fs::create_dir(&directory).map_err(|e| format!("Escolha uma pasta nova e vazia. O destino existente não será alterado: {e}"))?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?; }
    let filename = data["download"]["filename"].as_str().ok_or("Receita inválida.")?;
    let target = directory.join(filename);
    let id = request.id.clone();
    let url = reqwest::Url::parse(data["download"]["url"].as_str().ok_or("Receita inválida.")?).map_err(|e|e.to_string())?;
    if !trusted_host(&id, &url) { return Err("Origem do instalador não autorizada.".into()); }
    let policy_id = id.clone();
    let client = reqwest::Client::builder().user_agent(concat!("PeliGames/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(900))
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 || !trusted_host(&policy_id, attempt.url()) { attempt.stop() } else { attempt.follow() }
        })).build().map_err(|e|e.to_string())?;
    let mut response = client.get(url).send().await.map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?;
    if response.status().is_redirection() || !trusted_host(&id, response.url()) { return Err("Redirecionamento do instalador não autorizado.".into()); }
    const LIMIT: u64 = 512 * 1024 * 1024;
    if response.content_length().is_some_and(|n| n > LIMIT) { return Err("Instalador excede o tamanho permitido.".into()); }
    let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|e|e.to_string())?;
    let mut size = 0u64; let mut hash = Sha256::new();
    while let Some(chunk) = response.chunk().await.map_err(|e|e.to_string())? {
        if cancel.load(Ordering::Acquire) { return Err("Download cancelado. Nenhum executável foi iniciado.".into()); }
        size += chunk.len() as u64;
        if size > LIMIT { return Err("Instalador excede o tamanho permitido.".into()); }
        hash.update(&chunk); file.write_all(&chunk).map_err(|e|e.to_string())?;
    }
    file.sync_all().map_err(|e|e.to_string())?; drop(file);
    crate::core::game_installation::validate_executable(&target.to_string_lossy())?;
    let audit = serde_json::json!({"launcher":id,"url":response.url().as_str(),"sha256":format!("{:x}",hash.finalize()),"bytes":size,"proton":request.proton});
    fs::write(directory.join("installer-download.json"), serde_json::to_vec_pretty(&audit).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    let paths = data["executable"]["relative_paths"].as_array().ok_or("Receita sem executável.")?.iter().filter_map(|p|p.as_str().map(str::to_string)).collect();
    let installation = crate::core::game_installation::InstallationRequest { name:data["name"].as_str().ok_or("Receita inválida.")?.into(), directory:directory.to_string_lossy().into_owned(), executable:target.to_string_lossy().into_owned(), proton:request.proton, cover_url:Some(cover) };
    let plan = crate::core::game_installation::LauncherInstallPlan { id:request.id, executable_paths:paths, dependencies:request.dependencies, cancel:cancel.clone() };
    let result = tauri::async_runtime::spawn_blocking(move || {
        verify_signature(&id, &target)?;
        if cancel.load(Ordering::Acquire) { return Err("Instalação cancelada.".into()); }
        crate::core::game_installation::run_launcher(installation, plan)
    }).await.map_err(|e|e.to_string())??;
    let _ = app.emit("peligames-library-changed", ());
    Ok(result)
}
pub fn category(entry: &crate::core::installed_library::InstalledEntry) -> bool {
    if entry.shortcut.is_some() { return false; }
    let marker=PathBuf::from(&entry.prefix).join(".peligames-launcher.json");
    fs::read(marker).ok().and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok()).is_some_and(|data| data["executable"].as_str() == Some(entry.executable.as_str()))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn rejects_foreign_downloads() {
        for url in ["http://ea.com/x", "https://ea.com.evil.test/x", "https://localhost/x", "https://user@ea.com/x"] { assert!(!trusted_host("ea-app", &reqwest::Url::parse(url).unwrap())); }
        assert!(trusted_host("ea-app", &reqwest::Url::parse("https://origin-a.akamaihd.net/x").unwrap()));
    }
    #[test] fn recipes_are_pinned() { for id in ["ubisoft-connect","ea-app","battle-net"] { assert!(recipe(id).is_ok(),"{id}"); } assert!(recipe("gog-galaxy").is_err()); assert!(recipe("epic-games").is_err()); }
    #[test] fn category_only_marks_the_launcher_executable() {
        let prefix=std::env::temp_dir().join(format!("peligames-launcher-test-{}",uuid::Uuid::new_v4()));
        fs::create_dir(&prefix).unwrap();
        let executable=prefix.join("Launcher.exe").to_string_lossy().into_owned();
        let mut entry:crate::core::installed_library::InstalledEntry=serde_json::from_value(serde_json::json!({
            "name":"Test launcher","path":executable,"directory":prefix,"prefix":prefix,
            "executable":executable,"proton":"/tmp/runner","launcher":"PeliGames","cover_url":null
        })).unwrap();
        assert!(!category(&entry));
        fs::write(prefix.join(".peligames-launcher.json"),serde_json::to_vec(&serde_json::json!({"executable":executable})).unwrap()).unwrap();
        assert!(category(&entry));
        entry.shortcut=Some("game-shortcut".into()); assert!(!category(&entry));
        entry.shortcut=None; entry.executable=prefix.join("Game.exe").to_string_lossy().into_owned(); assert!(!category(&entry));
        fs::remove_dir_all(prefix).unwrap();
    }
    #[test] fn binds_leaf_publisher() {
        assert!(publisher_matches("ea-app", "Signer's certificate:\n Subject: /C=US/O=Electronic Arts, Inc./CN=Electronic Arts, Inc.\n"));
        assert!(publisher_matches("ea-app", "Signer's certificate:\n Subject: CN=Electronic Arts\\, Inc.,O=Electronic Arts\\, Inc.,C=US\n"));
        assert!(!publisher_matches("ea-app", "Signer's certificate:\n Subject: /O=Attacker/CN=Electronic Arts, Inc.\n"));
    }
}
