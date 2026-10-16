//! Public store catalog searches. No launcher installation or account credentials required.
use reqwest::Client;
use serde_json::{json, Value};
use std::{sync::OnceLock, time::Duration};

pub(super) fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(format!("PeliGames/{}", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(12))
            .build()
            .expect("valid store client configuration")
    })
}

fn normalized_title(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric() && *c != '™' && *c != '®')
        .flat_map(char::to_lowercase)
        .collect()
}

/// Heroic/custom libraries may include the original release year. The PC
/// catalogs list the 2018 God of War simply as "God of War". Keep other games
/// and years intact so this alias cannot match Ragnarök or the 2005 original.
pub(super) fn catalog_title(title: &str) -> &str {
    if normalized_title(title) == "godofwar2018" {
        "God of War"
    } else {
        title
    }
}

fn without_brand(title: &str) -> &str {
    for prefix in ["disneypixar", "easports", "disney"] {
        if let Some(rest) = title.strip_prefix(prefix).filter(|rest| !rest.is_empty()) {
            return rest;
        }
    }
    title
}

pub(super) fn title_score(query: &str, title: &str) -> u8 {
    let query = normalized_title(catalog_title(query));
    let title = normalized_title(catalog_title(title));
    if query.is_empty() {
        return 0;
    }
    if query == title {
        return 100;
    }
    let query = without_brand(&query);
    let title = without_brand(&title);
    if query == title {
        return 95;
    }
    // Only allow known edition suffixes; a sequel, DLC or demo is a different game.
    const EDITIONS: &[&str] = &[
        "standardedition",
        "ultimateedition",
        "definitiveedition",
        "completeedition",
        "enhancededition",
        "goldedition",
        "deluxeedition",
        "gameoftheyearedition",
        "gotyedition",
    ];
    if query.len() >= 4
        && EDITIONS.iter().any(|edition| {
            title.strip_prefix(query) == Some(*edition)
                || query.strip_prefix(title) == Some(*edition)
        })
    {
        return 90;
    }
    0
}

fn image_url(value: &Value) -> Option<String> {
    let url = value.as_str()?;
    url.starts_with("https://").then(|| url.to_string())
}

fn epic_cover(name: &str, response: &Value) -> Option<String> {
    let elements = response
        .pointer("/data/Catalog/searchStore/elements")?
        .as_array()?;
    let mut candidates: Vec<_> = elements
        .iter()
        .filter_map(|game| {
            let score = title_score(name, game["title"].as_str()?);
            if score == 0 {
                return None;
            }
            let images = game["keyImages"].as_array()?;
            let cover = [
                "DieselGameBoxTall",
                "OfferImageTall",
                "DieselStoreFrontTall",
            ]
            .iter()
            .find_map(|kind| {
                images
                    .iter()
                    .filter(|image| image["type"].as_str() == Some(kind))
                    .find_map(|image| image_url(&image["url"]))
            })?;
            Some((score, cover))
        })
        .collect();
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().next().map(|(_, url)| url)
}

fn gog_cover(name: &str, response: &Value) -> Option<String> {
    let products = response["products"].as_array()?;
    products
        .iter()
        .filter_map(|game| {
            let score = title_score(name, game["title"].as_str()?);
            (score > 0)
                .then_some(score)
                .zip(image_url(&game["coverVertical"]))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, url)| url)
}

fn steam_asset_url(item: &Value, kind: &str) -> Option<String> {
    if item["success"].as_u64() != Some(1) {
        return None;
    }
    let app_id = item["appid"].as_u64()?;
    let assets = &item["assets"];
    let template = assets["asset_url_format"].as_str()?;
    let file = assets[kind].as_str()?;
    if !template.starts_with(&format!("steam/apps/{app_id}/"))
        || !template.contains("${FILENAME}")
        || file.is_empty()
        || file.contains("..")
    {
        return None;
    }
    Some(format!(
        "https://shared.akamai.steamstatic.com/store_item_assets/{}",
        template.replace("${FILENAME}", file)
    ))
}

pub(super) async fn fetch_steam_assets(app_ids: &[u32]) -> std::collections::HashMap<u32, String> {
    fetch_steam_asset_kind(app_ids, "library_capsule").await
}

async fn fetch_steam_asset_kind(app_ids: &[u32], kind: &str) -> std::collections::HashMap<u32, String> {
    if app_ids.is_empty() {
        return Default::default();
    }
    let input = json!({"ids":app_ids.iter().map(|id| json!({"appid":id})).collect::<Vec<_>>(),
        "context":{"language":"english","country_code":"US"},"data_request":{"include_assets":true}});
    let response = client()
        .get(format!(
            "https://api.steampowered.com/IStoreBrowseService/GetItems/v1/?input_json={}",
            urlencoding::encode(&input.to_string())
        ))
        .send()
        .await;
    let Ok(response) = response else {
        return Default::default();
    };
    let Ok(response) = response.error_for_status() else {
        return Default::default();
    };
    let Ok(data) = response.json::<Value>().await else {
        return Default::default();
    };
    data.pointer("/response/store_items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id = u32::try_from(item["appid"].as_u64()?).ok()?;
                    Some((id, steam_asset_url(item, kind)?))
                })
                .collect()
        })
        .unwrap_or_default()
}

