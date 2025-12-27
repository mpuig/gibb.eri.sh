//! Tauri commands for skill management.

use crate::SharedState;
use serde::Serialize;
use tauri::State;

/// Result of reloading skills.
#[derive(Debug, Serialize)]
pub struct SkillReloadResult {
    pub skill_count: usize,
    pub tool_count: usize,
    pub errors: Vec<String>,
}

/// Reload all skills from disk.
#[tauri::command]
pub async fn reload_skills(state: State<'_, SharedState>) -> Result<SkillReloadResult, String> {
    let mut guard = state.lock().await;
    let result = guard.reload_skills();

    Ok(SkillReloadResult {
        skill_count: result.skill_count,
        tool_count: result.tool_count,
        errors: result
            .errors
            .iter()
            .map(|(path, err)| format!("{}: {}", path.display(), err))
            .collect(),
    })
}

/// Get information about loaded skills.
#[derive(Debug, Serialize)]
pub struct SkillInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub tool_count: usize,
    pub tools: Vec<String>,
}

/// List all loaded skills.
#[tauri::command]
pub async fn list_skills(state: State<'_, SharedState>) -> Result<Vec<SkillInfo>, String> {
    let guard = state.lock().await;

    let skills: Vec<SkillInfo> = guard
        .skills
        .skills()
        .map(|loaded| SkillInfo {
            name: loaded.definition.name.clone(),
            version: loaded.definition.version.clone(),
            description: loaded.definition.description.clone(),
            tool_count: loaded.tools.len(),
            tools: loaded.tools.iter().map(|t| t.name.to_string()).collect(),
        })
        .collect();

    Ok(skills)
}
