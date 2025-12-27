//! Adapters that bridge external crates to internal abstractions.
//!
//! This module contains implementations of traits defined in other crates,
//! following the Adapter pattern from clean architecture.

mod event_bus;
mod focus;

pub use event_bus::TauriEventBus;
pub use focus::PlatformFocusChecker;