async fn steam_search_ids(name: &str) -> Option<Vec<u32>> {
    let response = client()
        .get(format!(
            "https://store.steampowered.com/api/storesearch/?term={}&l=english&cc=US",
            urlencoding::encode(&catalog_title(name).replace('_', " "))
        ))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<Value>()
        .await
        .ok()?;
    Some(steam_match_ids(name, &response))
}

fn steam_match_ids(name: &str, response: &Value) -> Vec<u32> {
    let Some(items) = response["items"].as_array() else { return Vec::new(); };
    let mut candidates: Vec<_> = items
        .iter()
        .filter_map(|game| {
            let score = title_score(name, game["name"].as_str()?);
            if score == 0 {
                return None;
            }
            Some((score, u32::try_from(game["id"].as_u64()?).ok()?))
        })
        .collect();
    candidates.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    candidates.into_iter().take(3).map(|(_, id)| id).collect()
}

pub(super) async fn fetch_steam_cover(name: &str) -> Option<String> {
    let ids = steam_search_ids(name).await?;
    let assets = fetch_steam_assets(&ids).await;
    ids.iter().find_map(|id| assets.get(id).cloned())
}

pub(super) async fn fetch_epic_cover(name: &str) -> Option<String> {
    // serde_json quotes user-supplied titles safely inside the GraphQL query.
    let query = format!("query {{ Catalog {{ searchStore(keywords:{}, country:\"US\", locale:\"en-US\", count:10) {{ elements {{ title keyImages {{ type url }} }} }} }} }}", json!(catalog_title(name).replace('_', " ")));
    let response = client()
        .get(format!(
            "https://store.epicgames.com/graphql?query={}",
            urlencoding::encode(&query)
        ))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<Value>()
        .await
        .ok()?;
    epic_cover(name, &response)
}

pub(super) async fn fetch_gog_cover(name: &str) -> Option<String> {
    let query = format!("like:{}", catalog_title(name).replace('_', " "));
    let response = client()
        .get(format!("https://catalog.gog.com/v1/catalog?query={}&limit=10&productType=in%3Agame%2Cpack&countryCode=US&locale=en-US&currencyCode=USD", urlencoding::encode(&query)))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<Value>()
        .await
        .ok()?;
    gog_cover(name, &response)
}

pub(super) async fn fetch_cover(name: &str) -> Option<String> {
    // Bound total fallback time by searching both catalogs concurrently.
    let epic_name = name.to_string();
    let epic_task = tokio::spawn(async move { fetch_epic_cover(&epic_name).await });
    let gog = fetch_gog_cover(name).await;
    epic_task.await.ok().flatten().or(gog)
}

/// Horizontal artwork is independent of the launcher that installed the game.
/// Return candidates: the UI verifies that an image loads before replacing its cover fallback.
pub(super) async fn fetch_steam_banners(name: &str, app_id: Option<&str>) -> Option<Vec<String>> {
    let ids = match app_id.and_then(|value| value.parse::<u32>().ok()).filter(|id| *id > 0) {
        Some(id) => vec![id],
        None => steam_search_ids(name).await?,
    };
    let assets = fetch_steam_asset_kind(&ids, "library_hero").await;
    let mut urls = Vec::new();
    for id in ids {
        if let Some(url) = assets.get(&id) { urls.push(url.clone()); }
        let legacy = format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/library_hero.jpg");
        if !urls.contains(&legacy) { urls.push(legacy); }
    }
    Some(urls)
}

#[cfg(test)]
mod banner_tests {
    use super::*;
    #[test]
    fn name_search_does_not_confuse_sequels_and_dlc() {
        let response = json!({"items":[
            {"id":2,"name":"Game Two"}, {"id":3,"name":"Game DLC"},
            {"id":4,"name":"Game Deluxe Edition"}, {"id":1,"name":"Game"}
        ]});
        assert_eq!(steam_match_ids("Game", &response), vec![1,4]);
        assert!(steam_match_ids("Other game", &response).is_empty());
    }
    #[test]
    fn banner_uses_horizontal_asset_not_the_portrait() {
        let item = json!({"success":1,"appid":42,"assets":{
            "asset_url_format":"steam/apps/42/hash/${FILENAME}",
            "library_hero":"hero.jpg","library_capsule":"portrait.jpg"}});
        assert!(steam_asset_url(&item,"library_hero").unwrap().ends_with("/hero.jpg"));
        assert!(steam_asset_url(&item,"library_capsule").unwrap().ends_with("/portrait.jpg"));
        assert!(steam_asset_url(&item,"missing").is_none());
    }
    #[test]
    fn missing_search_and_invalid_asset_do_not_create_a_banner() {
        assert!(steam_match_ids("Game", &json!({})).is_empty());
        assert!(steam_asset_url(&json!({"success":0}),"library_hero").is_none());
        let item = json!({"success":1,"appid":42,"assets":{
            "asset_url_format":"steam/apps/42/${FILENAME}","library_hero":"../hero.jpg"}});
        assert!(steam_asset_url(&item,"library_hero").is_none());
    }
}
