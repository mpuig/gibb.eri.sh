//! Generic tool result cache.

use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Instant;

/// Generic cache entry holding any JSON-serializable result.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub fetched_at: Instant,
    pub payload: serde_json::Value,
    pub event_name: Cow<'static, str>,
}

/// Generic cache for tool results.
///
/// Uses string keys provided by tools (e.g., "en:barcelona" for city lookups).
/// This keeps the cache layer tool-agnostic.
#[derive(Debug, Default)]
pub struct CacheState {
    entries: HashMap<String, CacheEntry>,
}

impl CacheState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&CacheEntry> {
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: String, entry: CacheEntry) {
        self.entries.insert(key, entry);
    }
}
