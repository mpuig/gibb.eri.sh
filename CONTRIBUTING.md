# Contributing to Gibberish

Welcome! This guide helps you understand how to develop and extend Gibberish.

## 🛠 Project Setup

1.  **Prerequisites**:
    *   Rust (latest stable)
    *   Node.js 18+
    *   macOS (currently the primary target due to Core Audio dependencies)

2.  **Installation**:
    ```bash
    git clone https://github.com/mpuig/gibb.eri.sh.git
    cd gibb.eri.sh
    npm install
    ```

3.  **Run Development Mode**:
    ```bash
    npm run tauri dev
    ```

## 🧩 Plugin Architecture

The app is composed of modular plugins in `plugins/`.

### Anatomy of a Plugin
A standard plugin (e.g., `plugins/my-feature`) has:
*   `Cargo.toml`: Defines dependencies (often `tauri`, `tokio`, `serde`).
*   `src/lib.rs`: The entry point with an `init()` function.
*   `src/commands.rs`: Tauri commands exposed to the frontend.
*   `src/state.rs`: Internal state (wrapped in `Arc`/`Mutex`).

### Creating a New Plugin
1.  Create a new library crate in `plugins/`.
2.  Implement the `init` function:
    ```rust
    pub fn init<R: Runtime>() -> TauriPlugin<R> {
        Builder::new("my-feature")
            .invoke_handler(tauri::generate_handler![commands::my_command])
            .setup(|app, _api| {
                app.manage(MyState::default());
                Ok(())
            })
            .build()
    }
    ```
3.  Register it in `apps/desktop/src-tauri/src/lib.rs`.

## ⚡️ Concurrency & State Management

Gibberish is a highly concurrent, real-time application. Follow these rules to avoid deadlocks and bugs.

### 1. State Sharing
*   **Pattern**: Use `Arc<T>` for state that must be shared between UI commands and background threads.
*   **Why**: Tauri's `State<T>` is a borrow. You cannot move it into a `tokio::spawn` closure.
*   **Example**:
    ```rust
    // In lib.rs setup
    let state = Arc::new(MyState::new());
    app.manage(state);

    // In commands.rs
    fn my_command(state: State<'_, Arc<MyState>>) {
        let state_ref = state.inner().clone(); // Cheap Arc clone
        tokio::spawn(async move {
            state_ref.do_something(); // Safe access
        });
    }
    ```

### 2. The Audio Hot Path
*   **Rule**: **NEVER** block the audio processing loop.
*   **Avoid**: `std::sync::Mutex`, long computations, or awaiting I/O.
*   **Use**:
    *   `tokio::sync::broadcast` / `mpsc` for communication.
    *   `std::sync::atomic` types for shared metrics (e.g., `PipelineStatus`).
    *   `Arc<[f32]>` for zero-copy audio chunk passing.

### 3. Task Lifecycle
*   **Use `CancellationToken`** (from `tokio_util`) instead of `AtomicBool` for shutdown signals. It allows instant cancellation via `tokio::select!`.

## 📡 Events & Commands Contract

### Key Events (Backend -> Frontend)
| Event | Payload | Frequency | Description |
|-------|---------|-----------|-------------|
| `stt:stream_result` | `StreamingResultDto` | High (10Hz+) | Partial text updates (volatile). |
| `stt:stream_commit` | `StreamingCommitPayload` | Low (Sentences) | Finalized text segments. |
| `recorder:audio-level` | `f32` (0.0-1.0) | High (20Hz) | Microphone volume for visualizer. |

### Core Commands (Frontend -> Backend)
| Command | Purpose |
|---------|---------|
| `start_recording` | Initializes audio capture. |
| `stop_recording` | Finalizes file and cleanup. |
| `stt_start_listening` | Starts the autonomous STT loop. |
| `stt_get_pipeline_status`| Returns real-time lag/RTF metrics. |

## 🏗 Directory Structure logic

*   **`crates/`**: Pure Rust libraries. **Domain logic** goes here.
    *   `audio`: DSP, resampling, mixing.
    *   `bus`: The shared communication backbone.
    *   `stt`: Engine traits (Parakeet/Sherpa abstraction).
*   **`plugins/`**: Tauri integration. **Application logic** goes here.
    *   Binds `crates/` logic to Tauri commands/events.
*   **`apps/desktop/`**: The host application.
    *   Wires everything together.

## 🧪 Testing

*   **Unit Tests**: `cargo test` in individual crates.
*   **Integration**: Harder due to Tauri context. Prefer moving logic to `crates/` where it can be tested without a GUI.
