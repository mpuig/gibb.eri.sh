//! Platform-specific implementations.

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacOSProvider;

#[cfg(target_os = "macos")]
pub use macos::{get_browser_url, get_clipboard_preview, get_selection_preview, is_browser};

// Re-export the appropriate provider for the current platform
#[cfg(target_os = "macos")]
pub type PlatformProvider = MacOSProvider;

#[cfg(not(target_os = "macos"))]
pub type PlatformProvider = crate::provider::NullProvider;

// Stub implementations for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub fn get_clipboard_preview() -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn get_selection_preview() -> Option<String> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn is_browser(_bundle_id: &str) -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn get_browser_url(_bundle_id: &str) -> Option<String> {
    None
}
