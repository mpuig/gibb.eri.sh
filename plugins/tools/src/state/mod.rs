//! Tools plugin state management.

mod cache;
mod functiongemma;
mod router;

pub use cache::{CacheEntry, CacheState};
pub use functiongemma::{FunctionGemmaDownload, FunctionGemmaModel, FunctionGemmaState};
pub use router::RouterState;

use std::sync::atomic::{AtomicBool, Ordering};
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

/// Global abort flag for panic hotkey (Esc x3).
/// When set, all input operations should stop immediately.
pub type GlobalAbortFlag = Arc<AtomicBool>;

/// Combined state for the tools plugin.
pub struct ToolsState {
    pub client: reqwest::Client,
    pub router: RouterState,
    pub functiongemma: FunctionGemmaState,
    pub cache: CacheState,
    pub context: ContextState,
    pub event_bus: EventBusRef,
    /// Global abort flag set by panic hotkey (Esc x3).
    pub global_abort: GlobalAbortFlag,
}

impl ToolsState {
    /// Create a new ToolsState with the given event bus.
    pub fn new(event_bus: EventBusRef) -> Self {
        Self::with_abort_flag(event_bus, Arc::new(AtomicBool::new(false)))
    }

    /// Create a new ToolsState with a shared abort flag.
    pub fn with_abort_flag(event_bus: EventBusRef, global_abort: GlobalAbortFlag) -> Self {
        Self {
            client: reqwest::Client::new(),
            router: RouterState::default(),
            functiongemma: FunctionGemmaState::default(),
            cache: CacheState::default(),
            context: ContextState::default(),
            event_bus,
            global_abort,
        }
    }

    /// Check if the global abort flag is set.
    pub fn is_aborted(&self) -> bool {
        self.global_abort.load(Ordering::SeqCst)
    }

    /// Clear the global abort flag.
    pub fn clear_abort(&self) {
        self.global_abort.store(false, Ordering::SeqCst);
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
            .field("global_abort", &self.is_aborted())
            .finish()
    }
}

impl Default for ToolsState {
    fn default() -> Self {
        Self::new(Arc::new(NullEventBus))
    }
}
