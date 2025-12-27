//! Tools plugin state management.

mod cache;
mod functiongemma;
mod router;

pub use cache::{CacheEntry, CacheState};
pub use functiongemma::{FunctionGemmaDownload, FunctionGemmaModel, FunctionGemmaState};
pub use router::RouterState;

use std::sync::Arc;

use gibberish_context::ContextState;
use gibberish_events::{EventBusRef, NullEventBus};

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
pub struct ToolsState {
    pub client: reqwest::Client,
    pub router: RouterState,
    pub functiongemma: FunctionGemmaState,
    pub cache: CacheState,
    pub context: ContextState,
    pub event_bus: EventBusRef,
}

impl ToolsState {
    /// Create a new ToolsState with the given event bus.
    pub fn new(event_bus: EventBusRef) -> Self {
        Self {
            client: reqwest::Client::new(),
            router: RouterState::default(),
            functiongemma: FunctionGemmaState::default(),
            cache: CacheState::default(),
            context: ContextState::default(),
            event_bus,
        }
    }
}

impl std::fmt::Debug for ToolsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolsState")
            .field("client", &"reqwest::Client")
            .field("router", &self.router)
            .field("functiongemma", &self.functiongemma)
            .field("cache", &self.cache)
            .field("context", &self.context)
            .field("event_bus", &"EventBusRef")
            .finish()
    }
}

impl Default for ToolsState {
    fn default() -> Self {
        Self::new(Arc::new(NullEventBus))
    }
}
