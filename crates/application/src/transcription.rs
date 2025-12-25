use crate::{StreamingTranscriber, TimedWord};
use gibberish_stt::{Segment, SttEngine, Word};
use gibberish_turn::{TurnDetector, TurnPrediction};
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<i32>,
}

impl From<Segment> for TranscriptSegment {
    fn from(seg: Segment) -> Self {
        Self {
            text: seg.text,
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            speaker: seg.speaker,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamingResult {
    pub text: String,
    pub volatile_text: String,
    pub is_partial: bool,
    pub buffer_duration_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("no model loaded")]
    NoModelLoaded,
    #[error("transcription failed: {0}")]
    TranscriptionFailed(String),
    #[error("unsupported operation: {0}")]
    UnsupportedOperation(String),
}

pub struct TranscriptionService;

impl TranscriptionService {
    pub fn transcribe_samples(
        engine: &dyn SttEngine,
        samples: &[f32],
    ) -> Result<Vec<TranscriptSegment>, TranscriptionError> {
        tracing::debug!("Transcribing {} samples", samples.len());

        let segments = engine
            .transcribe(samples)
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?;

        Ok(segments.into_iter().map(TranscriptSegment::from).collect())
    }

    fn split_words_on_boundaries(
        words: &[Word],
        turn_boundaries_ms: &[u64],
        speaker: Option<i32>,
    ) -> Vec<TranscriptSegment> {
        if words.is_empty() || turn_boundaries_ms.is_empty() {
            return Vec::new();
        }

        let mut boundaries = turn_boundaries_ms.to_vec();
        boundaries.sort_unstable();
        boundaries.dedup();

        let mut segments: Vec<TranscriptSegment> = Vec::new();
        let mut current_words: Vec<&Word> = Vec::new();
        let mut boundary_idx = 0usize;

        for word in words {
            while boundary_idx < boundaries.len() && boundaries[boundary_idx] <= word.start_ms {
                if !current_words.is_empty() {
                    let start_ms = current_words.first().unwrap().start_ms;
                    let end_ms = current_words.last().unwrap().end_ms;
                    let text = current_words
                        .iter()
                        .map(|w| w.text.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    segments.push(TranscriptSegment {
                        text,
                        start_ms,
                        end_ms,
                        speaker,
                    });
                    current_words.clear();
                }
                boundary_idx += 1;
            }

            current_words.push(word);
        }

        if !current_words.is_empty() {
            let start_ms = current_words.first().unwrap().start_ms;
            let end_ms = current_words.last().unwrap().end_ms;
            let text = current_words
                .iter()
                .map(|w| w.text.trim())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            segments.push(TranscriptSegment {
                text,
                start_ms,
                end_ms,
                speaker,
            });
        }

        segments
    }

    pub fn transcribe_file(
        engine: Arc<dyn SttEngine>,
        file_path: &str,
        turn_boundaries_ms: &[u64],
    ) -> Result<Vec<TranscriptSegment>, TranscriptionError> {
        tracing::info!("Transcribing file: {}", file_path);

        // Use the trait method - each engine can optimize for file transcription
        let segments = engine.transcribe_file(Path::new(file_path)).map_err(|e| {
            tracing::error!("Transcription failed: {}", e);
            TranscriptionError::TranscriptionFailed(e.to_string())
        })?;

        if turn_boundaries_ms.is_empty() {
            return Ok(segments.into_iter().map(TranscriptSegment::from).collect());
        }

        // Split on turn boundaries if provided
        let words: Vec<Word> = segments
            .iter()
            .flat_map(|s| s.words.iter())
            .cloned()
            .collect();
        let speaker = segments.first().and_then(|s| s.speaker);

        if !words.is_empty() {
            let split = Self::split_words_on_boundaries(&words, turn_boundaries_ms, speaker);
            if !split.is_empty() {
                return Ok(split);
            }
        }

        Ok(segments.into_iter().map(TranscriptSegment::from).collect())
    }

    pub fn process_streaming_chunk(
        streamer: &mut StreamingTranscriber,
        engine: Option<Arc<dyn SttEngine>>,
        audio_chunk: &[f32],
        turn_detector: Option<Arc<dyn TurnDetector>>,
        turn_enabled: bool,
        turn_threshold: f32,
    ) -> Result<StreamingResult, TranscriptionError> {
        static CHUNK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let chunk_count = CHUNK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        streamer.add_samples(audio_chunk);

        // If VAD says we're in a pause, optionally use Smart Turn to decide if it's a "real" end.
        if turn_enabled && streamer.needs_turn_prediction() {
            if let Some(detector) = turn_detector.as_deref() {
                let threshold = turn_threshold.clamp(0.0, 1.0);
                match detector.predict_endpoint_probability(streamer.get_buffer()) {
                    Ok(probability) => {
                        streamer.set_turn_prediction(TurnPrediction {
                            probability,
                            threshold,
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Turn detector error (ignoring): {}", e);
                    }
                }
            }
        }

        if chunk_count % 10 == 0 {
            tracing::info!(
                chunk = chunk_count,
                chunk_samples = audio_chunk.len(),
                buffer_duration_ms = streamer.buffer_duration_ms(),
                should_transcribe = streamer.should_transcribe(),
                has_engine = engine.is_some(),
                "streaming_chunk_received"
            );
        }

        if !streamer.should_transcribe() {
            return Ok(StreamingResult {
                text: streamer.committed_text().to_string(),
                volatile_text: String::new(),
                is_partial: true,
                buffer_duration_ms: streamer.buffer_duration_ms(),
            });
        }

        let engine = match engine {
            Some(e) => e,
            None => {
                tracing::warn!("No engine loaded for streaming transcription");
                return Ok(StreamingResult {
                    text: streamer.committed_text().to_string(),
                    volatile_text: String::new(),
                    is_partial: true,
                    buffer_duration_ms: streamer.buffer_duration_ms(),
                });
            }
        };

        let buffer = streamer.get_buffer().to_vec();
        tracing::info!(
            buffer_len = buffer.len(),
            buffer_duration_ms = streamer.buffer_duration_ms(),
            "streaming_transcription_starting"
        );

        let segments = engine
            .transcribe(&buffer)
            .map_err(|e| TranscriptionError::TranscriptionFailed(e.to_string()))?;

        let words: Vec<TimedWord> = segments
            .iter()
            .flat_map(|s| s.words.iter())
            .map(|w| TimedWord {
                text: w.text.clone(),
                start_ms: w.start_ms,
                end_ms: w.end_ms,
            })
            .collect();

        // Extract segment text for engines without word-level timestamps (e.g., Whisper)
        let segment_text: String = segments
            .iter()
            .map(|s| s.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let has_word_timestamps = !words.is_empty();

        tracing::info!(
            segment_count = segments.len(),
            word_count = words.len(),
            has_word_timestamps = has_word_timestamps,
            first_segment_text = %segments.first().map(|s| s.text.as_str()).unwrap_or(""),
            "streaming_transcription_result"
        );

        streamer.mark_transcribed();

        // Handle engines without word-level timestamps (e.g., Whisper batch models).
        // Use segment text directly as volatile text, commit on speech end.
        if !has_word_timestamps {
            let committed = streamer.committed_text().to_string();

            // When VAD detects speech end, commit the segment text
            if streamer.should_commit() && !segment_text.is_empty() {
                streamer.commit_segment_text(&segment_text);
                let new_committed = streamer.committed_text().to_string();
                return Ok(StreamingResult {
                    text: new_committed,
                    volatile_text: String::new(),
                    is_partial: streamer.buffer_duration_ms() > 0,
                    buffer_duration_ms: streamer.buffer_duration_ms(),
                });
            }

            return Ok(StreamingResult {
                text: committed,
                volatile_text: segment_text,
                is_partial: true,
                buffer_duration_ms: streamer.buffer_duration_ms(),
            });
        }

        // Update word tracking with timestamp-based alignment
        streamer.update_words(&words);

        if streamer.should_commit() {
            let alignment = streamer.analyze_words(&words);

            if alignment.stable_word_count > 0 {
                streamer.commit(&alignment);
                streamer.clear_word_cache();

                let (text, volatile_text) = streamer.build_full_display_text();
                return Ok(StreamingResult {
                    text,
                    volatile_text,
                    is_partial: streamer.buffer_duration_ms() > 0,
                    buffer_duration_ms: streamer.buffer_duration_ms(),
                });
            }
        }

        // Build display text from tracked stable words + volatile tail
        let (text, volatile_text) = streamer.build_full_display_text();
        tracing::info!(
            text_len = text.len(),
            volatile_len = volatile_text.len(),
            text_preview = %text.chars().take(50).collect::<String>(),
            volatile_preview = %volatile_text.chars().take(30).collect::<String>(),
            "streaming_display_text"
        );
        Ok(StreamingResult {
            text,
            volatile_text,
            is_partial: true,
            buffer_duration_ms: streamer.buffer_duration_ms(),
        })
    }
}
