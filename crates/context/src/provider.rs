//! Provider traits for system state detection.
//!
//! These traits abstract platform-specific implementations,
//! allowing the domain logic to remain pure and testable.

use crate::state::AppInfo;

/// Provider for detecting the currently focused application.
pub trait ActiveAppProvider: Send + Sync {
    /// Get the currently focused application.
    fn get_active_app(&self) -> Option<AppInfo>;
}

/// Provider for detecting microphone activity.
pub trait MicActivityProvider: Send + Sync {
    /// Check if any application is using the microphone.
    fn is_mic_active(&self) -> bool;

    /// Get the bundle IDs of apps currently using the microphone.
    fn get_mic_using_apps(&self) -> Vec<String>;
}

/// Combined provider for all system state.
pub trait SystemStateProvider: Send + Sync {
    /// Get the currently focused application.
    fn get_active_app(&self) -> Option<AppInfo>;

    /// Check if any application is using the microphone.
    fn is_mic_active(&self) -> bool;

    /// Get the bundle ID of a detected meeting app (if any).
    fn get_meeting_app(&self) -> Option<String>;
}

/// Null implementation for testing or unsupported platforms.
pub struct NullProvider;

impl ActiveAppProvider for NullProvider {
    fn get_active_app(&self) -> Option<AppInfo> {
        None
    }
}

impl MicActivityProvider for NullProvider {
    fn is_mic_active(&self) -> bool {
        false
    }

    fn get_mic_using_apps(&self) -> Vec<String> {
        Vec::new()
    }
}

impl SystemStateProvider for NullProvider {
    fn get_active_app(&self) -> Option<AppInfo> {
        None
    }

    fn is_mic_active(&self) -> bool {
        false
    }

    fn get_meeting_app(&self) -> Option<String> {
        None
    }
}
