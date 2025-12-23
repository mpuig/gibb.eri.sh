use tauri::{command, AppHandle, Runtime, State};

use crate::SharedState;

#[command]
pub async fn list_installed_applications() -> Vec<gibberish_detect::InstalledApp> {
    gibberish_detect::list_installed_apps()
}

#[command]
pub async fn list_mic_using_applications() -> Vec<gibberish_detect::InstalledApp> {
    gibberish_detect::list_mic_using_apps()
}

#[command]
pub async fn set_ignored_bundle_ids<R: Runtime>(
    _app: AppHandle<R>,
    state: State<'_, SharedState>,
    bundle_ids: Vec<String>,
) -> Result<(), String> {
    let mut state_guard = state.lock().await;
    state_guard.ignored_bundle_ids = bundle_ids;
    Ok(())
}

#[command]
pub async fn list_default_ignored_bundle_ids() -> Vec<String> {
    crate::handler::default_ignored_bundle_ids()
}
