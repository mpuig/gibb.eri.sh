//! Adapters that bridge external crates to internal abstractions.
//!
//! This module contains implementations of traits defined in other crates,
//! following the Adapter pattern from clean architecture.

mod focus;

pub use focus::PlatformFocusChecker;
