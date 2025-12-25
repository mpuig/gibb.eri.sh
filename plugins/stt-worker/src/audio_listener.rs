//! Audio bus listener for autonomous STT streaming.
//!
//! Listens to audio chunks from the bus and routes them to the appropriate
//! transcription backend based on model capabilities:
//! - Streaming models (Sherpa): Use dedicated streaming worker
//! - Batch models (Parakeet, future models): Use batch_transcriber module
//!
//! This module is model-agnostic - routing is based on capabilities, not model identity.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

use gibberish_bus::{AudioBusReceiver, AudioChunk, PipelineStatus};
use gibberish_sherpa::InferenceResult;
use tauri::{Emitter, Runtime};
use tokio_util::sync::CancellationToken;

use crate::batch_transcriber;
use crate::dto::{StreamingCommitPayload, StreamingResultDto, VadSilencePayload};
use crate::state::SttState;

/// Shared storage for the AudioBusReceiver so it can be returned after listener stops.
pub type ReceiverStorage = Arc<Mutex<Option<AudioBusReceiver>>>;

/// Controls the audio listener task.
///
/// Supports restartability: each start() creates a fresh CancellationToken,
/// so stop() + start() works correctly.
pub struct AudioListenerHandle {
    running: Arc<AtomicBool>,
    /// Protected by mutex to allow creating fresh tokens on restart.
    cancel_token: Mutex<CancellationToken>,
}

impl AudioListenerHandle {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cancel_token: Mutex::new(CancellationToken::new()),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    pub fn stop(&self) {
        // Cancel the current token
        if let Ok(token) = self.cancel_token.lock() {
            token.cancel();
        }
        self.running.store(false, Ordering::Release);
    }

    /// Start and return a fresh cancellation token.
    /// Also returns a clone of the running flag so the task can clear it on exit.
    fn start(&self) -> (CancellationToken, Arc<AtomicBool>) {
        // Create a fresh token for this run (allows restartability)
        let new_token = CancellationToken::new();
        let child = new_token.child_token();

        if let Ok(mut token) = self.cancel_token.lock() {
            *token = new_token;
        }

        self.running.store(true, Ordering::Release);
        (child, Arc::clone(&self.running))
    }
}

impl Default for AudioListenerHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Start the audio listener task.
///
/// This spawns an async task that:
/// - Receives chunks from the AudioBusReceiver
/// - Processes them through the STT worker
/// - Emits transcript events
/// - Returns the receiver to storage when stopped (for restartability)
pub fn start_audio_listener<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: Arc<SttState>,
    receiver: AudioBusReceiver,
    receiver_storage: ReceiverStorage,
    pipeline_status: Arc<PipelineStatus>,
    handle: Arc<AudioListenerHandle>,
) {
    let (cancel_token, running_flag) = handle.start();

    tauri::async_runtime::spawn(async move {
        tracing::info!("Audio listener started");
        let mut receiver = receiver;
        let mut chunks_processed = 0u64;

        loop {
            // Use select! with cancellation token - no polling needed
            let chunk = tokio::select! {
                biased;  // Check cancellation first
                _ = cancel_token.cancelled() => {
                    tracing::info!("Audio listener cancelled");
                    break;
                }
                chunk = receiver.recv() => chunk,
            };

            let Some(chunk) = chunk else {
                tracing::info!("Audio bus closed, stopping listener");
                break;
            };

            // Update pipeline status atomically (no lock needed)
            pipeline_status.update_lag(chunk.ts_ms);
            pipeline_status.increment_chunks_processed();
            pipeline_status.add_audio_processed_ms(chunk.duration_ms());

            // Process the chunk through STT
            if let Err(e) = process_audio_chunk(&app, &state, &chunk).await {
                tracing::warn!(error = %e, "Failed to process audio chunk");
            }

            chunks_processed += 1;
            if chunks_processed % 20 == 0 {
                tracing::debug!(chunks_processed, "Audio listener progress");
            }
        }

        // Return the receiver to storage so it can be reused for next recording
        if let Ok(mut guard) = receiver_storage.lock() {
            *guard = Some(receiver);
            tracing::debug!("Returned receiver to storage for reuse");
        }

        // Mark as not running when task exits (whether cancelled or bus closed)
        running_flag.store(false, Ordering::Release);
        tracing::info!(chunks_processed, "Audio listener stopped");
    });
}

