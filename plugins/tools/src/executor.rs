//! Tool execution with caching and cooldown logic.
//!
//! Generic executor that works with any tool through the Tool trait.
//! Caching and cooldown are driven by tool-provided keys, not hardcoded checks.

use std::sync::Arc;
use std::time::Instant;

use gibberish_events::event_names;
use tauri::{Emitter, Runtime};

use crate::environment::RealSystemEnvironment;
use crate::policy::{CACHE_TTL, CITY_COOLDOWN};
use crate::registry::ToolRegistry;
use crate::state::{CacheEntry, ToolsState};
use crate::tools::{ToolContext, ToolError};

/// How to execute a tool - avoids "Boolean Blindness".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Execute immediately without user approval.
    AutoRun,
    /// Emit a proposal for user approval first.
    RequireApproval,
}

/// Result of tool execution attempt.
pub enum ExecutionOutcome {
    /// Tool was executed successfully, result emitted.
    Executed,
    /// Tool was skipped due to cooldown.
    Cooldown,
    /// Cache hit, result emitted from cache.
    CacheHit,
    /// Tool execution requires user approval (not auto-run).
    ProposalEmitted,
    /// Tool not found in registry.
    NotFound,
    /// Tool execution failed.
    Failed(ToolError),
}

/// Execute a tool with caching and cooldown logic.
///
/// Caching and cooldown are controlled by the tool through its ToolResult:
/// - If `cooldown_key` is set, repeated calls are throttled
/// - If `cache_key` is set, results are cached and reused
pub async fn execute_tool<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &crate::SharedState,
    registry: &ToolRegistry,
    tool_name: &str,
    args: &serde_json::Value,
    evidence: &str,
    mode: ExecutionMode,
) -> ExecutionOutcome {
    let Some(tool) = registry.get(tool_name) else {
        return ExecutionOutcome::NotFound;
    };

    // If not auto-run, emit proposal and return
    if mode == ExecutionMode::RequireApproval {
        emit_proposal(app, tool_name, args, evidence);
        return ExecutionOutcome::ProposalEmitted;
    }

    // Build potential cache/cooldown key from args (tools define their key format)
    // For now, we check cooldown using a simple normalized key from common args
    let potential_key = build_potential_key(tool_name, args);

    // Check cooldown and cache in single lock scope
    let check_result = {
        let mut guard = state.lock().await;

        // Check cooldown first (if we have a potential key)
        if let Some(ref key) = potential_key {
            if !should_execute(&mut guard, key) {
                return ExecutionOutcome::Cooldown;
            }
        }

        // Check cache
        potential_key.as_ref().and_then(|key| {
            guard
                .cache
                .get(key)
                .filter(|entry| entry.fetched_at.elapsed() <= CACHE_TTL)
                .map(|entry| (entry.payload.clone(), entry.event_name))
        })
    };

    // Return cached result if found
    if let Some((cached_payload, event_name)) = check_result {
        let _ = app.emit(event_name, &cached_payload);
        emit_tool_done(app, tool_name, true);
        return ExecutionOutcome::CacheHit;
    }

    // Execute the tool with system environment
    let (client, default_lang) = {
        let guard = state.lock().await;
        (guard.client.clone(), guard.router.default_lang.clone())
    };

    let env = Arc::new(RealSystemEnvironment::new(client));
    let ctx = ToolContext::new(env, default_lang);

    emit_tool_start(app, tool_name, args);

    match tool.execute(args, &ctx).await {
        Ok(result) => {
            // Cache the result if tool provided a cache key
            if let Some(ref cache_key) = result.cache_key {
                let mut guard = state.lock().await;
                guard.cache.insert(
                    cache_key.clone(),
                    CacheEntry {
                        fetched_at: Instant::now(),
                        payload: result.payload.clone(),
                        event_name: result.event_name,
                    },
                );
            }

            // Emit the result using tool-provided event name and payload
            let _ = app.emit(result.event_name, &result.payload);
            emit_tool_done(app, tool_name, false);
            ExecutionOutcome::Executed
        }
        Err(e) => {
            emit_tool_error(app, tool_name, &e.to_string());
            ExecutionOutcome::Failed(e)
        }
    }
}

/// Build a potential cache/cooldown key from tool args.
///
/// This allows pre-execution cache/cooldown checks. The key format
/// matches what tools produce in their ToolResult.
fn build_potential_key(tool_name: &str, args: &serde_json::Value) -> Option<String> {
    match tool_name {
        "wikipedia_city_lookup" => {
            let city = args.get("city").and_then(|v| v.as_str())?;
            let lang = args.get("lang").and_then(|v| v.as_str()).unwrap_or("en");
            Some(format!("{}:{}", lang, city.trim().to_lowercase()))
        }
        _ => None,
    }
}

/// Check if we should execute based on cooldown.
fn should_execute(state: &mut ToolsState, cooldown_key: &str) -> bool {
    if cooldown_key.is_empty() {
        return true;
    }

    if let Some(last_at) = state.router.cooldowns.get(cooldown_key) {
        if last_at.elapsed() < CITY_COOLDOWN {
            return false;
        }
    }

    state
        .router
        .cooldowns
        .insert(cooldown_key.to_string(), Instant::now());
    true
}

fn emit_proposal<R: Runtime>(
    app: &tauri::AppHandle<R>,
    tool: &str,
    args: &serde_json::Value,
    evidence: &str,
) {
    let _ = app.emit(
        event_names::ACTION_PROPOSED,
        serde_json::json!({
            "tool": tool,
            "args": args,
            "evidence": evidence,
        }),
    );
    emit_router_status(
        app,
        "proposal",
        serde_json::json!({
            "tool": tool,
            "args": args,
            "evidence": evidence,
        }),
    );
}

fn emit_tool_start<R: Runtime>(app: &tauri::AppHandle<R>, tool: &str, args: &serde_json::Value) {
    emit_router_status(
        app,
        "tool_start",
        serde_json::json!({
            "tool": tool,
            "args": args,
        }),
    );
}

fn emit_tool_done<R: Runtime>(app: &tauri::AppHandle<R>, tool: &str, cached: bool) {
    emit_router_status(
        app,
        "tool_result",
        serde_json::json!({
            "tool": tool,
            "cached": cached,
        }),
    );
}

fn emit_tool_error<R: Runtime>(app: &tauri::AppHandle<R>, tool: &str, error: &str) {
    emit_router_status(
        app,
        "tool_error",
        serde_json::json!({
            "tool": tool,
            "error": error,
        }),
    );
    let _ = app.emit(
        event_names::TOOL_ERROR,
        serde_json::json!({ "tool": tool, "error": error }),
    );
}

fn emit_router_status<R: Runtime>(
    app: &tauri::AppHandle<R>,
    phase: &str,
    payload: serde_json::Value,
) {
    let _ = app.emit(
        event_names::ROUTER_STATUS,
        serde_json::json!({
            "phase": phase,
            "ts_ms": chrono::Utc::now().timestamp_millis(),
            "payload": payload,
        }),
    );
}
