//! macOS-specific implementation of system state providers.

use crate::provider::{ActiveAppProvider, MicActivityProvider, SystemStateProvider};
use crate::state::AppInfo;
use std::process::Command;

// Native Cocoa imports for efficient frontmost app detection
use objc::runtime::{Class, Object};
use objc::{msg_send, sel, sel_impl};

/// macOS implementation using native Cocoa APIs for active window detection.
///
/// Uses NSWorkspace.frontmostApplication for efficient frontmost app queries
/// without subprocess overhead.
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
        self.mic_active.load(std::sync::atomic::Ordering::SeqCst)
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
        self.mic_active.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn get_meeting_app(&self) -> Option<String> {
        self.meeting_app.read().ok().and_then(|guard| guard.clone())
    }
}

/// Get the frontmost application using native Cocoa APIs.
///
/// Returns bundle ID and name of the currently focused app.
/// Uses NSWorkspace.sharedWorkspace.frontmostApplication for efficiency.
fn get_frontmost_app() -> Option<AppInfo> {
    unsafe {
        // Get NSWorkspace class
        let workspace_class = Class::get("NSWorkspace")?;

        // Get shared workspace: [NSWorkspace sharedWorkspace]
        let shared_workspace: *mut Object = msg_send![workspace_class, sharedWorkspace];
        if shared_workspace.is_null() {
            return None;
        }

        // Get frontmost application: [workspace frontmostApplication]
        let frontmost_app: *mut Object = msg_send![shared_workspace, frontmostApplication];
        if frontmost_app.is_null() {
            return None;
        }

        // Get bundle identifier: [app bundleIdentifier]
        let bundle_id_ns: *mut Object = msg_send![frontmost_app, bundleIdentifier];
        let bundle_id = nsstring_to_string(bundle_id_ns)?;

        if bundle_id.is_empty() {
            return None;
        }

        // Get localized name: [app localizedName]
        let name_ns: *mut Object = msg_send![frontmost_app, localizedName];
        let name = nsstring_to_string(name_ns);

        Some(AppInfo { bundle_id, name })
    }
}

/// Convert NSString to Rust String.
unsafe fn nsstring_to_string(nsstring: *mut Object) -> Option<String> {
    if nsstring.is_null() {
        return None;
    }

    // Get UTF8 C string: [nsstring UTF8String]
    let c_str: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
    if c_str.is_null() {
        return None;
    }

    let rust_str = std::ffi::CStr::from_ptr(c_str).to_str().ok()?;
    Some(rust_str.to_string())
}

/// Get the current clipboard text contents.
///
/// Returns a preview (first ~500 chars) of the clipboard if it contains text.
/// Returns None if clipboard is empty or contains non-text data.
pub fn get_clipboard_preview() -> Option<String> {
    let output = Command::new("pbpaste").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let text = text.trim();

    if text.is_empty() {
        return None;
    }

    // Truncate to ~500 chars for efficiency
    let preview = if text.len() > 500 {
        format!("{}...", &text[..500])
    } else {
        text.to_string()
    };

    Some(preview)
}

/// Known browser bundle IDs for URL detection.
pub const BROWSER_BUNDLE_IDS: &[&str] = &[
    "com.apple.Safari",
    "com.google.Chrome",
    "org.mozilla.firefox",
    "company.thebrowser.Browser", // Arc
    "com.brave.Browser",
    "com.microsoft.edgemac",
    "com.vivaldi.Vivaldi",
    "com.operasoftware.Opera",
];

/// Check if the given bundle ID is a browser.
pub fn is_browser(bundle_id: &str) -> bool {
    BROWSER_BUNDLE_IDS.iter().any(|&b| b == bundle_id)
}

