use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::{
    plugin::{Builder, TauriPlugin},
    Emitter, Manager, Runtime, State,
};

use gibberish_audio::{AudioRecorder, AudioSource, AudioStream};
use gibberish_bus::{AudioBusSender, CHUNK_SAMPLES, SAMPLE_RATE};

pub struct RecorderState {
    is_recording: Arc<AtomicBool>,
    /// Listen-only mode: captures audio but doesn't save when stopped
    is_listen_only: Arc<AtomicBool>,
    recorder: AudioRecorder,
    stop_signal: Arc<AtomicBool>,
    /// Handle to the recording thread, so we can join it on stop
    thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl Default for RecorderState {
    fn default() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            is_listen_only: Arc::new(AtomicBool::new(false)),
            recorder: AudioRecorder::new(),
            stop_signal: Arc::new(AtomicBool::new(false)),
            thread_handle: Mutex::new(None),
        }
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("gibberish-recorder")
        .invoke_handler(tauri::generate_handler![
            start_recording,
            stop_recording,
            start_listening,
            stop_listening,
            get_recording_state,
            list_audio_devices,
            has_virtual_device,
        ])
        .setup(|app, _api| {
            let state = RecorderState::default();
            app.manage(state);
            Ok(())
        })
        .build()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioSourceType {
    Microphone,
    System,
    Combined,
    /// System audio via Core Audio Tap (macOS only, no virtual device needed, no permission required)
    SystemNative,
    /// Combined mic + native system audio (macOS only)
    CombinedNative,
}

#[tauri::command]
async fn start_recording<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, RecorderState>,
    bus_sender: State<'_, AudioBusSender>,
    device_id: Option<String>,
    source_type: Option<AudioSourceType>,
    system_device_id: Option<String>,
) -> Result<(), String> {
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Already recording".to_string());
    }

    state.recorder.clear();
    state.stop_signal.store(false, Ordering::SeqCst);
    state.is_recording.store(true, Ordering::SeqCst);

    let recorder = state.recorder.clone();
    let stop_signal = Arc::clone(&state.stop_signal);
    let is_recording = Arc::clone(&state.is_recording);
    let is_listen_only = Arc::clone(&state.is_listen_only);
    let app_clone = app.clone();
    let bus_sender = bus_sender.inner().clone();

    // Rolling buffer duration for listen-only mode (30 seconds)
    const LISTEN_BUFFER_SECS: f32 = 30.0;

    let handle = thread::spawn(move || {
        tracing::info!("Recording thread started (listen_only={})", is_listen_only.load(Ordering::SeqCst));
        let source = match source_type.unwrap_or(AudioSourceType::Microphone) {
            AudioSourceType::Microphone => AudioSource::Microphone { device_id },
            AudioSourceType::System => AudioSource::SystemAudio {
                device_id: system_device_id,
            },
            AudioSourceType::Combined => AudioSource::Combined {
                mic_device_id: device_id,
                system_device_id,
            },
            #[cfg(target_os = "macos")]
            AudioSourceType::SystemNative => AudioSource::SystemAudioNative,
            #[cfg(target_os = "macos")]
            AudioSourceType::CombinedNative => AudioSource::CombinedNative {
                mic_device_id: device_id,
            },
            #[cfg(not(target_os = "macos"))]
            AudioSourceType::SystemNative | AudioSourceType::CombinedNative => {
                let _ = app_clone.emit(
                    "recorder:error",
                    "Native system audio is only available on macOS",
                );
                return;
            }
        };
        tracing::info!("Creating AudioStream");
        let mut stream = match AudioStream::new(source) {
            Ok(s) => {
                tracing::info!("AudioStream created successfully");
                s
            }
            Err(e) => {
                tracing::error!("AudioStream creation failed: {}", e);
                let _ = app_clone.emit("recorder:error", e.to_string());
                is_recording.store(false, Ordering::SeqCst);
                return;
            }
        };

        let rx = match stream.take_receiver() {
            Some(rx) => rx,
            None => {
                tracing::error!("Audio stream receiver already taken");
                let _ = app_clone.emit("recorder:error", "Audio stream receiver already taken");
                is_recording.store(false, Ordering::SeqCst);
                return;
            }
        };
        // Ring buffer for O(1) drain from front (avoids O(n) Vec shift)
        let mut bus_buffer: VecDeque<f32> = VecDeque::with_capacity(CHUNK_SAMPLES * 2);

        let initial_stop_signal = stop_signal.load(Ordering::SeqCst);
        tracing::info!(initial_stop_signal, "Recording loop starting");
        if initial_stop_signal {
            tracing::error!("stop_signal is already true at loop start!");
        }
        let mut recv_count = 0u64;
        let mut bus_chunks_sent = 0u64;

        // Monotonic timestamp tracking: base time + sample count for stable timing
        let recording_start = Instant::now();
        let wall_clock_start_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let mut total_samples_sent: u64 = 0;

        // Blocking recv with timeout for efficient CPU usage (no polling)
        loop {
            // Use recv_timeout for efficient blocking with periodic stop checks
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(samples) => {
                    recv_count += 1;
                    if recv_count <= 3 {
                        tracing::info!(
                            recv_count,
                            samples_len = samples.len(),
                            "Received audio samples"
                        );
                    }
                    recorder.push_samples(&samples);

                    // In listen-only mode, keep only the last 30 seconds (rolling buffer)
                    if is_listen_only.load(Ordering::SeqCst) {
                        recorder.trim_to_duration(LISTEN_BUFFER_SECS);
                    }

                    // Emit audio level for UI visualization
                    let level = calculate_level(&samples);
                    let _ = app_clone.emit("recorder:audio-level", level);

                    // Buffer samples for the audio bus (50ms chunks for responsive streaming)
                    bus_buffer.extend(samples.iter().copied());
                    while bus_buffer.len() >= CHUNK_SAMPLES {
                        // Monotonic timestamp: wall clock start + sample-based offset
                        // This avoids wall clock jumps and is more stable for audio
                        let sample_offset_ms = (total_samples_sent * 1000) / SAMPLE_RATE as u64;
                        let ts_ms = wall_clock_start_ms + sample_offset_ms as i64;

                        // Drain from VecDeque (O(1) per element) into owned Vec
                        let chunk_vec: Vec<f32> = bus_buffer.drain(..CHUNK_SAMPLES).collect();

                        // Move Vec directly into Arc<[f32]> (no copy, just realloc)
                        if bus_sender.send(ts_ms, SAMPLE_RATE, chunk_vec) {
                            bus_chunks_sent += 1;
                            total_samples_sent += CHUNK_SAMPLES as u64;
                            if bus_chunks_sent % 20 == 0 {
                                tracing::debug!(
                                    bus_chunks_sent,
                                    elapsed_ms = recording_start.elapsed().as_millis(),
                                    "Audio bus chunks sent"
                                );
                            }
                        }
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Timeout - check stop signal and continue
                    if stop_signal.load(Ordering::SeqCst) {
                        break;
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    tracing::warn!("Recording loop receiver disconnected, exiting");
                    break;
                }
            }

            // Also check stop signal after successful recv
            if stop_signal.load(Ordering::SeqCst) {
                break;
            }
        }
        tracing::info!(
            recv_count,
            bus_chunks_sent,
            total_samples = recorder.sample_count(),
            "Recording loop ended"
        );

        // Send any remaining samples to the bus.
        if !bus_buffer.is_empty() {
            // Use monotonic timestamp for remaining samples too
            let sample_offset_ms = (total_samples_sent * 1000) / SAMPLE_RATE as u64;
            let ts_ms = wall_clock_start_ms + sample_offset_ms as i64;
            let remaining: Vec<f32> = bus_buffer.drain(..).collect();
            bus_sender.send(ts_ms, SAMPLE_RATE, remaining);
        }
        tracing::info!("Recording thread exiting");
        is_recording.store(false, Ordering::SeqCst);
    });

    // Store the thread handle so we can join it on stop
    match state.thread_handle.lock() {
        Ok(mut guard) => *guard = Some(handle),
        Err(poisoned) => {
            tracing::warn!("Thread handle mutex poisoned, recovering");
            *poisoned.into_inner() = Some(handle);
        }
    }

    let _ = app.emit("recorder:started", ());

    Ok(())
}

