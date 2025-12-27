//! Shared event contracts for cross-plugin communication.
//!
//! This crate defines the formal contracts (DTOs) for events that flow
//! between plugins. Using shared types prevents runtime deserialization
//! errors from mismatched field names.
//!
//! Also provides the `EventBus` trait for decoupled event emission.

mod bus;

pub use bus::{EmittedEvent, EventBus, EventBusRef, InMemoryEventBus, NullEventBus};

use gibberish_context::Mode;
use serde::{Deserialize, Serialize};

/// Event emitted when STT produces a transcript commit.
///
/// Producers: stt-worker plugin
/// Consumers: tools plugin (router)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamCommitEvent {
    /// Transcribed text.
    pub text: String,
    /// Timestamp in milliseconds since epoch.
    #[serde(default)]
    pub ts_ms: Option<i64>,
}

/// Event emitted when system context changes.
///
/// Producers: tools plugin (context poller)
/// Consumers: frontend, tools plugin (router)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChangedEvent {
    /// Current context mode.
    pub mode: Mode,
    /// Detected mode before any pinning.
    pub detected_mode: Mode,
    /// User-pinned mode (overrides detection).
    #[serde(default)]
    pub pinned_mode: Option<Mode>,
    /// Active application bundle ID.
    #[serde(default)]
    pub active_app: Option<String>,
    /// Active application name.
    #[serde(default)]
    pub active_app_name: Option<String>,
    /// Whether a meeting is detected.
    #[serde(default)]
    pub is_meeting: bool,
    /// Timestamp in milliseconds.
    #[serde(default)]
    pub timestamp_ms: i64,
}

/// Event emitted when a tool is proposed for execution.
///
/// Producers: tools plugin (router)
/// Consumers: frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionProposedEvent {
    /// Tool name.
    pub tool: String,
    /// Tool arguments.
    pub args: serde_json::Value,
    /// Evidence text that triggered the proposal.
    pub evidence: String,
}

/// Event emitted when a tool execution completes.
///
/// Producers: tools plugin (executor)
/// Consumers: frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultEvent {
    /// Tool name.
    pub tool: String,
    /// Whether result was from cache.
    #[serde(default)]
    pub cached: bool,
    /// Tool-specific payload.
    pub payload: serde_json::Value,
}

/// Event emitted when a tool execution fails.
///
/// Producers: tools plugin (executor)
/// Consumers: frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolErrorEvent {
    /// Tool name.
    pub tool: String,
    /// Error message.
    pub error: String,
}

/// Event names as constants to prevent typos.
pub mod event_names {
    /// STT stream commit event.
    pub const STT_STREAM_COMMIT: &str = "stt:stream_commit";
    /// Context changed event.
    pub const CONTEXT_CHANGED: &str = "context:changed";
    /// Action proposed event.
    pub const ACTION_PROPOSED: &str = "tools:action_proposed";
    /// Router status event.
    pub const ROUTER_STATUS: &str = "tools:router_status";
    /// Tool error event.
    pub const TOOL_ERROR: &str = "tools:tool_error";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_commit_deserialize() {
        let json = r#"{"text": "hello world", "ts_ms": 12345}"#;
        let event: StreamCommitEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.text, "hello world");
        assert_eq!(event.ts_ms, Some(12345));
    }

    #[test]
    fn test_stream_commit_deserialize_minimal() {
        let json = r#"{"text": "hello"}"#;
        let event: StreamCommitEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.text, "hello");
        assert_eq!(event.ts_ms, None);
    }
}
