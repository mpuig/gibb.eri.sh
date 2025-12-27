//! Focus checking adapter for the input crate.
//!
//! Bridges gibberish-context's PlatformProvider to gibberish-input's FocusChecker trait.

use gibberish_context::platform::PlatformProvider;
use gibberish_context::ActiveAppProvider;
use gibberish_input::FocusChecker;

/// FocusChecker implementation using the platform provider.
///
/// Adapts `gibberish_context::PlatformProvider` to implement
/// `gibberish_input::FocusChecker`, allowing the input crate to
/// verify window focus without depending on the context crate.
pub struct PlatformFocusChecker {
    provider: PlatformProvider,
}

impl PlatformFocusChecker {
    /// Create a new platform-based focus checker.
    pub fn new() -> Self {
        Self {
            provider: PlatformProvider::new(),
        }
    }
}

impl Default for PlatformFocusChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusChecker for PlatformFocusChecker {
    fn get_current_focus(&self) -> Option<String> {
        self.provider.get_active_app().map(|app| app.bundle_id)
    }
}
