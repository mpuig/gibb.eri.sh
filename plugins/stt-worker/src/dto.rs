use serde::{Deserialize, Serialize};

/// Transcript segment returned from transcription
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegmentDto {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<i32>,
}

impl From<gibberish_application::TranscriptSegment> for TranscriptSegmentDto {
    fn from(seg: gibberish_application::TranscriptSegment) -> Self {
        Self {
            text: seg.text,
            start_ms: seg.start_ms,
            end_ms: seg.end_ms,
            speaker: seg.speaker,
        }
    }
}

/// Model information for listing available models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfoDto {
    pub name: String,
    pub dir_name: String,
    pub is_downloaded: bool,
    pub size_bytes: u64,
    /// Supported language codes. Empty means multilingual with auto-detect.
    pub supported_languages: Vec<String>,
}

impl From<crate::services::ModelInfo> for ModelInfoDto {
    fn from(info: crate::services::ModelInfo) -> Self {
        Self {
            name: info.name,
            dir_name: info.dir_name,
            is_downloaded: info.is_downloaded,
            size_bytes: info.size_bytes,
            supported_languages: info.supported_languages,
        }
    }
}

/// Turn model information for listing available endpoint detectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnModelInfoDto {
    pub name: String,
    pub dir_name: String,
    pub is_downloaded: bool,
    pub size_bytes: u64,
}

/// Turn detection settings
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TurnSettingsDto {
    pub enabled: bool,
    pub threshold: f32,
}

/// Streaming transcription result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingResultDto {
    pub text: String,
    pub volatile_text: String,
    pub is_partial: bool,
    pub buffer_duration_ms: u64,
}

impl From<gibberish_application::StreamingResult> for StreamingResultDto {
    fn from(result: gibberish_application::StreamingResult) -> Self {
        Self {
            text: result.text,
            volatile_text: result.volatile_text,
            is_partial: result.is_partial,
            buffer_duration_ms: result.buffer_duration_ms,
        }
    }
}

/// Session summary for listing sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummaryDto {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub duration_ms: u64,
    pub preview: String,
}

/// Full session with segments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDto {
    pub id: String,
    pub title: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub duration_ms: u64,
    pub segments: Vec<SessionSegmentDto>,
}

/// Session segment for save/get operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSegmentDto {
    pub id: String,
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: Option<i32>,
}

// --- Event Payloads ---
// These are emitted via Tauri events to the frontend

/// Payload for stt:stream_commit events
#[derive(Debug, Clone, Serialize)]
pub struct StreamingCommitPayload {
    pub text: String,
    pub ts_ms: i64,
}

/// Payload for stt:turn_prediction events
#[derive(Debug, Clone, Serialize)]
pub struct TurnPredictionPayload {
    pub probability: f32,
    pub threshold: f32,
    pub is_complete: bool,
    pub ts_ms: i64,
}

/// Payload for stt:vad_silence events (speech-to-silence transition)
/// Emitted by any STT engine when VAD detects end of speech.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadSilencePayload {
    pub ts_ms: i64,
    pub buffer_duration_ms: u64,
}
