//! System context state structures.

use crate::mode::Mode;
use serde::{Deserialize, Serialize};

/// Information about the currently focused application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppInfo {
    /// Bundle ID (e.g., "com.microsoft.VSCode")
    pub bundle_id: String,

    /// Display name (e.g., "Visual Studio Code")
    pub name: Option<String>,
}

/// Snapshot of the current system context.
///
/// This is the "world view" that drives mode resolution and tool filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemContext {
    /// Currently focused application
    pub active_app: Option<AppInfo>,

    /// Whether any app is using the microphone
    pub is_mic_active: bool,

    /// Bundle ID of detected meeting app (if any)
    pub meeting_app: Option<String>,

    /// Timestamp when this context was captured
    pub timestamp_ms: i64,

    /// Preview of clipboard contents (truncated for prompt injection)
    #[serde(default)]
    pub clipboard_preview: Option<String>,

    /// Preview of currently selected text (truncated for prompt injection)
    #[serde(default)]
    pub selection_preview: Option<String>,
}

impl Default for SystemContext {
    fn default() -> Self {
        Self {
            active_app: None,
            is_mic_active: false,
            meeting_app: None,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            clipboard_preview: None,
            selection_preview: None,
        }
    }
}

impl SystemContext {
    /// Get the bundle ID of the active app, if any.
    pub fn active_bundle_id(&self) -> Option<&str> {
        self.active_app.as_ref().map(|a| a.bundle_id.as_str())
    }

    /// Check if a meeting app is detected.
    pub fn has_meeting_app(&self) -> bool {
        self.meeting_app.is_some()
    }
}

/// Resolved context with mode and override support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextState {
    /// Raw system context
    pub system: SystemContext,

    /// Automatically detected mode
    pub detected_mode: Mode,

    /// User-pinned mode (overrides detected mode)
    pub pinned_mode: Option<Mode>,
}

impl Default for ContextState {
    fn default() -> Self {
        Self {
            system: SystemContext::default(),
            detected_mode: Mode::Global,
            pinned_mode: None,
        }
    }
}

impl ContextState {
    /// Create a new context state from system context.
    pub fn from_system(system: SystemContext) -> Self {
        let detected_mode = crate::mode::resolve_mode(
            system.active_bundle_id(),
            system.is_mic_active,
            system.has_meeting_app(),
        );

        Self {
            system,
            detected_mode,
            pinned_mode: None,
        }
    }

    /// Get the effective mode (pinned takes priority).
    pub fn effective_mode(&self) -> Mode {
        self.pinned_mode.unwrap_or(self.detected_mode)
    }

    /// Pin a specific mode (disables auto-switching).
    pub fn pin_mode(&mut self, mode: Mode) {
        self.pinned_mode = Some(mode);
    }

    /// Unpin mode (re-enable auto-switching).
    pub fn unpin_mode(&mut self) {
        self.pinned_mode = None;
    }

    /// Update with new system context.
    pub fn update(&mut self, system: SystemContext) {
        self.detected_mode = crate::mode::resolve_mode(
            system.active_bundle_id(),
            system.is_mic_active,
            system.has_meeting_app(),
        );
        self.system = system;
    }

    /// Generate a prompt snippet describing the current context.
    ///
    /// Used to inject context into FunctionGemma prompts, enabling
    /// implicit references like "search this error" or "summarize this".
    pub fn to_prompt_snippet(&self) -> String {
        let mut lines = Vec::new();

        // Mode
        lines.push(format!("Mode: {}", self.effective_mode()));

        // Active app
        if let Some(ref app) = self.system.active_app {
            let app_name = app.name.as_deref().unwrap_or(&app.bundle_id);
            lines.push(format!("Active App: {}", app_name));
        }

        // Meeting status
        if self.system.has_meeting_app() && self.system.is_mic_active {
            lines.push("In Meeting: yes".to_string());
        }

        // Clipboard preview (truncate to ~200 chars for prompt efficiency)
        if let Some(ref clip) = self.system.clipboard_preview {
            let preview = if clip.len() > 200 {
                format!("{}...", &clip[..200])
            } else {
                clip.clone()
            };
            lines.push(format!("Clipboard: \"{}\"", preview.replace('\n', " ")));
        }

        // Selection preview
        if let Some(ref sel) = self.system.selection_preview {
            let preview = if sel.len() > 200 {
                format!("{}...", &sel[..200])
            } else {
                sel.clone()
            };
            lines.push(format!("Selection: \"{}\"", preview.replace('\n', " ")));
        }

        // Current date/time (useful for scheduling tools)
        let now = chrono::Utc::now();
        lines.push(format!("Date: {}", now.format("%Y-%m-%d")));

        lines.join("\n")
    }
}

/// Event emitted when context changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChangedEvent {
    /// Effective mode (after applying pin)
    pub mode: Mode,

    /// Active app bundle ID
    pub active_app: Option<String>,

    /// Active app display name
    pub active_app_name: Option<String>,

    /// Whether in a meeting
    pub is_meeting: bool,

    /// Timestamp
    pub timestamp_ms: i64,
}

impl From<&ContextState> for ContextChangedEvent {
    fn from(state: &ContextState) -> Self {
        Self {
            mode: state.effective_mode(),
            active_app: state
                .system
                .active_app
                .as_ref()
                .map(|a| a.bundle_id.clone()),
            active_app_name: state
                .system
                .active_app
                .as_ref()
                .and_then(|a| a.name.clone()),
            is_meeting: state.system.has_meeting_app() && state.system.is_mic_active,
            timestamp_ms: state.system.timestamp_ms,
        }
    }
}
