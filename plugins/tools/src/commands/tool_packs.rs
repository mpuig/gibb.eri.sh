//! Tauri commands for tool pack management.

use crate::tools::Tool;
use crate::SharedState;
use serde::Serialize;
use tauri::State;

/// Result of reloading tool packs.
#[derive(Debug, Serialize)]
pub struct ToolPackReloadResult {
    pub pack_count: usize,
    pub errors: Vec<String>,
}

/// Reload all tool packs from disk.
#[tauri::command]
pub async fn reload_tool_packs(
    state: State<'_, SharedState>,
) -> Result<ToolPackReloadResult, String> {
    let mut guard = state.lock().await;
    let result = guard.reload_tool_packs();

    Ok(ToolPackReloadResult {
        pack_count: result.pack_count,
        errors: result.errors.iter().map(|err| err.to_string()).collect(),
    })
}

/// Information about a loaded tool pack.
#[derive(Debug, Serialize)]
pub struct ToolPackInfo {
    pub name: String,
    pub version: Option<String>,
    pub description: String,
    pub modes: Vec<String>,
    pub read_only: bool,
    pub requires_network: bool,
    pub timeout_secs: u64,
}

/// List all loaded tool packs.
#[tauri::command]
pub async fn list_tool_packs(state: State<'_, SharedState>) -> Result<Vec<ToolPackInfo>, String> {
    let guard = state.lock().await;

    let packs: Vec<ToolPackInfo> = guard
        .tool_packs
        .tools()
        .map(|tool| {
            // Access the underlying ToolPack through the tool
            ToolPackInfo {
                name: tool.name().to_string(),
                version: None, // Would need to expose version from ToolPackTool
                description: tool.description().to_string(),
                modes: tool
                    .modes()
                    .iter()
                    .map(|m| format!("{:?}", m))
                    .collect(),
                read_only: tool.is_read_only(),
                requires_network: false, // Would need to expose this
                timeout_secs: 30,        // Would need to expose this
            }
        })
        .collect();

    Ok(packs)
}

/// Reload all tools (skills + tool packs) from disk.
#[tauri::command]
pub async fn reload_all_tools(state: State<'_, SharedState>) -> Result<ReloadAllResult, String> {
    let mut guard = state.lock().await;

    // Reload both
    let skills_result = guard.skills.reload();
    let packs_result = guard.tool_packs.reload();

    // Update router with all sources
    let mode = guard.context.effective_mode();
    let registry =
        crate::registry::ToolRegistry::build_all_sources(&guard.skills, &guard.tool_packs);
    guard.router.update_with_registry(&registry, mode);

    Ok(ReloadAllResult {
        skill_count: skills_result.skill_count,
        skill_tool_count: skills_result.tool_count,
        pack_count: packs_result.pack_count,
        skill_errors: skills_result
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect(),
        pack_errors: packs_result
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect(),
    })
}

/// Result of reloading all tools.
#[derive(Debug, Serialize)]
pub struct ReloadAllResult {
    pub skill_count: usize,
    pub skill_tool_count: usize,
    pub pack_count: usize,
    pub skill_errors: Vec<String>,
    pub pack_errors: Vec<String>,
}
