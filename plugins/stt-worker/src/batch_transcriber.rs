//! Model-agnostic batch transcription coordinator.
//!
//! Provides hybrid transcription for any non-streaming STT engine:
//! - Periodic inference for real-time feedback (volatile text)
//! - VAD-triggered inference for confirmed text (final results)
//!
//! This module is decoupled from specific model implementations.
//! Any engine implementing `SttEngine` can be used.

use std::sync::Arc;

use gibberish_application::{StreamingResult, TranscriptionService};
use gibberish_stt::SttEngine;
use tauri::{Emitter, Runtime};

use crate::dto::{StreamingCommitPayload, StreamingResultDto, VadSilencePayload};
use crate::state::SttState;

/// Result of a batch transcription operation.
pub struct BatchTranscriptionResult {
    /// The streaming result with text and volatile text.
    pub result: StreamingResult,
    /// Delta text that was committed, if any.
    pub committed_delta: Option<String>,
    /// Whether VAD detected a silence (speech-to-silence transition).
    pub vad_silence_detected: bool,
    /// Current buffer duration in milliseconds.
    pub buffer_duration_ms: u64,
}

/// Process audio through any batch-capable STT engine.
///
/// This function:
/// 1. Adds audio to the streaming buffer
/// 2. Runs periodic inference based on `should_transcribe()` heuristics
/// 3. Detects VAD silence transitions
/// 4. Returns results for event emission
///
/// The caller is responsible for emitting events based on the result.
pub async fn process_batch_audio(
    state: &SttState,
    engine: Arc<dyn SttEngine>,
    samples: &[f32],
) -> Result<BatchTranscriptionResult, String> {
    let (result, committed_delta, vad_silence_detected, buffer_duration_ms) = state
        .with_streaming_mut(|streamer| {
            // Run transcription (handles buffering and periodic inference internally)
            let result = TranscriptionService::process_streaming_chunk(
                streamer,
                Some(engine),
                samples,
                None,  // Turn detection handled by separate event listener
                false, // Turn detection disabled here
                0.5,   // Unused when disabled
            )?;

            let committed_delta = streamer.take_last_committed_delta();
            let vad_silence = streamer.take_silence_injection_pending();
            let duration = streamer.buffer_duration_ms();

            Ok::<_, gibberish_application::TranscriptionError>((
                result,
                committed_delta,
                vad_silence,
                duration,
            ))
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(BatchTranscriptionResult {
        result,
        committed_delta,
        vad_silence_detected,
        buffer_duration_ms,
    })
}

/// Run confirmed transcription on VAD silence.
///
/// When VAD detects end of speech, this function runs inference on the
/// complete buffer and commits the result as final text.
///
/// Returns the final transcription result.
pub async fn process_vad_confirmed(
    state: &SttState,
    engine: Arc<dyn SttEngine>,
) -> Result<BatchTranscriptionResult, String> {
    let (result, committed_delta, buffer_duration_ms) = state
        .with_streaming_mut(|streamer| {
            // Force transcription regardless of periodic timing
            let buffer = streamer.get_buffer().to_vec();
            if buffer.is_empty() {
                return Ok((
                    StreamingResult {
                        text: streamer.committed_text().to_string(),
                        volatile_text: String::new(),
                        is_partial: false,
                        buffer_duration_ms: 0,
                    },
                    None,
                    0u64,
                ));
            }

            // Run inference on complete buffer
            let segments = engine.transcribe(&buffer).map_err(|e| {
                gibberish_application::TranscriptionError::TranscriptionFailed(e.to_string())
            })?;

            // Extract words for alignment
            let words: Vec<gibberish_application::TimedWord> = segments
                .iter()
                .flat_map(|s| s.words.iter())
                .map(|w| gibberish_application::TimedWord {
                    text: w.text.clone(),
                    start_ms: w.start_ms,
                    end_ms: w.end_ms,
                })
                .collect();

            streamer.mark_transcribed();
            streamer.update_words(&words);

            // Force commit on VAD silence (this is confirmed text)
            let alignment = streamer.analyze_words(&words);
            if alignment.stable_word_count > 0 {
                streamer.commit(&alignment);
                streamer.clear_word_cache();
            }

            let committed_delta = streamer.take_last_committed_delta();
            let duration = streamer.buffer_duration_ms();
            let (text, volatile_text) = streamer.build_full_display_text();

            Ok::<_, gibberish_application::TranscriptionError>((
                StreamingResult {
                    text,
                    volatile_text,
                    is_partial: false, // VAD confirmed = final
                    buffer_duration_ms: duration,
                },
                committed_delta,
                duration,
            ))
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(BatchTranscriptionResult {
        result,
        committed_delta,
        vad_silence_detected: true,
        buffer_duration_ms,
    })
}

/// Emit all events based on batch result (including VAD silence).
///
/// Use this from the audio listener where VAD silence originates.
///
/// Emits:
/// - `stt:vad_silence` if VAD detected speech-to-silence
/// - `stt:stream_commit` if there's a committed delta
/// - `stt:stream_result` with the transcription result
pub fn emit_batch_events<R: Runtime>(app: &tauri::AppHandle<R>, result: &BatchTranscriptionResult) {
    // Emit VAD silence event for decoupled turn detection
    if result.vad_silence_detected {
        let _ = app.emit(
            "stt:vad_silence",
            VadSilencePayload {
                ts_ms: chrono::Utc::now().timestamp_millis(),
                buffer_duration_ms: result.buffer_duration_ms,
            },
        );
    }

    emit_transcription_events(app, result);
}

/// Emit transcription events only (no VAD silence).
///
/// Use this when responding to a VAD silence event to avoid infinite loops.
///
/// Emits:
/// - `stt:stream_commit` if there's a committed delta
/// - `stt:stream_result` with the transcription result
pub fn emit_transcription_events<R: Runtime>(
    app: &tauri::AppHandle<R>,
    result: &BatchTranscriptionResult,
) {
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
    let dto = StreamingResultDto::from(result.result.clone());
    let _ = app.emit("stt:stream_result", dto);
}
