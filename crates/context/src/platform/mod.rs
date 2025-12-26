//! Platform-specific implementations.

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacOSProvider;

// Re-export the appropriate provider for the current platform
#[cfg(target_os = "macos")]
pub type PlatformProvider = MacOSProvider;

#[cfg(not(target_os = "macos"))]
pub type PlatformProvider = crate::provider::NullProvider;
