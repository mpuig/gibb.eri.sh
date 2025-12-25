//! Word stability tracking for streaming transcription.

use crate::constants::TRIM_PADDING_MS;

/// Represents a word with timing information (from transcription).
#[derive(Debug, Clone)]
pub struct TimedWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// A word with stability tracking across decodes.
#[derive(Debug, Clone)]
pub struct TrackedWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    /// How many consecutive decodes this word appeared with similar text.
    pub stability: u8,
    /// Unique ID for tracking identity across decodes.
    pub id: u64,
}

/// Result of processing a transcription with word alignment.
#[derive(Debug, Clone)]
pub struct AlignmentResult {
    /// Text to commit (stable words).
    pub stable_text: String,
    /// Number of stable words found.
    pub stable_word_count: usize,
    /// Timestamp (ms) to trim buffer from.
    pub trim_from_ms: Option<u64>,
    /// End timestamp (ms) of the last stable word.
    pub stable_end_ms: u64,
}

/// Minimum stability count before a word is considered stable.
const MIN_STABILITY_COUNT: u8 = 2;

/// Minimum IoU (Intersection over Union) for timestamp-based word matching.
const MIN_TIMESTAMP_IOU: f32 = 0.3;

/// Words ending this many ms before buffer end are candidates for commit.
const COMMIT_WINDOW_MS: u64 = 500;

/// Don't show words that end within this tail window unless they're stable.
const DISPLAY_TAIL_MS: u64 = 600;

/// Tracks word stability across transcription decodes.
#[derive(Debug, Default)]
pub struct WordTracker {
    tracked_words: Vec<TrackedWord>,
    next_word_id: u64,
    committed_text: String,
    committed_end_ms: u64,
    /// Text committed on the most recent commit (cleared after read).
    last_committed_delta: Option<String>,
    /// Insert a paragraph break before the next displayed/committed words.
    paragraph_break_pending: bool,
}

