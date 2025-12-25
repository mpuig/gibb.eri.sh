//! Voice Activity Detection state management.

use gibberish_turn::TurnPrediction;
use gibberish_vad::{SileroVad, VadEvent};

use crate::constants::SAMPLE_RATE;

pub use gibberish_vad::VadSettings;

/// Tracks VAD state and turn detection for streaming transcription.
pub struct VadState {
    vad: Option<SileroVad>,
    settings: VadSettings,
    in_speech: bool,
    speech_end_pending: bool,
    speech_end_turn_checked: bool,
    /// Number of transcriptions performed since VAD reported a speech end.
    speech_end_transcription_count: u8,
    /// Most recent turn prediction.
    last_turn_prediction: Option<TurnPrediction>,
    /// Timestamp (ms) of the most recently detected end-of-turn boundary.
    last_turn_end_ms: Option<u64>,
    /// Flag indicating silence injection is needed (speech-to-silence transition just occurred).
    silence_injection_pending: bool,
}

impl Default for VadState {
    fn default() -> Self {
        Self::with_settings(VadSettings::default())
    }
}

impl VadState {
    pub fn with_settings(settings: VadSettings) -> Self {
        let vad = SileroVad::with_settings(SAMPLE_RATE as u32, settings).ok();
        if vad.is_none() {
            tracing::warn!("VAD initialization failed, using time-based commits only");
        }
        Self {
            vad,
            settings,
            in_speech: false,
            speech_end_pending: false,
            speech_end_turn_checked: false,
            speech_end_transcription_count: 0,
            last_turn_prediction: None,
            last_turn_end_ms: None,
            silence_injection_pending: false,
        }
    }

    /// Get current VAD settings.
    pub fn settings(&self) -> VadSettings {
        self.settings
    }

    /// Update VAD settings. Takes effect on next reset.
    pub fn set_settings(&mut self, settings: VadSettings) {
        self.settings = settings;
    }

