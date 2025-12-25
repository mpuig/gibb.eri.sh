use crate::state::{CoordinatesDto, WikiSummaryDto};
use reqwest::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum WikipediaError {
    #[error("invalid language code")]
    InvalidLang,
    #[error("request failed: {0}")]
    RequestFailed(String),
    #[error("no page found for '{0}'")]
    NotFound(String),
}

#[derive(Debug, serde::Deserialize)]
struct SummaryResponse {
    #[serde(default)]
    title: String,
    #[serde(default)]
    extract: String,
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    content_urls: Option<ContentUrls>,
    #[serde(default)]
    thumbnail: Option<Thumbnail>,
    #[serde(default)]
    coordinates: Option<Coordinates>,
}

#[derive(Debug, serde::Deserialize)]
struct ContentUrls {
    desktop: Option<ContentUrl>,
}

#[derive(Debug, serde::Deserialize)]
struct ContentUrl {
    page: String,
}

#[derive(Debug, serde::Deserialize)]
struct Thumbnail {
    source: String,
}

#[derive(Debug, serde::Deserialize)]
struct Coordinates {
    lat: f64,
    lon: f64,
}

#[derive(Debug, serde::Deserialize)]
struct SearchResponse {
    pages: Vec<SearchPage>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchPage {
    title: String,
}

fn is_valid_lang(lang: &str) -> bool {
    let lang = lang.trim();
    !lang.is_empty()
        && lang.len() <= 12
        && lang
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn trim_to_sentences(text: &str, max_sentences: u8) -> String {
    if max_sentences == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut sentence_count: u8 = 0;
    for ch in text.chars() {
        out.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            sentence_count += 1;
            if sentence_count >= max_sentences {
                break;
            }
        }
    }
    out.trim().to_string()
}

async fn get_summary_with_client(
    client: &reqwest::Client,
    lang: &str,
    title: &str,
) -> Result<SummaryResponse, WikipediaError> {
    // No trailing slash - push() will add the title as a new segment
    let base = format!("https://{lang}.wikipedia.org/api/rest_v1/page/summary");
    let mut url = reqwest::Url::parse(&base).map_err(|_| WikipediaError::InvalidLang)?;
    url.path_segments_mut()
        .map_err(|_| WikipediaError::InvalidLang)?
        .push(title);

    tracing::info!(url = %url, title = title, title_bytes = ?title.as_bytes(), "wikipedia: fetching summary");

    let resp = client
        .get(url.clone())
        .header(
            "User-Agent",
            "gibberish-desktop/0.1 (https://github.com/mpuig/gibb.eri.sh)",
        )
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, url = %url, "wikipedia: request failed");
            WikipediaError::RequestFailed(e.to_string())
        })?;

    let status = resp.status();
    tracing::info!(status = %status, url = %url, "wikipedia: response received");

    if status == StatusCode::NOT_FOUND {
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(body = body, "wikipedia: 404 response body");
        return Err(WikipediaError::NotFound(title.to_string()));
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::error!(status = %status, body = body, "wikipedia: non-success response");
        return Err(WikipediaError::RequestFailed(status.to_string()));
    }

    resp.json::<SummaryResponse>().await.map_err(|e| {
        tracing::error!(error = %e, "wikipedia: json parse failed");
        WikipediaError::RequestFailed(e.to_string())
    })
}

async fn get_summary(lang: &str, title: &str) -> Result<SummaryResponse, WikipediaError> {
    let client = reqwest::Client::new();
    get_summary_with_client(&client, lang, title).await
}

async fn search_title_with_client(
    client: &reqwest::Client,
    lang: &str,
    query: &str,
) -> Result<Option<String>, WikipediaError> {
    let url = format!("https://{lang}.wikipedia.org/w/rest.php/v1/search/title");
    tracing::info!(url = url, query = query, "wikipedia: searching title");

    let resp = client
        .get(&url)
        .query(&[("q", query), ("limit", "5")])
        .header(
            "User-Agent",
            "gibberish-desktop/0.1 (https://github.com/mpuig/gibb.eri.sh)",
        )
        .send()
        .await
        .map_err(|e| WikipediaError::RequestFailed(e.to_string()))?;

    tracing::info!(status = %resp.status(), "wikipedia: search response");

    if !resp.status().is_success() {
        return Err(WikipediaError::RequestFailed(resp.status().to_string()));
    }

    let result = resp
        .json::<SearchResponse>()
        .await
        .map_err(|e| WikipediaError::RequestFailed(e.to_string()))?;

    let found = result.pages.into_iter().next().map(|p| p.title);
    tracing::info!(found = ?found, "wikipedia: search result");
    Ok(found)
}

