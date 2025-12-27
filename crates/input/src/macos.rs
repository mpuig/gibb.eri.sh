//! macOS-specific functionality for input emulation.

use std::process::Command;

/// Check if the application has accessibility permissions on macOS.
///
/// Uses the `AXIsProcessTrustedWithOptions` API to check if the app
/// is trusted for accessibility features.
pub fn has_accessibility_access() -> bool {
    // Use the AXIsProcessTrusted function from ApplicationServices
    // This is the simplest check - returns true if already trusted
    unsafe {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        AXIsProcessTrusted()
    }
}

/// Prompt the user to grant accessibility permissions.
///
/// Opens the macOS System Settings to the Accessibility pane.
/// The user must manually add the app to the allowed list.
pub fn prompt_accessibility_access() {
    // Open System Settings to the Accessibility pane
    let _ = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// Check if accessibility is granted, prompting if not.
///
/// Returns `true` if access is granted (either already or after prompt).
/// Note: Even after prompting, this will return `false` until the user
/// actually grants permission and restarts the app.
pub fn ensure_accessibility_access() -> bool {
    if has_accessibility_access() {
        return true;
    }

    tracing::warn!("Accessibility permission not granted. Opening System Settings...");
    prompt_accessibility_access();
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_accessibility_access() {
        // This test just verifies the function doesn't panic
        // The actual result depends on system permissions
        let _result = has_accessibility_access();
        println!("Accessibility access: {}", _result);
    }
}
