//! Error types for input emulation.

use thiserror::Error;

/// Errors that can occur during input emulation.
#[derive(Debug, Error)]
pub enum InputError {
    /// Accessibility permission not granted (macOS).
    #[error("accessibility permission not granted - open System Settings > Privacy & Security > Accessibility")]
    AccessibilityNotGranted,

    /// The active window changed during typing.
    #[error("focus changed during typing - aborting for safety (was: {expected}, now: {actual})")]
    FocusChanged { expected: String, actual: String },

    /// Typing was aborted by user request (panic button).
    #[error("typing aborted by user")]
    Aborted,

    /// Failed to initialize the input controller.
    #[error("failed to initialize input controller: {0}")]
    InitFailed(String),

    /// Failed to type text.
    #[error("failed to type text: {0}")]
    TypeFailed(String),

    /// Failed to simulate key press.
    #[error("failed to simulate key: {0}")]
    KeyFailed(String),
}
