//! Domain policy functions for tool execution.
//!
//! Centralizes policy decisions like confidence thresholds, cooldowns, and auto-run rules.

use std::time::Duration;

/// Cooldown between queries for the same city.
pub const CITY_COOLDOWN: Duration = Duration::from_secs(45);

/// Debounce delay for router queue processing.
pub const DEBOUNCE: Duration = Duration::from_millis(650);

/// Time-to-live for cached Wikipedia results.
pub const CACHE_TTL: Duration = Duration::from_secs(60 * 15);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert!(CITY_COOLDOWN.as_secs() > 0);
        assert!(DEBOUNCE.as_millis() > 0);
        assert!(CACHE_TTL.as_secs() > 0);
    }
}
