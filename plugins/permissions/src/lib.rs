use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

#[cfg(target_os = "macos")]
mod macos {
    use core_graphics::access::ScreenCaptureAccess;

    /// Checks if screen recording permission has been granted.
    pub fn check_screen_recording() -> bool {
        let access = ScreenCaptureAccess::default();
        access.preflight()
    }

    /// Requests screen recording permission.
    /// Returns true if permission was granted, false otherwise.
    /// Note: On macOS, this opens the System Preferences if permission hasn't been granted.
    pub fn request_screen_recording() -> bool {
        let access = ScreenCaptureAccess::default();
        access.request()
    }
}

#[tauri::command]
async fn check_screen_recording_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::check_screen_recording()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[tauri::command]
async fn request_screen_recording_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::request_screen_recording()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[tauri::command]
async fn is_screen_capture_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        // ScreenCaptureKit requires macOS 12.3+
        // For simplicity, we assume it's available if we're on macOS
        true
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gibberish-permissions")
        .invoke_handler(tauri::generate_handler![
            check_screen_recording_permission,
            request_screen_recording_permission,
            is_screen_capture_available,
        ])
        .build()
}
