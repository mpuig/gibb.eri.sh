//! Global panic hotkey (Esc x3) for emergency abort.
//!
//! Listens for 3 consecutive Escape key presses within a short time window
//! and triggers an abort callback. This provides a safety mechanism to
//! immediately stop any ongoing input operation.
//!
//! IMPLEMENTATION NOTE:
//! We use `device_query` (polling) instead of `rdev` (event hooks) to avoid
//! crashing on macOS. `rdev` calls `TSMCurrentKeyboardInputSourceRefCreate`
//! which is not thread-safe and causes a `dispatch_assert_queue_fail` on
//! recent macOS versions when run in a background thread.
//!
//! Polling at 50ms is sufficient to detect human key presses (usually >100ms)
//! while using negligible CPU.

use device_query::{DeviceQuery, DeviceState, Keycode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Number of Esc presses required to trigger panic.
const ESC_COUNT_THRESHOLD: usize = 3;

/// Time window for consecutive Esc presses (1 second).
const ESC_WINDOW: Duration = Duration::from_secs(1);

/// Polling interval for key state.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Handle to control the panic hotkey listener.
pub struct PanicHotkeyHandle {
    running: Arc<AtomicBool>,
}

impl PanicHotkeyHandle {
    /// Stop the panic hotkey listener.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if the listener is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for PanicHotkeyHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the global panic hotkey listener.
///
/// This spawns a background thread that polls global key state.
/// When 3 Escape presses are detected within 1 second, the provided
/// abort flag is set to true.
pub fn start_panic_hotkey_listener(abort_flag: Arc<AtomicBool>) -> PanicHotkeyHandle {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&running);

    std::thread::spawn(move || {
        let device_state = DeviceState::new();
        let mut esc_times: Vec<Instant> = Vec::with_capacity(ESC_COUNT_THRESHOLD);
        let mut was_esc_pressed = false;

        while running_clone.load(Ordering::SeqCst) {
            let keys = device_state.get_keys();
            let is_esc_pressed = keys.contains(&Keycode::Escape);

            // Detect Rising Edge (Press)
            if is_esc_pressed && !was_esc_pressed {
                let now = Instant::now();

                // Remove old timestamps outside the window
                esc_times.retain(|&t| now.duration_since(t) < ESC_WINDOW);

                // Add current press
                esc_times.push(now);

                tracing::trace!(
                    count = esc_times.len(),
                    "Escape key pressed, tracking for panic hotkey"
                );

                // Check if threshold reached
                if esc_times.len() >= ESC_COUNT_THRESHOLD {
                    tracing::warn!("Panic hotkey triggered (Esc x3)! Aborting input operations.");
                    abort_flag.store(true, Ordering::SeqCst);
                    esc_times.clear();
                }
            }

            was_esc_pressed = is_esc_pressed;
            std::thread::sleep(POLL_INTERVAL);
        }
    });

    PanicHotkeyHandle { running }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_stop() {
        let flag = Arc::new(AtomicBool::new(false));
        let handle = PanicHotkeyHandle {
            running: Arc::new(AtomicBool::new(true)),
        };

        assert!(handle.is_running());
        handle.stop();
        assert!(!handle.is_running());

        // Flag should be unchanged
        assert!(!flag.load(Ordering::SeqCst));
    }
}