    /// Reinitialize VAD with current settings.
    pub fn reinitialize(&mut self) {
        let new_vad = SileroVad::with_settings(SAMPLE_RATE as u32, self.settings).ok();
        if new_vad.is_none() {
            tracing::warn!("VAD reinitialization failed");
        }
        self.vad = new_vad;
        // Reset state
        self.in_speech = false;
        self.speech_end_pending = false;
        self.speech_end_turn_checked = false;
        self.speech_end_transcription_count = 0;
        self.last_turn_prediction = None;
        self.last_turn_end_ms = None;
        self.silence_injection_pending = false;
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// Process audio samples through VAD.
    pub fn process(&mut self, samples: &[f32]) {
        let Some(ref mut vad) = self.vad else {
            return;
        };

        match vad.process(samples) {
            Ok(events) => {
                for event in events {
                    match event {
                        VadEvent::SpeechStart { .. } => {
                            self.in_speech = true;
                            self.speech_end_pending = false;
                            self.speech_end_transcription_count = 0;
                            self.speech_end_turn_checked = false;
                        }
                        VadEvent::SpeechEnd { .. } => {
                            self.in_speech = false;
                            if !self.speech_end_pending {
                                self.speech_end_pending = true;
                                self.speech_end_transcription_count = 0;
                                self.speech_end_turn_checked = false;
                                self.silence_injection_pending = true;
                                tracing::debug!("VAD detected speech end");
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("VAD processing error: {}", e);
            }
        }
    }

    /// Check if VAD is enabled.
    pub fn is_enabled(&self) -> bool {
        self.vad.is_some()
    }

    /// Check if currently in speech.
    #[allow(dead_code)]
    pub fn in_speech(&self) -> bool {
        self.in_speech
    }

    /// Check if VAD detected a speech end (pause/silence).
    pub fn has_speech_end(&self) -> bool {
        self.speech_end_pending
    }

    /// Check if turn prediction is needed.
    pub fn needs_turn_prediction(&self) -> bool {
        self.speech_end_pending && !self.speech_end_turn_checked
    }

    /// Set the turn prediction result.
    pub fn set_turn_prediction(&mut self, prediction: TurnPrediction) {
        self.last_turn_prediction = Some(prediction);
        self.speech_end_turn_checked = true;

        if !prediction.is_complete() {
            // Not a semantic end-of-turn: treat it like a mid-utterance pause.
            self.speech_end_pending = false;
            self.speech_end_transcription_count = 0;
            self.speech_end_turn_checked = false;
        }
    }

    pub fn is_semantic_turn_end(&self) -> bool {
        self.speech_end_pending
            && self
                .last_turn_prediction
                .as_ref()
                .map(|p| p.is_complete())
                .unwrap_or(false)
    }

    pub fn set_last_turn_end_ms(&mut self, end_ms: u64) {
        self.last_turn_end_ms = Some(end_ms);
    }

    pub fn take_last_turn_end_ms(&mut self) -> Option<u64> {
        self.last_turn_end_ms.take()
    }

    /// Check and consume the silence injection pending flag.
    /// Returns true if silence should be injected to help acoustic model reset.
    pub fn take_silence_injection_pending(&mut self) -> bool {
        std::mem::take(&mut self.silence_injection_pending)
    }

    /// Take the last turn prediction (consumes it).
    pub fn take_last_turn_prediction(&mut self) -> Option<TurnPrediction> {
        self.last_turn_prediction.take()
    }

    /// Check if we should transcribe based on VAD state.
    ///
    /// Returns true if:
    /// - VAD is disabled, OR
    /// - We're in speech, OR
    /// - We have a pending speech end
    pub fn should_transcribe(&self) -> bool {
        if !self.is_enabled() {
            return true;
        }
        self.in_speech || self.speech_end_pending
    }

    /// Check if we should force transcription due to speech end.
    pub fn should_force_transcribe(&self) -> bool {
        self.speech_end_pending && self.speech_end_transcription_count == 0
    }

    /// Mark that a transcription was performed.
    pub fn mark_transcribed(&mut self) {
        if self.speech_end_pending {
            self.speech_end_transcription_count =
                self.speech_end_transcription_count.saturating_add(1);
        }
    }

    /// Check if we should commit based on VAD state.
    pub fn should_commit(&self) -> bool {
        self.speech_end_pending && self.speech_end_transcription_count >= 1
    }

    /// Clear the speech end state after processing.
    pub fn clear_speech_end(&mut self) {
        self.speech_end_pending = false;
        self.speech_end_turn_checked = false;
        self.speech_end_transcription_count = 0;
    }

    /// Reset all state for a new recording.
    pub fn reset(&mut self) {
        self.in_speech = false;
        self.speech_end_pending = false;
        self.speech_end_turn_checked = false;
        self.speech_end_transcription_count = 0;
        self.last_turn_prediction = None;
        self.last_turn_end_ms = None;
        self.silence_injection_pending = false;
        if let Some(ref mut vad) = self.vad {
            vad.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = VadState::new();
        assert!(!state.in_speech());
        assert!(!state.has_speech_end());
        assert!(!state.needs_turn_prediction());
    }

    #[test]
    fn test_should_transcribe_when_disabled() {
        let mut state = VadState::new();
        state.vad = None; // Disable VAD
        assert!(state.should_transcribe());
    }

    #[test]
    fn test_clear_speech_end() {
        let mut state = VadState::new();
        state.speech_end_pending = true;
        state.speech_end_turn_checked = true;
        state.speech_end_transcription_count = 2;

        state.clear_speech_end();

        assert!(!state.has_speech_end());
        assert!(!state.needs_turn_prediction());
    }

    #[test]
    fn test_turn_prediction_cancels_speech_end() {
        let mut state = VadState::new();
        state.speech_end_pending = true;

        let prediction = TurnPrediction {
            probability: 0.3,
            threshold: 0.5, // probability < threshold means continue speaking
        };
        state.set_turn_prediction(prediction);

        // Speech end should be cancelled since turn prediction says "continue"
        assert!(!state.has_speech_end());
    }

    #[test]
    fn test_turn_prediction_keeps_speech_end() {
        let mut state = VadState::new();
        state.speech_end_pending = true;

        let prediction = TurnPrediction {
            probability: 0.7,
            threshold: 0.5, // probability >= threshold means end of turn
        };
        state.set_turn_prediction(prediction);

        // Speech end should remain since turn prediction says "end"
        assert!(state.has_speech_end());
    }
}
