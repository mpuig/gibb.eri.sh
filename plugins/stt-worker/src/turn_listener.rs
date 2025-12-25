//! Event-driven turn detection and VAD-confirmed transcription listener.
//!
//! Listens for `stt:vad_silence` events and:
//! 1. Runs turn detection on the audio buffer
//! 2. Triggers confirmed transcription for batch engines
//!
//! This is decoupled from any specific STT engine - any engine can emit the event.

use std::sync::Arc;

use tauri::{Emitter, Listener, Runtime};
use tokio_util::sync::CancellationToken;

use crate::batch_transcriber;
use crate::dto::{TurnPredictionPayload, VadSilencePayload};
use crate::state::SttState;

/// Start the VAD silence listener.
///
/// This listens for `stt:vad_silence` events and:
/// 1. Runs turn detection on the audio buffer (if enabled)
/// 2. Triggers confirmed transcription for batch engines
pub fn start_turn_listener<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: Arc<SttState>,
    cancel_token: CancellationToken,
) {
    let app_for_listener = app.clone();
    let state_for_listener = Arc::clone(&state);

    // Listen for VAD silence events from any STT engine
    let _listener = app.listen("stt:vad_silence", move |event| {
        let payload: VadSilencePayload = match serde_json::from_str(event.payload()) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to parse vad_silence payload: {}", e);
                return;
            }
        };

        // Clone for the async block
        let app = app_for_listener.clone();
        let state = Arc::clone(&state_for_listener);
        let cancel = cancel_token.clone();

        // Spawn async task to run turn detection and confirmed transcription
        tauri::async_runtime::spawn(async move {
            if cancel.is_cancelled() {
                return;
            }

            // Run turn detection
            if let Err(e) = run_turn_detection(&app, &state, &payload).await {
                tracing::warn!("Turn detection failed: {}", e);
            }

            // Run VAD-confirmed transcription for batch engines
            // (Skip if streaming worker is active - it handles its own flow)
            if !state.has_streaming_worker() {
                if let Err(e) = run_vad_confirmed_transcription(&app, &state).await {
                    tracing::warn!("VAD-confirmed transcription failed: {}", e);
                }
            }
        });
    });

    tracing::info!("VAD silence listener started (turn detection + confirmed transcription)");
}

/// Run turn detection on the current audio buffer.
async fn run_turn_detection<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SttState,
    _payload: &VadSilencePayload,
) -> Result<(), String> {
    // Check if turn detection is enabled
    let settings = state.get_turn_settings().await;
    if !settings.enabled {
        return Ok(());
    }

    // Get the turn detector
    let detector = match state.get_turn_detector().await {
        Some(d) => d,
        None => return Ok(()), // No detector loaded, skip silently
    };

    // Get the audio buffer from StreamingTranscriber
    let audio_buffer = state
        .with_streaming(|streamer| streamer.get_buffer().to_vec())
        .await;

    if audio_buffer.is_empty() {
        return Ok(());
    }

    // Run turn detection
    let probability = detector
        .predict_endpoint_probability(&audio_buffer)
        .map_err(|e| e.to_string())?;

    let prediction = gibberish_turn::TurnPrediction {
        probability,
        threshold: settings.threshold,
    };

    // Record turn boundary if complete
    if prediction.is_complete() {
        let end_ms = state.with_streaming(|s| s.buffer_duration_ms()).await;
        state.record_turn_boundary(end_ms).await;
    }

    // Emit turn prediction event
    let _ = app.emit(
        "stt:turn_prediction",
        TurnPredictionPayload {
            probability: prediction.probability,
            threshold: prediction.threshold,
            is_complete: prediction.is_complete(),
            ts_ms: chrono::Utc::now().timestamp_millis(),
        },
    );

    tracing::debug!(
        probability = prediction.probability,
        threshold = prediction.threshold,
        is_complete = prediction.is_complete(),
        "Turn prediction"
    );

    Ok(())
}

/// Run VAD-confirmed transcription on the current audio buffer.
///
/// This is called when VAD detects speech-to-silence transition, producing
/// higher-quality "final" transcription results. Works with any batch engine.
async fn run_vad_confirmed_transcription<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SttState,
) -> Result<(), String> {
    // Get the current engine (model-agnostic)
    let engine = match state.get_engine().await {
        Some(e) => e,
        None => return Ok(()), // No engine loaded, skip silently
    };

    // Run confirmed transcription
    let result = batch_transcriber::process_vad_confirmed(state, engine).await?;

    // Emit only transcription events (NOT vad_silence - we're already responding to it)
    batch_transcriber::emit_transcription_events(app, &result);

    tracing::debug!(
        text_len = result.result.text.len(),
        is_partial = result.result.is_partial,
        "VAD-confirmed transcription"
    );

    Ok(())
}
