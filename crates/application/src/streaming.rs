use super::constants::*;
use gibberish_vad::{SileroVad, VadEvent};

/// Represents a word with timing information (from transcription)
#[derive(Debug, Clone)]
pub struct TimedWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// A word with stability tracking across decodes
#[derive(Debug, Clone)]
struct TrackedWord {
    text: String,
    start_ms: u64,
    end_ms: u64,
    /// How many consecutive decodes this word appeared with similar text
    stability: u8,
    /// Unique ID for tracking identity across decodes
    id: u64,
}


/// Result of processing a transcription with word alignment
#[derive(Debug, Clone)]
pub struct AlignmentResult {
    /// Text to commit (stable words)
    pub stable_text: String,
    /// Number of stable words found
    pub stable_word_count: usize,
    /// Timestamp (ms) to trim buffer from
    pub trim_from_ms: Option<u64>,
    /// End timestamp (ms) of the last stable word
    pub stable_end_ms: u64,
}

/// Pure domain logic for streaming transcription with proper word stability tracking
pub struct StreamingTranscriber {
    buffer: Vec<f32>,
    committed_text: String,
    last_transcription_len: usize,
    vad: Option<SileroVad>,
    speech_end_pending: bool,
    /// Total samples that have been trimmed (for calculating elapsed time)
    trimmed_samples_total: usize,
    /// End timestamp (ms) of the last committed word
    committed_end_ms: u64,
    /// Words from the previous decode with stability tracking
    tracked_words: Vec<TrackedWord>,
    /// Counter for generating unique word IDs
    next_word_id: u64,
    /// Timestamp offset: added to word timestamps after buffer trim
    /// This converts buffer-relative timestamps to absolute timestamps
    timestamp_offset_ms: u64,
    /// Number of transcriptions performed since VAD reported a speech end.
    /// Used to avoid committing too early on the first post-VAD decode.
    speech_end_transcription_count: u8,
    /// Text committed on the most recent `commit` call (cleared after read).
    last_committed_delta: Option<String>,
}

/// Minimum stability count before a word is considered stable
const MIN_STABILITY_COUNT: u8 = 2;

/// Minimum IoU (Intersection over Union) for timestamp-based word matching
const MIN_TIMESTAMP_IOU: f32 = 0.3;

/// Words ending this many ms before buffer end are candidates for commit
const COMMIT_WINDOW_MS: u64 = 500;

/// Don't show words that end within this tail window unless they're stable.
/// This improves perceived responsiveness (you'll see earlier words quickly)
/// while keeping the most volatile part of the hypothesis from "dancing".
const DISPLAY_TAIL_MS: u64 = 600;

impl Default for StreamingTranscriber {
    fn default() -> Self {
        let vad = SileroVad::new(SAMPLE_RATE as u32).ok();
        if vad.is_none() {
            tracing::warn!("VAD initialization failed, using time-based commits only");
        }
        Self {
            buffer: Vec::new(),
            committed_text: String::new(),
            last_transcription_len: 0,
            vad,
            speech_end_pending: false,
            trimmed_samples_total: 0,
            committed_end_ms: 0,
            tracked_words: Vec::new(),
            next_word_id: 0,
            timestamp_offset_ms: 0,
            speech_end_transcription_count: 0,
            last_committed_delta: None,
        }
    }
}

impl StreamingTranscriber {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add audio samples to the buffer and process VAD
    pub fn add_samples(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);

