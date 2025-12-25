//! Tool trait and implementations.
//!
//! Tools are adapters that execute specific actions (e.g., Wikipedia lookups).
//! Each tool implements the `Tool` trait and is registered in the `ToolRegistry`.

mod wikipedia;

pub use wikipedia::WikipediaTool;

use async_trait::async_trait;

/// Context passed to tool execution.
#[derive(Clone)]
pub struct ToolContext {
    pub client: reqwest::Client,
    pub default_lang: String,
}

/// Result of tool execution.
///
/// Tools return ready-to-emit payloads and optional caching hints.
/// This keeps the executor generic and tool-agnostic.
#[derive(Debug)]
pub struct ToolResult {
    /// Event name to emit (e.g., "tools:wikipedia_city").
    pub event_name: &'static str,
    /// Ready-to-emit payload (tool formats this, not the executor).
    pub payload: serde_json::Value,
    /// Optional cache key. If set, the executor will cache this result.
    /// Format is tool-specific (e.g., "en:barcelona" for city lookups).
    pub cache_key: Option<String>,
    /// Optional cooldown key. If set, repeated calls with same key are throttled.
    /// Typically same as cache_key but tools can customize.
    #[allow(dead_code)]
    pub cooldown_key: Option<String>,
}

/// Error during tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("missing required argument: {0}")]
    MissingArg(&'static str),

    #[error("invalid argument: {field} - {reason}")]
    #[allow(dead_code)]
    InvalidArg { field: &'static str, reason: String },

    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl From<crate::wikipedia::WikipediaError> for ToolError {
    fn from(e: crate::wikipedia::WikipediaError) -> Self {
        ToolError::ExecutionFailed(e.to_string())
    }
}

/// Trait for executable tools.
///
/// Uses async_trait to make the trait dyn-compatible.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name (matches the manifest name).
    fn name(&self) -> &'static str;

    /// Execute the tool with given arguments.
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError>;
}
