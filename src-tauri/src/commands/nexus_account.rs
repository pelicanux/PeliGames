//! Personal-key connection for local testing. Secrets never enter config files.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
static REQUEST_TIMING: tokio::sync::Mutex<Option<Instant>> = tokio::sync::Mutex::const_new(None);
static COOLDOWN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn epoch() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
pub(super) async fn before_request() -> Result<(), String> {
    if COOLDOWN.load(std::sync::atomic::Ordering::Relaxed) > epoch() {
        return Err("Limite de consultas Nexus atingido. Aguarde antes de tentar novamente.".into());
    }
    let mut previous = REQUEST_TIMING.lock().await;
    if let Some(last) = *previous { let delay = Duration::from_millis(200).saturating_sub(last.elapsed()); tokio::time::sleep(delay).await; }
    *previous = Some(Instant::now());
    Ok(())
}
pub(super) fn observe_response(response: &reqwest::Response) {
    let headers = response.headers();
    let exhausted = ["x-rl-hourly-remaining", "x-rl-daily-remaining"].iter().any(|key| headers.get(*key).and_then(|v| v.to_str().ok()) == Some("0"));
    if response.status().as_u16() == 429 || exhausted {
        let delay = headers.get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()).unwrap_or(60).max(1);
        COOLDOWN.fetch_max(epoch().saturating_add(delay), std::sync::atomic::Ordering::Relaxed);
    }
}
static ACCOUNT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

// Dropping a download future must not drop the lock halfway through token
// rotation. The worker owns both the operation and its lock until persistence
// finishes; logout and account changes wait for that same worker.
pub(super) async fn with_account<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, String>> + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let _lock = ACCOUNT_LOCK.lock().await;
        operation.await
    })
    .await
    .map_err(|_| "Não foi possível concluir a operação da conta Nexus.".to_string())?
}

#[derive(Serialize, Deserialize)]
pub struct NexusAccount {
    pub user_id: u64,
    pub name: String,
    #[serde(default)]
    pub is_premium: bool,
    #[serde(default)]
    pub is_supporter: bool,
}

