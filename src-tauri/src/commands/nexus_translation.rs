//! On-demand translation of public mod descriptions. No account credentials are sent.
use std::{collections::HashMap, time::Duration};
use tokio::sync::Mutex;
static CACHE: Mutex<Option<HashMap<String, String>>> = Mutex::const_new(None);

fn segments(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut rest = text;
    while rest.len() > 500 {
        let mut end = 500;
        while !rest.is_char_boundary(end) { end -= 1; }
        if let Some((index, _)) = rest[..end].char_indices().rev().find(|(i, c)| *i > 250 && c.is_whitespace()) { end = index; }
        result.push(&rest[..end]);
        rest = &rest[end..];
    }
    if !rest.is_empty() { result.push(rest); }
    result
}

#[tauri::command]
pub async fn translate_nexus_description(text: String) -> Result<String, String> {
    if text.trim().is_empty() { return Ok(text); }
    if text.len() > 60_000 { return Err("Descrição muito longa para traduzir.".into()); }
    let client = reqwest::Client::builder().timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none()).build().map_err(|_| "Não foi possível iniciar a tradução.")?;
    let mut output = String::new();
    // Preserve paragraph boundaries and whitespace around segments.
    for line in text.split_inclusive('\n') {
        for segment in segments(line) {
            let source = segment.trim();
            if source.is_empty() { output.push_str(segment); continue; }
            let cached = CACHE.lock().await.as_ref().and_then(|cache| cache.get(source).cloned());
            let translated = if let Some(value) = cached { value } else {
                let url = format!("https://api.mymemory.translated.net/get?q={}&langpair=en%7Cpt-BR", urlencoding::encode(source));
                let response = client.get(url).send().await
                    .map_err(|_| "Não foi possível acessar o serviço de tradução. Verifique a conexão.")?;
                if !response.status().is_success() { return Err("Serviço de tradução indisponível. Tente novamente mais tarde.".into()); }
                let data: serde_json::Value = response.json().await.map_err(|_| "Resposta inválida do serviço de tradução.")?;
                if data["quotaFinished"].as_bool() == Some(true) || data["responseStatus"].as_u64() != Some(200) {
                    return Err("O serviço de tradução atingiu o limite de uso ou está indisponível. Tente mais tarde.".into());
                }
                let value = data["responseData"]["translatedText"].as_str().filter(|s| !s.trim().is_empty())
                    .ok_or("O serviço não retornou uma tradução.")?.to_owned();
                let mut guard = CACHE.lock().await;
                let cache = guard.get_or_insert_with(HashMap::new);
                if cache.len() >= 256 { cache.clear(); }
                cache.insert(source.to_owned(), value.clone());
                value
            };
            let prefix = segment.len() - segment.trim_start().len();
            let suffix = segment.trim_end().len();
            output.push_str(&segment[..prefix]);
            output.push_str(&translated);
            output.push_str(&segment[suffix..]);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segments_preserve_unicode_and_all_text() {
        let text = "Descrição de um mod. ".repeat(100);
        let parts = segments(&text);
        assert!(parts.iter().all(|part| part.len() <= 500));
        assert_eq!(parts.concat(), text);
        let unbroken = "界".repeat(500);
        assert_eq!(segments(&unbroken).concat(), unbroken);
        assert!(segments(&unbroken).iter().all(|part| part.len() <= 500));
    }
}
