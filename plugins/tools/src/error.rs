use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum ToolsError {
    #[error("Wikipedia fetch error: {0}")]
    WikipediaFetch(String),

    #[error("FunctionGemma model not loaded")]
    ModelNotLoaded,

    #[error("FunctionGemma model not downloaded")]
    ModelNotDownloaded,

    #[error("Download already in progress: {0}")]
    DownloadInProgress(String),

    #[error("Unsupported model variant: {0}")]
    UnsupportedVariant(String),

    #[error("Model load error: {0}")]
    ModelLoad(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Tool manifest error: {0}")]
    ToolManifest(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Download error: {0}")]
    Download(String),
}

impl Serialize for ToolsError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for ToolsError {
    fn from(e: std::io::Error) -> Self {
        ToolsError::Io(e.to_string())
    }
}

impl From<crate::functiongemma_download::DownloadError> for ToolsError {
    fn from(e: crate::functiongemma_download::DownloadError) -> Self {
        ToolsError::Download(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ToolsError>;
