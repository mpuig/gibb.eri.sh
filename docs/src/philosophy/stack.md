# Rust + Tauri

## The Stack

| Layer | Technology | Why |
|-------|------------|-----|
| Core Logic | Rust | Performance, safety, no GC |
| Desktop Shell | Tauri v2 | Lightweight, secure |
| UI | React | Developer familiarity |
| Inference | ONNX Runtime | Universal model format |

## Why Tauri?

Tauri uses the system's native webview instead of bundling Chromium, which reduces binary size and RAM usage. For a voice assistant that may run continuously, lower idle resource usage helps.

Note: The app requires ~500MB of model downloads on first run, so the binary size savings are offset by the ML models. The main benefit is runtime efficiency.

## The Tauri Architecture

```
┌─────────────────────────────────────────────────┐
│                  Tauri App                       │
│  ┌───────────────────┐  ┌────────────────────┐  │
│  │   Rust Backend    │  │   WebView (UI)     │  │
│  │                   │  │                    │  │
│  │  ┌─────────────┐  │  │  React + TypeScript│  │
│  │  │ Audio Bus   │  │  │                    │  │
│  │  │ STT Engine  │◀─┼──┼─ invoke()          │  │
│  │  │ VAD         │──┼──┼─▶ events           │  │
│  │  └─────────────┘  │  │                    │  │
│  └───────────────────┘  └────────────────────┘  │
└─────────────────────────────────────────────────┘
```

- **Rust Backend**: All heavy lifting (audio, inference, VAD)
- **WebView**: Native OS webview (not bundled Chromium)
- **Communication**: Tauri's IPC (commands + events)

## The UI is a Passenger

The React frontend is intentionally "dumb":
- It **displays** text from the backend
- It **sends** commands (start/stop recording)
- It **never** touches audio data directly

This separation means:
1. UI bugs can't crash the audio pipeline
2. The UI can be replaced without touching core logic
3. Heavy computation never blocks rendering

## Security Model

Tauri uses a **capability-based** permission system:

```json
// plugins/recorder/permissions/default.json
{
  "permissions": ["recorder:start", "recorder:stop"],
  "deny": ["fs:write", "shell:execute"]
}
```

Each plugin declares exactly what it needs. Everything else is denied by default.

