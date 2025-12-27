//! Tauri event bus adapter.
//!
//! Implements the EventBus trait using Tauri's event system.

use gibberish_events::EventBus;
use tauri::{AppHandle, Emitter, Runtime};

/// EventBus implementation that emits events via Tauri.
pub struct TauriEventBus<R: Runtime> {
    app: AppHandle<R>,
}

impl<R: Runtime> TauriEventBus<R> {
    /// Create a new TauriEventBus wrapping a Tauri AppHandle.
    pub fn new(app: AppHandle<R>) -> Self {
        Self { app }
    }
}

impl<R: Runtime> EventBus for TauriEventBus<R> {
    fn emit(&self, topic: &str, payload: serde_json::Value) {
        let _ = self.app.emit(topic, payload);
    }
}
