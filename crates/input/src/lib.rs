//! Input emulation for gibb.eri.sh.
//!
//! Provides safe, rate-limited text typing and key sequence simulation.
//! Designed for voice-controlled input with safety guardrails.
//!
//! # Safety Features
//!
//! - **Rate limiting**: Configurable delay between keystrokes to prevent app crashes
//! - **Focus verification**: Aborts if the active window changes during typing
//! - **Accessibility check**: Verifies macOS accessibility permissions before use
//!
//! # Example
//!
//! ```ignore
//! use gibberish_input::{InputController, TypeOptions};
//!
//! let controller = InputController::new(None)?;
//!
//! // Type text with default rate limiting (10ms per character)
//! controller.type_text("Hello, world!", TypeOptions::default())?;
//! ```

mod controller;
mod error;

#[cfg(target_os = "macos")]
mod macos;

use std::sync::Arc;

pub use controller::{InputController, TypeOptions, TypeResult};
pub use error::InputError;

/// Trait for checking window focus during typing.
///
/// Implement this to provide focus verification for your platform.
/// The input controller will abort typing if focus changes mid-operation.
pub trait FocusChecker: Send + Sync {
    /// Get the current focused window/app identifier.
    ///
    /// Returns `None` if focus cannot be determined.
    fn get_current_focus(&self) -> Option<String>;
}

/// No-op focus checker that always returns None (disables focus verification).
pub struct NoFocusChecker;

impl FocusChecker for NoFocusChecker {
    fn get_current_focus(&self) -> Option<String> {
        None
    }
}

/// Type alias for the focus checker.
pub type FocusCheckerRef = Arc<dyn FocusChecker>;

/// Check if the application has accessibility permissions.
///
/// On macOS, input simulation requires Accessibility permission.
/// Returns `true` if permissions are granted, `false` otherwise.
///
/// On other platforms, always returns `true`.
pub fn has_accessibility_access() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::has_accessibility_access()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Prompt the user to grant accessibility permissions.
///
/// On macOS, this opens the System Settings to the Accessibility pane.
/// On other platforms, this is a no-op.
pub fn prompt_accessibility_access() {
    #[cfg(target_os = "macos")]
    {
        macos::prompt_accessibility_access();
    }
}

/// Check if accessibility is granted, prompting if not.
///
/// Returns `true` if access is granted.
/// Note: Even after prompting, this will return `false` until the user
/// actually grants permission and restarts the app.
///
/// On non-macOS platforms, always returns `true`.
pub fn ensure_accessibility_access() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::ensure_accessibility_access()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
