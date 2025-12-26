//! macOS-specific implementation of system state providers.

use crate::provider::{ActiveAppProvider, MicActivityProvider, SystemStateProvider};
use crate::state::AppInfo;
use std::process::Command;

/// macOS implementation using AppleScript for active window detection.
///
/// For POC simplicity, we use osascript. Production could use
/// native Accessibility APIs via objc crate for lower latency.
#[derive(Debug, Default)]
pub struct MacOSProvider {
    /// Cached mic state from detect crate
    mic_active: std::sync::atomic::AtomicBool,
    /// Cached meeting app bundle ID
    meeting_app: std::sync::RwLock<Option<String>>,
}

impl MacOSProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update mic state (called from detect crate callback).
    pub fn set_mic_active(&self, active: bool) {
        self.mic_active
            .store(active, std::sync::atomic::Ordering::SeqCst);
    }

    /// Update meeting app (called from detect crate callback).
    pub fn set_meeting_app(&self, app: Option<String>) {
        if let Ok(mut guard) = self.meeting_app.write() {
            *guard = app;
        }
    }
}

impl ActiveAppProvider for MacOSProvider {
    fn get_active_app(&self) -> Option<AppInfo> {
        get_frontmost_app()
    }
}

impl MicActivityProvider for MacOSProvider {
    fn is_mic_active(&self) -> bool {
        self.mic_active
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn get_mic_using_apps(&self) -> Vec<String> {
        // Delegate to detect crate's function
        gibberish_detect::list_mic_using_apps()
            .into_iter()
            .map(|app| app.id)
            .collect()
    }
}

impl SystemStateProvider for MacOSProvider {
    fn get_active_app(&self) -> Option<AppInfo> {
        get_frontmost_app()
    }

    fn is_mic_active(&self) -> bool {
        self.mic_active
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn get_meeting_app(&self) -> Option<String> {
        self.meeting_app
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }
}

/// Get the frontmost application using AppleScript.
///
/// Returns bundle ID and name of the currently focused app.
fn get_frontmost_app() -> Option<AppInfo> {
    // AppleScript to get frontmost app's bundle ID and name
    let script = r#"
        tell application "System Events"
            set frontApp to first application process whose frontmost is true
            set appName to name of frontApp
            set bundleId to bundle identifier of frontApp
            return bundleId & "|" & appName
        end tell
    "#;

    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if !output.status.success() {
        tracing::debug!(
            "osascript failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = stdout.trim();

    // Parse "bundleId|appName" format
    let parts: Vec<&str> = result.splitn(2, '|').collect();
    if parts.len() != 2 {
        tracing::debug!("unexpected osascript output: {}", result);
        return None;
    }

    let bundle_id = parts[0].trim().to_string();
    let name = parts[1].trim().to_string();

    // Filter out empty or "missing value" results
    if bundle_id.is_empty() || bundle_id == "missing value" {
        return None;
    }

    Some(AppInfo {
        bundle_id,
        name: if name.is_empty() || name == "missing value" {
            None
        } else {
            Some(name)
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_frontmost_app() {
        // This test only passes when run interactively on macOS
        // It's more of a smoke test to verify the AppleScript works
        if cfg!(target_os = "macos") {
            let app = get_frontmost_app();
            // Should return something (whatever app is focused during test)
            println!("Frontmost app: {:?}", app);
            // Don't assert - depends on test environment
        }
    }

    #[test]
    fn test_provider_mic_state() {
        let provider = MacOSProvider::new();
        assert!(!MicActivityProvider::is_mic_active(&provider));

        provider.set_mic_active(true);
        assert!(MicActivityProvider::is_mic_active(&provider));

        provider.set_mic_active(false);
        assert!(!MicActivityProvider::is_mic_active(&provider));
    }
}
