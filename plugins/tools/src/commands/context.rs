//! Context awareness commands.

use crate::SharedState;
use gibberish_context::Mode;
use tauri::State;

/// DTO for context state response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContextDto {
    pub mode: Mode,
    pub detected_mode: Mode,
    pub pinned_mode: Option<Mode>,
    pub active_app: Option<String>,
    pub active_app_name: Option<String>,
    pub is_meeting: bool,
    pub timestamp_ms: i64,
}

impl From<&gibberish_context::ContextState> for ContextDto {
    fn from(state: &gibberish_context::ContextState) -> Self {
        Self {
            mode: state.effective_mode(),
            detected_mode: state.detected_mode,
            pinned_mode: state.pinned_mode,
            active_app: state
                .system
                .active_app
                .as_ref()
                .map(|a| a.bundle_id.clone()),
            active_app_name: state
                .system
                .active_app
                .as_ref()
                .and_then(|a| a.name.clone()),
            is_meeting: state.system.has_meeting_app() && state.system.is_mic_active,
            timestamp_ms: state.system.timestamp_ms,
        }
    }
}

/// Get current context state.
#[tauri::command]
pub async fn get_context(state: State<'_, SharedState>) -> Result<ContextDto, String> {
    let guard = state.lock().await;
    Ok(ContextDto::from(&guard.context))
}

/// Pin a specific mode (disables auto-switching).
#[tauri::command]
pub async fn pin_context_mode(
    state: State<'_, SharedState>,
    mode: Mode,
) -> Result<ContextDto, String> {
    let mut guard = state.lock().await;
    let prev_mode = guard.context.effective_mode();
    guard.context.pin_mode(mode);

    // Update router manifest if mode changed
    if prev_mode != mode {
        guard.router.update_for_mode(mode);
    }

    Ok(ContextDto::from(&guard.context))
}

/// Unpin mode (re-enable auto-switching).
#[tauri::command]
pub async fn unpin_context_mode(state: State<'_, SharedState>) -> Result<ContextDto, String> {
    let mut guard = state.lock().await;
    let prev_mode = guard.context.effective_mode();
    guard.context.unpin_mode();
    let new_mode = guard.context.effective_mode();

    // Update router manifest if mode changed
    if prev_mode != new_mode {
        guard.router.update_for_mode(new_mode);
    }

    Ok(ContextDto::from(&guard.context))
}