// Pass secrets on stdin, never as process arguments. Only our own item is queried.
fn credential(action: &str, key: Option<&str>) -> Result<Option<String>, String> {
    credential_for(action, key, "nexusmods-personal-api", "PeliGames — Nexus Mods")
}
pub(super) fn credential_for(action: &str, key: Option<&str>, service: &str, label: &str) -> Result<Option<String>, String> {
    let mut command = Command::new("secret-tool");
    command.arg(action);
    // The system helper must use system libraries even when launched from AppImage.
    command
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD");
    if action == "store" {
        command.arg(format!("--label={label}"));
    }
    command
        .args([
            "application",
            "com.pelicano.peligames",
            "service",
            service,
        ])
        .stdin(if key.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|_| "Não foi possível acessar o chaveiro. Instale secret-tool (libsecret-tools no Debian/Ubuntu ou libsecret no Fedora) e habilite um serviço de chaveiro compatível.")?;
    if let Some(key) = key {
        let written = child
            .stdin
            .take()
            .ok_or("Chaveiro indisponível.")?
            .write_all(key.as_bytes());
        if written.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Não foi possível enviar a chave ao chaveiro.".into());
        }
    }
    let stdout = child.stdout.take().ok_or("Chaveiro indisponível.")?;
    let output_task = std::thread::spawn(move || {
        let mut output = String::new();
        stdout.take(65537).read_to_string(&mut output).map(|_| output)
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < Duration::from_secs(60) => {
                std::thread::sleep(Duration::from_millis(50))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "O chaveiro não respondeu. Desbloqueie o chaveiro e tente novamente.".into(),
                );
            }
        }
    };
    let output = output_task.join().map_err(|_| "Não foi possível ler o chaveiro.")?
        .map_err(|_| "Não foi possível ler o chaveiro.")?;
    if output.len() > 65536 { return Err("Credencial excedeu o limite do chaveiro.".into()); }
    let mut diagnostic = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let _ = stderr.take(4096).read_to_end(&mut diagnostic);
    }
    credential_result(action, status.code(), output, &diagnostic)
}
fn credential_result(action: &str, code: Option<i32>, output: String, diagnostic: &[u8]) -> Result<Option<String>, String> {
    if code != Some(0) {
        // Both lookup and clear return 1 without diagnostics for a missing item.
        // Clearing an already absent credential is successful (idempotent).
        if matches!(action, "lookup" | "clear")
            && code == Some(1)
            && output.is_empty()
            && diagnostic.is_empty()
        {
            return Ok(None);
        }
        return Err("Não foi possível concluir a operação no chaveiro. Verifique se ele está disponível e desbloqueado.".into());
    }
    if action == "lookup" {
        Ok(Some(output.trim().to_string()))
    } else {
        Ok(None)
    }
}
async fn stored(action: &'static str, key: Option<String>) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || credential(action, key.as_deref()))
        .await
        .map_err(|_| "Falha ao acessar o chaveiro.".to_string())?
}
fn checked_key(key: &str) -> Result<&str, String> {
    let key = key.trim();
    if key.is_empty() || key.len() > 4096 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err("Informe uma chave pessoal Nexus válida, sem espaços internos.".into());
    }
    Ok(key)
}
async fn validate(key: &str) -> Result<NexusAccount, String> {
    let mut header =
        reqwest::header::HeaderValue::from_str(checked_key(key)?).map_err(|_| "Chave inválida.")?;
    header.set_sensitive(true);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Não foi possível iniciar a conexão Nexus.")?;
    before_request().await?;
    let response = client
        .get("https://api.nexusmods.com/v1/users/validate.json")
        .header(reqwest::header::USER_AGENT, concat!("PeliGames/", env!("CARGO_PKG_VERSION")))
        .header("apikey", header)
        .header("Application-Name", "PeliGames")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|_| {
            "Não foi possível conectar ao Nexus Mods. Verifique sua conexão e tente novamente."
        })?;
    observe_response(&response);
    match response.status().as_u16() {
        200 => {}
        401 | 403 => return Err(
            "A chave foi recusada pelo Nexus Mods. Confira se ela está correta e não foi revogada."
                .into(),
        ),
        429 => {
            return Err(
                "Limite de consultas Nexus atingido. Aguarde antes de tentar novamente.".into(),
            )
        }
        _ => {
            return Err(
                "O Nexus Mods não conseguiu validar a conta. Tente novamente mais tarde.".into(),
            )
        }
    }
    // The validation response may contain the API key and email. Deserialize only
    // the account summary; never return the raw response to the webview or logs.
    let account: NexusAccount = response
        .json()
        .await
        .map_err(|_| "Resposta de conta Nexus inválida.")?;
    if account.user_id == 0 || account.name.is_empty() {
        return Err("Resposta de conta Nexus incompleta.".into());
    }
    Ok(account)
}
#[tauri::command]
pub async fn connect_nexus_account(api_key: String) -> Result<NexusAccount, String> {
    super::nexus_oauth::cancel_nexus_oauth().await;
    with_account(async move {
        let key = checked_key(&api_key)?.to_string();
        let account = validate(&key).await?;
        stored("store", Some(key)).await?;
        super::nexus_oauth::clear().await?;
        Ok(account)
    }).await
}
#[tauri::command]
pub async fn load_nexus_account() -> Result<Option<NexusAccount>, String> {
    with_account(async {
        if let Some(account) = super::nexus_oauth::load_account().await? {
            return Ok(Some(account));
        }
        match stored("lookup", None).await? {
            Some(key) => validate(&key).await.map(Some),
            None => Ok(None),
        }
    }).await
}
#[tauri::command]
pub async fn disconnect_nexus_account() -> Result<(), String> {
    super::nexus_oauth::cancel_nexus_oauth().await;
    with_account(async {
        // Try both removals even when one service reports an actual failure.
        let oauth = super::nexus_oauth::clear().await;
        let personal = stored("clear", None).await;
        oauth?;
        personal?;
        Ok(())
    }).await
}

