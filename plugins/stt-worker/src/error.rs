use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum SttError {
    #[error("No model loaded")]
    NoModelLoaded,

    #[error("Database not initialized")]
    DatabaseNotInitialized,

    #[error("Download already in progress")]
    DownloadInProgress,

    #[error("Model is not being downloaded")]
    NotDownloading,

    #[error("Invalid model name: {0}")]
    InvalidModelName(String),

    #[error("Model error: {0}")]
    Model(String),

    #[error("Transcription error: {0}")]
    Transcription(String),

    #[error("Turn detection error: {0}")]
    Turn(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Invalid UUID: {0}")]
    InvalidUuid(String),
}

impl Serialize for SttError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<gibberish_models::ModelError> for SttError {
    fn from(e: gibberish_models::ModelError) -> Self {
        SttError::Model(e.to_string())
    }
}

impl From<gibberish_application::TranscriptionError> for SttError {
    fn from(e: gibberish_application::TranscriptionError) -> Self {
        SttError::Transcription(e.to_string())
    }
}

impl From<gibberish_turn::TurnError> for SttError {
    fn from(e: gibberish_turn::TurnError) -> Self {
        SttError::Turn(e.to_string())
    }
}

impl From<gibberish_storage::StorageError> for SttError {
    fn from(e: gibberish_storage::StorageError) -> Self {
        SttError::Database(e.to_string())
    }
}

impl From<uuid::Error> for SttError {
    fn from(e: uuid::Error) -> Self {
        SttError::InvalidUuid(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SttError>;
