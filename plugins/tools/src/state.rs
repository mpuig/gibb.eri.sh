use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, serde::Serialize)]
pub struct WikiSummaryDto {
    pub title: String,
    pub summary: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<CoordinatesDto>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CoordinatesDto {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Eq)]
pub struct CacheKey {
    lang: String,
    city_normalized: String,
}

impl CacheKey {
    pub fn new(lang: &str, city: &str) -> Self {
        Self {
            lang: lang.trim().to_lowercase(),
            city_normalized: city.trim().to_lowercase(),
        }
    }
}

impl PartialEq for CacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.lang == other.lang && self.city_normalized == other.city_normalized
    }
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.lang.hash(state);
        self.city_normalized.hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub fetched_at: Instant,
    pub value: WikiSummaryDto,
}

#[derive(Debug)]
pub struct FunctionGemmaModel {
    pub variant: String,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub runner: Arc<crate::functiongemma::FunctionGemmaRunner>,
}

#[derive(Debug)]
pub struct FunctionGemmaDownload {
    pub variant: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub current_file: Option<String>,
    pub started_at: Instant,
    pub cancel: CancellationToken,
}

pub struct ToolsState {
    pub client: reqwest::Client,
    pub cache: HashMap<CacheKey, CacheEntry>,
    pub router_enabled: bool,
    pub router_auto_run_read_only: bool,
    pub router_default_lang: String,
    pub router_last_city_query: Option<(String, Instant)>,
    pub router_pending_text: String,
    pub router_inflight: bool,
    pub functiongemma: Option<FunctionGemmaModel>,
    pub functiongemma_download: Option<FunctionGemmaDownload>,
    pub functiongemma_last_error: Option<String>,
}

impl Default for ToolsState {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            cache: HashMap::new(),
            router_enabled: true,
            router_auto_run_read_only: true,
            router_default_lang: "en".to_string(),
            router_last_city_query: None,
            router_pending_text: String::new(),
            router_inflight: false,
            functiongemma: None,
            functiongemma_download: None,
            functiongemma_last_error: None,
        }
    }
}
