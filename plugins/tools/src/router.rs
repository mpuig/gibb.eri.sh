//! Router for triggering tools based on STT commits.
//!
//! Receives transcript commits, runs FunctionGemma inference,
//! and dispatches tool execution via the registry.
//!
//! Architecture:
//! - `context_injector`: JIT context enrichment (clipboard, selection, URL)
//! - `inference`: Inference orchestration (primary, followup, summary)
//! - `router_logic`: Pure decision logic (testable)
//! - `executor`: Tool execution

use crate::context_injector;
use crate::executor::{execute_tool, ExecutionMode, ExecutionOutcome};
use crate::inference::{self, InferenceResult};
use crate::pipeline::{self, ChainDecision, PipelineContext};
use crate::policy::DEBOUNCE;
use crate::registry::ToolRegistry;
use crate::router_logic::{self, RouterConfig};
use crate::tool_manifest::ToolPolicy;
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
        guard.functiongemma.model.as_ref().map(|m| Arc::clone(&m.runner))
    }?;

    let result = inference::validate_and_repair_args(
        runner,
        developer_context.clone(),
        pending_text,
        tool_name,
        args,
        |a| policy.validate_args(a).map(|_| ()),
    )
    .await;

    match result {
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

    // Build registry once - tools don't change at runtime, only mode filtering does
    let registry = ToolRegistry::build_all();

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

        // Phase 2: Context Injection via dedicated injector
        let injected = {
            let mut guard = state.lock().await;
            context_injector::inject_context(&mut guard.context)
        };

        // Build context-enriched developer prompt
        let enriched_developer_context =
            context_injector::enrich_developer_context(&developer_context, &injected.snippet);

        emit_router_status(
            &*event_bus,
            "infer_start",
            serde_json::json!({
                "context": injected.snippet,
                "has_clipboard": injected.has_clipboard,
                "has_selection": injected.has_selection,
                "has_url": injected.has_url,
            }),
        );

        // Run primary inference via inference module
        let model_out = match inference::run_primary_inference(
            Arc::clone(&runner),
            Arc::from(enriched_developer_context),
            pending_text.clone(),
            infer_cancel.clone(),
        )
        .await
        {
            InferenceResult::Success(out) => {
                emit_router_status(
                    &*event_bus,
                    "infer_done",
                    serde_json::json!({ "raw": out.raw_text }),
                );
                out
            }
            InferenceResult::Cancelled => {
                emit_router_status(&*event_bus, "infer_cancelled", serde_json::json!({}));
                continue;
            }
            InferenceResult::Error(err) => {
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

        // Check if proposal needs clarification (low confidence)
        if router_logic::needs_clarification(proposal) {
            let suggestions = router_logic::clarification_suggestions(proposal, &pending_text);
            emit_router_status(
                &*event_bus,
                "needs_clarification",
                serde_json::json!({
                    "tool": proposal.tool,
                    "confidence": proposal.confidence,
                    "evidence": proposal.evidence,
                    "suggestions": suggestions,
                    "text": pending_text.chars().take(100).collect::<String>()
                }),
            );
            // Emit dedicated clarification event for UI
            event_bus.emit(
                "tools:clarification_needed",
                serde_json::json!({
                    "tool": proposal.tool,
                    "confidence": proposal.confidence,
                    "suggestions": suggestions,
                    "original_text": pending_text,
                    "ts_ms": chrono::Utc::now().timestamp_millis()
                }),
            );
            continue;
        }

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

        // Tool chaining: after a tool executes, give the model a chance to
        // propose a next tool (e.g., `web_search` -> `typer`) based on the tool output.
        // Uses formal pipeline with depth limits (see pipeline.rs).
        if let Some(ref output) = tool_output {
            let pipeline_ctx = PipelineContext::new(pending_text.clone(), developer_context.clone());

            // Check depth limit before attempting followup
            if !pipeline_ctx.can_chain() {
                emit_router_status(
                    &*event_bus,
                    "chain_depth_limit",
                    serde_json::json!({
                        "max_depth": pipeline::MAX_CHAIN_DEPTH,
                        "after_tool": proposal.tool
                    }),
                );
            } else {
                let runner_for_followup = {
                    let guard = state.lock().await;
                    guard.functiongemma.model.as_ref().map(|m| Arc::clone(&m.runner))
                };

                if let Some(followup_runner) = runner_for_followup {
                    emit_router_status(
                        &*event_bus,
                        "followup_infer_start",
                        serde_json::json!({
                            "after_tool": proposal.tool.as_str(),
                            "depth": pipeline_ctx.depth
                        }),
                    );

                    let followup_result = inference::run_followup_inference(
                        followup_runner,
                        developer_context.clone(),
                        &pending_text,
                        &proposal.tool,
                        output,
                    )
                    .await;

                    if let InferenceResult::Success(followup_out) = followup_result {
                        emit_router_status(
                            &*event_bus,
                            "followup_infer_done",
                            serde_json::json!({ "raw": followup_out.raw_text }),
                        );

                        // Use pipeline to decide if we should chain
                        let chain_decision = pipeline::should_chain(
                            &pipeline_ctx,
                            &followup_out.proposals,
                            router_settings.min_confidence,
                            |t| tool_policies.contains_key(t),
                        );

                        match chain_decision {
                            ChainDecision::Continue(step) => {
                                if let Some(next_policy) = policy_for_tool(&tool_policies, &step.tool) {
                                    let validated_args = validate_and_repair_args(
                                        &state,
                                        &event_bus,
                                        &developer_context,
                                        &pending_text,
                                        next_policy,
                                        &step.tool,
                                        step.args.clone(),
                                        true,
                                    )
                                    .await;

                                    if let Some(next_args) = validated_args {
                                        let next_execution_mode =
                                            if router_logic::determine_execution_mode(next_policy, &router_settings) {
                                                ExecutionMode::AutoRun
                                            } else {
                                                ExecutionMode::RequireApproval
                                            };

                                        emit_router_status(
                                            &*event_bus,
                                            "chain_execute",
                                            serde_json::json!({
                                                "tool": step.tool,
                                                "depth": step.depth
                                            }),
                                        );

                                        let _ = execute_tool(
                                            &state,
                                            &registry,
                                            &step.tool,
                                            &next_args,
                                            &step.evidence,
                                            next_execution_mode,
                                        )
                                        .await;
                                    }
                                }
                            }
                            ChainDecision::LimitReached => {
                                emit_router_status(
                                    &*event_bus,
                                    "chain_depth_limit",
                                    serde_json::json!({
                                        "max_depth": pipeline::MAX_CHAIN_DEPTH
                                    }),
                                );
                            }
                            ChainDecision::Stop => {
                                // No followup needed
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
                guard.functiongemma.model.as_ref().map(|m| Arc::clone(&m.runner))
            };

            if let Some(summary_runner) = runner_for_summary {
                emit_router_status(
                    &*event_bus,
                    "summary_start",
                    serde_json::json!({ "tool": proposal.tool }),
                );

                let summary_result = inference::generate_summary(
                    summary_runner,
                    proposal.tool.clone(),
                    output,
                    pending_text.clone(),
                )
                .await;

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
