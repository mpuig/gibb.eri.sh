//! Paste tool for context-aware clipboard pasting.
//!
//! Handles "paste this" style commands by simulating Cmd+V (macOS) or Ctrl+V.
//! Faster than typing for large text, and preserves formatting.

use super::{Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use gibberish_input::InputController;
use serde_json::json;
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Tool for pasting clipboard contents via keyboard shortcut.
pub struct PasteTool;

#[async_trait]
impl Tool for PasteTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("paste")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Paste clipboard contents using system shortcut")
    }

    fn example_phrases(&self) -> &'static [&'static str] {
        &[
            "paste this",
            "paste the clipboard",
            "paste what I copied",
            "paste it here",
        ]
    }

    fn few_shot_examples(&self) -> &'static [&'static str] {
        &[
            "User: paste this\n<start_function_call>call:paste{}<end_function_call>",
            "User: paste what I copied\n<start_function_call>call:paste{}<end_function_call>",
        ]
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
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
        _args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        // Check for abort before starting
        if ctx.is_aborted() {
            return Err(ToolError::ExecutionFailed(
                "Aborted by panic hotkey".to_string(),
            ));
        }

        // Clone the abort flag for the blocking task
        let abort_flag = Arc::clone(&ctx.abort_flag);

        // Run paste on a blocking thread
        tokio::task::spawn_blocking(move || paste_blocking(abort_flag))
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Task join error: {}", e)))??;

        Ok(ToolResult {
            event_name: Cow::Borrowed("tools:paste_result"),
            payload: json!({
                "success": true,
            }),
            cache_key: None,
            cooldown_key: None,
        })
    }
}

/// Execute paste on the current thread (called via spawn_blocking).
fn paste_blocking(abort_flag: Arc<AtomicBool>) -> Result<(), ToolError> {
    // Check abort before starting
    if abort_flag.load(Ordering::SeqCst) {
        return Err(ToolError::ExecutionFailed(
            "Aborted by panic hotkey".to_string(),
        ));
    }

    // Create input controller (no focus checker needed for simple paste)
    let mut controller = InputController::new(None)?;

    // Execute paste
    controller.paste()?;

    Ok(())
}
