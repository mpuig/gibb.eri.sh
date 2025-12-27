//! Context injector for JIT context enrichment.
//!
//! Handles just-in-time fetching of contextual information (clipboard, selection, URL)
//! and enriches the system context before inference.

use gibberish_context::platform::{get_active_browser_url, get_clipboard_preview, get_selection_preview};
use gibberish_context::ContextState;

/// Result of context injection.
#[derive(Debug, Clone)]
pub struct InjectedContext {
    /// The prompt snippet with all context values.
    pub snippet: String,
    /// Whether clipboard content was available.
    pub has_clipboard: bool,
    /// Whether selection content was available.
    pub has_selection: bool,
    /// Whether browser URL was available.
    pub has_url: bool,
}

/// Inject JIT context into the context state and return a prompt snippet.
///
/// This fetches expensive context values (clipboard, selection, browser URL)
/// just-in-time before inference, rather than on every poll cycle.
pub fn inject_context(context: &mut ContextState) -> InjectedContext {
    // Fetch JIT context values
    context.system.clipboard_preview = get_clipboard_preview();
    context.system.selection_preview = get_selection_preview();
    context.system.active_url = get_active_browser_url();

    InjectedContext {
        snippet: context.to_prompt_snippet(),
        has_clipboard: context.system.clipboard_preview.is_some(),
        has_selection: context.system.selection_preview.is_some(),
        has_url: context.system.active_url.is_some(),
    }
}

/// Build enriched developer context with system context.
pub fn enrich_developer_context(base_context: &str, context_snippet: &str) -> String {
    format!(
        "{}\n\nCurrent Context:\n{}",
        base_context, context_snippet
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrich_developer_context() {
        let base = "You are a model";
        let snippet = "Mode: Global\nDate: 2024-01-01";
        let enriched = enrich_developer_context(base, snippet);

        assert!(enriched.contains("You are a model"));
        assert!(enriched.contains("Current Context:"));
        assert!(enriched.contains("Mode: Global"));
    }
}