#[tauri::command]
async fn stop_recording<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, RecorderState>,
) -> Result<String, String> {
    if !state.is_recording.load(Ordering::SeqCst) {
        return Err("Not recording".to_string());
    }

    state.stop_signal.store(true, Ordering::SeqCst);
    state.is_recording.store(false, Ordering::SeqCst);

    // Join the recording thread to ensure audio resources are fully released
    let maybe_handle = match state.thread_handle.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => {
            tracing::warn!("Thread handle mutex poisoned, recovering");
            poisoned.into_inner().take()
        }
    };
    if let Some(handle) = maybe_handle {
        tracing::info!("Waiting for recording thread to finish...");
        let start = Instant::now();
        let timeout = Duration::from_secs(3);
        while !handle.is_finished() && start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(20));
        }
        if handle.is_finished() {
            if let Err(e) = handle.join() {
                tracing::error!("Recording thread panicked: {:?}", e);
            }
            tracing::info!("Recording thread finished");
        } else {
            tracing::error!(
                "Timed out waiting for recording thread; detaching to avoid UI deadlock"
            );
        }
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let recordings_dir = format!("{home}/Library/Application Support/gibberish/recordings");
    std::fs::create_dir_all(&recordings_dir).map_err(|e| e.to_string())?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let path = format!("{recordings_dir}/recording_{timestamp}.wav");

    state.recorder.save_wav(&path).map_err(|e| e.to_string())?;

    let duration_secs = state.recorder.duration_secs();
    state.recorder.clear();

    let _ = app.emit(
        "recorder:stopped",
        serde_json::json!({
            "path": path,
            "duration_secs": duration_secs,
        }),
    );

    Ok(path)
}

