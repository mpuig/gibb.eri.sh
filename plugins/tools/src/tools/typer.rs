//! Typer tool for voice-controlled text input.
//!
//! Allows users to type text by voice command.
//! Requires accessibility permissions on macOS.

use super::{Tool, ToolContext, ToolError, ToolResult};
use crate::adapters::PlatformFocusChecker;
use async_trait::async_trait;
use gibberish_input::{FocusChecker, InputController, TypeOptions};
use serde_json::json;
use std::sync::Arc;

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

    fn few_shot_examples(&self) -> &'static [&'static str] {
        &[
            "User: type hello world\n<start_function_call>call:typer{text:<escape>hello world<escape>}<end_function_call>",
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
        false
    }

    fn cache_key(&self, _args: &serde_json::Value) -> Option<String> {
        None
    }

    fn cooldown_key(&self, _args: &serde_json::Value) -> Option<String> {
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

        // Run typing on a blocking thread since InputController contains
        // platform-specific types that may not be Send
        let result = tokio::task::spawn_blocking(move || type_text_blocking(&text))
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

        Ok(ToolResult {
            event_name: "tools:typer_result",
            payload: json!({
                "text": result.text,
                "chars_typed": result.chars_typed,
                "completed": result.completed,
            }),
            cache_key: None,
            cooldown_key: None,
        })
    }
}

/// Result from blocking typing operation.
struct TyperResult {
    text: String,
    chars_typed: usize,
    completed: bool,
}

/// Execute typing on the current thread (called via spawn_blocking).
fn type_text_blocking(text: &str) -> Result<TyperResult, ToolError> {
    // Create focus checker using the shared adapter
    let focus_checker: Arc<dyn FocusChecker> = Arc::new(PlatformFocusChecker::new());

    // Create input controller with focus verification
    let mut controller = InputController::new(Some(focus_checker))?;

    // Type with default options
    let result = controller.type_text(text, TypeOptions::default())?;

    Ok(TyperResult {
        text: text.to_string(),
        chars_typed: result.chars_typed,
        completed: result.completed,
    })
}
