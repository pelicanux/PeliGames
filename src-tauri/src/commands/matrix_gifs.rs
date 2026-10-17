//! GIF links sent to the chat (Discord's GIF picker sends Tenor or Giphy links
//! through the bridge): the media file behind them, shown inline. Only known
//! GIF services are contacted, and only the media URL comes back.
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize, Debug, PartialEq)]
pub struct GifMedia { pub src: String, pub video: bool }

/// Hosts that serve the media files themselves.
fn media_host(host: &str) -> bool {
    matches!(host, "media.tenor.com" | "media1.tenor.com" | "c.tenor.com" | "i.giphy.com" | "media.giphy.com" | "cdn.discordapp.com" | "media.discordapp.net" | "i.imgur.com")
        || (host.starts_with("media") && host.ends_with(".giphy.com") && host.len() < 32)
}
/// A direct link to a GIF, WebP or short video on a media host.
fn direct(url: &reqwest::Url) -> Option<GifMedia> {
    let host = url.host_str()?.to_ascii_lowercase();
    if url.scheme() != "https" || !media_host(&host) { return None; }
    let path = url.path().to_ascii_lowercase();
    if host == "i.imgur.com" && path.ends_with(".gifv") {
        let mut mp4 = url.clone();
        let base = &url.path()[..url.path().len() - 5];
        mp4.set_path(&format!("{base}.mp4"));
        return Some(GifMedia { src: mp4.to_string(), video: true });
    }
    let video = path.ends_with(".mp4") || path.ends_with(".webm");
    (video || path.ends_with(".gif") || path.ends_with(".webp")).then(|| GifMedia { src: url.to_string(), video })
}
/// giphy.com/gifs/name-ID (or /gifs/ID): the GIF is i.giphy.com/ID.gif.
fn giphy_page(url: &reqwest::Url) -> Option<GifMedia> {
    let host = url.host_str()?.to_ascii_lowercase();
    if !matches!(host.as_str(), "giphy.com" | "www.giphy.com") { return None; }
    let mut segments = url.path_segments()?;
    if segments.next()? != "gifs" { return None; }
    let id = segments.next()?.rsplit('-').next()?.to_string();
    (!id.is_empty() && id.len() <= 40 && id.bytes().all(|b| b.is_ascii_alphanumeric())).then(|| GifMedia { src: format!("https://i.giphy.com/{id}.gif"), video: false })
}
/// The `content` of `<meta property="…">` (or name=…) in a page.
fn meta(html: &str, property: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(start) = lower[from..].find("<meta").map(|i| i + from) {
        let end = lower[start..].find('>').map(|i| i + start)?;
        let tag = &html[start..end];
        let tag_lower = &lower[start..end];
        if tag_lower.contains(&format!("property=\"{property}\"")) || tag_lower.contains(&format!("name=\"{property}\"")) {
            let at = tag_lower.find("content=\"")? + 9;
            let value = &tag[at..at + tag[at..].find('"')?];
            return Some(value.replace("&amp;", "&"));
        }
        from = end;
    }
    None
}
fn tenor_page(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && matches!(url.host_str().map(str::to_ascii_lowercase).as_deref(), Some("tenor.com" | "www.tenor.com")) && url.path().starts_with("/view/")
}

#[tauri::command]
pub async fn matrix_gif_media(url: String) -> Result<Option<GifMedia>, String> {
    if url.len() > 2048 { return Ok(None); }
    let Ok(url) = reqwest::Url::parse(&url) else { return Ok(None) };
    if let Some(media) = direct(&url).or_else(|| giphy_page(&url)) { return Ok(Some(media)); }
    if !tenor_page(&url) { return Ok(None); }
    // A Tenor page names its media in its Open Graph tags (video first: smaller).
    let client = reqwest::Client::builder().timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::custom(|attempt| if attempt.previous().len() < 3 && attempt.url().host_str().is_some_and(|host| host.ends_with("tenor.com")) { attempt.follow() } else { attempt.stop() }))
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) PeliGames").build().map_err(|_| "GIF indisponível.")?;
    let Ok(mut response) = client.get(url).send().await else { return Ok(None) };
    if !response.status().is_success() { return Ok(None); }
    let mut html = Vec::new();
    while let Ok(Some(chunk)) = response.chunk().await { if html.len() + chunk.len() > 1_500_000 { break; } html.extend_from_slice(&chunk); }
    let html = String::from_utf8_lossy(&html);
    for property in ["og:video:secure_url", "og:video", "og:image"] {
        if let Some(found) = meta(&html, property).and_then(|value| reqwest::Url::parse(&value).ok()).and_then(|value| direct(&value)) { return Ok(Some(found)); }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn url(value: &str) -> reqwest::Url { reqwest::Url::parse(value).unwrap() }

    #[test]
    fn direct_media_links_on_known_hosts_only() {
        assert_eq!(direct(&url("https://media.tenor.com/abc/cat.gif")), Some(GifMedia { src: "https://media.tenor.com/abc/cat.gif".into(), video: false }));
        assert_eq!(direct(&url("https://media.tenor.com/abc/cat.mp4")).map(|m| m.video), Some(true));
        assert_eq!(direct(&url("https://i.imgur.com/AbC123.gifv")), Some(GifMedia { src: "https://i.imgur.com/AbC123.mp4".into(), video: true }));
        assert!(direct(&url("https://evil.example/cat.gif")).is_none());
        assert!(direct(&url("http://media.tenor.com/abc/cat.gif")).is_none());
        assert!(direct(&url("https://media.tenor.com/abc/page.html")).is_none());
    }
    #[test]
    fn giphy_pages_point_at_their_gif() {
        assert_eq!(giphy_page(&url("https://giphy.com/gifs/funny-cat-3o7TKSjRrfIPjeiVyM")).map(|m| m.src), Some("https://i.giphy.com/3o7TKSjRrfIPjeiVyM.gif".into()));
        assert!(giphy_page(&url("https://giphy.com/stickers/x-1")).is_none());
    }
    #[test]
    fn open_graph_media_is_read_from_the_page() {
        let html = r#"<head><meta property="og:image" content="https://media.tenor.com/x/a.gif"><meta property="og:video" content="https://media.tenor.com/x/a.mp4?y=1&amp;z=2"></head>"#;
        assert_eq!(meta(html, "og:video").as_deref(), Some("https://media.tenor.com/x/a.mp4?y=1&z=2"));
        assert_eq!(meta(html, "og:image").as_deref(), Some("https://media.tenor.com/x/a.gif"));
        assert!(meta(html, "og:title").is_none());
        assert!(tenor_page(&url("https://tenor.com/view/cat-dance-gif-123")) && !tenor_page(&url("https://tenor.com/search/cat")));
    }
}
