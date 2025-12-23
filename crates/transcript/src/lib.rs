use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Repository trait for transcript persistence.
/// Implemented by storage layer, allowing domain to remain decoupled.
pub trait TranscriptRepository: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    fn save(&self, transcript: &Transcript) -> Result<(), Self::Error>;
    fn get(&self, id: &Uuid) -> Result<Transcript, Self::Error>;
    fn list(&self) -> Result<Vec<Transcript>, Self::Error>;
    fn delete(&self, id: &Uuid) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: Uuid,
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub words: Vec<Word>,
    pub speaker: Option<i32>,
    pub is_final: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub id: Uuid,
    pub title: Option<String>,
    pub segments: Vec<Segment>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub duration_ms: u64,
}

impl Transcript {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: None,
            segments: Vec::new(),
            created_at: now,
            updated_at: now,
            duration_ms: 0,
        }
    }

    pub fn full_text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Default for Transcript {
    fn default() -> Self {
        Self::new()
    }
}