/// Start listening (audio capture without saving).
/// Similar to start_recording but doesn't save a file when stopped.
#[tauri::command]
async fn start_listening<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, RecorderState>,
    bus_sender: State<'_, AudioBusSender>,
    source_type: Option<AudioSourceType>,
) -> Result<(), String> {
    if state.is_recording.load(Ordering::SeqCst) {
        return Err("Already recording/listening".to_string());
    }

    // Set listen-only mode
    state.is_listen_only.store(true, Ordering::SeqCst);

    // Reuse start_recording logic
    start_recording(app, state, bus_sender, None, source_type, None).await
}

/// Stop listening (discard audio, don't save).
#[tauri::command]
async fn stop_listening<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, RecorderState>,
) -> Result<(), String> {
    if !state.is_recording.load(Ordering::SeqCst) {
        return Err("Not listening".to_string());
    }

    state.stop_signal.store(true, Ordering::SeqCst);
    state.is_recording.store(false, Ordering::SeqCst);
    state.is_listen_only.store(false, Ordering::SeqCst);

    // Join the recording thread
    let maybe_handle = match state.thread_handle.lock() {
        Ok(mut guard) => guard.take(),
        Err(poisoned) => {
            tracing::warn!("Thread handle mutex poisoned, recovering");
            poisoned.into_inner().take()
        }
    };
    if let Some(handle) = maybe_handle {
        tracing::info!("Waiting for listening thread to finish...");
        let start = Instant::now();
        let timeout = Duration::from_secs(3);
        while !handle.is_finished() && start.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(20));
        }
        if handle.is_finished() {
            if let Err(e) = handle.join() {
                tracing::error!("Listening thread panicked: {:?}", e);
            }
            tracing::info!("Listening thread finished");
        } else {
            tracing::error!("Timed out waiting for listening thread");
        }
    }

    // Clear the buffer (discard audio)
    state.recorder.clear();

    let _ = app.emit("recorder:listening_stopped", ());

    Ok(())
}

#[tauri::command]
fn get_recording_state(state: State<'_, RecorderState>) -> bool {
    state.is_recording.load(Ordering::SeqCst)
}

#[tauri::command]
fn list_audio_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    let devices = gibberish_audio::list_devices().map_err(|e| e.to_string())?;
    Ok(devices
        .into_iter()
        .map(|d| {
            let is_virtual = d.is_virtual();
            AudioDeviceInfo {
                id: d.id,
                name: d.name,
                is_default: d.is_default,
                is_virtual,
            }
        })
        .collect())
}

#[tauri::command]
fn has_virtual_device() -> Result<bool, String> {
    let device = gibberish_audio::find_virtual_device().map_err(|e| e.to_string())?;
    Ok(device.is_some())
}

fn calculate_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s.abs()).sum();
    (sum / samples.len() as f32).min(1.0)
}

#[derive(serde::Serialize)]
struct AudioDeviceInfo {
    id: String,
    name: String,
    is_default: bool,
    is_virtual: bool,
}