// Internal only: callers select fixed metadata endpoints and return typed summaries.
/// GraphQL v2 request with the user's key; GraphQL errors are refused.
pub(super) async fn graphql(body: serde_json::Value) -> Result<serde_json::Value, String> {
    let data = request_metadata("https://api.nexusmods.com/v2/graphql", Some(body)).await?;
    if data.get("errors").is_some_and(|value| value.as_array().is_none_or(|errors| !errors.is_empty())) {
        return Err("O Nexus recusou a consulta.".into());
    }
    Ok(data)
}
/// Authenticated GET on an api.nexusmods.com URL.
pub(super) async fn api(url: &str) -> Result<serde_json::Value, String> {
    if !url.starts_with("https://api.nexusmods.com/") { return Err("Endereço Nexus inválido.".into()); }
    request_metadata(url, None).await
}
pub(super) async fn metadata(path: &str) -> Result<serde_json::Value, String> {
    request_metadata(&format!("https://api.nexusmods.com/v1/{path}"), None).await
}
pub(super) async fn catalog_page(domain: &str, offset: u32, count: u32, most_downloaded: bool, search: &str) -> Result<serde_json::Value, String> {
    let sort = if most_downloaded {
        serde_json::json!([{ "downloads": { "direction": "DESC" } }])
    } else {
        // A deterministic default order makes pagination stable; no category filter.
        serde_json::json!([{ "createdAt": { "direction": "DESC" } }])
    };
    let body = catalog_request(domain, offset, count, sort, search);
    let data = request_metadata("https://api.nexusmods.com/v2/graphql", Some(body)).await?;
    if data.get("errors").is_some_and(|value| value.as_array().is_none_or(|errors| !errors.is_empty())) {
        return Err("Não foi possível consultar o catálogo completo Nexus.".into());
    }
    data.pointer("/data/mods").filter(|value| value.is_object()).cloned()
        .ok_or_else(|| "Catálogo Nexus indisponível.".into())
}
/// `supportsVortex` of each mod: the Nexus page offers the mod manager download.
pub(super) async fn vortex_support(domain: &str, ids: &[u64]) -> Result<std::collections::HashMap<u64, bool>, String> {
    let mut support = std::collections::HashMap::new();
    for chunk in ids.chunks(100) {
        let keys: Vec<_> = chunk.iter().map(|id| serde_json::json!({"gameDomain": domain, "modId": id})).collect();
        let data = graphql(serde_json::json!({"query": "query PeliGamesVortex($ids: [CompositeDomainWithIdInput!]!) { legacyModsByDomain(ids: $ids) { nodes { modId supportsVortex } } }", "variables": serde_json::json!({"ids": keys})})).await?;
        for node in data.pointer("/data/legacyModsByDomain/nodes").and_then(|v| v.as_array()).ok_or("Resposta Nexus inválida.")? {
            if let (Some(id), Some(flag)) = (node["modId"].as_u64(), node["supportsVortex"].as_bool()) { support.insert(id, flag); }
        }
    }
    Ok(support)
}
/// `supportsVortex` of one mod. The per-file `manager` flag is not used: current
/// main files of SMAPI, Jotunn and CET report 0 although the site offers the
/// mod manager download for them.
pub(super) async fn supports_vortex(domain: &str, mod_id: u64) -> Result<bool, String> {
    vortex_support(domain, &[mod_id]).await?.get(&mod_id).copied().ok_or_else(|| "Mod não encontrado no Nexus.".into())
}
fn catalog_request(domain: &str, offset: u32, count: u32, sort: serde_json::Value, search: &str) -> serde_json::Value {
    // Only mods installable by a mod manager (the Nexus "Vortex" button).
    let mut filter = serde_json::json!({"op":"AND", "gameDomainName":[{"value":domain,"op":"EQUALS"}], "supportsVortex":[{"value":true,"op":"EQUALS"}]});
    if !search.is_empty() {
        filter["nameStemmed"] = serde_json::json!([{"value":search,"op":"MATCHES"}]);
    }
    serde_json::json!({
        "query": "query PeliGamesCatalog($filter: ModsFilter!, $offset: Int!, $count: Int!, $sort: [ModsSort!]) { mods(filter: $filter, sort: $sort, count: $count, offset: $offset) { totalCount nodes { mod_id: modId name summary picture_url: pictureUrl author version downloads } } }",
        "variables": {"filter":filter,"offset":offset,"count":count,"sort":sort}
    })
}
#[cfg(test)]
mod catalog_search_tests {
    use super::*;
    #[test]
    fn search_preserves_game_scope_and_server_pagination() {
        let body = catalog_request("cyberpunk2077", 40, 20, serde_json::json!([]), "Cyber Engine");
        let vars = &body["variables"];
        assert_eq!(vars["filter"]["op"], "AND");
        assert_eq!(vars["filter"]["gameDomainName"][0]["value"], "cyberpunk2077");
        assert_eq!(vars["filter"]["nameStemmed"][0]["value"], "Cyber Engine");
        assert_eq!(vars["filter"]["nameStemmed"][0]["op"], "MATCHES");
        assert_eq!(vars["filter"]["supportsVortex"][0]["value"], true, "só mods com download por gerenciador");
        assert_eq!(vars["offset"], 40); assert_eq!(vars["count"], 20);
        let body = catalog_request("palworld", 0, 30, serde_json::json!([]), "");
        assert!(body["variables"]["filter"].get("nameStemmed").is_none());
    }
}
pub(super) async fn mod_requirements(
    game_id: u64,
    mod_id: u64,
) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({
        "query": "query modRequirements($modId: ID!, $gameId: ID!) { mod(modId: $modId, gameId: $gameId) { legacyModRequirementsEnabled modRequirements { nexusRequirements(count: 100) { nodes { externalRequirement gameId id modId modName notes url } totalCount } dlcRequirements { gameExpansion { name } notes } } } }",
        "variables": { "modId": mod_id.to_string(), "gameId": game_id.to_string() }
    });
    let data = request_metadata("https://api.nexusmods.com/v2/graphql", Some(body)).await?;
    if data
        .get("errors")
        .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
    {
        return Err("Não foi possível consultar os requisitos Nexus.".into());
    }
    data.pointer("/data/mod")
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| "Requisitos Nexus indisponíveis.".into())
}
// Fixed official endpoints only; never send the account key to metadata URLs.
pub(super) async fn file_requirements(game_domain: &str, file_id: u64) -> Result<serde_json::Value, String> {
    super::nexus_catalog::domain(game_domain)?;
    if file_id == 0 { return Err("Arquivo Nexus inválido.".into()); }
    let version = request_metadata(&format!("https://api.nexusmods.com/v3/games/{game_domain}/mod-file-versions/{file_id}"), None).await?;
    let id = version.pointer("/data/id").and_then(|v| v.as_str())
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        .ok_or("Identificação da versão Nexus indisponível.")?;
    request_metadata(&format!("https://api.nexusmods.com/v3/mod-file-versions/{id}/dependencies"), None).await
}
async fn request_metadata(
    url: &str,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let url = url.to_owned();
    with_account(async move { request_metadata_locked(&url, body).await }).await
}
async fn request_metadata_locked(
    url: &str,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let oauth = super::nexus_oauth::access(false).await?;
    let mut header = if let Some(token) = oauth.as_ref() {
        reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
    } else {
        let key = stored("lookup", None).await?.ok_or("Conecte sua conta na aba Conta Nexus.")?;
        reqwest::header::HeaderValue::from_str(checked_key(&key)?)
    }.map_err(|_| "Credencial Nexus inválida.")?;
    header.set_sensitive(true);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Falha ao iniciar a consulta Nexus.")?;
    let request = || {
        let request = if let Some(body) = body.as_ref() { client.post(url).json(body) } else { client.get(url) };
        request.header(reqwest::header::USER_AGENT, concat!("PeliGames/", env!("CARGO_PKG_VERSION")))
            .header("Application-Name", "PeliGames")
            .header("Application-Version", env!("CARGO_PKG_VERSION"))
    };
    let auth_header = if oauth.is_some() { "Authorization" } else { "apikey" };
    before_request().await?;
    let mut response = request().header(auth_header, header).send().await
        .map_err(|_| "Não foi possível consultar o Nexus Mods. Verifique sua conexão.")?;
    // Retry authentication once only; never turn rate-limit responses into loops.
    if response.status().as_u16() == 401 && oauth.is_some() {
        observe_response(&response);
        let token = super::nexus_oauth::access(true).await?.ok_or("Reconecte sua conta Nexus.")?;
        before_request().await?;
        response = request().bearer_auth(token).send().await
            .map_err(|_| "Não foi possível consultar o Nexus Mods. Verifique sua conexão.")?;
    }
    observe_response(&response);
    match response.status().as_u16() {
        200 => {}
        401 | 403 => return Err("Acesso recusado. Verifique a conexão na aba Conta Nexus.".into()),
        404 => return Err("Jogo, mod ou arquivo não encontrado no Nexus.".into()),
        429 => {
            return Err(
                "Limite de consultas Nexus atingido. Aguarde antes de tentar novamente.".into(),
            )
        }
        _ => return Err("O Nexus Mods está indisponível. Tente novamente mais tarde.".into()),
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "Falha ao receber os dados do Nexus.")?
    {
        if body.len() + chunk.len() > 16 * 1024 * 1024 {
            return Err("Resposta Nexus excedeu o limite.".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "Resposta Nexus inválida.".into())
}

#[cfg(test)]
mod auth_regression_tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn missing_keyring_items_can_be_cleared_repeatedly() {
        for action in ["lookup", "clear", "clear"] {
            assert_eq!(credential_result(action, Some(1), String::new(), &[]).unwrap(), None);
        }
        assert_eq!(credential_result("clear", Some(0), String::new(), &[]).unwrap(), None);
        assert_eq!(credential_result("lookup", Some(0), "test-credential\n".into(), &[]).unwrap(), Some("test-credential".into()));
    }

    #[test]
    fn keyring_failures_are_not_mistaken_for_missing_items() {
        for (action, code, output, diagnostic) in [
            ("clear", Some(1), "", "keyring is locked: private-token"),
            ("lookup", Some(1), "unexpected output", ""),
            ("store", Some(1), "", ""),
            ("clear", Some(2), "", ""),
            ("clear", None, "", ""),
        ] {
            let error = credential_result(action, code, output.into(), diagnostic.as_bytes()).unwrap_err();
            assert!(!error.contains("private-token"));
        }
    }

    #[test]
    fn cancelled_request_finishes_rotation_before_logout() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            // Exercise cancellation both before the refresh result arrives and
            // while a blocking keyring write is underway. No real keyring is used.
            for during_write in [false, true] {
                let credentials = Arc::new(Mutex::new(Some("old-refresh".to_string())));
                let write_credentials = credentials.clone();
                let (started_tx, started_rx) = tokio::sync::oneshot::channel();
                let (release_tx, release_rx) = std::sync::mpsc::channel();
                let requester = tokio::spawn(with_account(async move {
                    if during_write {
                        tauri::async_runtime::spawn_blocking(move || {
                            let _ = started_tx.send(());
                            release_rx.recv().map_err(|_| "test cancelled".to_string())?;
                            *write_credentials.lock().unwrap() = Some("rotated-refresh".into());
                            Ok::<_, String>(())
                        }).await.map_err(|_| "test writer failed".to_string())??;
                    } else {
                        let _ = started_tx.send(());
                        tauri::async_runtime::spawn_blocking(move || release_rx.recv())
                            .await.map_err(|_| "test wait failed".to_string())?
                            .map_err(|_| "test cancelled".to_string())?;
                        *write_credentials.lock().unwrap() = Some("rotated-refresh".into());
                    }
                    Ok(())
                }));
                tokio::time::timeout(Duration::from_secs(5), started_rx).await.unwrap().unwrap();
                requester.abort();
                assert!(requester.await.unwrap_err().is_cancelled());
                assert!(ACCOUNT_LOCK.try_lock().is_err(), "cancellation released the account lock before persistence");

                let logout_credentials = credentials.clone();
                let logout = tokio::spawn(with_account(async move {
                    // Logout must see the committed rotation, then erase it.
                    assert_eq!(logout_credentials.lock().unwrap().as_deref(), Some("rotated-refresh"));
                    *logout_credentials.lock().unwrap() = None;
                    Ok(())
                }));
                release_tx.send(()).unwrap();
                tokio::time::timeout(Duration::from_secs(5), logout).await.unwrap().unwrap().unwrap();
                assert!(credentials.lock().unwrap().is_none(), "late write restored credentials after logout");
            }
        });
    }
}
