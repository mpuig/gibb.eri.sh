//! App launcher tool implementation.
//!
//! Opens applications and switches between windows.

use super::{Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Tool for launching and switching to applications.
pub struct AppLauncherTool;

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use tokio::process::Command;

    pub async fn open_app(app_name: &str) -> Result<String, ToolError> {
        // Try to open by name first
        let output = Command::new("open")
            .args(["-a", app_name])
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if output.status.success() {
            Ok(format!("Opened {}", app_name))
        } else {
            // Try with fuzzy matching via osascript
            let script = format!(
                r#"
                tell application "System Events"
                    set appList to name of every process whose background only is false
                end tell
                set matchedApp to ""
                repeat with appName in appList
                    if appName contains "{}" then
                        set matchedApp to appName
                        exit repeat
                    end if
                end repeat
                if matchedApp is not "" then
                    tell application matchedApp to activate
                    return matchedApp
                else
                    error "App not found: {}"
                end if
                "#,
                app_name, app_name
            );

            let output = Command::new("osascript")
                .args(["-e", &script])
                .output()
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            if output.status.success() {
                let opened_app = String::from_utf8_lossy(&output.stdout).trim().to_string();
                Ok(format!("Activated {}", opened_app))
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(ToolError::ExecutionFailed(format!(
                    "Could not open {}: {}",
                    app_name, stderr
                )))
            }
        }
    }

    pub async fn list_running_apps() -> Result<Vec<String>, ToolError> {
        let script = r#"
            tell application "System Events"
                set appList to name of every process whose background only is false
                set output to ""
                repeat with appName in appList
                    set output to output & appName & linefeed
                end repeat
                return output
            end tell
        "#;

        let output = Command::new("osascript")
            .args(["-e", script])
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        if output.status.success() {
            let apps: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect();
            Ok(apps)
        } else {
            Err(ToolError::ExecutionFailed(
                "Failed to list running apps".to_string(),
            ))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    use super::*;

    pub async fn open_app(_app_name: &str) -> Result<String, ToolError> {
        Err(ToolError::ExecutionFailed(
            "App launcher is only supported on macOS".to_string(),
        ))
    }

    pub async fn list_running_apps() -> Result<Vec<String>, ToolError> {
        Err(ToolError::ExecutionFailed(
            "App launcher is only supported on macOS".to_string(),
        ))
    }
}

use std::borrow::Cow;

#[async_trait]
impl Tool for AppLauncherTool {
    fn name(&self) -> Cow<'static, str> {
        "app_launcher".into()
    }

    fn description(&self) -> Cow<'static, str> {
        "Open applications or switch to running apps".into()
    }

    fn selection_hint(&self) -> Option<Cow<'static, str>> {
        Some("open/launch/switch to app".into())
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "open Safari",
            "switch to Slack",
            "launch VS Code",
            "what apps are running",
        ]
    }

    fn few_shot_examples(&self) -> &'static [&'static str] {
        &[
            "User: open Safari\n<start_function_call>call:app_launcher{action:<escape>open<escape>,app:<escape>Safari<escape>}<end_function_call>",
            "User: switch to Slack\n<start_function_call>call:app_launcher{action:<escape>switch<escape>,app:<escape>Slack<escape>}<end_function_call>",
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
                    "description": "Action: open (launch/switch to app), list (show running apps)",
                    "enum": ["open", "switch", "list"],
                    "default": "open"
                },
                "app": {
                    "type": "string",
                    "description": "Application name (required for open/switch)"
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
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("open");

        match action {
            "open" | "switch" => {
                let app_name = args
                    .get("app")
                    .and_then(|v| v.as_str())
                    .ok_or(ToolError::MissingArg("app"))?;

                let message = macos::open_app(app_name).await?;

                Ok(ToolResult {
                    event_name: Cow::Borrowed("tools:app_launcher"),
                    payload: serde_json::json!({
                        "action": action,
                        "app": app_name,
                        "success": true,
                        "message": message,
                    }),
                    cache_key: None,
                    cooldown_key: Some(format!("open:{}", app_name.to_lowercase())),
                })
            }
            "list" => {
                let apps = macos::list_running_apps().await?;

                Ok(ToolResult {
                    event_name: Cow::Borrowed("tools:app_launcher"),
                    payload: serde_json::json!({
                        "action": "list",
                        "apps": apps,
                        "count": apps.len(),
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