impl WordTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the committed text.
    pub fn committed_text(&self) -> &str {
        &self.committed_text
    }

    /// Get the end timestamp of committed content.
    #[allow(dead_code)]
    pub fn committed_end_ms(&self) -> u64 {
        self.committed_end_ms
    }

    pub fn set_paragraph_break_pending(&mut self) {
        self.paragraph_break_pending = true;
    }

    /// Update tracked words with new transcription.
    pub fn update(&mut self, new_words: &[TimedWord], timestamp_offset_ms: u64) {
        // Don't wipe tracking on empty results - likely a transient model glitch
        if new_words.is_empty() && !self.tracked_words.is_empty() {
            tracing::warn!(
                tracked_count = self.tracked_words.len(),
                "Ignoring empty transcription, preserving tracked words"
            );
            return;
        }

        let old_count = self.tracked_words.len();
        self.tracked_words = self.align_and_track(new_words, timestamp_offset_ms);

        let matched_count = self
            .tracked_words
            .iter()
            .filter(|w| w.stability > 1)
            .count();
        let new_count = self
            .tracked_words
            .iter()
            .filter(|w| w.stability == 1)
            .count();

        tracing::debug!(
            old_tracked = old_count,
            new_words = new_words.len(),
            matched = matched_count,
            new = new_count,
            offset_ms = timestamp_offset_ms,
            "word_alignment_result"
        );
    }

    /// Analyze words and determine which are stable enough to commit.
    pub fn analyze(&self, buffer_end_abs_ms: u64) -> AlignmentResult {
        let committable = self.get_committable_words(buffer_end_abs_ms);

        let stable_text: String = committable
            .iter()
            .map(|w| w.text.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let stable_end_ms = committable.last().map(|w| w.end_ms).unwrap_or(0);
        let stable_word_count = committable.len();

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

    /// Commit stable text.
    pub fn commit(&mut self, alignment: &AlignmentResult, buffer_end_abs_ms: u64) {
        let committed_ids: std::collections::HashSet<u64> = self
            .get_committable_words(buffer_end_abs_ms)
            .into_iter()
            .map(|w| w.id)
            .collect();

        if !alignment.stable_text.is_empty() {
            self.last_committed_delta = Some(alignment.stable_text.trim().to_string());
            if self.paragraph_break_pending && !self.committed_text.is_empty() {
                self.committed_text.push_str("\n\n");
                self.paragraph_break_pending = false;
            } else if !self.committed_text.is_empty() {
                self.committed_text.push(' ');
            }
            self.committed_text.push_str(&alignment.stable_text);
        }

        self.committed_end_ms = alignment.stable_end_ms;

        if alignment.trim_from_ms.is_some() {
            self.tracked_words
                .retain(|w| !committed_ids.contains(&w.id));
        } else {
            self.tracked_words.clear();
        }
    }

    /// Commit text directly (for engines without word-level timestamps).
    ///
    /// This is used by batch engines like Whisper that return segment text
    /// without individual word timings.
    pub fn commit_text(&mut self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }

        self.last_committed_delta = Some(trimmed.to_string());

        if self.paragraph_break_pending && !self.committed_text.is_empty() {
            self.committed_text.push_str("\n\n");
            self.paragraph_break_pending = false;
        } else if !self.committed_text.is_empty() {
            self.committed_text.push(' ');
        }

        self.committed_text.push_str(trimmed);
        self.tracked_words.clear();
    }

    /// Build display text from committed + non-tail tracked words.
    pub fn build_display_text(&self, buffer_end_abs_ms: u64) -> String {
        let display_cutoff = buffer_end_abs_ms.saturating_sub(DISPLAY_TAIL_MS);

        let displayable: Vec<&TrackedWord> = self
            .tracked_words
            .iter()
            .filter(|w| {
                if w.end_ms <= self.committed_end_ms {
                    return false;
                }
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
        } else if self.paragraph_break_pending {
            format!("{}\n\n{}", self.committed_text, partial_text)
        } else {
            format!("{} {}", self.committed_text, partial_text)
        }
    }

    /// Build display text including volatile tail words.
    pub fn build_full_display_text(&self, buffer_end_abs_ms: u64) -> (String, String) {
        let display_cutoff = buffer_end_abs_ms.saturating_sub(DISPLAY_TAIL_MS);
        let main_text = self.build_display_text(buffer_end_abs_ms);

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

    /// Take the last committed delta (consumes it).
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

    /// Reset all state for a new recording.
    pub fn reset(&mut self) {
        self.tracked_words.clear();
        self.next_word_id = 0;
        self.committed_text.clear();
        self.committed_end_ms = 0;
        self.last_committed_delta = None;
        self.paragraph_break_pending = false;
    }

    /// Clear word cache (API compatibility).
    pub fn clear_cache(&mut self) {
        // With the new tracking system, we retain uncommitted words
    }

    // --- Private helpers ---

    fn get_committable_words(&self, buffer_end_abs_ms: u64) -> Vec<&TrackedWord> {
        let commit_cutoff = buffer_end_abs_ms.saturating_sub(COMMIT_WINDOW_MS);

        self.tracked_words
            .iter()
            .filter(|w| w.stability >= MIN_STABILITY_COUNT && w.end_ms <= commit_cutoff)
            .collect()
    }

    #[cfg(test)]
    fn get_stable_words(&self) -> Vec<&TrackedWord> {
        self.tracked_words
            .iter()
            .filter(|w| w.stability >= MIN_STABILITY_COUNT)
            .collect()
    }

    fn align_and_track(
        &mut self,
        new_words: &[TimedWord],
        timestamp_offset_ms: u64,
    ) -> Vec<TrackedWord> {
        let mut result = Vec::with_capacity(new_words.len());
        let mut used_prev_indices: Vec<bool> = vec![false; self.tracked_words.len()];

        if !new_words.is_empty() {
            let words_summary: Vec<String> = new_words
                .iter()
                .take(5)
                .map(|w| format!("{}@{}-{}", w.text, w.start_ms, w.end_ms))
                .collect();
            tracing::debug!(
                words = ?words_summary,
                offset_ms = timestamp_offset_ms,
                prev_tracked = self.tracked_words.len(),
                "align_incoming_words"
            );
        }

        for new_word in new_words {
            let new_abs_start = new_word.start_ms + timestamp_offset_ms;
            let new_abs_end = new_word.end_ms + timestamp_offset_ms;

            let mut best_match: Option<(usize, f32)> = None;

            for (i, prev_word) in self.tracked_words.iter().enumerate() {
                if used_prev_indices[i] {
                    continue;
                }

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

                if Self::words_match(&prev_word.text, &new_word.text) {
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

    fn normalize_text(text: &str) -> String {
        text.trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    }

    fn words_match(a: &str, b: &str) -> bool {
        let norm_a = Self::normalize_text(a);
        let norm_b = Self::normalize_text(b);

        if norm_a.is_empty() && norm_b.is_empty() {
            return a.trim() == b.trim();
        }

        if norm_a.is_empty() || norm_b.is_empty() {
            return false;
        }

        norm_a == norm_b
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
        assert!((WordTracker::compute_iou(0, 100, 0, 100) - 1.0).abs() < 0.01);
        assert!((WordTracker::compute_iou(0, 100, 50, 150) - 0.333).abs() < 0.01);
        assert!((WordTracker::compute_iou(0, 100, 200, 300) - 0.0).abs() < 0.01);
        assert!((WordTracker::compute_iou(0, 100, 25, 75) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_words_match() {
        assert!(WordTracker::words_match("Hello", "hello"));
        assert!(WordTracker::words_match("Hello,", "hello"));
        assert!(WordTracker::words_match("don't", "dont"));
        assert!(!WordTracker::words_match("hello", "world"));
        assert!(WordTracker::words_match(",", ","));
        assert!(!WordTracker::words_match(",", "."));
    }

    #[test]
    fn test_word_stability_tracking() {
        let mut tracker = WordTracker::new();

        let words1 = vec![make_word("Hello", 0, 500), make_word("world", 500, 1000)];
        tracker.update(&words1, 0);
        assert!(tracker.tracked_words.iter().all(|w| w.stability == 1));

        let words2 = vec![make_word("Hello", 0, 500), make_word("world", 500, 1000)];
        tracker.update(&words2, 0);
        assert!(tracker.tracked_words.iter().all(|w| w.stability == 2));
        assert_eq!(tracker.get_stable_words().len(), 2);
    }

    #[test]
    fn test_word_change_resets_stability() {
        let mut tracker = WordTracker::new();

        for _ in 0..3 {
            let words = vec![make_word("Hello", 0, 500)];
            tracker.update(&words, 0);
        }
        assert!(tracker.tracked_words[0].stability >= 3);

        let words = vec![make_word("Help", 0, 500)];
        tracker.update(&words, 0);
        assert_eq!(tracker.tracked_words[0].stability, 1);
    }

    #[test]
    fn test_reset_clears_all() {
        let mut tracker = WordTracker::new();
        tracker.committed_text = "some text".to_string();
        tracker.next_word_id = 100;

        tracker.reset();

        assert!(tracker.committed_text().is_empty());
        assert_eq!(tracker.next_word_id, 0);
        assert!(tracker.tracked_words.is_empty());
    }
}
