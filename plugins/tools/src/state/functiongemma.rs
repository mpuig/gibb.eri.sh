//! FunctionGemma model state.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct FunctionGemmaModel {
    pub variant: String,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub runner: Arc<crate::functiongemma::FunctionGemmaRunner>,
}

#[derive(Debug)]
pub struct FunctionGemmaDownload {
    pub variant: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub current_file: Option<String>,
    pub started_at: Instant,
    pub cancel: CancellationToken,
}

/// State for FunctionGemma model management.
#[derive(Debug, Default)]
pub struct FunctionGemmaState {
    pub model: Option<FunctionGemmaModel>,
    pub download: Option<FunctionGemmaDownload>,
    pub last_error: Option<String>,
}

impl FunctionGemmaState {
    pub fn new() -> Self {
        Self::default()
    }
}
