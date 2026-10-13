//! Discord OAuth is handled by our relay; creator secrets never enter the launcher.
use serde::{Deserialize, Serialize};
static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
// Public service origin. OAuth secrets remain in Cloudflare.
const DEFAULT_DISCORD_RELAY: &str = "https://peligames-discord-auth.nairomarttins97.workers.dev";
#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    id: String,
    name: String,
    in_server: bool,
    benefit: Option<String>,
    valid_until: Option<u64>,
}
#[derive(Serialize, Deserialize)]
struct Session { id: String, secret: String, account: Option<Account>, #[serde(default)] entitlement: Option<String> }
#[derive(Serialize)]
pub struct Status { configured: bool, pending: bool, offline: bool, account: Option<Account>, session_id: Option<String>, entitlement: Option<String> }
#[derive(Deserialize)]
struct Started { id: String, secret: String, browser_url: String }
#[derive(Deserialize)]
struct Response { #[serde(default)] pending: bool, account: Option<Account>, #[serde(default)] entitlement: Option<String> }
fn relay() -> Result<reqwest::Url, String> {
    let value = option_env!("PELIGAMES_DISCORD_RELAY").unwrap_or(DEFAULT_DISCORD_RELAY);
    let url = reqwest::Url::parse(value).map_err(|_| "Endereço do serviço Discord inválido.")?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() || url.path() != "/" {
        return Err("O serviço Discord exige uma origem HTTPS válida.".into());
    }
    Ok(url)
}
async fn credential(action: &'static str, value: Option<String>) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || super::nexus_account::credential_for(action, value.as_deref(), "discord-oauth-session", "PeliGames — Discord"))
        .await.map_err(|_| "Não foi possível acessar o chaveiro.".to_string())?
}
async fn session() -> Result<Option<Session>, String> {
    credential("lookup", None).await?.map(|value| serde_json::from_str(&value).map_err(|_| "Sessão Discord inválida. Desconecte e tente novamente.".to_string())).transpose()
}
async fn save(value: &Session) -> Result<(), String> {
    credential("store", Some(serde_json::to_string(value).map_err(|_| "Sessão inválida.")?)).await?;
    Ok(())
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder().timeout(std::time::Duration::from_secs(25)).redirect(reqwest::redirect::Policy::none()).build().map_err(|_| "Não foi possível iniciar a conexão.".into())
}
enum Failure { Expired, Other(String) }
impl From<Failure> for String {
    fn from(failure: Failure) -> String {
        match failure { Failure::Expired => "A conexão Discord expirou. Entre novamente.".into(), Failure::Other(error) => error }
    }
}
/// `Expired`: the relay no longer knows this session (it was removed after 90
/// days without use, or the sign-in expired); only the local copy is left.
async fn request(path: &str, session: &Session) -> Result<Response, Failure> {
    let url = relay().map_err(Failure::Other)?.join(path).map_err(|_| Failure::Other("Endereço inválido.".into()))?;
    let response = client().map_err(Failure::Other)?.post(url).header("X-Session-Id", &session.id).bearer_auth(&session.secret).send().await.map_err(|_| Failure::Other("Sem conexão com o serviço Discord.".into()))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED { return Err(Failure::Expired); }
    if !response.status().is_success() { return Err(Failure::Other("Não foi possível validar a conexão Discord. Tente novamente ou reconecte a conta.".into())); }
    response.json().await.map_err(|_| Failure::Other("Resposta Discord inválida.".into()))
}
/// Revokes the session on the relay; one it no longer knows is already gone.
async fn revoke(session: &Session) -> Result<(), String> {
    match request("disconnect", session).await { Ok(_) | Err(Failure::Expired) => Ok(()), Err(error) => Err(error.into()) }
}
#[tauri::command]
pub async fn load_discord_account() -> Result<Status, String> {
    let _guard = LOCK.lock().await;
    if relay().is_err() { return Ok(Status { configured: false, pending: false, offline: false, account: None, session_id: None, entitlement: None }); }
    let Some(mut stored) = session().await? else { return Ok(Status { configured: true, pending: false, offline: false, account: None, session_id: None, entitlement: None }); };
    match request("status", &stored).await {
        Ok(value) => {
            stored.account = value.account.clone(); stored.entitlement = value.entitlement.clone(); save(&stored).await?;
            Ok(Status { configured: true, pending: value.pending, offline: false, account: value.account, session_id: Some(stored.id), entitlement: value.entitlement })
        },
        Err(Failure::Expired) => {
            credential("clear", None).await?;
            Ok(Status { configured: true, pending: false, offline: false, account: None, session_id: None, entitlement: None })
        },
        Err(Failure::Other(error)) => {
            if stored.account.is_none() { return Err(error); }
            // The webview verifies the cached entitlement with its pinned public key.
            Ok(Status { configured: true, pending: false, offline: true, account: stored.account, session_id: Some(stored.id), entitlement: stored.entitlement })
        }
    }
}
#[tauri::command]
pub async fn connect_discord_account(app: tauri::AppHandle) -> Result<(), String> {
    let _guard = LOCK.lock().await;
    if let Some(existing) = session().await? {
        revoke(&existing).await?;
        credential("clear", None).await?;
    }
    let base = relay()?;
    let response = client()?.post(base.join("start").map_err(|_| "Endereço inválido.")?).send().await.map_err(|_| "Não foi possível iniciar o login Discord.")?;
    if !response.status().is_success() { return Err("Serviço Discord indisponível ou não configurado.".into()); }
    let started: Started = response.json().await.map_err(|_| "Resposta do serviço inválida.")?;
    let url = reqwest::Url::parse(&started.browser_url).map_err(|_| "Link de login inválido.")?;
    if url.origin() != base.origin() || url.path() != "/authorize" || !url.username().is_empty() || url.password().is_some() { return Err("Link de login não autorizado.".into()); }
    let stored = Session { id: started.id, secret: started.secret, account: None, entitlement: None };
    save(&stored).await?;
    if let Err(error) = super::browser::open(app, url.to_string()).await {
        let _ = request("disconnect", &stored).await;
        let _ = credential("clear", None).await;
        return Err(error);
    }
    Ok(())
}
#[tauri::command]
pub async fn disconnect_discord_account() -> Result<(), String> {
    let _guard = LOCK.lock().await;
    if let Some(stored) = session().await? { revoke(&stored).await?; }
    credential("clear", None).await?;
    Ok(())
}
