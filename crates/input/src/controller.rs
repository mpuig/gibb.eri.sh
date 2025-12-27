//! Input controller for safe text typing and key simulation.

use crate::error::InputError;
use crate::FocusCheckerRef;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Default delay between keystrokes (10ms).
pub const DEFAULT_TYPING_DELAY_MS: u64 = 10;

/// Options for text typing.
#[derive(Debug, Clone)]
pub struct TypeOptions {
    /// Delay between each character in milliseconds.
    pub delay_ms: u64,
    /// Whether to verify focus hasn't changed during typing.
    pub verify_focus: bool,
    /// Whether to run in preview mode (don't actually type).
    pub preview: bool,
}

impl Default for TypeOptions {
    fn default() -> Self {
        Self {
            delay_ms: DEFAULT_TYPING_DELAY_MS,
            verify_focus: true,
            preview: false,
        }
    }
}

/// Result of a typing operation.
#[derive(Debug)]
pub struct TypeResult {
    /// Number of characters typed.
    pub chars_typed: usize,
    /// Whether typing was completed fully.
    pub completed: bool,
    /// If preview mode, the text that would have been typed.
    pub preview_text: Option<String>,
}

/// Controller for input emulation.
///
/// Provides safe, rate-limited text typing with focus verification.
pub struct InputController {
    enigo: Enigo,
    abort_flag: Arc<AtomicBool>,
    focus_checker: Option<FocusCheckerRef>,
}

impl InputController {
    /// Create a new input controller.
    ///
    /// # Arguments
    ///
    /// * `focus_checker` - Optional focus checker for verifying window focus.
    ///   Pass `None` to disable focus verification.
    ///
    /// # Errors
    ///
    /// Returns an error if accessibility permissions are not granted (macOS)
    /// or if the input system fails to initialize.
    pub fn new(focus_checker: Option<FocusCheckerRef>) -> Result<Self, InputError> {
        // Check accessibility permissions first
        if !crate::has_accessibility_access() {
            return Err(InputError::AccessibilityNotGranted);
        }

        let enigo =
            Enigo::new(&Settings::default()).map_err(|e| InputError::InitFailed(e.to_string()))?;

        Ok(Self {
            enigo,
            abort_flag: Arc::new(AtomicBool::new(false)),
            focus_checker,
        })
    }

    /// Get a handle to the abort flag.
    ///
    /// Set this to `true` to abort any ongoing typing operation.
    pub fn abort_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.abort_flag)
    }

    /// Abort any ongoing typing operation.
    pub fn abort(&self) {
        self.abort_flag.store(true, Ordering::SeqCst);
    }

    /// Reset the abort flag.
    pub fn reset_abort(&self) {
        self.abort_flag.store(false, Ordering::SeqCst);
    }

    /// Type text with rate limiting and optional focus verification.
    ///
    /// This is a synchronous operation that blocks the calling thread.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to type.
    /// * `options` - Typing options (delay, focus verification, preview).
    ///
    /// # Returns
    ///
    /// A `TypeResult` indicating how many characters were typed and whether
    /// the operation completed successfully.
    ///
    /// # Errors
    ///
    /// - `InputError::FocusChanged` if the active window changes during typing
    /// - `InputError::Aborted` if the abort flag was set
    /// - `InputError::TypeFailed` if a character fails to type
    pub fn type_text(&mut self, text: &str, options: TypeOptions) -> Result<TypeResult, InputError> {
        // Reset abort flag at start
        self.abort_flag.store(false, Ordering::SeqCst);

        // Preview mode - just return what would be typed
        if options.preview {
            return Ok(TypeResult {
                chars_typed: 0,
                completed: true,
                preview_text: Some(text.to_string()),
            });
        }

        // Get initial focus for verification (only if we have a checker and verify_focus is true)
        let initial_focus = if options.verify_focus {
            self.focus_checker
                .as_ref()
                .and_then(|checker| checker.get_current_focus())
        } else {
            None
        };

        let delay = Duration::from_millis(options.delay_ms);
        let mut chars_typed = 0;

        for ch in text.chars() {
            // Check abort flag
            if self.abort_flag.load(Ordering::SeqCst) {
                return Err(InputError::Aborted);
            }

            // Verify focus hasn't changed (only if we captured initial focus)
            if let Some(ref expected) = initial_focus {
                if let Some(ref checker) = self.focus_checker {
                    if let Some(current) = checker.get_current_focus() {
                        if &current != expected {
                            return Err(InputError::FocusChanged {
                                expected: expected.clone(),
                                actual: current,
                            });
                        }
                    }
                }
            }

            // Type the character
            self.enigo
                .text(&ch.to_string())
                .map_err(|e| InputError::TypeFailed(e.to_string()))?;

            chars_typed += 1;

            // Rate limiting delay
            if options.delay_ms > 0 {
                thread::sleep(delay);
            }
        }

        Ok(TypeResult {
            chars_typed,
            completed: true,
            preview_text: None,
        })
    }

    /// Type text without focus verification (faster but less safe).
    ///
    /// Use this only when you're certain the target window won't change.
    pub fn type_text_fast(&mut self, text: &str, delay_ms: u64) -> Result<TypeResult, InputError> {
        self.type_text(
            text,
            TypeOptions {
                delay_ms,
                verify_focus: false,
                preview: false,
            },
        )
    }

    /// Simulate a paste operation (Cmd+V on macOS, Ctrl+V elsewhere).
    ///
    /// This is a quick operation that triggers the system paste shortcut.
    /// Unlike `type_text`, this preserves formatting and handles large text.
    pub fn paste(&mut self) -> Result<(), InputError> {
        // Check abort flag
        if self.abort_flag.load(Ordering::SeqCst) {
            return Err(InputError::Aborted);
        }

        // Use Meta (Cmd) on macOS, Control on other platforms
        #[cfg(target_os = "macos")]
        let modifier = Key::Meta;
        #[cfg(not(target_os = "macos"))]
        let modifier = Key::Control;

        // Press modifier
        self.enigo
            .key(modifier, Direction::Press)
            .map_err(|e| InputError::KeyFailed(e.to_string()))?;

        // Small delay to ensure modifier is registered
        thread::sleep(Duration::from_millis(10));

        // Press and release V
        self.enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| InputError::KeyFailed(e.to_string()))?;

        // Release modifier
        self.enigo
            .key(modifier, Direction::Release)
            .map_err(|e| InputError::KeyFailed(e.to_string()))?;

        Ok(())
    }
}

impl std::fmt::Debug for InputController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputController")
            .field("abort_flag", &self.abort_flag.load(Ordering::SeqCst))
            .field("has_focus_checker", &self.focus_checker.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_mode() {
        // Skip if no accessibility (can't create controller)
        let mut controller = match InputController::new(None) {
            Ok(c) => c,
            Err(InputError::AccessibilityNotGranted) => {
                println!("Skipping test - no accessibility permission");
                return;
            }
            Err(e) => panic!("Unexpected error: {}", e),
        };

        let result = controller
            .type_text(
                "Hello",
                TypeOptions {
                    preview: true,
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(result.preview_text, Some("Hello".to_string()));
        assert_eq!(result.chars_typed, 0);
        assert!(result.completed);
    }

    #[test]
    fn test_abort_flag() {
        let controller = match InputController::new(None) {
            Ok(c) => c,
            Err(_) => return, // Skip if can't create controller
        };

        assert!(!controller.abort_flag.load(Ordering::SeqCst));
        controller.abort();
        assert!(controller.abort_flag.load(Ordering::SeqCst));
        controller.reset_abort();
        assert!(!controller.abort_flag.load(Ordering::SeqCst));
    }
}
