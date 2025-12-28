//! Tools plugin state management.

mod cache;
mod functiongemma;
mod router;

pub use cache::{CacheEntry, CacheState};
pub use functiongemma::{FunctionGemmaDownload, FunctionGemmaModel, FunctionGemmaState};
pub use router::RouterState;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::skill_loader::SkillManager;
use crate::tool_pack_loader::ToolPackManager;
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
    /// Loaded skills for user-defined tools (legacy SKILL.md format).
    pub skills: SkillManager,
    /// Loaded tool packs (primary .tool.json format).
    pub tool_packs: ToolPackManager,
}

impl ToolsState {
    /// Create a new ToolsState with the given event bus.
    pub fn new(event_bus: EventBusRef) -> Self {
        Self::with_abort_flag(event_bus, Arc::new(AtomicBool::new(false)))
    }

    /// Create a new ToolsState with a shared abort flag.
    pub fn with_abort_flag(event_bus: EventBusRef, global_abort: GlobalAbortFlag) -> Self {
        // Load skills at startup (legacy SKILL.md format)
        let skills = SkillManager::new();

        // Load tool packs at startup (primary .tool.json format)
        let tool_packs = ToolPackManager::new();

        // Create router with all tool sources
        let router = RouterState::with_all_tools(&skills, &tool_packs);

        Self {
            client: reqwest::Client::new(),
            router,
            functiongemma: FunctionGemmaState::default(),
            cache: CacheState::default(),
            context: ContextState::default(),
            event_bus,
            global_abort,
            skills,
            tool_packs,
        }
    }

    /// Reload skills from disk and update the router.
    pub fn reload_skills(&mut self) -> crate::skill_loader::ReloadResult {
        let result = self.skills.reload();

        // Update router with all tool sources
        self.update_router();

        tracing::info!(
            skill_count = result.skill_count,
            tool_count = result.tool_count,
            error_count = result.errors.len(),
            "Skills reloaded"
        );

        result
    }

    /// Reload tool packs from disk and update the router.
    pub fn reload_tool_packs(&mut self) -> crate::tool_pack_loader::ReloadResult {
        let result = self.tool_packs.reload();

        // Update router with all tool sources
        self.update_router();

        tracing::info!(
            pack_count = result.pack_count,
            error_count = result.errors.len(),
            "Tool packs reloaded"
        );

        result
    }

    /// Reload all external tools (skills + tool packs) and update the router.
    pub fn reload_all_tools(&mut self) {
        let skills_result = self.skills.reload();
        let packs_result = self.tool_packs.reload();

        self.update_router();

        tracing::info!(
            skill_count = skills_result.skill_count,
            pack_count = packs_result.pack_count,
            "All tools reloaded"
        );
    }

    /// Update the router with the current tool sources.
    fn update_router(&mut self) {
        let mode = self.context.effective_mode();
        let registry =
            crate::registry::ToolRegistry::build_all_sources(&self.skills, &self.tool_packs);
        self.router.update_with_registry(&registry, mode);
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
            .field("skills", &self.skills)
            .field("tool_packs", &self.tool_packs)
            .finish()
    }
}

impl Default for ToolsState {
    fn default() -> Self {
        Self::new(Arc::new(NullEventBus))
    }
}
