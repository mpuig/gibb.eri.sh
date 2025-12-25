use std::time::Duration;

pub use silero::{VadConfig, VadSession, VadTransition};

/// User-configurable VAD settings.
#[derive(Debug, Clone, Copy)]
pub struct VadSettings {
    /// Time to wait (ms) after speech ends before finalizing (500 = default).
    /// Lower values give faster commits but may cut mid-sentence pauses.
    pub redemption_time_ms: u32,
    /// Minimum speech duration (ms) to trigger detection (100 = default).
    pub min_speech_time_ms: u32,
}

impl Default for VadSettings {
    fn default() -> Self {
        Self {
            redemption_time_ms: 500,
            min_speech_time_ms: 100,
        }
    }
}

impl VadSettings {
    /// Responsive mode (250ms). Matches Sherpa C++ example.
    /// Best for fast speakers, may cut mid-pause for slower speakers.
    pub fn responsive() -> Self {
        Self {
            redemption_time_ms: 250,
            min_speech_time_ms: 100,
        }
    }

    /// Create dictation-mode settings (fast commits).
    pub fn dictation() -> Self {
        Self {
            redemption_time_ms: 300,
            min_speech_time_ms: 100,
        }
    }

    /// Create meeting-mode settings (longer pauses allowed).
    pub fn meeting() -> Self {
        Self {
            redemption_time_ms: 1000,
            min_speech_time_ms: 150,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VadError {
    #[error("model not loaded")]
    ModelNotLoaded,
    #[error("session creation failed")]
    SessionCreationFailed,
    #[error("inference error: {0}")]
    InferenceError(String),
}

pub type Result<T> = std::result::Result<T, VadError>;

#[derive(Debug, Clone)]
pub struct SpeechSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub samples: Vec<f32>,
}

pub struct SileroVad {
    session: VadSession,
    sample_rate: u32,
}

impl SileroVad {
    pub fn new(sample_rate: u32) -> Result<Self> {
        Self::with_settings(sample_rate, VadSettings::default())
    }

    pub fn with_settings(sample_rate: u32, settings: VadSettings) -> Result<Self> {
        let config = VadConfig {
            sample_rate: sample_rate as usize,
            redemption_time: Duration::from_millis(settings.redemption_time_ms as u64),
            pre_speech_pad: Duration::from_millis(200),
            post_speech_pad: Duration::from_millis(200),
            min_speech_time: Duration::from_millis(settings.min_speech_time_ms as u64),
            ..Default::default()
        };

        let session = VadSession::new(config).map_err(|_| VadError::SessionCreationFailed)?;

        Ok(Self {
            session,
            sample_rate,
        })
    }

    pub fn with_config(config: VadConfig) -> Result<Self> {
        let sample_rate = config.sample_rate as u32;
        let session = VadSession::new(config).map_err(|_| VadError::SessionCreationFailed)?;

        Ok(Self {
            session,
            sample_rate,
        })
    }

    pub fn process(&mut self, samples: &[f32]) -> Result<Vec<VadEvent>> {
        let transitions = self
            .session
            .process(samples)
            .map_err(|e| VadError::InferenceError(e.to_string()))?;

        Ok(transitions
            .into_iter()
            .map(|t| match t {
                VadTransition::SpeechStart { timestamp_ms } => VadEvent::SpeechStart {
                    timestamp_ms: timestamp_ms as u64,
                },
                VadTransition::SpeechEnd {
                    start_timestamp_ms,
                    end_timestamp_ms,
                    samples,
                } => VadEvent::SpeechEnd {
                    start_ms: start_timestamp_ms as u64,
                    end_ms: end_timestamp_ms as u64,
                    samples,
                },
            })
            .collect())
    }

    pub fn reset(&mut self) {
        self.session.reset();
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[derive(Debug, Clone)]
pub enum VadEvent {
    SpeechStart {
        timestamp_ms: u64,
    },
    SpeechEnd {
        start_ms: u64,
        end_ms: u64,
        samples: Vec<f32>,
    },
}

pub trait VoiceActivityDetector: Send + Sync {
    fn detect(&mut self, audio: &[f32]) -> Result<Vec<VadEvent>>;
    fn reset(&mut self);
}

impl VoiceActivityDetector for SileroVad {
    fn detect(&mut self, audio: &[f32]) -> Result<Vec<VadEvent>> {
        self.process(audio)
    }

    fn reset(&mut self) {
        self.session.reset();
    }
}
