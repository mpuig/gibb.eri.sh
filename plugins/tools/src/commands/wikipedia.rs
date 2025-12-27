use crate::policy::CACHE_TTL;
use crate::state::CacheEntry;
use crate::{SharedState, WikiSummaryDto};
use std::time::Instant;
use tauri::State;

const DEFAULT_LANG: &str = "en";
const DEFAULT_SENTENCES: u8 = 2;

#[tauri::command]
pub async fn wikipedia_city_lookup(
    state: State<'_, SharedState>,
    city: String,
    lang: Option<String>,
    sentences: Option<u8>,
) -> Result<WikiSummaryDto, String> {
    let lang = lang.unwrap_or_else(|| DEFAULT_LANG.to_string());
    let sentences = sentences.unwrap_or(DEFAULT_SENTENCES).clamp(1, 10);

    // Build cache key using same format as WikipediaTool
    let cache_key = format!("{}:{}", lang, city.trim().to_lowercase());

    // Cache hit
    {
        let guard = state.lock().await;
        if let Some(entry) = guard.cache.get(&cache_key) {
            if entry.fetched_at.elapsed() <= CACHE_TTL {
                // Deserialize from cached JSON payload
                if let Ok(summary) = serde_json::from_value::<WikiSummaryDto>(
                    entry.payload.get("result").cloned().unwrap_or_default(),
                ) {
                    return Ok(summary);
                }
            }
        }
    }

    // Cache miss (or expired): fetch outside lock
    let fetched = crate::wikipedia::fetch_city_summary(&lang, &city, sentences)
        .await
        .map_err(|e| e.to_string())?;

    // Store in cache with same payload format as WikipediaTool
    {
        let mut guard = state.lock().await;
        guard.cache.insert(
            cache_key,
            CacheEntry {
                fetched_at: Instant::now(),
                payload: serde_json::json!({
                    "city": fetched.title,
                    "result": fetched,
                }),
                event_name: std::borrow::Cow::Borrowed("tools:wikipedia_city"),
            },
        );
    }

    Ok(fetched)
}
