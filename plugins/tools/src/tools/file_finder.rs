//! File finder tool implementation (The Navigator).
//!
//! Finds and opens files via Spotlight/mdfind on macOS.

use super::{Mode, Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::process::Command;

/// Tool for finding and opening files.
pub struct FileFinderTool;

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    /// Search for files using mdfind (Spotlight).
    pub fn find_files(
        query: &str,
        scope: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, ToolError> {
        let mut cmd = Command::new("mdfind");

        // Add scope if provided
        if let Some(dir) = scope {
            cmd.arg("-onlyin").arg(dir);
        }

        cmd.arg(query);

        let output = cmd
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("mdfind failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::ExecutionFailed(format!(
                "mdfind error: {}",
                stderr
            )));
        }

        let results: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .take(limit)
            .map(String::from)
            .collect();

        Ok(results)
    }

    /// Open a file in the default application.
    pub fn open_file(path: &str) -> Result<String, ToolError> {
        let output = Command::new("open")
            .arg(path)
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("open failed: {}", e)))?;

        if output.status.success() {
            Ok(format!("Opened {}", path))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ToolError::ExecutionFailed(format!(
                "Failed to open: {}",
                stderr
            )))
        }
    }

    /// Open a file in a specific application.
    pub fn open_with(path: &str, app: &str) -> Result<String, ToolError> {
        let output = Command::new("open")
            .args(["-a", app, path])
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("open failed: {}", e)))?;

        if output.status.success() {
            Ok(format!("Opened {} with {}", path, app))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ToolError::ExecutionFailed(format!(
                "Failed to open: {}",
                stderr
            )))
        }
    }

    /// Reveal a file in Finder.
    pub fn reveal_in_finder(path: &str) -> Result<String, ToolError> {
        let output = Command::new("open")
            .args(["-R", path])
            .output()
            .map_err(|e| ToolError::ExecutionFailed(format!("open failed: {}", e)))?;

        if output.status.success() {
            Ok(format!("Revealed {} in Finder", path))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ToolError::ExecutionFailed(format!(
                "Failed to reveal: {}",
                stderr
            )))
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod macos {
    use super::*;

    pub fn find_files(
        _query: &str,
        _scope: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<String>, ToolError> {
        Err(ToolError::ExecutionFailed(
            "File finder is only supported on macOS".to_string(),
        ))
    }

    pub fn open_file(_path: &str) -> Result<String, ToolError> {
        Err(ToolError::ExecutionFailed(
            "File finder is only supported on macOS".to_string(),
        ))
    }

    pub fn open_with(_path: &str, _app: &str) -> Result<String, ToolError> {
        Err(ToolError::ExecutionFailed(
            "File finder is only supported on macOS".to_string(),
        ))
    }

    pub fn reveal_in_finder(_path: &str) -> Result<String, ToolError> {
        Err(ToolError::ExecutionFailed(
            "File finder is only supported on macOS".to_string(),
        ))
    }
}

#[async_trait]
impl Tool for FileFinderTool {
    fn name(&self) -> &'static str {
        "file_finder"
    }

    fn description(&self) -> &'static str {
        "Find and open files using Spotlight search"
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "find config files",
            "search for readme",
            "open package.json",
            "reveal in Finder",
        ]
    }

    fn few_shot_examples(&self) -> &'static [&'static str] {
        &[
            "User: find config files\n<start_function_call>call:file_finder{action:<escape>find<escape>,query:<escape>config<escape>}<end_function_call>",
        ]
    }

    // Available in Dev mode
    fn modes(&self) -> &'static [Mode] {
        &[Mode::Dev]
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Action: find (search files), open (open file), reveal (show in Finder)",
                    "enum": ["find", "search", "open", "reveal"],
                    "default": "find"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (for find action)"
                },
                "path": {
                    "type": "string",
                    "description": "File path (for open/reveal actions)"
                },
                "scope": {
                    "type": "string",
                    "description": "Directory to search in (for find action)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results",
                    "default": 10
                },
                "app": {
                    "type": "string",
                    "description": "Application to open file with"
                }
            },
            "required": []
        })
    }

    fn cache_key(&self, args: &serde_json::Value) -> Option<String> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("find");
        if action == "find" || action == "search" {
            args.get("query")
                .and_then(|v| v.as_str())
                .map(|q| format!("find:{}", q.to_lowercase()))
        } else {
            None
        }
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("find");

        match action {
            "find" | "search" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or(ToolError::MissingArg("query"))?;

                let scope = args.get("scope").and_then(|v| v.as_str());
                let limit = args
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(10);

                let results = macos::find_files(query, scope, limit)?;

                Ok(ToolResult {
                    event_name: "tools:file_finder",
                    payload: serde_json::json!({
                        "action": "find",
                        "query": query,
                        "results": results,
                        "count": results.len(),
                    }),
                    cache_key: Some(format!("find:{}", query.to_lowercase())),
                    cooldown_key: None,
                })
            }
            "open" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or(ToolError::MissingArg("path"))?;

                let message = if let Some(app) = args.get("app").and_then(|v| v.as_str()) {
                    macos::open_with(path, app)?
                } else {
                    macos::open_file(path)?
                };

                Ok(ToolResult {
                    event_name: "tools:file_finder",
                    payload: serde_json::json!({
                        "action": "open",
                        "path": path,
                        "message": message,
                    }),
                    cache_key: None,
                    cooldown_key: Some(format!("open:{}", path)),
                })
            }
            "reveal" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or(ToolError::MissingArg("path"))?;

                let message = macos::reveal_in_finder(path)?;

                Ok(ToolResult {
                    event_name: "tools:file_finder",
                    payload: serde_json::json!({
                        "action": "reveal",
                        "path": path,
                        "message": message,
                    }),
                    cache_key: None,
                    cooldown_key: Some(format!("reveal:{}", path)),
                })
            }
            _ => Err(ToolError::ExecutionFailed(format!(
                "Unknown action: {}",
                action
            ))),
        }
    }
}
