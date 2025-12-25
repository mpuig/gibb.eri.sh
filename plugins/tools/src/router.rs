//! Router for triggering tools based on STT commits.
//!
//! Receives transcript commits, runs FunctionGemma inference,
//! and dispatches tool execution via the registry.

use crate::executor::{execute_tool, ExecutionOutcome};
use crate::policy::DEBOUNCE;
use crate::registry::ToolRegistry;
use crate::tool_manifest::ToolPolicy;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Emitter, Manager, Runtime};
use tokio_util::sync::CancellationToken;

#[derive(Debug, serde::Deserialize)]
struct SttStreamCommitPayload {
    text: String,
    #[allow(dead_code)]
    ts_ms: Option<i64>,
}

fn emit_router_status<R: Runtime>(
    app: &tauri::AppHandle<R>,
    phase: &str,
    payload: serde_json::Value,
) {
    let _ = app.emit(
        "tools:router_status",
        serde_json::json!({
            "phase": phase,
            "ts_ms": chrono::Utc::now().timestamp_millis(),
            "payload": payload,
        }),
    );
}

fn policy_for_tool<'a>(
    policies: &'a HashMap<String, ToolPolicy>,
    tool: &str,
) -> Option<&'a ToolPolicy> {
    policies.get(tool)
}

/// Snapshot of router settings for use during processing.
#[derive(Debug)]
struct RouterSettingsSnapshot {
    auto_run_read_only: bool,
}

