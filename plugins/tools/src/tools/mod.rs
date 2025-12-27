//! Tool trait and implementations.
//!
//! Tools are adapters that execute specific actions (e.g., Wikipedia lookups).
//! Each tool implements the `Tool` trait and is registered in the `ToolRegistry`.

mod add_todo;
mod app_launcher;
mod file_finder;
mod git_voice;
mod help;
mod system_control;
mod transcript_marker;
mod web_search;
mod wikipedia;

pub use add_todo::AddTodoTool;
pub use app_launcher::AppLauncherTool;
pub use file_finder::FileFinderTool;
pub use git_voice::GitVoiceTool;
pub use help::{ToolInfo, ToolInfoProvider};
pub use system_control::SystemControlTool;
pub use transcript_marker::TranscriptMarkerTool;
pub use web_search::WebSearchTool;

use async_trait::async_trait;
use gibberish_context::Mode;
use std::sync::Arc;

use crate::environment::SystemEnvironment;

/// JSON schema definition for a tool, used to build dynamic manifests.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub read_only: bool,
    pub args_schema: serde_json::Value,
}

/// Context passed to tool execution.
///
/// Contains the system environment abstraction for testability.
pub struct ToolContext {
    /// System environment for OS operations.
    pub env: Arc<dyn SystemEnvironment>,
    /// Default language for tools that support i18n.
    pub default_lang: String,
}

impl ToolContext {
    /// Create a new context with the given environment.
    pub fn new(env: Arc<dyn SystemEnvironment>, default_lang: String) -> Self {
        Self { env, default_lang }
    }

    /// Get the HTTP client from the environment.
    pub fn client(&self) -> &reqwest::Client {
        self.env.http_client()
    }
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

    /// Human-readable description of what the tool does.
    fn description(&self) -> &'static str {
        ""
    }

    /// Example voice phrases that trigger this tool.
    fn example_phrases(&self) -> &'static [&'static str] {
        &[]
    }

    /// Modes in which this tool is available.
    /// Return empty slice for tools that are always available (Global mode).
    /// Return specific modes for context-filtered tools.
    fn modes(&self) -> &'static [Mode] {
        // Default: available in all modes (Global)
        &[]
    }

    /// Check if this tool is available in the given mode.
    fn is_available_in(&self, mode: Mode) -> bool {
        let modes = self.modes();
        // Empty modes means available everywhere
        if modes.is_empty() {
            return true;
        }
        // Check if current mode is in the list
        modes.contains(&mode)
    }

    /// Whether this tool is read-only (no side effects).
    fn is_read_only(&self) -> bool {
        true
    }

    /// JSON schema for the tool's arguments.
    fn args_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    /// Build the tool definition for the dynamic manifest.
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            read_only: self.is_read_only(),
            args_schema: self.args_schema(),
        }
    }

    /// Generate a cache key for the given arguments.
    /// Returns None if caching is not supported for this tool.
    #[allow(dead_code)]
    fn cache_key(&self, _args: &serde_json::Value) -> Option<String> {
        None
    }

    /// Execute the tool with given arguments.
    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError>;
}
