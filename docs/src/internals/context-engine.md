# The Context Engine

gibb.eri.sh knows what you're doing. Here's how.

## The Goal

To enable **Context-Aware AI**, we need to know the user's state without burning the CPU.
- Are they coding? (Enable Git tools)
- Are they in a meeting? (Enable Transcription tools)
- Are they looking at a specific URL? (Provide deep context)

## The Implementation

We use a high-frequency polling loop in `crates/context` that build a realtime snapshot of the OS state.

### 1. Active App Detection (Native Cocoa)

We use the macOS **Cocoa API** (`NSWorkspace`) via the `objc` crate to detect the focused application.

**Why Native instead of AppleScript?**
- **Performance:** Sub-millisecond execution. No subprocess fork/exec overhead.
- **Efficiency:** Negligible CPU usage even at 1s polling intervals.
- **Reliability:** Directly queries the Window Server for the `frontmostApplication`.

### 2. Browser Deep Context (URL Detection)

When a supported browser (Chrome, Safari, Arc, Brave) is focused, we go deeper.

- **Mechanism:** We use a targeted AppleScript call to fetch the `URL` of the active tab.
- **Optimization:** We only trigger the AppleScript if the active application is a browser, preventing unnecessary overhead.
- **Value:** This allows "Summarize this page" to work by feeding the URL directly to our extraction tools.

### 3. Meeting Detection (The Activity)

We monitor CoreAudio to see if known meeting apps (Zoom, Teams) are accessing the microphone.
- **Crate:** `crates/detect` (wrapped by `context`)
- **Logic:** `is_mic_active && is_meeting_app(bundle_id)`

## Privacy

- **Local Only:** No context data leaves the device.
- **Targeted:** We only care about specific `bundle_id`s. We don't read window titles or keystrokes.
- **Incognito Awareness:** We attempt to detect and ignore private browsing windows to avoid leaking sensitive URLs into the LLM context.
