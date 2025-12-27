//! Transcript marker tool implementation.
//!
//! Allows marking moments in transcripts during meetings.

use super::{Mode, Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Tool for marking moments in meeting transcripts.
pub struct TranscriptMarkerTool;

/// Type of marker to add.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerType {
    ActionItem,
    Decision,
    Question,
    Important,
    FollowUp,
    Bookmark,
}

impl MarkerType {
    fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "action" | "action_item" | "todo" | "task" => Some(MarkerType::ActionItem),
            "decision" | "decided" => Some(MarkerType::Decision),
            "question" | "ask" | "?" => Some(MarkerType::Question),
            "important" | "key" | "!" => Some(MarkerType::Important),
            "followup" | "follow_up" | "follow-up" => Some(MarkerType::FollowUp),
            "bookmark" | "mark" | "flag" => Some(MarkerType::Bookmark),
            _ => None,
        }
    }

    fn emoji(&self) -> &'static str {
        match self {
            MarkerType::ActionItem => "📋",
            MarkerType::Decision => "✅",
            MarkerType::Question => "❓",
            MarkerType::Important => "⭐",
            MarkerType::FollowUp => "🔄",
            MarkerType::Bookmark => "🔖",
        }
    }
}

/// A marker in the transcript.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Marker {
    pub marker_type: MarkerType,
    pub note: Option<String>,
    pub timestamp_ms: i64,
}

#[async_trait]
impl Tool for TranscriptMarkerTool {
    fn name(&self) -> &'static str {
        "transcript_marker"
    }

    fn description(&self) -> &'static str {
        "Mark important moments in meeting transcripts"
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "mark this as important",
            "flag as action item",
            "bookmark this",
            "mark decision",
            "highlight this moment",
        ]
    }

    fn few_shot_examples(&self) -> &'static [&'static str] {
        &[
            "User: mark this as important\n<start_function_call>call:transcript_marker{action:<escape>mark<escape>,type:<escape>important<escape>}<end_function_call>",
        ]
    }

    // Available in Meeting mode
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Meeting]
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Action: mark (add marker)",
                    "enum": ["mark", "add"],
                    "default": "mark"
                },
                "type": {
                    "type": "string",
                    "description": "Marker type",
                    "enum": ["action_item", "decision", "question", "important", "follow_up", "bookmark"],
                    "default": "bookmark"
                },
                "note": {
                    "type": "string",
                    "description": "Optional note to attach to the marker"
                }
            },
            "required": []
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("mark");

        match action {
            "mark" | "add" => {
                let marker_type_str = args
                    .get("type")
                    .or_else(|| args.get("marker_type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("bookmark");

                let marker_type =
                    MarkerType::parse(marker_type_str).unwrap_or(MarkerType::Bookmark);

                let note = args.get("note").and_then(|v| v.as_str()).map(String::from);

                let timestamp_ms = chrono::Utc::now().timestamp_millis();

                let marker = Marker {
                    marker_type,
                    note: note.clone(),
                    timestamp_ms,
                };

                // The frontend will handle storing and displaying markers
                Ok(ToolResult {
                    event_name: "tools:transcript_marker",
                    payload: serde_json::json!({
                        "action": "mark",
                        "marker": marker,
                        "emoji": marker_type.emoji(),
                        "message": format!(
                            "{} Marked as {}{}",
                            marker_type.emoji(),
                            marker_type_str,
                            note.map(|n| format!(": {}", n)).unwrap_or_default()
                        ),
                    }),
                    cache_key: None,
                    cooldown_key: None,
                })
            }
            _ => Err(ToolError::ExecutionFailed(format!(
                "Unknown action: {}",
                action
            ))),
        }
    }
}
