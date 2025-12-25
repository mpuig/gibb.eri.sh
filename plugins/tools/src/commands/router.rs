use crate::SharedState;
use tauri::State;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionRouterSettingsDto {
    pub enabled: bool,
    pub auto_run_read_only: bool,
    pub default_lang: String,
    pub tool_manifest: String,
    pub functiongemma_instructions: String,
    pub min_confidence: f32,
}

#[tauri::command]
pub async fn get_action_router_settings(
    state: State<'_, SharedState>,
) -> Result<ActionRouterSettingsDto, String> {
    let guard = state.lock().await;
    Ok(ActionRouterSettingsDto {
        enabled: guard.router.enabled,
        auto_run_read_only: guard.router.auto_run_read_only,
        default_lang: guard.router.default_lang.clone(),
        tool_manifest: guard.router.tool_manifest.as_ref().to_string(),
        functiongemma_instructions: guard.router.functiongemma_instructions.as_ref().to_string(),
        min_confidence: guard.router.min_confidence,
    })
}

#[tauri::command]
pub async fn set_action_router_settings(
    state: State<'_, SharedState>,
    enabled: Option<bool>,
    auto_run_read_only: Option<bool>,
    default_lang: Option<String>,
    tool_manifest: Option<String>,
    functiongemma_instructions: Option<String>,
    min_confidence: Option<f32>,
) -> Result<ActionRouterSettingsDto, String> {
    let mut guard = state.lock().await;
    if let Some(v) = enabled {
        guard.router.enabled = v;
    }
    if let Some(v) = auto_run_read_only {
        guard.router.auto_run_read_only = v;
    }
    if let Some(lang) = default_lang {
        let trimmed = lang.trim();
        if !trimmed.is_empty() {
            guard.router.default_lang = trimmed.to_string();
        }
    }
    let mut tool_manifest_changed = false;
    if let Some(m) = tool_manifest {
        let trimmed = m.trim();
        let compiled = crate::tool_manifest::validate_and_compile(trimmed)?;
        guard.router.tool_manifest = std::sync::Arc::from(trimmed.to_string());
        guard.router.tool_policies = std::sync::Arc::new(compiled.policies);
        guard.router.functiongemma_declarations =
            std::sync::Arc::from(compiled.function_declarations);
        tool_manifest_changed = true;
    }

    let mut instructions_changed = false;
    if let Some(v) = functiongemma_instructions {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            guard.router.functiongemma_instructions = std::sync::Arc::from(trimmed.to_string());
            instructions_changed = true;
        }
    }

    if tool_manifest_changed || instructions_changed {
        guard.router.functiongemma_developer_context = std::sync::Arc::from(format!(
            "You are a model that can do function calling with the following functions\n{}\n{}",
            guard.router.functiongemma_instructions, guard.router.functiongemma_declarations
        ));
    }
    if let Some(v) = min_confidence {
        if !(0.0..=1.0).contains(&v) {
            return Err("min_confidence must be between 0 and 1".to_string());
        }
        guard.router.min_confidence = v;
    }
    Ok(ActionRouterSettingsDto {
        enabled: guard.router.enabled,
        auto_run_read_only: guard.router.auto_run_read_only,
        default_lang: guard.router.default_lang.clone(),
        tool_manifest: guard.router.tool_manifest.as_ref().to_string(),
        functiongemma_instructions: guard.router.functiongemma_instructions.as_ref().to_string(),
        min_confidence: guard.router.min_confidence,
    })
}