        // Process VAD to detect speech end
        if let Some(ref mut vad) = self.vad {
            match vad.process(samples) {
                Ok(events) => {
                    for event in events {
                        if matches!(event, VadEvent::SpeechEnd { .. }) {
                            // VAD can emit multiple SpeechEnd events during silence.
                            // Only treat the first one as a commit trigger; otherwise we'd
                            // keep resetting the post-VAD transcription counter and delay commits.
                            if !self.speech_end_pending {
                                self.speech_end_pending = true;
                                self.speech_end_transcription_count = 0;
                                tracing::debug!("VAD detected speech end");
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("VAD processing error: {}", e);
                }
            }
        }
    }

    /// Check if VAD detected a speech end (pause/silence)
    pub fn has_speech_end(&self) -> bool {
        self.speech_end_pending
    }

    /// Clear the speech end flag after processing
    pub fn clear_speech_end(&mut self) {
        self.speech_end_pending = false;
    }

    /// Check if enough new audio has accumulated to warrant transcription
    pub fn should_transcribe(&self) -> bool {
        let new_audio_len = self.buffer.len().saturating_sub(self.last_transcription_len);
        new_audio_len >= TRANSCRIBE_THRESHOLD
    }

    /// Get the current buffer for transcription
    pub fn get_buffer(&self) -> &[f32] {
        &self.buffer
    }

    /// Get buffer duration in milliseconds (just the current buffer)
    fn current_buffer_duration_ms(&self) -> u64 {
        (self.buffer.len() as u64 * 1000) / SAMPLE_RATE as u64
    }

    /// Get total elapsed duration in milliseconds (buffer + trimmed samples)
    pub fn buffer_duration_ms(&self) -> u64 {
        let total_samples = self.buffer.len() + self.trimmed_samples_total;
        (total_samples as u64 * 1000) / SAMPLE_RATE as u64
    }

    /// Check if we should commit stable words
    pub fn should_commit(&self) -> bool {
        if self.speech_end_pending {
            // Avoid committing on the very first decode after VAD triggers.
            // Parakeet often revises hypotheses immediately after the speech end,
            // so waiting for an additional decode increases stability.
            return self.speech_end_transcription_count >= 2;
        }
        self.buffer.len() >= COMMIT_THRESHOLD
    }

    /// Get the current committed text
    pub fn committed_text(&self) -> &str {
        &self.committed_text
    }

    /// Mark that transcription was performed at current buffer length
    pub fn mark_transcribed(&mut self) {
        self.last_transcription_len = self.buffer.len();
        if self.speech_end_pending {
            self.speech_end_transcription_count = self.speech_end_transcription_count.saturating_add(1);
        }
    }

    /// Compute Intersection over Union for two time ranges
    fn compute_iou(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> f32 {
        let intersection_start = a_start.max(b_start);
        let intersection_end = a_end.min(b_end);

        if intersection_start >= intersection_end {
            return 0.0;
        }

        let intersection = (intersection_end - intersection_start) as f32;
        let union = (a_end - a_start + b_end - b_start) as f32 - intersection;

        if union <= 0.0 {
            0.0
        } else {
            intersection / union
        }
    }

    /// Normalize text for comparison (lowercase, alphanumeric only)
    fn normalize_text(text: &str) -> String {
        text.trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    }

    /// Check if two words are similar (normalized text match)
    fn words_match(a: &str, b: &str) -> bool {
        let norm_a = Self::normalize_text(a);
        let norm_b = Self::normalize_text(b);

        // Both empty (punctuation-only) - compare raw
        if norm_a.is_empty() && norm_b.is_empty() {
            return a.trim() == b.trim();
        }

        // One empty, one not - no match
        if norm_a.is_empty() || norm_b.is_empty() {
            return false;
        }

        norm_a == norm_b
    }

    /// Align new words with tracked words using timestamp overlap
    /// Returns updated tracked words with stability counts
    fn align_and_track(&mut self, new_words: &[TimedWord]) -> Vec<TrackedWord> {
        let mut result = Vec::with_capacity(new_words.len());
        let mut used_prev_indices: Vec<bool> = vec![false; self.tracked_words.len()];

        // Log incoming words for debugging
        if !new_words.is_empty() {
            let words_summary: Vec<String> = new_words
                .iter()
                .take(5)
                .map(|w| format!("{}@{}-{}", w.text, w.start_ms, w.end_ms))
                .collect();
            tracing::debug!(
                words = ?words_summary,
                offset_ms = self.timestamp_offset_ms,
                prev_tracked = self.tracked_words.len(),
                "align_incoming_words"
            );
        }

        for new_word in new_words {
            // Convert to absolute timestamp for matching
            let new_abs_start = new_word.start_ms + self.timestamp_offset_ms;
            let new_abs_end = new_word.end_ms + self.timestamp_offset_ms;

            // Find best matching previous word by timestamp IoU
            let mut best_match: Option<(usize, f32)> = None;

            for (i, prev_word) in self.tracked_words.iter().enumerate() {
                if used_prev_indices[i] {
                    continue;
                }

                // Previous words already have absolute timestamps
                let iou = Self::compute_iou(
                    prev_word.start_ms,
                    prev_word.end_ms,
                    new_abs_start,
                    new_abs_end,
                );

                if iou >= MIN_TIMESTAMP_IOU {
                    if let Some((_, best_iou)) = best_match {
                        if iou > best_iou {
                            best_match = Some((i, iou));
                        }
                    } else {
                        best_match = Some((i, iou));
                    }
                }
            }

            let tracked = if let Some((prev_idx, iou)) = best_match {
                used_prev_indices[prev_idx] = true;
                let prev_word = &self.tracked_words[prev_idx];

                // Check if text matches
                if Self::words_match(&prev_word.text, &new_word.text) {
                    // Same word, increment stability
                    tracing::trace!(
                        text = %new_word.text,
                        prev_text = %prev_word.text,
                        iou = iou,
                        new_stability = prev_word.stability + 1,
                        "word_matched"
                    );
                    TrackedWord {
                        text: new_word.text.clone(),
                        start_ms: new_abs_start,
                        end_ms: new_abs_end,
                        stability: prev_word.stability.saturating_add(1),
                        id: prev_word.id,
                    }
                } else {
                    // Position matches but text changed - reset stability but keep ID
                    tracing::trace!(
                        new_text = %new_word.text,
                        prev_text = %prev_word.text,
                        iou = iou,
                        "word_text_changed"
                    );
                    TrackedWord {
                        text: new_word.text.clone(),
                        start_ms: new_abs_start,
                        end_ms: new_abs_end,
                        stability: 1,
                        id: prev_word.id,
                    }
                }
            } else {
                // New word, no match
                let id = self.next_word_id;
                self.next_word_id += 1;
                TrackedWord {
                    text: new_word.text.clone(),
                    start_ms: new_abs_start,
                    end_ms: new_abs_end,
                    stability: 1,
                    id,
                }
            };

            result.push(tracked);
        }

        result
    }

    /// Update tracked words with new transcription
    pub fn update_words(&mut self, new_words: &[TimedWord]) {
        // Don't wipe tracking on empty results - likely a transient model glitch
        if new_words.is_empty() && !self.tracked_words.is_empty() {
            tracing::warn!(
                tracked_count = self.tracked_words.len(),
                "Ignoring empty transcription, preserving tracked words"
            );
            return;
        }

        let old_count = self.tracked_words.len();
        self.tracked_words = self.align_and_track(new_words);

        // Log alignment results for debugging
        let matched_count = self.tracked_words.iter().filter(|w| w.stability > 1).count();
        let new_count = self.tracked_words.iter().filter(|w| w.stability == 1).count();

        tracing::debug!(
            old_tracked = old_count,
            new_words = new_words.len(),
            matched = matched_count,
            new = new_count,
            offset_ms = self.timestamp_offset_ms,
            "word_alignment_result"
        );
    }

    /// Get words that are stable (appeared unchanged for MIN_STABILITY_COUNT decodes)
    #[cfg(test)]
    fn get_stable_words(&self) -> Vec<&TrackedWord> {
        self.tracked_words
            .iter()
            .filter(|w| w.stability >= MIN_STABILITY_COUNT)
            .collect()
    }

    /// Get words that are stable AND old enough to commit
    /// (ending before buffer_end - COMMIT_WINDOW_MS)
    fn get_committable_words(&self) -> Vec<&TrackedWord> {
        let buffer_end_abs = self.timestamp_offset_ms + self.current_buffer_duration_ms();
        let commit_cutoff = buffer_end_abs.saturating_sub(COMMIT_WINDOW_MS);

        self.tracked_words
            .iter()
            .filter(|w| w.stability >= MIN_STABILITY_COUNT && w.end_ms <= commit_cutoff)
            .collect()
    }

    /// Analyze words and determine which are stable enough to commit
    pub fn analyze_words(&self, _words: &[TimedWord]) -> AlignmentResult {
        let committable = self.get_committable_words();

        let stable_text: String = committable
            .iter()
            .map(|w| w.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let stable_end_ms = committable.last().map(|w| w.end_ms).unwrap_or(0);
        let stable_word_count = committable.len();

        // Find first non-committable word for trim point
        let first_uncommittable = self
            .tracked_words
            .iter()
            .find(|w| w.stability < MIN_STABILITY_COUNT || w.end_ms > stable_end_ms);

        let trim_from_ms = first_uncommittable.map(|w| w.start_ms.saturating_sub(TRIM_PADDING_MS));

        AlignmentResult {
            stable_text,
            stable_word_count,
            trim_from_ms,
            stable_end_ms,
        }
    }

    /// Commit stable text and trim the buffer
    pub fn commit(&mut self, alignment: &AlignmentResult) {
        let committed_ids: std::collections::HashSet<u64> = self
            .get_committable_words()
            .into_iter()
            .map(|w| w.id)
            .collect();

        // Append stable text to committed
        if !alignment.stable_text.is_empty() {
            self.last_committed_delta = Some(alignment.stable_text.trim().to_string());
            if !self.committed_text.is_empty() {
                self.committed_text.push(' ');
            }
            self.committed_text.push_str(&alignment.stable_text);
        }

        // Track end timestamp of committed words
        self.committed_end_ms = alignment.stable_end_ms;

        // Trim buffer and adjust timestamp offset
        if let Some(trim_from_ms) = alignment.trim_from_ms {
            // Convert absolute trim point to buffer-relative
            let trim_rel_ms = trim_from_ms.saturating_sub(self.timestamp_offset_ms);
            let trim_samples = (trim_rel_ms as usize * SAMPLE_RATE) / 1000;

            if trim_samples < self.buffer.len() {
                self.buffer.drain(0..trim_samples);
                self.trimmed_samples_total += trim_samples;
                self.timestamp_offset_ms = trim_from_ms;
            }

            // Remove committed words from tracking
            self.tracked_words.retain(|w| !committed_ids.contains(&w.id));
        } else {
            // No uncommitted words - clear entire buffer
            self.trimmed_samples_total += self.buffer.len();
            let buffer_duration = self.current_buffer_duration_ms();
            self.timestamp_offset_ms += buffer_duration;
            self.buffer.clear();
            self.tracked_words.clear();
        }

        // Reset transcription marker and VAD flag
        self.last_transcription_len = 0;
        self.speech_end_pending = false;
        self.speech_end_transcription_count = 0;
    }

    /// Build display text from committed + non-tail tracked words
    /// Shows:
    /// - stable words (stability >= MIN_STABILITY_COUNT), and
    /// - words that are old enough (ending before buffer_end - DISPLAY_TAIL_MS),
    /// while hiding the most volatile tail unless it stabilizes.
    pub fn build_display_text(&self) -> String {
        let buffer_end_abs = self.timestamp_offset_ms + self.current_buffer_duration_ms();
        let display_cutoff = buffer_end_abs.saturating_sub(DISPLAY_TAIL_MS);

        let displayable: Vec<&TrackedWord> = self
            .tracked_words
            .iter()
            .filter(|w| {
                // Never re-show already committed audio overlap.
                if w.end_ms <= self.committed_end_ms {
                    return false;
                }
                // Show if stable OR old enough to be unlikely to change.
                w.stability >= MIN_STABILITY_COUNT || w.end_ms <= display_cutoff
            })
            .collect();

        let partial_text: String = displayable
            .iter()
            .map(|w| w.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if self.committed_text.is_empty() {
            partial_text
        } else if partial_text.is_empty() {
            self.committed_text.clone()
        } else {
            format!("{} {}", self.committed_text, partial_text)
        }
    }

    /// Build display text including volatile tail words (shown differently in UI)
    pub fn build_full_display_text(&self) -> (String, String) {
        let buffer_end_abs = self.timestamp_offset_ms + self.current_buffer_duration_ms();
        let display_cutoff = buffer_end_abs.saturating_sub(DISPLAY_TAIL_MS);

        // Main text (committed + stable OR old-enough uncommitted)
        let main_text = self.build_display_text();

        // Unstable tail (words not yet stable)
        let tail: Vec<&TrackedWord> = self
            .tracked_words
            .iter()
            .filter(|w| {
                w.end_ms > self.committed_end_ms
                    && w.stability < MIN_STABILITY_COUNT
                    && w.end_ms > display_cutoff
            })
            .collect();

        let tail_text: String = tail
            .iter()
            .map(|w| w.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        (main_text, tail_text)
    }

    /// Reset all state for a new recording
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.committed_text.clear();
        self.last_transcription_len = 0;
        self.speech_end_pending = false;
        self.trimmed_samples_total = 0;
        self.committed_end_ms = 0;
        self.tracked_words.clear();
        self.next_word_id = 0;
        self.timestamp_offset_ms = 0;
        self.speech_end_transcription_count = 0;
        self.last_committed_delta = None;
        if let Some(ref mut vad) = self.vad {
            vad.reset();
        }
    }

    /// Clear word tracking (call after commit if needed)
    pub fn clear_word_cache(&mut self) {
        // With the new tracking system, we retain uncommitted words
        // This method is kept for API compatibility
    }

    pub fn take_last_committed_delta(&mut self) -> Option<String> {
        self.last_committed_delta.take().and_then(|s| {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
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
    fn test_compute_iou() {
        // Perfect overlap
        assert!((StreamingTranscriber::compute_iou(0, 100, 0, 100) - 1.0).abs() < 0.01);

        // 50% overlap
        assert!((StreamingTranscriber::compute_iou(0, 100, 50, 150) - 0.333).abs() < 0.01);

        // No overlap
        assert!((StreamingTranscriber::compute_iou(0, 100, 200, 300) - 0.0).abs() < 0.01);

        // Contained
        assert!((StreamingTranscriber::compute_iou(0, 100, 25, 75) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_words_match() {
        assert!(StreamingTranscriber::words_match("Hello", "hello"));
        assert!(StreamingTranscriber::words_match("Hello,", "hello"));
        assert!(StreamingTranscriber::words_match("don't", "dont"));
        assert!(!StreamingTranscriber::words_match("hello", "world"));
        assert!(StreamingTranscriber::words_match(",", ","));
        assert!(!StreamingTranscriber::words_match(",", "."));
    }

    #[test]
    fn test_word_stability_tracking() {
        let mut transcriber = StreamingTranscriber::new();

        // First decode
        let words1 = vec![
            make_word("Hello", 0, 500),
            make_word("world", 500, 1000),
        ];
        transcriber.update_words(&words1);

        // All words have stability 1
        assert!(transcriber.tracked_words.iter().all(|w| w.stability == 1));

        // Second decode with same words at similar timestamps
        let words2 = vec![
            make_word("Hello", 0, 500),
            make_word("world", 500, 1000),
        ];
        transcriber.update_words(&words2);

        // Words should now have stability 2
        assert!(transcriber.tracked_words.iter().all(|w| w.stability == 2));
        assert_eq!(transcriber.get_stable_words().len(), 2);
    }

    #[test]
    fn test_word_change_resets_stability() {
        let mut transcriber = StreamingTranscriber::new();

        // Build up stability
        for _ in 0..3 {
            let words = vec![make_word("Hello", 0, 500)];
            transcriber.update_words(&words);
        }
        assert!(transcriber.tracked_words[0].stability >= 3);

        // Change the word
        let words = vec![make_word("Help", 0, 500)];
        transcriber.update_words(&words);

        // Stability should reset
        assert_eq!(transcriber.tracked_words[0].stability, 1);
    }

    #[test]
    fn test_timestamp_alignment() {
        let mut transcriber = StreamingTranscriber::new();

        // First decode
        let words1 = vec![
            make_word("Hello", 0, 500),
            make_word("world", 500, 1000),
        ];
        transcriber.update_words(&words1);
        let id1 = transcriber.tracked_words[0].id;
        let id2 = transcriber.tracked_words[1].id;

        // Second decode with slightly shifted timestamps
        let words2 = vec![
            make_word("Hello", 50, 550),  // Shifted by 50ms
            make_word("world", 550, 1050),
        ];
        transcriber.update_words(&words2);

        // IDs should be preserved (words aligned by timestamp)
        assert_eq!(transcriber.tracked_words[0].id, id1);
        assert_eq!(transcriber.tracked_words[1].id, id2);
        assert!(transcriber.tracked_words.iter().all(|w| w.stability == 2));
    }

    #[test]
    fn test_build_display_text() {
        let mut transcriber = StreamingTranscriber::new();

        // Add some buffer so words can fall outside the tail window.
        let samples_2sec = vec![0.0f32; 32000]; // 2 seconds at 16kHz
        transcriber.add_samples(&samples_2sec);

        // First decode: words are old enough (outside tail), so they should display.
        let words = vec![
            make_word("Hello", 0, 500),
            make_word("world", 500, 1000),
        ];
        transcriber.update_words(&words);

        let text = transcriber.build_display_text();
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn test_committed_words_excluded_from_display() {
        let mut transcriber = StreamingTranscriber::new();
        transcriber.committed_text = "Hello".to_string();
        transcriber.committed_end_ms = 500;

        // Add enough buffer to have words outside the volatile tail
        let samples_2sec = vec![0.0f32; 32000]; // 2 seconds at 16kHz
        transcriber.add_samples(&samples_2sec);

        // Add tracked words:
        // - "Hello" overlaps with committed (starts at 0, before committed_end_ms=500)
        // - "world" starts at committed boundary (500) - should be shown
        transcriber.tracked_words = vec![
            TrackedWord {
                text: "Hello".to_string(),
                start_ms: 0,
                end_ms: 500,
                stability: 3,
                id: 0,
            },
            TrackedWord {
                text: "world".to_string(),
                start_ms: 500,
                end_ms: 1000,
                stability: 3,
                id: 1,
            },
        ];

        let text = transcriber.build_display_text();
        // Should show "Hello world", not "Hello Hello world"
        // "Hello" tracked word is excluded because start_ms < committed_end_ms
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn test_reset_clears_all_state() {
        let mut transcriber = StreamingTranscriber::new();
        transcriber.add_samples(&vec![0.0; 1000]);
        transcriber.committed_text = "some text".to_string();
        transcriber.timestamp_offset_ms = 5000;
        transcriber.next_word_id = 100;

        transcriber.reset();

        assert!(transcriber.buffer.is_empty());
        assert!(transcriber.committed_text.is_empty());
        assert_eq!(transcriber.timestamp_offset_ms, 0);
        assert_eq!(transcriber.next_word_id, 0);
        assert!(transcriber.tracked_words.is_empty());
    }
}
