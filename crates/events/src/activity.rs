//! Activity model for the unified activity feed.
//!
//! This is the single source of truth for activity data.
//! Frontend TypeScript mirrors this, SQLite persists this.

use serde::{Deserialize, Serialize};

/// Type of activity in the feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    /// Transcribed speech (STT output).
    Transcript,
    /// Voice command detected by router.
    VoiceCommand,
    /// Tool execution result.
    ToolResult,
    /// Tool execution error.
    ToolError,
    /// Recording session.
    Recording,
    /// Context/mode transition (only mode changes, not window focus).
    ContextChange,
}

/// Status of an activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityStatus {
    /// Activity not yet started.
    Pending,
    /// Activity in progress.
    Running,
    /// Activity completed successfully.
    Completed,
    /// Activity failed with error.
    Error,
}

/// Type-specific content for an activity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityContent {
    /// Transcript text or command text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Tool name (for VoiceCommand, ToolResult, ToolError).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,

    /// Tool arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,

    /// Tool result payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,

    /// Error message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Recording duration in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,

    /// Current mode (for ContextChange).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,

    /// Previous mode (for ContextChange transitions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_mode: Option<String>,

    /// Active app name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
}

/// Unified activity item for the feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Unique identifier (UUID).
    pub id: String,

    /// Type of activity.
    #[serde(rename = "type")]
    pub activity_type: ActivityType,

    /// Timestamp in milliseconds since epoch.
    pub timestamp: i64,

    /// Current status.
    pub status: ActivityStatus,

    /// Parent activity ID (for threading tool_result -> voice_command).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,

    /// Type-specific content.
    pub content: ActivityContent,

    /// UI display state (not persisted to DB).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
}

impl Activity {
    /// Create a new activity with generated ID.
    pub fn new(activity_type: ActivityType, status: ActivityStatus) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            activity_type,
            timestamp: chrono::Utc::now().timestamp_millis(),
            status,
            parent_id: None,
            content: ActivityContent::default(),
            expanded: None,
        }
    }

    /// Create a transcript activity.
    pub fn transcript(text: impl Into<String>) -> Self {
        let mut activity = Self::new(ActivityType::Transcript, ActivityStatus::Completed);
        activity.content.text = Some(text.into());
        activity
    }

    /// Create a voice command activity (pending tool execution).
    pub fn voice_command(text: impl Into<String>, tool: impl Into<String>) -> Self {
        let mut activity = Self::new(ActivityType::VoiceCommand, ActivityStatus::Running);
        activity.content.text = Some(text.into());
        activity.content.tool = Some(tool.into());
        activity
    }

    /// Create a tool result activity linked to a voice command.
    pub fn tool_result(
        parent_id: impl Into<String>,
        tool: impl Into<String>,
        result: serde_json::Value,
    ) -> Self {
        let mut activity = Self::new(ActivityType::ToolResult, ActivityStatus::Completed);
        activity.parent_id = Some(parent_id.into());
        activity.content.tool = Some(tool.into());
        activity.content.result = Some(result);
        activity
    }

    /// Create a tool error activity linked to a voice command.
    pub fn tool_error(
        parent_id: impl Into<String>,
        tool: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        let mut activity = Self::new(ActivityType::ToolError, ActivityStatus::Error);
        activity.parent_id = Some(parent_id.into());
        activity.content.tool = Some(tool.into());
        activity.content.error = Some(error.into());
        activity
    }

    /// Create a recording activity.
    pub fn recording(duration_ms: u64) -> Self {
        let mut activity = Self::new(ActivityType::Recording, ActivityStatus::Completed);
        activity.content.duration = Some(duration_ms);
        activity
    }

    /// Create a context change activity (mode transition only).
    pub fn context_change(prev_mode: impl Into<String>, new_mode: impl Into<String>) -> Self {
        let mut activity = Self::new(ActivityType::ContextChange, ActivityStatus::Completed);
        activity.content.prev_mode = Some(prev_mode.into());
        activity.content.mode = Some(new_mode.into());
        activity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_serialization() {
        let activity = Activity::transcript("Hello world");
        let json = serde_json::to_string(&activity).unwrap();
        assert!(json.contains("\"type\":\"transcript\""));
        assert!(json.contains("\"text\":\"Hello world\""));
    }

    #[test]
    fn test_voice_command_with_result() {
        let cmd = Activity::voice_command("open Safari", "app_launcher");
        let result = Activity::tool_result(&cmd.id, "app_launcher", serde_json::json!({"launched": true}));
        assert_eq!(result.parent_id, Some(cmd.id));
    }

    #[test]
    fn test_context_change() {
        let activity = Activity::context_change("Global", "Dev");
        assert_eq!(activity.content.prev_mode, Some("Global".to_string()));
        assert_eq!(activity.content.mode, Some("Dev".to_string()));
    }
}
