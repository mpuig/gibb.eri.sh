//! Typer tool for voice-controlled text input.
//!
//! Allows users to type text by voice command.
//! Requires accessibility permissions on macOS.

use super::{Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::json;

/// Tool for typing text via voice command.
pub struct TyperTool;

#[async_trait]
impl Tool for TyperTool {
    fn name(&self) -> &'static str {
        "typer"
    }

    fn description(&self) -> &'static str {
        "Type text using keyboard simulation"
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "type hello world",
            "write my email address",
            "enter the password",
            "type I'll be there in 5 minutes",
        ]
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The text to type. Extract exactly what the user wants typed."
                }
            },
            "required": ["text"]
        })
    }

    fn is_read_only(&self) -> bool {
        // Typing is NOT read-only - it modifies external state
        false
    }

    fn cache_key(&self, _args: &serde_json::Value) -> Option<String> {
        // Never cache typing operations
        None
    }

    fn cooldown_key(&self, _args: &serde_json::Value) -> Option<String> {
        // No cooldown for typing - each request should execute
        None
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .ok_or(ToolError::MissingArg("text"))?
            .to_string();

        // Check accessibility permissions first
        if !gibberish_input::has_accessibility_access() {
            return Err(ToolError::PermissionDenied(
                "Accessibility permission required. Enable in System Settings > Privacy & Security > Accessibility".to_string()
            ));
        }

        // Run typing on a blocking thread since InputController is not Send
        // (contains CGEventSource which can't cross thread boundaries via async)
        let text_clone = text.clone();
        let result = tokio::task::spawn_blocking(move || {
            type_text_sync(&text_clone)
        })
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))?
        .map_err(|e| ToolError::ExecutionFailed(e))?;

        Ok(ToolResult {
            event_name: "tools:typer_result",
            payload: json!({
                "text": text,
                "chars_typed": result.chars_typed,
                "completed": result.completed,
            }),
            cache_key: None,
            cooldown_key: None,
        })
    }
}

/// Synchronous typing helper for spawn_blocking.
fn type_text_sync(text: &str) -> Result<TypeResultDto, String> {
    use gibberish_input::{InputController, TypeOptions};

    let mut controller = InputController::new()
        .map_err(|e| format!("Failed to initialize input controller: {}", e))?;

    // Create a runtime for the async type_text call
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|e| format!("Failed to create runtime: {}", e))?;

    let result = rt.block_on(controller.type_text(text, TypeOptions::default()))
        .map_err(|e| format!("Typing failed: {}", e))?;

    Ok(TypeResultDto {
        chars_typed: result.chars_typed,
        completed: result.completed,
    })
}

/// DTO for type result to cross thread boundaries.
struct TypeResultDto {
    chars_typed: usize,
    completed: bool,
}
