//! Commands for input emulation (The Typer).

/// Check if the application has accessibility permissions for input emulation.
#[tauri::command]
pub async fn check_input_access() -> bool {
    gibberish_input::has_accessibility_access()
}

/// Prompt the user to grant accessibility permissions.
///
/// On macOS, this opens System Settings to the Accessibility pane.
#[tauri::command]
pub async fn request_input_access() {
    gibberish_input::prompt_accessibility_access();
}
