//! Event bus abstraction for decoupled event emission.
//!
//! Provides a trait-based abstraction over event emission, allowing the core
//! logic to be tested without Tauri and enabling future CLI/headless modes.

use std::sync::{Arc, Mutex};

/// Trait for emitting events to subscribers.
///
/// This abstraction decouples the core logic from Tauri's event system,
/// enabling:
/// - Unit testing without Tauri runtime
/// - Future CLI mode
/// - Headless server deployment
pub trait EventBus: Send + Sync {
    /// Emit an event with a JSON payload.
    ///
    /// # Arguments
    /// * `topic` - Event name/topic (e.g., "tools:router_status")
    /// * `payload` - JSON payload to emit
    fn emit(&self, topic: &str, payload: serde_json::Value);
}

/// Type alias for shared event bus reference.
pub type EventBusRef = Arc<dyn EventBus>;

/// In-memory event bus for testing.
///
/// Captures all emitted events for later inspection.
#[derive(Default)]
pub struct InMemoryEventBus {
    events: Mutex<Vec<EmittedEvent>>,
}

/// A captured event from InMemoryEventBus.
#[derive(Debug, Clone)]
pub struct EmittedEvent {
    pub topic: String,
    pub payload: serde_json::Value,
}

impl InMemoryEventBus {
    /// Create a new in-memory event bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all captured events.
    pub fn events(&self) -> Vec<EmittedEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Get events for a specific topic.
    pub fn events_for(&self, topic: &str) -> Vec<EmittedEvent> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.topic == topic)
            .cloned()
            .collect()
    }

    /// Clear all captured events.
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }

    /// Get the number of captured events.
    pub fn len(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Check if no events have been captured.
    pub fn is_empty(&self) -> bool {
        self.events.lock().unwrap().is_empty()
    }
}

impl EventBus for InMemoryEventBus {
    fn emit(&self, topic: &str, payload: serde_json::Value) {
        self.events.lock().unwrap().push(EmittedEvent {
            topic: topic.to_string(),
            payload,
        });
    }
}

/// No-op event bus that discards all events.
///
/// Useful for benchmarking or when events are not needed.
pub struct NullEventBus;

impl EventBus for NullEventBus {
    fn emit(&self, _topic: &str, _payload: serde_json::Value) {
        // Intentionally empty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_in_memory_event_bus() {
        let bus = InMemoryEventBus::new();

        bus.emit("test:event1", json!({"key": "value1"}));
        bus.emit("test:event2", json!({"key": "value2"}));
        bus.emit("test:event1", json!({"key": "value3"}));

        assert_eq!(bus.len(), 3);
        assert_eq!(bus.events_for("test:event1").len(), 2);
        assert_eq!(bus.events_for("test:event2").len(), 1);
        assert_eq!(bus.events_for("test:missing").len(), 0);
    }

    #[test]
    fn test_in_memory_event_bus_clear() {
        let bus = InMemoryEventBus::new();

        bus.emit("test:event", json!({}));
        assert!(!bus.is_empty());

        bus.clear();
        assert!(bus.is_empty());
    }

    #[test]
    fn test_null_event_bus() {
        let bus = NullEventBus;
        // Should not panic
        bus.emit("test:event", json!({"data": "ignored"}));
    }
}
