//! Router for triggering tools based on STT commits.
//!
//! Receives transcript commits, runs FunctionGemma inference,
//! and dispatches tool execution via the registry.
//!
//! Pure decision logic is in `router_logic` module for testability.

use crate::executor::{execute_tool, ExecutionMode, ExecutionOutcome};
use crate::policy::DEBOUNCE;
use crate::registry::ToolRegistry;
use crate::router_logic::{self, RouterConfig};
use crate::tool_manifest::ToolPolicy;
use gibberish_context::platform::{get_clipboard_preview, get_selection_preview};
use gibberish_events::{event_names, EventBus, StreamCommitEvent};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{Manager, Runtime};
use tokio_util::sync::CancellationToken;

fn emit_router_status(event_bus: &dyn EventBus, phase: &str, payload: serde_json::Value) {
    event_bus.emit(
        event_names::ROUTER_STATUS,
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

/// Validate tool args and attempt repair via inference if validation fails.
/// Returns Some(args) if valid/repaired, None if repair failed.
async fn validate_and_repair_args(
    state: &tokio::sync::Mutex<crate::state::ToolsState>,
    event_bus: &Arc<dyn EventBus>,
    developer_context: &Arc<str>,
    pending_text: &str,
    policy: &ToolPolicy,
    tool_name: &str,
    args: serde_json::Value,
    is_followup: bool,
) -> Option<serde_json::Value> {
    // If args are already valid, return them as-is
    if policy.validate_args(&args).is_ok() {
        return Some(args);
    }

    let phase_prefix = if is_followup { "followup_" } else { "" };

    emit_router_status(
        &**event_bus,
        &format!("{}args_infer_start", phase_prefix),
        serde_json::json!({ "tool": tool_name }),
    );

    // Get runner for args repair
    let runner = {
        let guard = state.lock().await;
        guard
            .functiongemma
            .model
            .as_ref()
            .map(|m| std::sync::Arc::clone(&m.runner))
    }?;

    let dev_ctx = developer_context.clone();
    let tool = tool_name.to_string();
    let text = pending_text.to_string();

    let repaired = tokio::task::spawn_blocking(move || {
        runner.infer_args_object(dev_ctx.as_ref(), &tool, &text)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r.map_err(|e| e.to_string()));

    match repaired {
        Ok(v) => {
            emit_router_status(
                &**event_bus,
                &format!("{}args_infer_done", phase_prefix),
                serde_json::json!({ "tool": tool_name }),
            );
            Some(v)
        }
        Err(err) => {
            emit_router_status(
                &**event_bus,
                &format!("{}args_infer_error", phase_prefix),
                serde_json::json!({ "tool": tool_name, "error": err }),
            );
            None
        }
    }
}

/// Main router processing loop.
///
/// Event-driven debounce: waits for new text notifications, then
/// processes after DEBOUNCE duration of silence.
async fn process_router_queue<R: Runtime>(app: tauri::AppHandle<R>) {
    let state = app.state::<crate::SharedState>();

    // Get the event bus and notify handle
    let (event_bus, notify) = {
        let guard = state.lock().await;
        (
            Arc::clone(&guard.event_bus),
            Arc::clone(&guard.router.text_notify),
        )
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
        let (pending_text, runner, enabled, router_settings, infer_cancel) = {
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
                RouterConfig {
                    auto_run_read_only: guard.router.auto_run_read_only,
                    auto_run_all: guard.router.auto_run_all,
                    current_mode: guard.context.effective_mode(),
                    min_confidence: guard.router.min_confidence,
                },
                guard.router.infer_cancel.clone(),
            )
        };

        // Build developer context and tool policies JIT based on current mode
        // This ensures the model sees the correct tools for the current mode
        let registry = ToolRegistry::build_all();
        let (developer_context, tool_policies): (Arc<str>, Arc<HashMap<String, ToolPolicy>>) = {
            let instructions = registry.functiongemma_instructions_for_mode(router_settings.current_mode);
            let manifest_json = registry.manifest_json_for_mode(router_settings.current_mode);
            let compiled = crate::tool_manifest::validate_and_compile(&manifest_json)
                .unwrap_or_default();
            (
                Arc::from(format!(
                    "You are a model that can do function calling with the following functions\n{}\n{}",
                    instructions, compiled.function_declarations
                )),
                Arc::new(compiled.policies),
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
                emit_router_status(&*event_bus, "idle", serde_json::json!({}));
                return;
            }
            continue;
        }

        emit_router_status(&*event_bus, "queued", serde_json::json!({ "text": pending_text }));

        // Run FunctionGemma inference if model is loaded
        let Some(runner) = runner else {
            continue;
        };

        // Phase 2: Context Injection
        // Read clipboard/selection just-in-time and build context-enriched prompt
        let context_snippet = {
            let mut guard = state.lock().await;
            // Update context with current clipboard/selection
            guard.context.system.clipboard_preview = get_clipboard_preview();
            guard.context.system.selection_preview = get_selection_preview();
            guard.context.to_prompt_snippet()
        };

        // Build context-enriched developer prompt
        let enriched_developer_context = format!(
            "{}\n\nCurrent Context:\n{}",
            developer_context, context_snippet
        );

        emit_router_status(
            &*event_bus,
            "infer_start",
            serde_json::json!({
                "context": context_snippet
            }),
        );

        let pending_text_for_model = pending_text.clone();
        let developer_context_for_model: Arc<str> = Arc::from(enriched_developer_context);
        let infer_cancel_for_model = infer_cancel.clone();

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
                    emit_router_status(&*event_bus, "infer_cancelled", serde_json::json!({}));
                    continue;
                }
                emit_router_status(
                    &*event_bus,
                    "infer_done",
                    serde_json::json!({ "raw": out.raw_text }),
                );
                out
            }
            Err(err) => {
                if infer_cancel.is_cancelled() {
                    emit_router_status(&*event_bus, "infer_cancelled", serde_json::json!({}));
                    continue;
                }
                emit_router_status(&*event_bus, "infer_error", serde_json::json!({ "error": err }));
                continue;
            }
        };

        // Find the best proposal above confidence threshold
        let Some(proposal) =
            router_logic::find_best_proposal(&model_out.proposals, &tool_policies, router_settings.min_confidence)
        else {
            // No valid tool call found - emit feedback
            emit_router_status(
                &*event_bus,
                "no_match",
                serde_json::json!({
                    "message": "No matching tool found for this request",
                    "text": pending_text.chars().take(100).collect::<String>()
                }),
            );
            continue;
        };

        let Some(policy) = policy_for_tool(&tool_policies, &proposal.tool) else {
            // Tool proposed but not available in current mode
            emit_router_status(
                &*event_bus,
                "tool_unavailable",
                serde_json::json!({
                    "tool": proposal.tool,
                    "message": format!("Tool '{}' is not available in current mode", proposal.tool)
                }),
            );
            continue;
        };

        // Validate and potentially repair args
        let Some(args) = validate_and_repair_args(
            &state,
            &event_bus,
            &developer_context,
            &pending_text,
            policy,
            &proposal.tool,
            proposal.args.clone(),
            false, // not a followup
        )
        .await
        else {
            continue;
        };

        // Guard: skip if tool not available in current mode
        if let Some(tool) = registry.get(&proposal.tool) {
            if !tool.is_available_in(router_settings.current_mode) {
                tracing::debug!(
                    tool = %proposal.tool,
                    mode = %router_settings.current_mode,
                    "Tool not available in current mode, skipping"
                );
                emit_router_status(
                    &*event_bus,
                    "tool_mode_filtered",
                    serde_json::json!({
                        "tool": proposal.tool,
                        "mode": router_settings.current_mode.to_string()
                    }),
                );
                continue;
            }
        }

        // Determine execution mode using pure logic
        let execution_mode = if router_logic::determine_execution_mode(policy, &router_settings) {
            ExecutionMode::AutoRun
        } else {
            ExecutionMode::RequireApproval
        };

        let outcome = execute_tool(
            &state,
            &registry,
            &proposal.tool,
            &args,
            &proposal.evidence,
            execution_mode,
        )
        .await;

        // Phase 3: Feedback Loop - generate summary for executed tools
        let tool_output = match outcome {
            ExecutionOutcome::Executed(payload) => {
                tracing::debug!(tool = %proposal.tool, "Tool executed successfully");
                Some(payload)
            }
            ExecutionOutcome::CacheHit(payload) => {
                tracing::debug!(tool = %proposal.tool, "Tool result from cache");
                Some(payload)
            }
            ExecutionOutcome::Cooldown => {
                tracing::debug!(tool = %proposal.tool, "Tool skipped due to cooldown");
                None
            }
            ExecutionOutcome::ProposalEmitted => {
                tracing::debug!(tool = %proposal.tool, "Tool proposal emitted for approval");
                None
            }
            ExecutionOutcome::NotFound => {
                emit_router_status(
                    &*event_bus,
                    "proposal_unsupported",
                    serde_json::json!({ "tool": proposal.tool }),
                );
                None
            }
            ExecutionOutcome::Failed(e) => {
                tracing::warn!(tool = %proposal.tool, error = %e, "Tool execution failed");
                None
            }
        };

        // Optional follow-up: after a tool executes, give the model a chance to
        // propose a next tool (e.g., `web_search` -> `typer`) based on the tool output.
        if let Some(output) = tool_output.clone() {
            let runner_for_followup = {
                let guard = state.lock().await;
                guard
                    .functiongemma
                    .model
                    .as_ref()
                    .map(|m| std::sync::Arc::clone(&m.runner))
            };

            if let Some(runner) = runner_for_followup {
                // Keep the follow-up context small; only include a short JSON preview.
                let output_preview =
                    serde_json::to_string(&output).unwrap_or_else(|_| output.to_string());
                let output_preview = if output_preview.len() > 1400 {
                    format!("{}...", &output_preview[..1400])
                } else {
                    output_preview
                };

                let followup_text = format!(
                    "Original request:\n{pending_text}\n\nTool `{tool}` output (JSON):\n{output_preview}\n\nIf another tool should be called to fully satisfy the original request, call it now. Otherwise output <end_of_turn>.",
                    tool = proposal.tool.as_str()
                );

                emit_router_status(
                    &*event_bus,
                    "followup_infer_start",
                    serde_json::json!({
                        "after_tool": proposal.tool.as_str(),
                    }),
                );

                // Clone for args repair (after move into closure)
                let developer_context_for_args = developer_context.clone();

                let followup = tokio::task::spawn_blocking(move || {
                    runner.infer_once(developer_context.as_ref(), &followup_text)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));

                if let Ok(followup_out) = followup {
                    emit_router_status(
                        &*event_bus,
                        "followup_infer_done",
                        serde_json::json!({
                            "raw": followup_out.raw_text,
                        }),
                    );

                    if let Some(next) =
                        router_logic::find_best_proposal(&followup_out.proposals, &tool_policies, router_settings.min_confidence)
                    {
                        if let Some(next_policy) = policy_for_tool(&tool_policies, &next.tool) {
                            // Validate and repair args (same logic as primary flow)
                            let validated_args = validate_and_repair_args(
                                &state,
                                &event_bus,
                                &developer_context_for_args,
                                &pending_text,
                                next_policy,
                                &next.tool,
                                next.args.clone(),
                                true, // is_followup
                            )
                            .await;

                            if let Some(next_args) = validated_args {
                                let next_execution_mode = if router_logic::determine_execution_mode(next_policy, &router_settings) {
                                    ExecutionMode::AutoRun
                                } else {
                                    ExecutionMode::RequireApproval
                                };

                                let _ = execute_tool(
                                    &state,
                                    &registry,
                                    &next.tool,
                                    &next_args,
                                    &next.evidence,
                                    next_execution_mode,
                                )
                                .await;
                            }
                        }
                    }
                }
            }
        }

        // Generate natural language summary if we have tool output
        if let Some(output) = tool_output {
            let runner_for_summary = {
                let guard = state.lock().await;
                guard
                    .functiongemma
                    .model
                    .as_ref()
                    .map(|m| std::sync::Arc::clone(&m.runner))
            };

            if let Some(runner) = runner_for_summary {
                let tool_name = proposal.tool.clone();
                let user_text = pending_text.clone();
                let output_clone = output.clone();

                emit_router_status(
                    &*event_bus,
                    "summary_start",
                    serde_json::json!({
                        "tool": tool_name
                    }),
                );

                let summary_result = tokio::task::spawn_blocking(move || {
                    runner.summarize_tool_output(&tool_name, &output_clone, &user_text)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r.map_err(|e| e.to_string()));

                match summary_result {
                    Ok(summary) => {
                        emit_router_status(
                            &*event_bus,
                            "summary_done",
                            serde_json::json!({
                                "tool": proposal.tool,
                                "summary": summary
                            }),
                        );
                        // Emit a dedicated event for the UI to display/speak
                        event_bus.emit(
                            "tools:summary",
                            serde_json::json!({
                                "tool": proposal.tool,
                                "summary": summary,
                                "ts_ms": chrono::Utc::now().timestamp_millis()
                            }),
                        );
                    }
                    Err(e) => {
                        tracing::debug!(
                            tool = %proposal.tool,
                            error = %e,
                            "Failed to generate summary (non-fatal)"
                        );
                    }
                }
            }
        }
    }
}

/// Handle incoming STT stream commit events.
pub fn on_stt_stream_commit<R: Runtime>(app: &tauri::AppHandle<R>, payload_json: &str) {
    let Ok(payload) = serde_json::from_str::<StreamCommitEvent>(payload_json) else {
        return;
    };

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<crate::SharedState>();

        // Get event bus and emit status
        let event_bus = {
            let guard = state.lock().await;
            Arc::clone(&guard.event_bus)
        };

        emit_router_status(
            &*event_bus,
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
            emit_router_status(&*event_bus, "worker_start", serde_json::json!({}));
            process_router_queue(app).await;
        }
    });
}
