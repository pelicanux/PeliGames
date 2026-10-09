//! Personal-key connection for local testing. Secrets never enter config files.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
    time::{Duration, Instant},
};
static ACCOUNT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
    let mut command = Command::new("secret-tool");
    command.arg(action);
    // The system helper must use system libraries even when launched from AppImage.
    command
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD");
    if action == "store" {
        command.arg("--label=PeliGames — Nexus Mods");
    }
    command
        .args([
            "application",
            "com.pelicano.peligames",
            "service",
            "nexusmods-personal-api",
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
    let mut output = String::new();
    child
        .stdout
        .take()
        .ok_or("Chaveiro indisponível.")?
        .take(4097)
        .read_to_string(&mut output)
        .map_err(|_| "Não foi possível ler o chaveiro.")?;
    let mut diagnostic = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let _ = stderr.take(4096).read_to_end(&mut diagnostic);
    }
    if !status.success() {
        // secret-tool lookup returns 1 and no output when there is no matching item.
        if action == "lookup"
            && status.code() == Some(1)
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
    let response = client
        .get("https://api.nexusmods.com/v1/users/validate.json")
        .header("apikey", header)
        .header("Application-Name", "PeliGames")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|_| {
            "Não foi possível conectar ao Nexus Mods. Verifique sua conexão e tente novamente."
        })?;
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
    let _lock = ACCOUNT_LOCK.lock().await;
    let key = checked_key(&api_key)?.to_string();
    let account = validate(&key).await?;
    stored("store", Some(key)).await?;
    Ok(account)
}
#[tauri::command]
pub async fn load_nexus_account() -> Result<Option<NexusAccount>, String> {
    let _lock = ACCOUNT_LOCK.lock().await;
    match stored("lookup", None).await? {
        Some(key) => validate(&key).await.map(Some),
        None => Ok(None),
    }
}
#[tauri::command]
pub async fn disconnect_nexus_account() -> Result<(), String> {
    let _lock = ACCOUNT_LOCK.lock().await;
    stored("clear", None).await?;
    Ok(())
}

// Internal only: callers select fixed metadata endpoints and return typed summaries.
pub(super) async fn metadata(path: &str) -> Result<serde_json::Value, String> {
    request_metadata(&format!("https://api.nexusmods.com/v1/{path}"), None).await
}
pub(super) async fn catalog_page(domain: &str, offset: u32, count: u32, most_downloaded: bool) -> Result<serde_json::Value, String> {
    let sort = if most_downloaded {
        serde_json::json!([{ "downloads": { "direction": "DESC" } }])
    } else {
        // A deterministic default order makes pagination stable; no category filter.
        serde_json::json!([{ "createdAt": { "direction": "DESC" } }])
    };
    let body = serde_json::json!({
        "query": "query PeliGamesCatalog($domain: String!, $offset: Int!, $count: Int!, $sort: [ModsSort!]) { mods(filter: {gameDomainName: [{value: $domain, op: EQUALS}]}, sort: $sort, count: $count, offset: $offset) { totalCount nodes { mod_id: modId name summary picture_url: pictureUrl author version downloads } } }",
        "variables": { "domain": domain, "offset": offset, "count": count, "sort": sort }
    });
    let data = request_metadata("https://api.nexusmods.com/v2/graphql", Some(body)).await?;
    if data.get("errors").is_some_and(|value| value.as_array().is_none_or(|errors| !errors.is_empty())) {
        return Err("Não foi possível consultar o catálogo completo Nexus.".into());
    }
    data.pointer("/data/mods").filter(|value| value.is_object()).cloned()
        .ok_or_else(|| "Catálogo Nexus indisponível.".into())
}
pub(super) async fn mod_requirements(
    game_id: u64,
    mod_id: u64,
) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({
        "query": "query modRequirements($modId: ID!, $gameId: ID!) { mod(modId: $modId, gameId: $gameId) { modRequirements { nexusRequirements(count: 100) { nodes { externalRequirement gameId id modId modName notes url } totalCount } dlcRequirements { gameExpansion { name } notes } } } }",
        "variables": { "modId": mod_id.to_string(), "gameId": game_id.to_string() }
    });
    let data = request_metadata("https://api.nexusmods.com/v2/graphql", Some(body)).await?;
    if data
        .get("errors")
        .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
    {
        return Err("Não foi possível consultar os requisitos Nexus.".into());
    }
    data.pointer("/data/mod/modRequirements")
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| "Requisitos Nexus indisponíveis.".into())
}
async fn request_metadata(
    url: &str,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let _lock = ACCOUNT_LOCK.lock().await;
    let key = stored("lookup", None)
        .await?
        .ok_or("Conecte sua conta na aba Conta Nexus.")?;
    let mut header = reqwest::header::HeaderValue::from_str(checked_key(&key)?)
        .map_err(|_| "Chave inválida.")?;
    header.set_sensitive(true);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Falha ao iniciar a consulta Nexus.")?;
    let request = if let Some(body) = body {
        client.post(url).json(&body)
    } else {
        client.get(url)
    };
    let mut response = request
        .header("apikey", header)
        .header("Application-Name", "PeliGames")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
        .send()
        .await
        .map_err(|_| "Não foi possível consultar o Nexus Mods. Verifique sua conexão.")?;
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
