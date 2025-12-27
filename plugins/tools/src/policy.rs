//! Domain policy constants for tool execution.
//!
//! Centralizes policy values like confidence thresholds, cooldowns, and cache TTLs.

use std::time::Duration;

/// Default cooldown between repeated calls for the same tool+args key.
pub const DEFAULT_TOOL_COOLDOWN: Duration = Duration::from_secs(45);

/// Debounce delay for router queue processing.
pub const DEBOUNCE: Duration = Duration::from_millis(650);

/// Time-to-live for cached results (e.g., Wikipedia lookups).
pub const CACHE_TTL: Duration = Duration::from_secs(60 * 15);

/// Minimum confidence threshold for tool proposals.
pub const MIN_CONFIDENCE: f32 = 0.35;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert!(DEFAULT_TOOL_COOLDOWN.as_secs() > 0);
        assert!(DEBOUNCE.as_millis() > 0);
        assert!(CACHE_TTL.as_secs() > 0);
        assert!(MIN_CONFIDENCE > 0.0 && MIN_CONFIDENCE < 1.0);
    }
}
