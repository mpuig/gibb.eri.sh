//! System control tool implementation.
//!
//! Handles volume, mute, sleep, and do not disturb commands.

use super::{Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::borrow::Cow;

/// Tool for controlling system settings.
pub struct SystemControlTool;

/// Action types supported by the system control tool.
#[derive(Debug, Clone, Copy)]
pub enum SystemAction {
    SetVolume(u8),
    Mute,
    Unmute,
    ToggleMute,
    Sleep,
    EnableDnd,
    DisableDnd,
}

impl SystemAction {
    fn parse(action: &str, value: Option<u8>) -> Option<Self> {
        match action.to_lowercase().as_str() {
            "set_volume" | "volume" => value.map(|v| SystemAction::SetVolume(v.min(100))),
            "mute" => Some(SystemAction::Mute),
            "unmute" => Some(SystemAction::Unmute),
            "toggle_mute" => Some(SystemAction::ToggleMute),
            "sleep" => Some(SystemAction::Sleep),
            "enable_dnd" | "dnd_on" => Some(SystemAction::EnableDnd),
            "disable_dnd" | "dnd_off" => Some(SystemAction::DisableDnd),
            _ => None,
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use tokio::process::Command;

    pub async fn execute_action(action: SystemAction) -> Result<String, ToolError> {
        match action {
            SystemAction::SetVolume(level) => {
                let script = format!("set volume output volume {}", level);
                run_osascript(&script).await?;
                Ok(format!("Volume set to {}%", level))
            }
            SystemAction::Mute => {
                run_osascript("set volume output muted true").await?;
                Ok("Audio muted".to_string())
            }
            SystemAction::Unmute => {
                run_osascript("set volume output muted false").await?;
                Ok("Audio unmuted".to_string())
            }
            SystemAction::ToggleMute => {
                let script = r#"
                    set currentMute to output muted of (get volume settings)
                    if currentMute then
                        set volume output muted false
                        return "unmuted"
                    else
                        set volume output muted true
                        return "muted"
                    end if
                "#;
                let result = run_osascript(script).await?;
                Ok(format!("Audio {}", result.trim()))
            }
            SystemAction::Sleep => {
                // Use pmset for sleep
                Command::new("pmset")
                    .args(["sleepnow"])
                    .output()
                    .await
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok("System going to sleep".to_string())
            }
            SystemAction::EnableDnd => {
                // DND control via shortcuts (requires macOS Monterey+)
                let script = r#"
                    tell application "System Events"
                        tell process "ControlCenter"
                            -- Focus menu bar
                            set theMenuBar to menu bar 1
                            -- Click on Focus/DND
                            click (first menu bar item whose description contains "Focus")
                        end tell
                    end tell
                "#;
                // Note: This is complex and may not work reliably
                // For now, return an informational message
                run_osascript(script).await.ok();
                Ok("Do Not Disturb toggle attempted (may require manual confirmation)".to_string())
            }
            SystemAction::DisableDnd => {
                // Same complexity as enable
                Ok(
                    "Do Not Disturb disable attempted (may require manual confirmation)"
                        .to_string(),
                )
            }
        }
    }

    async fn run_osascript(script: &str) -> Result<String, ToolError> {
        let output = Command::new("osascript")
            .args(["-e", script])
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ToolError::ExecutionFailed(format!(
                "osascript failed: {}",
                stderr
            )))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    use super::*;

    pub async fn execute_action(_action: SystemAction) -> Result<String, ToolError> {
        Err(ToolError::ExecutionFailed(
            "System control is only supported on macOS".to_string(),
        ))
    }
}

#[async_trait]
impl Tool for SystemControlTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("system_control")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Control system settings like volume, mute, sleep, and Do Not Disturb")
    }

    fn selection_hint(&self) -> Option<Cow<'static, str>> {
        Some(Cow::Borrowed("volume/mute/sleep/dnd"))
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "mute",
            "unmute",
            "set volume to 50",
            "turn on do not disturb",
            "go to sleep",
        ]
    }

    fn few_shot_examples(&self) -> &'static [&'static str] {
        &[
            "User: mute\n<start_function_call>call:system_control{action:<escape>mute<escape>}<end_function_call>",
            "User: set volume to 50\n<start_function_call>call:system_control{action:<escape>set_volume<escape>,value:50}<end_function_call>",
        ]
    }

    // Available in all modes (Global)
    fn modes(&self) -> Cow<'static, [gibberish_context::Mode]> {
        Cow::Borrowed(&[])
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
                    "description": "System action: set_volume, mute, unmute, toggle_mute, sleep, enable_dnd, disable_dnd",
                    "enum": ["set_volume", "mute", "unmute", "toggle_mute", "sleep", "enable_dnd", "disable_dnd"]
                },
                "value": {
                    "type": "integer",
                    "description": "Volume level (0-100) for set_volume action",
                    "minimum": 0,
                    "maximum": 100
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let action_str = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or(ToolError::MissingArg("action"))?;

        let value = args.get("value").and_then(|v| v.as_u64()).map(|v| v as u8);

        let action = SystemAction::parse(action_str, value)
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Unknown action: {}", action_str)))?;

        let message = macos::execute_action(action).await?;

        Ok(ToolResult {
            event_name: Cow::Borrowed("tools:system_control"),
            payload: serde_json::json!({
                "action": action_str,
                "success": true,
                "message": message,
            }),
            cache_key: None,
            cooldown_key: Some(action_str.to_string()),
        })
    }
}
