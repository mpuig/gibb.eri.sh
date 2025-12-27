//! Clipboard adapter for deictic resolution.
//!
//! Implements ClipboardProvider using arboard for cross-platform clipboard access.

use crate::deictic::ClipboardProvider;

/// ClipboardProvider implementation using arboard.
pub struct PlatformClipboard;

impl PlatformClipboard {
    /// Create a new platform clipboard provider.
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlatformClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardProvider for PlatformClipboard {
    fn get_text(&self) -> Option<String> {
        arboard::Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok())
            .filter(|s| !s.is_empty())
    }
}
