//! Streaming transcription with VAD and word stability tracking.
//!
//! This module provides real-time transcription with:
//! - Audio buffering with timestamp tracking
//! - Voice Activity Detection (VAD) for speech boundaries
//! - Word stability tracking across transcription decodes
//! - Smart turn detection for semantic endpoint prediction

mod audio_buffer;
mod vad_state;
mod word_tracker;

use crate::constants::TRANSCRIBE_THRESHOLD;

pub use audio_buffer::AudioBuffer;
pub use vad_state::{VadSettings, VadState};
pub use word_tracker::{AlignmentResult, TimedWord, WordTracker};

use gibberish_turn::TurnPrediction;

/// Orchestrates streaming transcription with VAD and word tracking.
///
/// Coordinates three focused components:
/// - `AudioBuffer`: Manages audio samples and timestamps
/// - `VadState`: Tracks speech/silence boundaries
/// - `WordTracker`: Tracks word stability across decodes
pub struct StreamingTranscriber {
    buffer: AudioBuffer,
    vad: VadState,
    words: WordTracker,
}

impl Default for StreamingTranscriber {
    fn default() -> Self {
        Self {
            buffer: AudioBuffer::new(),
            vad: VadState::new(),
            words: WordTracker::new(),
        }
    }
}

impl StreamingTranscriber {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Audio buffer operations ---

    /// Add audio samples to the buffer and process VAD.
    pub fn add_samples(&mut self, samples: &[f32]) {
        self.buffer.push(samples);
        self.vad.process(samples);
    }

    /// Get the current buffer for transcription.
    pub fn get_buffer(&self) -> &[f32] {
        self.buffer.samples()
    }

    /// Get buffer duration in milliseconds (total elapsed time).
    pub fn buffer_duration_ms(&self) -> u64 {
        self.buffer.total_duration_ms()
    }

    // --- VAD state queries ---

    /// Check if VAD detected a speech end (pause/silence).
    pub fn has_speech_end(&self) -> bool {
        self.vad.has_speech_end()
    }

    /// Check if turn prediction is needed.
    pub fn needs_turn_prediction(&self) -> bool {
        self.vad.needs_turn_prediction()
    }

    /// Clear the speech end flag after processing.
    pub fn clear_speech_end(&mut self) {
        self.vad.clear_speech_end();
    }

    /// Set the turn prediction result.
    pub fn set_turn_prediction(&mut self, prediction: TurnPrediction) {
        self.vad.set_turn_prediction(prediction);
    }

    /// Take the last turn prediction (consumes it).
    pub fn take_last_turn_prediction(&mut self) -> Option<TurnPrediction> {
        self.vad.take_last_turn_prediction()
    }

    /// Take the last semantic end-of-turn timestamp (consumes it).
    pub fn take_last_turn_end_ms(&mut self) -> Option<u64> {
        self.vad.take_last_turn_end_ms()
    }

    /// Check and consume the silence injection pending flag.
    /// Returns true if silence should be injected to help acoustic model reset.
    pub fn take_silence_injection_pending(&mut self) -> bool {
        self.vad.take_silence_injection_pending()
    }

    // --- Transcription lifecycle ---

    /// Check if enough new audio has accumulated to warrant transcription.
    pub fn should_transcribe(&self) -> bool {
        if !self.vad.should_transcribe() {
            return false;
        }

        if self.vad.should_force_transcribe() {
            return true;
        }

        self.buffer.has_new_audio(TRANSCRIBE_THRESHOLD)
    }

    /// Mark that transcription was performed at current buffer length.
    pub fn mark_transcribed(&mut self) {
        self.buffer.mark_transcribed();
        self.vad.mark_transcribed();
    }

    /// Update tracked words with new transcription.
    pub fn update_words(&mut self, new_words: &[TimedWord]) {
        self.words
            .update(new_words, self.buffer.timestamp_offset_ms());
    }

    // --- Commit lifecycle ---

    /// Check if we should commit stable words.
    pub fn should_commit(&self) -> bool {
        if self.vad.should_commit() {
            return true;
        }
        self.buffer.exceeds_commit_threshold()
    }

