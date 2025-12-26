# The Context Engine

gibb.eri.sh knows what you're doing. Here's how.

## The Goal

To enable **Context-Aware AI**, we need to know the user's state.
- Are they coding? (Enable Git tools)
- Are they in a meeting? (Enable Transcription tools)
- Are they just browsing? (Enable App Launcher)

## The Implementation

We use a polling loop in `crates/context` that queries the OS every 1-2 seconds.

### 1. Active App Detection (The Focus)

We need to know which application window is currently focused.

#### macOS Strategy
We use **AppleScript** (via `osascript`) to query System Events.

```applescript
tell application "System Events"
    set frontApp to first application process whose frontmost is true
    return bundle identifier of frontApp
end tell
```

**Why AppleScript?**
- It's built-in (no extra binaries).
- It's permission-friendly (doesn't require Screen Recording permission, just Accessibility).
- It's fast enough (~50ms) for a 1s polling interval.

**Rust Wrapper:**
```rust
// crates/context/src/platform/macos.rs
fn get_frontmost_app() -> Option<AppInfo> {
    let output = Command::new("osascript").args(["-e", SCRIPT]).output()?;
    // ... parse "com.microsoft.VSCode|Code"
}
```

### 2. Meeting Detection (The Activity)

We need to know if the user is in a call.

**Mechanism:** We monitor CoreAudio to see if known meeting apps (Zoom, Teams) are accessing the microphone.
- **Crate:** `crates/detect` (wrapped by `context`)
- **Logic:** `is_mic_active && is_meeting_app(bundle_id)`

### 3. State Aggregation

The `ContextState` struct holds the world view:

```rust
pub struct ContextState {
    pub active_app: Option<AppInfo>, // "VS Code"
    pub mic_active: bool,            // true
    pub meeting_app: Option<String>, // "Zoom"
}

impl ContextState {
    pub fn effective_mode(&self) -> Mode {
        if self.meeting_app.is_some() && self.mic_active {
            return Mode::Meeting;
        }
        if let Some(app) = &self.active_app {
            if is_dev_tool(&app.bundle_id) {
                return Mode::Dev;
            }
        }
        Mode::Global
    }
}
```

## Privacy

This engine is powerful, so we keep it tight.
- **Local Only:** No context data leaves the device.
- **Ephemeral:** We don't log your window history. We only store the *current* state.
- **Targeted:** We only care about specific `bundle_id`s (IDEs, Meeting Apps). We don't read window titles (which might contain sensitive document names).

## Platform Support

| Platform | Active App | Mic Activity |
|----------|------------|--------------|
| macOS | AppleScript | CoreAudio |
| Linux | X11/Wayland | PulseAudio |
| Windows | Win32 API | WASAPI |

*Currently, only macOS is fully implemented.*