/// Get the active browser tab URL.
///
/// Uses AppleScript to query the frontmost browser for its active tab URL.
/// Returns None if:
/// - The frontmost app is not a supported browser
/// - The browser is in private/incognito mode
/// - AppleScript execution fails
pub fn get_browser_url(bundle_id: &str) -> Option<String> {
    if !is_browser(bundle_id) {
        return None;
    }

    let script = match bundle_id {
        "com.apple.Safari" => r#"
            tell application "Safari"
                if (count of windows) > 0 then
                    set frontWindow to front window
                    -- Check for private browsing (Private mode windows have no "current tab")
                    try
                        if frontWindow's name contains "Private" then
                            return ""
                        end if
                        return URL of current tab of frontWindow
                    on error
                        return ""
                    end try
                end if
            end tell
            return ""
        "#,
        "com.google.Chrome" | "com.brave.Browser" | "com.microsoft.edgemac" | "com.vivaldi.Vivaldi" => {
            // Chromium-based browsers use the same AppleScript structure
            let app_name = match bundle_id {
                "com.google.Chrome" => "Google Chrome",
                "com.brave.Browser" => "Brave Browser",
                "com.microsoft.edgemac" => "Microsoft Edge",
                "com.vivaldi.Vivaldi" => "Vivaldi",
                _ => return None,
            };
            return get_chromium_url(app_name);
        }
        "org.mozilla.firefox" => r#"
            tell application "Firefox"
                if (count of windows) > 0 then
                    try
                        -- Firefox's AppleScript support is limited
                        -- We get the URL via accessibility if available
                        tell application "System Events"
                            tell process "Firefox"
                                set frontWindow to front window
                                -- Check for private window
                                if name of frontWindow contains "Private" then
                                    return ""
                                end if
                                -- Try to get URL from address bar
                                set urlField to text field 1 of toolbar 1 of frontWindow
                                return value of urlField
                            end tell
                        end tell
                    on error
                        return ""
                    end try
                end if
            end tell
            return ""
        "#,
        "company.thebrowser.Browser" => r#"
            tell application "Arc"
                if (count of windows) > 0 then
                    try
                        set frontWindow to front window
                        return URL of active tab of frontWindow
                    on error
                        return ""
                    end try
                end if
            end tell
            return ""
        "#,
        "com.operasoftware.Opera" => {
            return get_chromium_url("Opera");
        }
        _ => return None,
    };

    execute_url_script(script)
}

/// Get URL from Chromium-based browser.
fn get_chromium_url(app_name: &str) -> Option<String> {
    let script = format!(
        r#"
        tell application "{}"
            if (count of windows) > 0 then
                set frontWindow to front window
                -- Check for incognito mode
                try
                    if mode of frontWindow is "incognito" then
                        return ""
                    end if
                on error
                    -- mode property might not exist, continue
                end try
                try
                    return URL of active tab of frontWindow
                on error
                    return ""
                end try
            end if
        end tell
        return ""
        "#,
        app_name
    );

    execute_url_script(&script)
}

/// Execute AppleScript to get URL.
fn execute_url_script(script: &str) -> Option<String> {
    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let url = String::from_utf8_lossy(&output.stdout);
    let url = url.trim();

    // Filter out empty results and common non-URLs
    if url.is_empty() || url == "missing value" {
        return None;
    }

    // Basic URL validation - should start with http(s) or file
    if !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.starts_with("file://")
    {
        return None;
    }

    Some(url.to_string())
}

/// Get the currently selected text using Accessibility API.
///
/// Returns the selected text in the frontmost application.
/// This requires Accessibility permission.
pub fn get_selection_preview() -> Option<String> {
    // AppleScript to get selected text via System Events
    let script = r#"
        tell application "System Events"
            set frontApp to first application process whose frontmost is true
            set appName to name of frontApp

            try
                tell frontApp
                    set selectedText to value of attribute "AXSelectedText" of (first UI element whose focused is true)
                    if selectedText is not missing value and selectedText is not "" then
                        return selectedText
                    end if
                end tell
            end try
        end tell
        return ""
    "#;

    let output = Command::new("osascript")
        .args(["-e", script])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let text = text.trim();

    if text.is_empty() {
        return None;
    }

    // Truncate to ~300 chars
    let preview = if text.len() > 300 {
        format!("{}...", &text[..300])
    } else {
        text.to_string()
    };

    Some(preview)
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