    /// Analyze words and determine which are stable enough to commit.
    pub fn analyze_words(&self, _words: &[TimedWord]) -> AlignmentResult {
        let buffer_end_abs = self.buffer.timestamp_offset_ms() + self.buffer.current_duration_ms();
        self.words.analyze(buffer_end_abs)
    }

    /// Commit stable text and trim the buffer.
    pub fn commit(&mut self, alignment: &AlignmentResult) {
        let buffer_end_abs = self.buffer.timestamp_offset_ms() + self.buffer.current_duration_ms();
        let is_semantic_turn_end = self.vad.is_semantic_turn_end();
        self.words.commit(alignment, buffer_end_abs);

        if let Some(trim_from_ms) = alignment.trim_from_ms {
            self.buffer.trim_from_ms(trim_from_ms);
        } else {
            self.buffer.clear();
        }

        self.vad.clear_speech_end();

        if is_semantic_turn_end {
            self.words.set_paragraph_break_pending();
            self.vad.set_last_turn_end_ms(self.words.committed_end_ms());
        }
    }

    /// Commit segment text directly (for engines without word-level timestamps).
    ///
    /// This is used by batch engines like Whisper that return segments without
    /// individual word timings.
    pub fn commit_segment_text(&mut self, text: &str) {
        let is_semantic_turn_end = self.vad.is_semantic_turn_end();
        self.words.commit_text(text);
        self.buffer.clear();
        self.vad.clear_speech_end();

        if is_semantic_turn_end {
            self.words.set_paragraph_break_pending();
        }
    }

    // --- Text accessors ---

    /// Get the current committed text.
    pub fn committed_text(&self) -> &str {
        self.words.committed_text()
    }

    /// Build display text from committed + stable tracked words.
    pub fn build_display_text(&self) -> String {
        let buffer_end_abs = self.buffer.timestamp_offset_ms() + self.buffer.current_duration_ms();
        self.words.build_display_text(buffer_end_abs)
    }

    /// Build display text including volatile tail words.
    pub fn build_full_display_text(&self) -> (String, String) {
        let buffer_end_abs = self.buffer.timestamp_offset_ms() + self.buffer.current_duration_ms();
        self.words.build_full_display_text(buffer_end_abs)
    }

    /// Take the last committed delta (consumes it).
    pub fn take_last_committed_delta(&mut self) -> Option<String> {
        self.words.take_last_committed_delta()
    }

    // --- Lifecycle ---

    /// Reset all state for a new recording.
    pub fn reset(&mut self) {
        self.buffer.reset();
        self.vad.reset();
        self.words.reset();
    }

    /// Clear word tracking (API compatibility).
    pub fn clear_word_cache(&mut self) {
        self.words.clear_cache();
    }

    // --- VAD Settings ---

    /// Get current VAD settings.
    pub fn vad_settings(&self) -> VadSettings {
        self.vad.settings()
    }

    /// Update VAD settings and reinitialize VAD.
    pub fn set_vad_settings(&mut self, settings: VadSettings) {
        self.vad.set_settings(settings);
        self.vad.reinitialize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(text: &str, start_ms: u64, end_ms: u64) -> TimedWord {
        TimedWord {
            text: text.to_string(),
            start_ms,
            end_ms,
        }
    }

    #[test]
    fn test_add_samples() {
        let mut transcriber = StreamingTranscriber::new();
        transcriber.add_samples(&[0.0; 1000]);
        assert_eq!(transcriber.get_buffer().len(), 1000);
    }

    #[test]
    fn test_build_display_text() {
        let mut transcriber = StreamingTranscriber::new();

        // Add enough buffer so words are outside the tail window
        let samples_2sec = vec![0.0f32; 32000];
        transcriber.add_samples(&samples_2sec);

        let words = vec![make_word("Hello", 0, 500), make_word("world", 500, 1000)];
        transcriber.update_words(&words);

        let text = transcriber.build_display_text();
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn test_reset_clears_all_state() {
        let mut transcriber = StreamingTranscriber::new();
        transcriber.add_samples(&vec![0.0; 1000]);

        transcriber.reset();

        assert!(transcriber.get_buffer().is_empty());
        assert!(transcriber.committed_text().is_empty());
    }
}
