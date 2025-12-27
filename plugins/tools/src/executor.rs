//! Tool execution with caching and cooldown logic.
//!
//! Generic executor that works with any tool through the Tool trait.
//! Caching and cooldown are driven by tool-provided keys, not hardcoded checks.
//! Deictic references (clipboard, selection, etc.) are resolved before execution.

use std::sync::Arc;
use std::time::Instant;

use gibberish_events::{event_names, EventBus};

use crate::adapters::PlatformClipboard;
use crate::deictic::{resolve_args, ResolverContext};
use crate::environment::RealSystemEnvironment;
use crate::policy::{CACHE_TTL, DEFAULT_TOOL_COOLDOWN};
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
    /// Contains the result payload for feedback loop.
    Executed(serde_json::Value),
    /// Tool was skipped due to cooldown.
    Cooldown,
    /// Cache hit, result emitted from cache.
    /// Contains the cached payload for feedback loop.
    CacheHit(serde_json::Value),
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
pub async fn execute_tool(
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

    // Get event bus reference
    let event_bus = {
        let guard = state.lock().await;
        Arc::clone(&guard.event_bus)
    };

    // If not auto-run, emit proposal and return
    if mode == ExecutionMode::RequireApproval {
        emit_proposal(&*event_bus, tool_name, args, evidence);
        return ExecutionOutcome::ProposalEmitted;
    }

    // Pre-compute keys from args, so caching/cooldowns work for any tool that
    // implements `cache_key`/`cooldown_key` (not hardcoded per tool).
    let cache_key = tool.cache_key(args);
    let cooldown_key = tool.cooldown_key(args);

    // Check cooldown and cache in single lock scope
    let check_result = {
        let mut guard = state.lock().await;

        // Check cooldown first (if we have a key)
        if let Some(ref key) = cooldown_key {
            if !should_execute(&mut guard, tool_name, key) {
                return ExecutionOutcome::Cooldown;
            }
        }

        // Check cache
        cache_key.as_ref().and_then(|key| {
            guard
                .cache
                .get(key)
                .filter(|entry| entry.fetched_at.elapsed() <= CACHE_TTL)
                .map(|entry| (entry.payload.clone(), entry.event_name))
        })
    };

    // Return cached result if found
    if let Some((cached_payload, event_name)) = check_result {
        event_bus.emit(event_name, cached_payload.clone());
        emit_tool_done(&*event_bus, tool_name, true, &cached_payload);
        return ExecutionOutcome::CacheHit(cached_payload);
    }

    // Execute the tool with system environment
    let (client, default_lang, abort_flag) = {
        let guard = state.lock().await;
        (
            guard.client.clone(),
            guard.router.default_lang.clone(),
            Arc::clone(&guard.global_abort),
        )
    };

    let env = Arc::new(RealSystemEnvironment::new(client));
    let ctx = ToolContext::with_abort(env, default_lang, abort_flag);

    // Resolve deictic references (clipboard, selection, etc.) before execution
    let clipboard_provider = PlatformClipboard::new();
    let resolver_ctx = ResolverContext {
        clipboard: Some(&clipboard_provider),
        selection: None, // TODO: Add selection provider when available
        transcript: None, // TODO: Add transcript provider when available
    };

    let resolved_args = match resolve_args(args, &resolver_ctx) {
        Ok(resolved) => resolved,
        Err(e) => {
            emit_tool_error(&*event_bus, tool_name, &format!("Failed to resolve args: {}", e));
            return ExecutionOutcome::Failed(ToolError::ExecutionFailed(format!(
                "Deictic resolution failed: {}",
                e
            )));
        }
    };

    emit_tool_start(&*event_bus, tool_name, &resolved_args, evidence);

    match tool.execute(&resolved_args, &ctx).await {
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

            // Record cooldown if tool provided a cooldown key.
            if let Some(ref cooldown_key) = result.cooldown_key {
                let mut guard = state.lock().await;
                guard
                    .router
                    .cooldowns
                    .insert(format!("{tool_name}:{cooldown_key}"), Instant::now());
            }

            // Emit the result using tool-provided event name and payload
            event_bus.emit(result.event_name, result.payload.clone());
            emit_tool_done(&*event_bus, tool_name, false, &result.payload);
            ExecutionOutcome::Executed(result.payload)
        }
        Err(e) => {
            emit_tool_error(&*event_bus, tool_name, &e.to_string());
            ExecutionOutcome::Failed(e)
        }
    }
}

/// Check if we should execute based on cooldown.
fn should_execute(state: &mut ToolsState, tool_name: &str, cooldown_key: &str) -> bool {
    let cooldown_key = cooldown_key.trim();
    if cooldown_key.is_empty() {
        return true;
    }

    let composite_key = format!("{tool_name}:{cooldown_key}");

    if let Some(last_at) = state.router.cooldowns.get(&composite_key) {
        if last_at.elapsed() < DEFAULT_TOOL_COOLDOWN {
            return false;
        }
    }

    state.router.cooldowns.insert(composite_key, Instant::now());
    true
}

fn emit_proposal(
    event_bus: &dyn EventBus,
    tool: &str,
    args: &serde_json::Value,
    evidence: &str,
) {
    event_bus.emit(
        event_names::ACTION_PROPOSED,
        serde_json::json!({
            "tool": tool,
            "args": args,
            "evidence": evidence,
        }),
    );
    emit_router_status(
        event_bus,
        "proposal",
        serde_json::json!({
            "tool": tool,
            "args": args,
            "evidence": evidence,
        }),
    );
}

fn emit_tool_start(event_bus: &dyn EventBus, tool: &str, args: &serde_json::Value, evidence: &str) {
    emit_router_status(
        event_bus,
        "tool_start",
        serde_json::json!({
            "tool": tool,
            "args": args,
            "evidence": evidence,
        }),
    );
}

fn emit_tool_done(event_bus: &dyn EventBus, tool: &str, cached: bool, result: &serde_json::Value) {
    emit_router_status(
        event_bus,
        "tool_result",
        serde_json::json!({
            "tool": tool,
            "cached": cached,
            "result": result,
        }),
    );
}

fn emit_tool_error(event_bus: &dyn EventBus, tool: &str, error: &str) {
    emit_router_status(
        event_bus,
        "tool_error",
        serde_json::json!({
            "tool": tool,
            "error": error,
        }),
    );
    event_bus.emit(
        event_names::TOOL_ERROR,
        serde_json::json!({ "tool": tool, "error": error }),
    );
}

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
