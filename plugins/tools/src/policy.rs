//! Domain policy functions for tool execution.
//!
//! Centralizes policy decisions like confidence thresholds, cooldowns, and auto-run rules.
//! All values have sensible defaults but can be overridden via PolicyConfig.

use std::time::Duration;

/// Centralized configuration for all policy values.
///
/// This struct allows runtime configuration of timing and threshold values
/// that were previously hardcoded constants.
#[derive(Debug, Clone)]
pub struct PolicyConfig {
    /// Cooldown between repeated calls for the same tool+args key.
    pub tool_cooldown: Duration,
    /// Debounce delay for router queue processing.
    pub debounce: Duration,
    /// Time-to-live for cached results (e.g., Wikipedia).
    pub cache_ttl: Duration,
    /// Minimum confidence threshold for tool proposals.
    pub min_confidence: f32,
    /// Confidence score for first successful parse attempt.
    pub first_attempt_confidence: f32,
    /// Confidence score for repair attempt (model needed guidance).
    pub repair_attempt_confidence: f32,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            tool_cooldown: Duration::from_secs(45),
            debounce: Duration::from_millis(650),
            cache_ttl: Duration::from_secs(60 * 15),
            min_confidence: 0.35,
            first_attempt_confidence: 0.85,
            repair_attempt_confidence: 0.55,
        }
    }
}

/// Default cooldown between repeated calls for the same tool+args key.
pub const DEFAULT_TOOL_COOLDOWN: Duration = Duration::from_secs(45);

/// Debounce delay for router queue processing.
pub const DEBOUNCE: Duration = Duration::from_millis(650);

/// Time-to-live for cached Wikipedia results.
pub const CACHE_TTL: Duration = Duration::from_secs(60 * 15);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert!(DEFAULT_TOOL_COOLDOWN.as_secs() > 0);
        assert!(DEBOUNCE.as_millis() > 0);
        assert!(CACHE_TTL.as_secs() > 0);
    }

    #[test]
    fn test_policy_config_defaults() {
        let config = PolicyConfig::default();
        assert_eq!(config.tool_cooldown, DEFAULT_TOOL_COOLDOWN);
        assert_eq!(config.debounce, DEBOUNCE);
        assert_eq!(config.cache_ttl, CACHE_TTL);
        assert!((config.min_confidence - 0.35).abs() < f32::EPSILON);
        assert!((config.first_attempt_confidence - 0.85).abs() < f32::EPSILON);
        assert!((config.repair_attempt_confidence - 0.55).abs() < f32::EPSILON);
    }
}
