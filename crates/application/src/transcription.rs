use crate::{StreamingTranscriber, TimedWord};
use gibberish_parakeet::ParakeetEngine;
use gibberish_stt::{Segment, SttEngine, Word};
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

    pub fn transcribe_file(
        engine: Arc<dyn SttEngine>,
        file_path: &str,
    ) -> Result<Vec<TranscriptSegment>, TranscriptionError> {
        let parakeet = engine
            .as_any()
            .downcast_ref::<ParakeetEngine>()
            .ok_or_else(|| {
                TranscriptionError::UnsupportedOperation(
                    "Engine does not support file transcription".to_string(),
                )
            })?;

        tracing::info!("Transcribing file: {}", file_path);

        let result = parakeet
            .transcribe_file(file_path)
            .map_err(|e| {
                tracing::error!("Transcription failed: {}", e);
                TranscriptionError::TranscriptionFailed(e.to_string())
            })?;

        tracing::info!(
            text_len = result.text.len(),
            tokens_count = result.tokens.len(),
            text_preview = %result.text.chars().take(100).collect::<String>(),
            "Transcription result"
        );

        let words: Vec<Word> = result
            .tokens
            .iter()
            .map(|t| {
                let start_ms = (t.start.max(0.0) * 1000.0).round() as u64;
                let mut end_ms = (t.end.max(0.0) * 1000.0).round() as u64;
                if end_ms <= start_ms {
                    end_ms = start_ms + 1;
                }
                Word {
                    text: t.text.clone(),
                    start_ms,
                    end_ms,
                    confidence: 1.0,
                }
            })
            .collect();

        let start_ms = words.first().map(|w| w.start_ms).unwrap_or(0);
        let end_ms = words.last().map(|w| w.end_ms).unwrap_or(0);

        Ok(vec![TranscriptSegment {
            text: result.text,
            start_ms,
            end_ms,
            speaker: None,
        }])
    }

    pub fn process_streaming_chunk(
        streamer: &mut StreamingTranscriber,
        engine: Option<Arc<dyn SttEngine>>,
        audio_chunk: &[f32],
    ) -> Result<StreamingResult, TranscriptionError> {
        static CHUNK_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let chunk_count = CHUNK_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        streamer.add_samples(audio_chunk);

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

        tracing::info!(
            segment_count = segments.len(),
            word_count = words.len(),
            first_segment_text = %segments.first().map(|s| s.text.as_str()).unwrap_or(""),
            "streaming_transcription_result"
        );

        streamer.mark_transcribed();

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
