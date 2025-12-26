//! Tools plugin state management.

mod cache;
mod functiongemma;
mod router;

pub use cache::{CacheEntry, CacheState};
pub use functiongemma::{FunctionGemmaDownload, FunctionGemmaModel, FunctionGemmaState};
pub use router::RouterState;

use gibberish_context::ContextState;

/// DTO for Wikipedia summary results.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WikiSummaryDto {
    pub title: String,
    pub summary: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub coordinates: Option<CoordinatesDto>,
}

/// DTO for geographic coordinates.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoordinatesDto {
    pub lat: f64,
    pub lon: f64,
}

/// Combined state for the tools plugin.
#[derive(Debug)]
pub struct ToolsState {
    pub client: reqwest::Client,
    pub router: RouterState,
    pub functiongemma: FunctionGemmaState,
    pub cache: CacheState,
    pub context: ContextState,
}

impl Default for ToolsState {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            router: RouterState::default(),
            functiongemma: FunctionGemmaState::default(),
            cache: CacheState::default(),
            context: ContextState::default(),
        }
    }
}