async fn search_title(lang: &str, query: &str) -> Result<Option<String>, WikipediaError> {
    let client = reqwest::Client::new();
    search_title_with_client(&client, lang, query).await
}

pub async fn fetch_city_summary(
    lang: &str,
    city: &str,
    sentences: u8,
) -> Result<WikiSummaryDto, WikipediaError> {
    if !is_valid_lang(lang) {
        return Err(WikipediaError::InvalidLang);
    }
    let city = city.trim();
    if city.is_empty() {
        return Err(WikipediaError::NotFound(city.to_string()));
    }

    // Prefer the city name directly, fallback to search if not found or disambiguation.
    let mut summary = match get_summary(lang, city).await {
        Ok(s) => s,
        Err(WikipediaError::NotFound(_)) => {
            let Some(title) = search_title(lang, city).await? else {
                return Err(WikipediaError::NotFound(city.to_string()));
            };
            get_summary(lang, &title).await?
        }
        Err(e) => return Err(e),
    };

    // If disambiguation, try search and pick the first result.
    if summary.r#type == "disambiguation" || summary.extract.is_empty() {
        let Some(title) = search_title(lang, city).await? else {
            return Err(WikipediaError::NotFound(city.to_string()));
        };
        summary = get_summary(lang, &title).await?;
    }

    let url = summary
        .content_urls
        .as_ref()
        .and_then(|u| u.desktop.as_ref())
        .map(|u| u.page.clone())
        .unwrap_or_else(|| format!("https://{}.wikipedia.org/wiki/{}", lang, summary.title));

    Ok(WikiSummaryDto {
        title: summary.title.clone(),
        summary: trim_to_sentences(&summary.extract, sentences),
        url,
        thumbnail_url: summary.thumbnail.map(|t| t.source),
        coordinates: summary.coordinates.map(|c| CoordinatesDto {
            lat: c.lat,
            lon: c.lon,
        }),
    })
}

/// Fetch city summary using a shared HTTP client.
pub async fn fetch_city_summary_with_client(
    client: &reqwest::Client,
    lang: &str,
    city: &str,
    sentences: u8,
) -> Result<WikiSummaryDto, WikipediaError> {
    if !is_valid_lang(lang) {
        return Err(WikipediaError::InvalidLang);
    }
    let city = city.trim();
    if city.is_empty() {
        return Err(WikipediaError::NotFound(city.to_string()));
    }

    let mut summary = match get_summary_with_client(client, lang, city).await {
        Ok(s) => s,
        Err(WikipediaError::NotFound(_)) => {
            let Some(title) = search_title_with_client(client, lang, city).await? else {
                return Err(WikipediaError::NotFound(city.to_string()));
            };
            get_summary_with_client(client, lang, &title).await?
        }
        Err(e) => return Err(e),
    };

    if summary.r#type == "disambiguation" || summary.extract.is_empty() {
        let Some(title) = search_title_with_client(client, lang, city).await? else {
            return Err(WikipediaError::NotFound(city.to_string()));
        };
        summary = get_summary_with_client(client, lang, &title).await?;
    }

    let url = summary
        .content_urls
        .as_ref()
        .and_then(|u| u.desktop.as_ref())
        .map(|u| u.page.clone())
        .unwrap_or_else(|| format!("https://{}.wikipedia.org/wiki/{}", lang, summary.title));

    Ok(WikiSummaryDto {
        title: summary.title.clone(),
        summary: trim_to_sentences(&summary.extract, sentences),
        url,
        thumbnail_url: summary.thumbnail.map(|t| t.source),
        coordinates: summary.coordinates.map(|c| CoordinatesDto {
            lat: c.lat,
            lon: c.lon,
        }),
    })
}
