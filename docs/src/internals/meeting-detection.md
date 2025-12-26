# Meeting Detection

The app knows when you're in a Zoom call. Here's how.

## The Feature

When a meeting app (Zoom, Teams, Slack) starts using the microphone, gibb.eri.sh can:
- Auto-start recording
- Switch to "meeting mode" (longer turn detection)
- Tag the transcript with the app name

## Implementation

We use macOS CoreAudio APIs to inspect audio sessions.

### Detection Logic

```rust
// crates/detect/src/lib.rs

pub const MEETING_APPS: &[&str] = &[
    "us.zoom.xos",
    "Cisco-Systems.Spark",  // Webex
    "com.microsoft.teams",
    "com.microsoft.teams2",
    "com.google.Chrome",    // For web-based meetings
    "com.brave.Browser",
    "com.apple.Safari",
    "org.mozilla.firefox",
    "com.discord.Discord",
    "com.slack.Slack",
];

pub fn detect_active_meeting() -> Option<String> {
    let sessions = get_audio_sessions()?;

    for session in sessions {
        if session.is_capturing_audio() {
            if let Some(bundle_id) = session.bundle_id() {
                if MEETING_APPS.contains(&bundle_id.as_str()) {
                    return Some(bundle_id);
                }
            }
        }
    }

    None
}
```

### CoreAudio Details

We query `AudioObjectGetPropertyData` with `kAudioHardwarePropertyProcessIsAudible`:

```rust
fn get_audio_sessions() -> Option<Vec<AudioSession>> {
    let mut sessions = Vec::new();

    // Get list of processes using audio
    let property = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyProcessIsRunning,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    };

    // ... query and iterate

    Some(sessions)
}
```

This is macOS-specific. Linux would use PulseAudio/PipeWire, Windows would use WASAPI.

## Browser Detection

Web-based meetings (Google Meet, Zoom Web) run in browsers. We detect the browser, not the meeting service.

This means:
- We can't distinguish "Meet call" from "YouTube video"
- We could inspect browser tabs, but that's invasive

For now, we detect that a browser is using audio. The user can decide if they want auto-recording.

## Polling vs Events

We poll every 5 seconds:

```rust
loop {
    if let Some(app) = detect_active_meeting() {
        if !currently_recording {
            app.emit("meeting:detected", &app)?;
        }
    }
    tokio::time::sleep(Duration::from_secs(5)).await;
}
```

CoreAudio has event-based APIs, but they're complex and unreliable for this use case. Polling at 5-second intervals has negligible overhead.

## Privacy

We query the CoreAudio session API to check which apps have active audio capture. We do **not**:
- Read window titles or screen content
- Access meeting participants or chat
- Record meeting audio without explicit user action
- Send any data externally

The detection uses only OS-level audio session metadata (bundle IDs of apps with active microphone sessions).

## Platform Support

| Platform | Supported | Notes |
|----------|-----------|-------|
| macOS | Yes | CoreAudio |
| Linux | Not yet | Needs PulseAudio/PipeWire |
| Windows | Not yet | Needs WASAPI |

PRs welcome for Linux/Windows support.

## Future Work

Potential improvements:
- Use Accessibility APIs to get the actual meeting URL/name
- Detect screen sharing (for "presenter mode")
- Integrate with calendar to know meeting context

These are invasive though—we'd need explicit user permission and clear benefit.
