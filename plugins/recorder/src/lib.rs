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
use tokio::sync::broadcast::error::TryRecvError;

use gibberish_audio::{AudioRecorder, AudioSource, AudioStream};

pub struct RecorderState {
    is_recording: Arc<AtomicBool>,
    recorder: AudioRecorder,
    stop_signal: Arc<AtomicBool>,
    /// Handle to the recording thread, so we can join it on stop
    thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl Default for RecorderState {
    fn default() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
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
    let app_clone = app.clone();

    let handle = thread::spawn(move || {
        tracing::info!("Recording thread started");
        let source = match source_type.unwrap_or(AudioSourceType::Microphone) {
            AudioSourceType::Microphone => AudioSource::Microphone { device_id },
            AudioSourceType::System => AudioSource::SystemAudio { device_id: system_device_id },
            AudioSourceType::Combined => AudioSource::Combined {
                mic_device_id: device_id,
                system_device_id,
            },
            #[cfg(target_os = "macos")]
            AudioSourceType::SystemNative => AudioSource::SystemAudioNative,
            #[cfg(target_os = "macos")]
            AudioSourceType::CombinedNative => AudioSource::CombinedNative { mic_device_id: device_id },
            #[cfg(not(target_os = "macos"))]
            AudioSourceType::SystemNative | AudioSourceType::CombinedNative => {
                let _ = app_clone.emit("recorder:error", "Native system audio is only available on macOS");
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

        let mut rx = stream.take_receiver().expect("receiver already taken");
        let mut chunk_buffer: Vec<f32> = Vec::with_capacity(8000); // 500ms at 16kHz

        let initial_stop_signal = stop_signal.load(Ordering::SeqCst);
        tracing::info!(initial_stop_signal, "Recording loop starting");
        if initial_stop_signal {
            tracing::error!("stop_signal is already true at loop start!");
        }
        let mut recv_count = 0u64;
        while !stop_signal.load(Ordering::SeqCst) {
            match rx.try_recv() {
                Ok(samples) => {
                    recv_count += 1;
                    if recv_count <= 3 {
                        tracing::info!(recv_count, samples_len = samples.len(), "Received audio samples");
                    }
                    recorder.push_samples(&samples);

                    // Emit audio level
                    let level = calculate_level(&samples);
                    let _ = app_clone.emit("recorder:audio-level", level);

                    // Buffer samples and emit chunks for real-time transcription
                    chunk_buffer.extend_from_slice(&samples);
                    if chunk_buffer.len() >= 8000 {
                        // Emit 500ms chunk for streaming transcription
                        static EMIT_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                        let count = EMIT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count % 10 == 0 {
                            tracing::info!(count = count, buffer_len = chunk_buffer.len(), "emitting_audio_chunk");
                        }
                        let _ = app_clone.emit("recorder:audio-chunk", chunk_buffer.clone());
                        chunk_buffer.clear();
                    }
                }
                Err(TryRecvError::Empty) => {
                    // Avoid blocking so stop_signal can be observed promptly.
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(TryRecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "Recorder receiver lagged; dropping samples");
                }
                Err(TryRecvError::Closed) => {
                    tracing::warn!("Recording loop receiver closed; exiting");
                    break;
                }
            }
        }
        tracing::info!(recv_count, total_samples = recorder.sample_count(), "Recording loop ended");

        // Emit any remaining samples
        if !chunk_buffer.is_empty() {
            let _ = app_clone.emit("recorder:audio-chunk", chunk_buffer);
        }
        tracing::info!("Recording thread exiting");
        is_recording.store(false, Ordering::SeqCst);
    });

    // Store the thread handle so we can join it on stop
    *state.thread_handle.lock().unwrap() = Some(handle);

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
    if let Some(handle) = state.thread_handle.lock().unwrap().take() {
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
    let recordings_dir = format!("{}/Library/Application Support/gibberish/recordings", home);
    std::fs::create_dir_all(&recordings_dir).map_err(|e| e.to_string())?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let path = format!("{}/recording_{}.wav", recordings_dir, timestamp);

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
