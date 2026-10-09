//! NXM tokens stay in memory; only public identifiers are exposed to the UI.
pub struct Request {
    pub domain: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub key: Option<String>,
    pub expires: Option<u64>,
    pub user_id: Option<u64>,
}
fn number(value: &str) -> Result<u64, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Identificador NXM inválido.".into());
    }
    value
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("Identificador NXM inválido.".into())
}
pub fn parse(value: &str) -> Result<Request, String> {
    if value.len() > 8192 {
        return Err("Link Nexus muito longo.".into());
    }
    let url = reqwest::Url::parse(value).map_err(|_| "Link NXM inválido.")?;
    if url.scheme() != "nxm"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err("Link NXM inválido.".into());
    }
    let domain = url
        .host_str()
        .ok_or("Jogo Nexus ausente.")?
        .to_ascii_lowercase();
    if domain.is_empty()
        || domain.len() > 100
        || !domain
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("Jogo Nexus inválido.".into());
    }
    let parts: Vec<_> = url.path().split('/').collect();
    if parts.len() != 5 || parts[0] != "" || parts[1] != "mods" || parts[3] != "files" {
        return Err("Use um link de arquivo do Nexus. Coleções não são suportadas.".into());
    }
    let mut request = Request {
        domain,
        mod_id: number(parts[2])?,
        file_id: number(parts[4])?,
        key: None,
        expires: None,
        user_id: None,
    };
    let mut seen = std::collections::HashSet::new();
    for (name, value) in url.query_pairs() {
        if !seen.insert(name.to_string()) {
            return Err("Parâmetro NXM duplicado.".into());
        }
        match name.as_ref() {
            "key" => {
                if value.is_empty()
                    || value.len() > 1024
                    || !value.bytes().all(|b| b.is_ascii_graphic())
                {
                    return Err("Autorização de download inválida.".into());
                }
                request.key = Some(value.into_owned());
            }
            "expires" => request.expires = Some(number(&value)?),
            "user_id" => request.user_id = Some(number(&value)?),
            "view" if value == "0" || value == "1" => {}
            _ => return Err("Parâmetro NXM não suportado.".into()),
        }
    }
    if request.key.is_some() != request.expires.is_some() {
        return Err("Link de download incompleto.".into());
    }
    request.check_expiry()?;
    Ok(request)
}
impl Request {
    pub fn check_expiry(&self) -> Result<(), String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "Relógio indisponível.")?
            .as_secs();
        if self.expires.is_some_and(|e| e <= now) {
            return Err(
                "Link Nexus expirado. Clique novamente em Mod Manager Download no site.".into(),
            );
        }
        Ok(())
    }
    pub fn download_endpoint(&self) -> String {
        let path = format!(
            "games/{}/mods/{}/files/{}/download_link.json",
            self.domain, self.mod_id, self.file_id
        );
        match (&self.key, self.expires) {
            (Some(key), Some(expires)) => {
                format!("{path}?key={}&expires={expires}", urlencoding::encode(key))
            }
            _ => path,
        }
    }
}
pub fn trusted_download(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.fragment().is_none()
        && url.host_str().is_some_and(|host| {
            ["nexusmods.com", "nexus-cdn.com"]
                .iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        })
}
