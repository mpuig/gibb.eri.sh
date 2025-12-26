//! Context awareness for gibb.eri.sh.
//!
//! This crate provides system context detection to enable mode-aware tool filtering.
//! It tracks:
//! - Active application (which app has focus)
//! - Microphone activity (is any app using the mic)
//! - Meeting detection (is a meeting app using the mic)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     Domain Layer                             │
//! │  mode.rs     - Mode enum and resolution logic (pure)        │
//! │  state.rs    - SystemContext, ContextState structs          │
//! │  provider.rs - Traits for system state detection            │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                  Infrastructure Layer                        │
//! │  platform/macos.rs - macOS-specific implementation          │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                   Application Layer                          │
//! │  poller.rs - Background polling and event emission          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use gibberish_context::{ContextPoller, platform::PlatformProvider};
//! use std::sync::Arc;
//!
//! let provider = Arc::new(PlatformProvider::new());
//! let mut poller = ContextPoller::new();
//!
//! poller.start(provider, Arc::new(|event| {
//!     println!("Mode: {}, App: {:?}", event.mode, event.active_app);
//! }));
//! ```

mod mode;
mod poller;
mod provider;
mod state;

pub mod platform;

// Re-export main types
pub use mode::{resolve_mode, Mode, DEV_MODE_APPS, WRITER_MODE_APPS};
pub use poller::{ContextCallback, ContextPoller, DEFAULT_POLL_INTERVAL};
pub use provider::{ActiveAppProvider, MicActivityProvider, NullProvider, SystemStateProvider};
pub use state::{AppInfo, ContextChangedEvent, ContextState, SystemContext};

// Re-export detect crate types that we depend on
pub use gibberish_detect::{is_meeting_app, MEETING_APPS};
