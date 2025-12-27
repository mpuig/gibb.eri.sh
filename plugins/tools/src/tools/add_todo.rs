//! Add todo tool implementation (The Scribe).
//!
//! Captures action items during meetings and adds them to task managers.

use super::{Mode, Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::process::Command;

/// Tool for adding todos/action items.
pub struct AddTodoTool;

/// Priority level for todos.
#[derive(Debug, Clone, Copy)]
pub enum Priority {
    Low,
    Medium,
    High,
}

impl Priority {
    fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "high" | "urgent" | "important" | "!" => Priority::High,
            "low" | "minor" => Priority::Low,
            _ => Priority::Medium,
        }
    }

    fn to_reminders_priority(&self) -> u8 {
        match self {
            Priority::High => 1,
            Priority::Medium => 5,
            Priority::Low => 9,
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    /// Add a reminder using AppleScript (Apple Reminders app).
    pub fn add_reminder(
        title: &str,
        notes: Option<&str>,
        list: Option<&str>,
        priority: Priority,
    ) -> Result<String, ToolError> {
        let list_name = list.unwrap_or("Reminders");
        let priority_num = priority.to_reminders_priority();

        let script = if let Some(n) = notes {
            format!(
                r#"
                tell application "Reminders"
                    set targetList to list "{}"
                    tell targetList
                        make new reminder with properties {{name:"{}", body:"{}", priority:{}}}
                    end tell
                end tell
                "#,
                list_name,
                title.replace('"', "\\\""),
                n.replace('"', "\\\""),
                priority_num
            )
        } else {
            format!(
                r#"
                tell application "Reminders"
                    set targetList to list "{}"
                    tell targetList
                        make new reminder with properties {{name:"{}", priority:{}}}
                    end tell
                end tell
                "#,
                list_name,
                title.replace('"', "\\\""),
                priority_num
            )
        };

        let output = Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("osascript failed: {}", e)))?;

        if output.status.success() {
            Ok(format!("Added reminder: {}", title))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ToolError::ExecutionFailed(format!(
                "Failed to add reminder: {}",
                stderr
            )))
        }
    }

    /// List available reminder lists.
    pub fn list_reminder_lists() -> Result<Vec<String>, ToolError> {
        let script = r#"
            tell application "Reminders"
                set listNames to name of every list
                set output to ""
                repeat with listName in listNames
                    set output to output & listName & linefeed
                end repeat
                return output
            end tell
        "#;

        let output = Command::new("osascript")
            .args(["-e", script])
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("osascript failed: {}", e)))?;

        if output.status.success() {
            let lists: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            Ok(lists)
        } else {
            Err(ToolError::ExecutionFailed(
                "Failed to list reminders".to_string(),
            ))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    use super::*;

    pub fn add_reminder(
        _title: &str,
        _notes: Option<&str>,
        _list: Option<&str>,
        _priority: Priority,
    ) -> Result<String, ToolError> {
        Err(ToolError::ExecutionFailed(
            "Add todo is only supported on macOS".to_string(),
        ))
    }

    pub fn list_reminder_lists() -> Result<Vec<String>, ToolError> {
        Err(ToolError::ExecutionFailed(
            "Add todo is only supported on macOS".to_string(),
        ))
    }
}

#[async_trait]
impl Tool for AddTodoTool {
    fn name(&self) -> &'static str {
        "add_todo"
    }

    fn description(&self) -> &'static str {
        "Add reminders and action items to Apple Reminders"
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "add todo follow up with John",
            "remind me to send the report",
            "action item: review proposal",
            "high priority: call client",
        ]
    }

    // Available in Meeting mode (for capturing action items)
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
                    "description": "Action: add (create reminder), list_lists (show reminder lists)",
                    "enum": ["add", "list_lists"],
                    "default": "add"
                },
                "title": {
                    "type": "string",
                    "description": "Reminder title/task description"
                },
                "notes": {
                    "type": "string",
                    "description": "Additional notes for the reminder"
                },
                "list": {
                    "type": "string",
                    "description": "Reminder list name (defaults to 'Reminders')"
                },
                "priority": {
                    "type": "string",
                    "description": "Priority level: high, medium, low",
                    "enum": ["high", "medium", "low"],
                    "default": "medium"
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
        let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("add");

        match action {
            "add" => {
                let title = args
                    .get("title")
                    .or_else(|| args.get("task"))
                    .or_else(|| args.get("text"))
                    .and_then(|v| v.as_str())
                    .ok_or(ToolError::MissingArg("title"))?;

                let notes = args.get("notes").and_then(|v| v.as_str());
                let list = args.get("list").and_then(|v| v.as_str());
                let priority = args
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .map(Priority::parse)
                    .unwrap_or(Priority::Medium);

                let message = macos::add_reminder(title, notes, list, priority)?;

                Ok(ToolResult {
                    event_name: "tools:add_todo",
                    payload: serde_json::json!({
                        "action": "add",
                        "title": title,
                        "list": list.unwrap_or("Reminders"),
                        "message": message,
                    }),
                    cache_key: None,
                    cooldown_key: Some(format!("todo:{}", title.to_lowercase())),
                })
            }
            "list_lists" => {
                let lists = macos::list_reminder_lists()?;

                Ok(ToolResult {
                    event_name: "tools:add_todo",
                    payload: serde_json::json!({
                        "action": "list_lists",
                        "lists": lists,
                    }),
                    cache_key: Some("reminder_lists".to_string()),
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
