//! Inference orchestration for FunctionGemma.
//!
//! Handles primary inference, follow-up inference, and summary generation
//! with proper event emission.

use crate::functiongemma::{FunctionGemmaError, FunctionGemmaRunner, ModelOutput};
use gibberish_events::EventBus;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Result of an inference operation.
pub enum InferenceResult {
    Success(ModelOutput),
    Cancelled,
    Error(String),
}

/// Run primary inference with cancellation support.
pub async fn run_primary_inference(
    runner: Arc<FunctionGemmaRunner>,
    developer_context: Arc<str>,
    user_text: String,
    cancel_token: CancellationToken,
) -> InferenceResult {
    let result = tokio::task::spawn_blocking(move || {
        if cancel_token.is_cancelled() {
            return Err(FunctionGemmaError::Inference("cancelled".to_string()));
        }
        runner.infer_once(developer_context.as_ref(), &user_text)
    })
    .await;

    match result {
        Ok(Ok(output)) => InferenceResult::Success(output),
        Ok(Err(FunctionGemmaError::Inference(msg))) if msg == "cancelled" => {
            InferenceResult::Cancelled
        }
        Ok(Err(e)) => InferenceResult::Error(e.to_string()),
        Err(e) => InferenceResult::Error(e.to_string()),
    }
}

/// Run follow-up inference after a tool execution.
pub async fn run_followup_inference(
    runner: Arc<FunctionGemmaRunner>,
    developer_context: Arc<str>,
    original_text: &str,
    tool_name: &str,
    tool_output: &serde_json::Value,
) -> InferenceResult {
    // Build follow-up prompt with output preview
    let output_preview = serde_json::to_string(tool_output).unwrap_or_else(|_| tool_output.to_string());
    let output_preview = if output_preview.len() > 1400 {
        format!("{}...", &output_preview[..1400])
    } else {
        output_preview
    };

    let followup_text = format!(
        "Original request:\n{}\n\nTool `{}` output (JSON):\n{}\n\nIf another tool should be called to fully satisfy the original request, call it now. Otherwise output <end_of_turn>.",
        original_text, tool_name, output_preview
    );

    let result = tokio::task::spawn_blocking(move || {
        runner.infer_once(developer_context.as_ref(), &followup_text)
    })
    .await;

    match result {
        Ok(Ok(output)) => InferenceResult::Success(output),
        Ok(Err(e)) => InferenceResult::Error(e.to_string()),
        Err(e) => InferenceResult::Error(e.to_string()),
    }
}

/// Generate a natural language summary of tool output.
pub async fn generate_summary(
    runner: Arc<FunctionGemmaRunner>,
    tool_name: String,
    tool_output: serde_json::Value,
    user_text: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        runner.summarize_tool_output(&tool_name, &tool_output, &user_text)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r.map_err(|e| e.to_string()))
}

/// Validate tool arguments and attempt repair via inference if invalid.
pub async fn validate_and_repair_args(
    runner: Arc<FunctionGemmaRunner>,
    developer_context: Arc<str>,
    pending_text: &str,
    tool_name: &str,
    args: serde_json::Value,
    validate_fn: impl FnOnce(&serde_json::Value) -> Result<(), String>,
) -> Result<serde_json::Value, String> {
    // If args are already valid, return them as-is
    if validate_fn(&args).is_ok() {
        return Ok(args);
    }

    // Attempt repair via inference
    let tool = tool_name.to_string();
    let text = pending_text.to_string();

    tokio::task::spawn_blocking(move || {
        runner.infer_args_object(developer_context.as_ref(), &tool, &text)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r.map_err(|e| e.to_string()))
}

/// Helper to emit inference status events.
pub fn emit_inference_status(event_bus: &dyn EventBus, phase: &str, payload: serde_json::Value) {
    event_bus.emit(
        gibberish_events::event_names::ROUTER_STATUS,
        serde_json::json!({
            "phase": phase,
            "ts_ms": chrono::Utc::now().timestamp_millis(),
            "payload": payload,
        }),
    );
}