/// Process a single audio chunk through STT.
///
/// Routes to the appropriate backend based on capabilities:
/// - Streaming worker available → use streaming path (Sherpa)
/// - Batch engine available → use batch transcriber (Parakeet, future models)
#[tracing::instrument(
    level = "trace",
    skip(app, state, chunk),
    fields(
        chunk_seq = chunk.seq,
        chunk_duration_ms = chunk.duration_ms(),
    )
)]
async fn process_audio_chunk<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SttState,
    chunk: &AudioChunk,
) -> Result<(), String> {
    // Capability-based routing (not model-specific)

    // 1. Use streaming worker if available (real-time streaming models like Sherpa)
    if state.has_streaming_worker() {
        return process_with_streaming_worker(app, state, chunk).await;
    }

    // 2. Use batch transcriber for any loaded engine (Parakeet, future models)
    if let Some(engine) = state.get_engine().await {
        return process_with_batch_engine(app, state, engine, chunk).await;
    }

    // No engine loaded
    Ok(())
}

/// Process audio using a streaming-capable worker (real-time streaming).
///
/// Used for models that support true streaming inference (e.g., Sherpa transducer).
async fn process_with_streaming_worker<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SttState,
    chunk: &AudioChunk,
) -> Result<(), String> {
    // Process VAD to detect speech-to-silence transitions
    let (needs_silence_injection, buffer_duration_ms) = state
        .with_streaming_mut(|streamer| {
            streamer.add_samples(&chunk.samples);
            let pending = streamer.take_silence_injection_pending();
            let duration = streamer.buffer_duration_ms();
            (pending, duration)
        })
        .await;

    // Emit VAD silence event for decoupled turn detection
    if needs_silence_injection {
        let _ = app.emit(
            "stt:vad_silence",
            VadSilencePayload {
                ts_ms: chrono::Utc::now().timestamp_millis(),
                buffer_duration_ms,
            },
        );

        // Inject silence into Sherpa to help acoustic model reset
        state.with_sherpa_worker(|worker| {
            let _ = worker.inject_silence(chunk.sample_rate);
        });
    }

    // Send chunk to worker (zero-copy: share Arc<[f32]>)
    let send_result = state.with_sherpa_worker(|worker| {
        worker.send_chunk(chunk.sample_rate, Arc::clone(&chunk.samples))
    });

    match send_result {
        Some(Ok(())) => {}
        Some(Err(_)) => {
            return Err("Worker channel send failed".to_string());
        }
        None => {
            return Ok(()); // No worker available
        }
    }

    // Drain results and emit events
    let result: Option<InferenceResult> = state
        .with_sherpa_worker(|worker| worker.drain_results().or_else(|| Some(worker.get_latest())))
        .flatten();

    if let Some(result) = result {
        // Emit commit event if there's a committed delta
        if let Some(ref delta) = result.committed_delta {
            let _ = app.emit(
                "stt:stream_commit",
                StreamingCommitPayload {
                    text: delta.clone(),
                    ts_ms: chrono::Utc::now().timestamp_millis(),
                },
            );
        }

        // Emit stream result
        let dto = StreamingResultDto::from(gibberish_application::StreamingResult {
            text: result.committed_text,
            volatile_text: result.partial_text,
            is_partial: result.is_partial,
            buffer_duration_ms: result.buffer_duration_ms,
        });

        let _ = app.emit("stt:stream_result", dto);
    }

    Ok(())
}

/// Process audio using any batch-capable STT engine.
///
/// This is model-agnostic - works with Parakeet, Whisper, or any future
/// engine implementing the `SttEngine` trait.
///
/// Uses hybrid approach:
/// - Periodic inference for real-time feedback (volatile text)
/// - VAD-triggered events for turn detection (handled by separate listener)
async fn process_with_batch_engine<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SttState,
    engine: std::sync::Arc<dyn gibberish_stt::SttEngine>,
    chunk: &AudioChunk,
) -> Result<(), String> {
    let result = batch_transcriber::process_batch_audio(state, engine, &chunk.samples).await?;
    batch_transcriber::emit_batch_events(app, &result);
    Ok(())
}
