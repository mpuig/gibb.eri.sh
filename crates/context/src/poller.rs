//! Context poller - background task that monitors system state.

use crate::provider::SystemStateProvider;
use crate::state::{ContextChangedEvent, ContextState, SystemContext};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Default polling interval for context changes.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(1500);

/// Callback type for context change events.
pub type ContextCallback = Arc<dyn Fn(ContextChangedEvent) + Send + Sync + 'static>;

/// Background poller for system context changes.
pub struct ContextPoller {
    running: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Default for ContextPoller {
    fn default() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }
}

impl ContextPoller {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start polling with the given provider and callback.
    pub fn start<P>(&mut self, provider: Arc<P>, callback: ContextCallback)
    where
        P: SystemStateProvider + 'static,
    {
        self.start_with_interval(provider, callback, DEFAULT_POLL_INTERVAL);
    }

    /// Start polling with a custom interval.
    pub fn start_with_interval<P>(
        &mut self,
        provider: Arc<P>,
        callback: ContextCallback,
        interval: Duration,
    ) where
        P: SystemStateProvider + 'static,
    {
        if self.running.load(Ordering::SeqCst) {
            tracing::warn!("ContextPoller already running");
            return;
        }

        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);

        let handle = std::thread::spawn(move || {
            tracing::info!("ContextPoller started with interval {:?}", interval);

            let mut state = ContextState::default();
            let mut last_event: Option<ContextChangedEvent> = None;

            while running.load(Ordering::SeqCst) {
                // Poll current system state
                // Note: clipboard/selection are populated just-in-time in router,
                // not on every poll (expensive)
                let system = SystemContext {
                    active_app: provider.get_active_app(),
                    is_mic_active: provider.is_mic_active(),
                    meeting_app: provider.get_meeting_app(),
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                    clipboard_preview: None,
                    selection_preview: None,
                };

                // Update state and check if mode changed
                state.update(system);

                let event = ContextChangedEvent::from(&state);

                // Only emit if something changed
                let should_emit = match &last_event {
                    None => true,
                    Some(last) => {
                        event.mode != last.mode
                            || event.active_app != last.active_app
                            || event.is_meeting != last.is_meeting
                    }
                };

                if should_emit {
                    tracing::debug!(
                        mode = %event.mode,
                        active_app = ?event.active_app,
                        is_meeting = event.is_meeting,
                        "context changed"
                    );
                    callback(event.clone());
                    last_event = Some(event);
                }

                std::thread::sleep(interval);
            }

            tracing::info!("ContextPoller stopped");
        });

        self.handle = Some(handle);
    }

    /// Stop the poller.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Check if the poller is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for ContextPoller {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::NullProvider;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_poller_lifecycle() {
        let mut poller = ContextPoller::new();
        assert!(!poller.is_running());

        let provider = Arc::new(NullProvider);
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let callback: ContextCallback = Arc::new(move |_event| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        poller.start_with_interval(provider, callback, Duration::from_millis(50));
        assert!(poller.is_running());

        // Wait for a few polls
        std::thread::sleep(Duration::from_millis(200));

        poller.stop();
        assert!(!poller.is_running());

        // Should have been called at least once (initial state)
        assert!(call_count.load(Ordering::SeqCst) >= 1);
    }
}
