//! Mode definitions and resolution logic.
//!
//! Pure domain logic - no I/O, no platform dependencies.

use serde::{Deserialize, Serialize};

/// Semantic mode representing the user's current activity context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// User is in a meeting (Zoom, Teams, etc.)
    /// Highest priority when mic is active + meeting app detected.
    Meeting,

    /// User is coding (IDE focused)
    Dev,

    /// User is writing/note-taking (Obsidian, Notion, etc.)
    Writer,

    /// Default mode - basic OS tools
    #[default]
    Global,
}

impl Mode {
    /// Returns a human-readable label for the mode.
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Meeting => "Meeting",
            Mode::Dev => "Dev",
            Mode::Writer => "Writer",
            Mode::Global => "Global",
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Bundle IDs that trigger Dev mode.
pub const DEV_MODE_APPS: &[&str] = &[
    "com.microsoft.VSCode",
    "com.microsoft.VSCodeInsiders",
    "dev.zed.Zed",
    "com.jetbrains.intellij",
    "com.jetbrains.intellij.ce",
    "com.jetbrains.WebStorm",
    "com.jetbrains.pycharm",
    "com.jetbrains.CLion",
    "com.jetbrains.goland",
    "com.jetbrains.rustrover",
    "com.sublimetext.4",
    "com.apple.dt.Xcode",
    "org.vim.MacVim",
    "com.googlecode.iterm2",
    "com.apple.Terminal",
    "io.alacritty",
    "com.github.wez.wezterm",
];

/// Bundle IDs that trigger Writer mode.
pub const WRITER_MODE_APPS: &[&str] = &[
    "md.obsidian",
    "notion.id",
    "com.apple.Notes",
    "com.ulysses.mac",
    "com.multimarkdown.composer2",
    "com.microsoft.Word",
    "com.google.Chrome.app.Docs", // Google Docs PWA
    "com.apple.iWork.Pages",
    "net.ia.iawriter",
    "co.noteplan.NotePlan3",
];

/// Resolve the effective mode from system state.
///
/// Priority:
/// 1. Meeting (if mic active + meeting app detected)
/// 2. Dev (if IDE focused)
/// 3. Writer (if notes app focused)
/// 4. Global (fallback)
///
/// This is pure business logic - no I/O.
pub fn resolve_mode(
    active_app_bundle_id: Option<&str>,
    is_mic_active: bool,
    meeting_app_detected: bool,
) -> Mode {
    // Meeting trumps everything if mic is active and meeting app is running
    if is_mic_active && meeting_app_detected {
        return Mode::Meeting;
    }

    // Check active app for Dev/Writer modes
    if let Some(bundle_id) = active_app_bundle_id {
        if DEV_MODE_APPS.iter().any(|&app| app == bundle_id) {
            return Mode::Dev;
        }

        if WRITER_MODE_APPS.iter().any(|&app| app == bundle_id) {
            return Mode::Writer;
        }
    }

    Mode::Global
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meeting_mode_priority() {
        // Meeting mode takes priority when mic is active and meeting detected
        let mode = resolve_mode(Some("com.microsoft.VSCode"), true, true);
        assert_eq!(mode, Mode::Meeting);
    }

    #[test]
    fn test_dev_mode() {
        let mode = resolve_mode(Some("com.microsoft.VSCode"), false, false);
        assert_eq!(mode, Mode::Dev);
    }

    #[test]
    fn test_writer_mode() {
        let mode = resolve_mode(Some("md.obsidian"), false, false);
        assert_eq!(mode, Mode::Writer);
    }

    #[test]
    fn test_global_fallback() {
        let mode = resolve_mode(Some("com.apple.Safari"), false, false);
        assert_eq!(mode, Mode::Global);
    }

    #[test]
    fn test_dev_mode_without_meeting() {
        // Dev mode when mic is active but no meeting app
        let mode = resolve_mode(Some("com.microsoft.VSCode"), true, false);
        assert_eq!(mode, Mode::Dev);
    }
}
