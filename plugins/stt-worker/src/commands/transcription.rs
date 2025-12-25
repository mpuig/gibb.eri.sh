use crate::dto::{
    StreamingCommitPayload, StreamingResultDto, TranscriptSegmentDto, TurnPredictionPayload,
};
use crate::error::{Result, SttError};
use crate::state::SttState;
use gibberish_application::TranscriptionService;
use gibberish_bus::SAMPLE_RATE;
use gibberish_sherpa::{InferenceResult, SherpaStreamingEngine};
use std::sync::Arc;
use tauri::{Emitter, Runtime, State};

#[tauri::command]
pub async fn transcribe_audio(
    state: State<'_, Arc<SttState>>,
    audio_samples: Vec<f32>,
) -> Result<Vec<TranscriptSegmentDto>> {
    let engine = state.get_engine().await.ok_or(SttError::NoModelLoaded)?;

    let segments = TranscriptionService::transcribe_samples(engine.as_ref(), &audio_samples)?;

    Ok(segments
        .into_iter()
        .map(TranscriptSegmentDto::from)
        .collect())
}

#[tauri::command]
pub async fn transcribe_file(
    state: State<'_, Arc<SttState>>,
    file_path: String,
) -> Result<Vec<TranscriptSegmentDto>> {
    let engine = state.get_engine().await.ok_or(SttError::NoModelLoaded)?;

    let turn_boundaries_ms = state.get_turn_boundaries().await;
    let segments = TranscriptionService::transcribe_file(engine, &file_path, &turn_boundaries_ms)?;

    Ok(segments
        .into_iter()
        .map(TranscriptSegmentDto::from)
        .collect())
}

#[tauri::command]
pub async fn transcribe_streaming_chunk<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: State<'_, Arc<SttState>>,
    audio_chunk: Vec<f32>,
) -> Result<Option<StreamingResultDto>> {
    let engine = state.get_engine().await;
    let turn_detector = state.get_turn_detector().await;
    let turn_settings = state.get_turn_settings().await;

    // Try non-blocking streaming worker first (channel-based inference)
    // Worker path needs separate VAD processing for silence injection
    if state.has_streaming_worker() {
        // Process VAD to detect speech-to-silence transitions
        let needs_silence_injection = state
            .with_streaming_mut(|streamer| {
                streamer.add_samples(&audio_chunk);
                streamer.take_silence_injection_pending()
            })
            .await;

        // Inject silence if VAD detected speech-to-silence transition
        if needs_silence_injection {
            state.with_sherpa_worker(|worker| {
                let _ = worker.inject_silence(SAMPLE_RATE);
            });
        }

        if let Some(result) = try_worker_streaming(&app, &state, &audio_chunk) {
            return Ok(Some(result));
        }
    }

    // Fall back to blocking Sherpa API if no worker available
    if let Some(engine) = engine.as_ref() {
        if let Some(sherpa) = engine.as_any().downcast_ref::<SherpaStreamingEngine>() {
            let (text, volatile_text, is_partial, buffer_duration_ms) = sherpa
                .accept_streaming_chunk(SAMPLE_RATE, &audio_chunk)
                .map_err(|e| SttError::Transcription(e.to_string()))?;

            if let Some(delta) = sherpa.take_last_committed_delta() {
                let _ = app.emit(
                    "stt:stream_commit",
                    StreamingCommitPayload {
                        text: delta,
                        ts_ms: chrono::Utc::now().timestamp_millis(),
                    },
                );
            }

            return Ok(Some(StreamingResultDto::from(
                gibberish_application::StreamingResult {
                    text,
                    volatile_text,
                    is_partial,
                    buffer_duration_ms,
                },
            )));
        }
    }

    let (result, committed_delta, turn_prediction, turn_end_ms) = state
        .with_streaming_mut(|streamer| {
            let result = TranscriptionService::process_streaming_chunk(
                streamer,
                engine,
                &audio_chunk,
                turn_detector.clone(),
                turn_settings.enabled,
                turn_settings.threshold,
            )?;
            let committed_delta = streamer.take_last_committed_delta();
            let turn_prediction = streamer.take_last_turn_prediction();
            let turn_end_ms = streamer.take_last_turn_end_ms();
            Ok::<_, gibberish_application::TranscriptionError>((
                result,
                committed_delta,
                turn_prediction,
                turn_end_ms,
            ))
        })
        .await?;

    if let Some(end_ms) = turn_end_ms {
        state.record_turn_boundary(end_ms).await;
    }

    if let Some(pred) = turn_prediction {
        let _ = app.emit(
            "stt:turn_prediction",
            TurnPredictionPayload {
                probability: pred.probability,
                threshold: pred.threshold,
                is_complete: pred.is_complete(),
                ts_ms: chrono::Utc::now().timestamp_millis(),
            },
        );
    }

    if let Some(delta) = committed_delta {
        let _ = app.emit(
            "stt:stream_commit",
            StreamingCommitPayload {
                text: delta,
                ts_ms: chrono::Utc::now().timestamp_millis(),
            },
        );
    }

    Ok(Some(StreamingResultDto::from(result)))
}

#[tauri::command]
pub async fn reset_streaming_buffer(state: State<'_, Arc<SttState>>) -> Result<()> {
    // Reset worker if available
    state.with_sherpa_worker(|worker| worker.reset());

    // Also reset the blocking engine's state (in case we fall back to it)
    if let Some(engine) = state.get_engine().await {
        if let Some(sherpa) = engine.as_any().downcast_ref::<SherpaStreamingEngine>() {
            sherpa.reset_streaming();
        }
    }
    state.with_streaming_mut(|s| s.reset()).await;
    state.clear_turn_boundaries().await;
    tracing::debug!("Streaming state reset");
    Ok(())
}

#[tauri::command]
pub async fn get_streaming_buffer_duration(state: State<'_, Arc<SttState>>) -> Result<u64> {
    // Try worker first
    if let Some(duration) =
        state.with_sherpa_worker(|worker| worker.get_latest().buffer_duration_ms)
    {
        return Ok(duration);
    }

    // Fall back to blocking engine
    if let Some(engine) = state.get_engine().await {
        if let Some(sherpa) = engine.as_any().downcast_ref::<SherpaStreamingEngine>() {
            return Ok(sherpa.buffer_duration_ms());
        }
    }
    Ok(state.with_streaming(|s| s.buffer_duration_ms()).await)
}

/// Attempt to use the non-blocking worker for streaming transcription.
/// Returns None if no worker is available.
fn try_worker_streaming<R: Runtime>(
    app: &tauri::AppHandle<R>,
    state: &SttState,
    audio_chunk: &[f32],
) -> Option<StreamingResultDto> {
    // Send the chunk to the worker (non-blocking)
    // Convert slice to Arc<[f32]> via Vec for owned data
    let samples: std::sync::Arc<[f32]> = audio_chunk.to_vec().into();
    let send_result = state.with_sherpa_worker(|worker| worker.send_chunk(SAMPLE_RATE, samples))?;

    if send_result.is_err() {
        tracing::warn!("Worker channel send failed, worker may have crashed");
        return None;
    }

    // Drain all pending results and get the latest
    let result: Option<InferenceResult> = state.with_sherpa_worker(|worker| {
        worker.drain_results().or_else(|| Some(worker.get_latest()))
    })?;

    let result = result?;

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

    Some(StreamingResultDto::from(
        gibberish_application::StreamingResult {
            text: result.committed_text,
            volatile_text: result.partial_text,
            is_partial: result.is_partial,
            buffer_duration_ms: result.buffer_duration_ms,
        },
    ))
}