/// Main router processing loop.
///
/// Event-driven debounce: waits for new text notifications, then
/// processes after DEBOUNCE duration of silence.
async fn process_router_queue<R: Runtime>(app: tauri::AppHandle<R>) {
    let state = app.state::<crate::SharedState>();

    // Get the notify handle for event-driven wakeup
    let notify = {
        let guard = state.lock().await;
        Arc::clone(&guard.router.text_notify)
    };

    loop {
        // Wait for debounce timeout OR new text notification
        // If notified, restart the debounce timer (more text may be coming)
        loop {
            tokio::select! {
                biased;
                _ = notify.notified() => {
                    // New text arrived - restart debounce timer
                    continue;
                }
                _ = tokio::time::sleep(DEBOUNCE) => {
                    // Debounce timeout - process pending text
                    break;
                }
            }
        }

        // Extract state snapshot under lock
        let (
            pending_text,
            runner,
            enabled,
            router_settings,
            developer_context,
            tool_policies,
            min_confidence,
            infer_cancel,
        ) = {
            let mut guard = state.lock().await;

            let pending_text = guard.router.pending_text.trim().to_string();
            guard.router.pending_text.clear();

            let runner = guard
                .functiongemma
                .model
                .as_ref()
                .map(|m| std::sync::Arc::clone(&m.runner));

            (
                pending_text,
                runner,
                guard.router.enabled,
                RouterSettingsSnapshot {
                    auto_run_read_only: guard.router.auto_run_read_only,
                },
                guard.router.functiongemma_developer_context.clone(),
                guard.router.tool_policies.clone(),
                guard.router.min_confidence,
                guard.router.infer_cancel.clone(),
            )
        };

        if !enabled {
            let mut guard = state.lock().await;
            guard.router.inflight = false;
            return;
        }

        if pending_text.is_empty() {
            let mut guard = state.lock().await;
            if guard.router.pending_text.trim().is_empty() {
                guard.router.inflight = false;
                emit_router_status(&app, "idle", serde_json::json!({}));
                return;
            }
            continue;
        }

        emit_router_status(&app, "queued", serde_json::json!({ "text": pending_text }));

        // Run FunctionGemma inference if model is loaded
        let Some(runner) = runner else {
            continue;
        };

        emit_router_status(&app, "infer_start", serde_json::json!({}));

        let pending_text_for_model = pending_text.clone();
        let developer_context_for_model = developer_context.clone();
        let infer_cancel_for_model = infer_cancel.clone();
        let runner_for_args = std::sync::Arc::clone(&runner);

        let result = tokio::task::spawn_blocking(move || {
            if infer_cancel_for_model.is_cancelled() {
                return Err(crate::functiongemma::FunctionGemmaError::Inference(
                    "cancelled".to_string(),
                ));
            }
            runner.infer_once(
                developer_context_for_model.as_ref(),
                &pending_text_for_model,
            )
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));

        let model_out = match result {
            Ok(out) => {
                if infer_cancel.is_cancelled() {
                    emit_router_status(&app, "infer_cancelled", serde_json::json!({}));
                    continue;
                }
                emit_router_status(
                    &app,
                    "infer_done",
                    serde_json::json!({ "raw": out.raw_text }),
                );
                out
            }
            Err(err) => {
                if infer_cancel.is_cancelled() {
                    emit_router_status(&app, "infer_cancelled", serde_json::json!({}));
                    continue;
                }
                emit_router_status(&app, "infer_error", serde_json::json!({ "error": err }));
                continue;
            }
        };

        // Find the best proposal above confidence threshold
        let mut best: Option<&crate::functiongemma::Proposal> = None;
        for p in &model_out.proposals {
            if p.confidence < min_confidence {
                continue;
            }
            if policy_for_tool(&tool_policies, &p.tool).is_none() {
                continue;
            }
            if best.map(|b| p.confidence > b.confidence).unwrap_or(true) {
                best = Some(p);
            }
        }

        let Some(proposal) = best else {
            continue;
        };

        let Some(policy) = policy_for_tool(&tool_policies, &proposal.tool) else {
            continue;
        };

        // Validate and potentially repair args
        let mut args = proposal.args.clone();
        if policy.validate_args(&args).is_err() {
            emit_router_status(
                &app,
                "args_infer_start",
                serde_json::json!({ "tool": proposal.tool }),
            );

            let developer_context_for_args = developer_context.clone();
            let pending_text_for_args = pending_text.clone();
            let tool_name_for_args = proposal.tool.clone();

            let repaired_args = tokio::task::spawn_blocking(move || {
                runner_for_args.infer_args_object(
                    developer_context_for_args.as_ref(),
                    &tool_name_for_args,
                    &pending_text_for_args,
                )
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));

            match repaired_args {
                Ok(v) => {
                    args = v;
                    emit_router_status(
                        &app,
                        "args_infer_done",
                        serde_json::json!({ "tool": proposal.tool }),
                    );
                }
                Err(err) => {
                    emit_router_status(
                        &app,
                        "args_infer_error",
                        serde_json::json!({ "tool": proposal.tool, "error": err }),
                    );
                    continue;
                }
            }
        }

        // Build registry and execute tool
        let registry = ToolRegistry::from_policies(&tool_policies);
        let auto_run = policy.read_only && router_settings.auto_run_read_only;

        let outcome = execute_tool(
            &app,
            &state,
            &registry,
            &proposal.tool,
            &args,
            &proposal.evidence,
            auto_run,
        )
        .await;

        match outcome {
            ExecutionOutcome::Executed => {
                tracing::debug!(tool = %proposal.tool, "Tool executed successfully");
            }
            ExecutionOutcome::CacheHit => {
                tracing::debug!(tool = %proposal.tool, "Tool result from cache");
            }
            ExecutionOutcome::Cooldown => {
                tracing::debug!(tool = %proposal.tool, "Tool skipped due to cooldown");
            }
            ExecutionOutcome::ProposalEmitted => {
                tracing::debug!(tool = %proposal.tool, "Tool proposal emitted for approval");
            }
            ExecutionOutcome::NotFound => {
                emit_router_status(
                    &app,
                    "proposal_unsupported",
                    serde_json::json!({ "tool": proposal.tool }),
                );
            }
            ExecutionOutcome::Failed(e) => {
                tracing::warn!(tool = %proposal.tool, error = %e, "Tool execution failed");
            }
        }
    }
}

/// Handle incoming STT stream commit events.
pub fn on_stt_stream_commit<R: Runtime>(app: &tauri::AppHandle<R>, payload_json: &str) {
    let Ok(payload) = serde_json::from_str::<SttStreamCommitPayload>(payload_json) else {
        return;
    };

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::SharedState>();

        emit_router_status(
            &app,
            "commit_received",
            serde_json::json!({ "text": payload.text }),
        );

        let (should_spawn, notify) = {
            let mut guard = state.lock().await;
            if !guard.router.enabled {
                return;
            }

            // Append text to pending buffer
            if !guard.router.pending_text.is_empty() {
                guard.router.pending_text.push(' ');
            }
            guard.router.pending_text.push_str(payload.text.trim());

            // Get notify handle to signal after lock is released
            let notify = Arc::clone(&guard.router.text_notify);

            // Cancel any in-flight inference
            if guard.router.inflight {
                guard.router.infer_cancel.cancel();
                guard.router.infer_cancel = CancellationToken::new();
            }

            let spawn = if guard.router.inflight {
                false
            } else {
                guard.router.inflight = true;
                true
            };
            (spawn, notify)
        };

        // Signal the notify to reset debounce timer (even if worker is already running)
        notify.notify_one();

        if should_spawn {
            emit_router_status(&app, "worker_start", serde_json::json!({}));
            process_router_queue(app).await;
        }
    });
}
