//! Audio buffer management for streaming transcription.
//!
//! Uses a cursor-based approach with lazy compaction for O(1) logical trim.
//! The actual memory compaction is deferred until the pending trim exceeds a threshold.

use crate::constants::{COMMIT_THRESHOLD, MAX_BUFFER_SAMPLES, SAMPLE_RATE};

/// Threshold for triggering actual memory compaction (16k samples = 1 second).
const COMPACT_THRESHOLD: usize = 16000;

/// Manages the audio sample buffer with timestamp tracking.
///
/// Uses a cursor-based approach: trim operations update `start_cursor` without
/// moving memory. Actual compaction only happens when pending trim exceeds threshold.
#[derive(Debug, Default)]
pub struct AudioBuffer {
    /// Raw audio samples
    samples: Vec<f32>,
    /// Cursor pointing to the logical start of valid data
    start_cursor: usize,
    /// Total samples that have been trimmed (for calculating elapsed time)
    trimmed_samples_total: usize,
    /// Timestamp offset: added to word timestamps after buffer trim
    timestamp_offset_ms: u64,
    /// Buffer length at last transcription (relative to start_cursor)
    last_transcription_len: usize,
}

impl AudioBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add audio samples to the buffer.
    ///
    /// Enforces max buffer cap to bound hypothesis variance. When exceeded,
    /// the oldest samples are trimmed to keep buffer within MAX_BUFFER_SAMPLES.
    pub fn push(&mut self, samples: &[f32]) {
        self.samples.extend_from_slice(samples);

        // Enforce max buffer cap to prevent unbounded growth
        let excess = self.logical_len().saturating_sub(MAX_BUFFER_SAMPLES);
        if excess > 0 {
            self.start_cursor += excess;
            self.trimmed_samples_total += excess;
            let trimmed_ms = (excess as u64 * 1000) / SAMPLE_RATE as u64;
            self.timestamp_offset_ms += trimmed_ms;

            // Compact if cursor exceeds threshold
            if self.start_cursor >= COMPACT_THRESHOLD {
                self.compact();
            }
        }
    }

    /// Get the current buffer for transcription.
    #[inline]
    pub fn samples(&self) -> &[f32] {
        &self.samples[self.start_cursor..]
    }

    /// Logical length of valid data (excluding cursor offset).
    #[inline]
    fn logical_len(&self) -> usize {
        self.samples.len() - self.start_cursor
    }

    /// Get current buffer duration in milliseconds.
    pub fn current_duration_ms(&self) -> u64 {
        (self.logical_len() as u64 * 1000) / SAMPLE_RATE as u64
    }

    /// Get total elapsed duration in milliseconds (buffer + trimmed samples).
    pub fn total_duration_ms(&self) -> u64 {
        let total_samples = self.logical_len() + self.trimmed_samples_total;
        (total_samples as u64 * 1000) / SAMPLE_RATE as u64
    }

    /// Get the timestamp offset (added to buffer-relative timestamps).
    pub fn timestamp_offset_ms(&self) -> u64 {
        self.timestamp_offset_ms
    }

    /// Check if enough new audio has accumulated since last transcription.
    pub fn has_new_audio(&self, threshold: usize) -> bool {
        let new_audio_len = self
            .logical_len()
            .saturating_sub(self.last_transcription_len);
        new_audio_len >= threshold
    }

    /// Mark that transcription was performed at current buffer length.
    pub fn mark_transcribed(&mut self) {
        self.last_transcription_len = self.logical_len();
    }

    /// Check if buffer exceeds commit threshold.
    pub fn exceeds_commit_threshold(&self) -> bool {
        self.logical_len() >= COMMIT_THRESHOLD
    }

    /// Trim the buffer from a given absolute timestamp.
    ///
    /// Uses O(1) cursor update with lazy compaction when threshold exceeded.
    /// Returns the number of samples trimmed.
    pub fn trim_from_ms(&mut self, trim_from_ms: u64) -> usize {
        let trim_rel_ms = trim_from_ms.saturating_sub(self.timestamp_offset_ms);
        let trim_samples = (trim_rel_ms as usize * SAMPLE_RATE) / 1000;

        if trim_samples < self.logical_len() {
            // O(1) logical trim via cursor update
            self.start_cursor += trim_samples;
            self.trimmed_samples_total += trim_samples;
            self.timestamp_offset_ms = trim_from_ms;
            self.last_transcription_len = 0;

            // Compact if cursor exceeds threshold (amortized cost)
            if self.start_cursor >= COMPACT_THRESHOLD {
                self.compact();
            }

            trim_samples
        } else {
            0
        }
    }

    /// Physically compact the buffer by removing data before cursor.
    fn compact(&mut self) {
        if self.start_cursor > 0 {
            self.samples.drain(0..self.start_cursor);
            self.start_cursor = 0;
        }
    }

    /// Clear the entire buffer.
    pub fn clear(&mut self) {
        self.trimmed_samples_total += self.logical_len();
        let buffer_duration = self.current_duration_ms();
        self.timestamp_offset_ms += buffer_duration;
        self.samples.clear();
        self.start_cursor = 0;
        self.last_transcription_len = 0;
    }

    /// Reset all state for a new recording.
    pub fn reset(&mut self) {
        self.samples.clear();
        self.start_cursor = 0;
        self.trimmed_samples_total = 0;
        self.timestamp_offset_ms = 0;
        self.last_transcription_len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_samples() {
        let mut buffer = AudioBuffer::new();
        buffer.push(&[1.0, 2.0, 3.0]);
        assert_eq!(buffer.samples(), &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_duration_calculation() {
        let mut buffer = AudioBuffer::new();
        buffer.push(&vec![0.0; 16000]); // 1 second at 16kHz
        assert_eq!(buffer.current_duration_ms(), 1000);
        assert_eq!(buffer.total_duration_ms(), 1000);
    }

    #[test]
    fn test_trim_updates_offset() {
        let mut buffer = AudioBuffer::new();
        buffer.push(&vec![0.0; 32000]); // 2 seconds

        buffer.trim_from_ms(1000); // Trim first second

        assert_eq!(buffer.timestamp_offset_ms(), 1000);
        assert_eq!(buffer.current_duration_ms(), 1000);
        assert_eq!(buffer.total_duration_ms(), 2000);
    }

    #[test]
    fn test_clear_preserves_total_duration() {
        let mut buffer = AudioBuffer::new();
        buffer.push(&vec![0.0; 16000]); // 1 second

        buffer.clear();

        assert!(buffer.samples().is_empty());
        assert_eq!(buffer.total_duration_ms(), 1000);
        assert_eq!(buffer.timestamp_offset_ms(), 1000);
    }

    #[test]
    fn test_reset_clears_everything() {
        let mut buffer = AudioBuffer::new();
        buffer.push(&vec![0.0; 16000]);
        buffer.trim_from_ms(500);

        buffer.reset();

        assert!(buffer.samples().is_empty());
        assert_eq!(buffer.timestamp_offset_ms(), 0);
        assert_eq!(buffer.total_duration_ms(), 0);
    }
}
