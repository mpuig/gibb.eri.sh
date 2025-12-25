//! Generic download tracking for model downloads.
//!
//! Provides cancellation token management for concurrent downloads.

use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Tracks active downloads with cancellation support.
#[derive(Debug, Default)]
pub struct DownloadTracker {
    tokens: RwLock<HashMap<String, CancellationToken>>,
}

impl DownloadTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a download is in progress.
    pub async fn has(&self, name: &str) -> bool {
        self.tokens.read().await.contains_key(name)
    }

    /// Start a new download and return its cancellation token.
    pub async fn start(&self, name: String) -> CancellationToken {
        let token = CancellationToken::new();
        let mut tokens = self.tokens.write().await;
        tokens.insert(name, token.clone());
        token
    }

    /// Cancel a download if it exists.
    ///
    /// Returns true if the download was found and cancelled.
    pub async fn cancel(&self, name: &str) -> bool {
        let tokens = self.tokens.read().await;
        if let Some(token) = tokens.get(name) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Mark a download as finished (removes the token).
    pub async fn finish(&self, name: &str) {
        let mut tokens = self.tokens.write().await;
        tokens.remove(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_has_returns_false_initially() {
        let tracker = DownloadTracker::new();
        assert!(!tracker.has("test-model").await);
    }

    #[tokio::test]
    async fn test_start_registers_download() {
        let tracker = DownloadTracker::new();
        let _token = tracker.start("test-model".to_string()).await;
        assert!(tracker.has("test-model").await);
    }

    #[tokio::test]
    async fn test_finish_removes_download() {
        let tracker = DownloadTracker::new();
        let _token = tracker.start("test-model".to_string()).await;
        tracker.finish("test-model").await;
        assert!(!tracker.has("test-model").await);
    }

    #[tokio::test]
    async fn test_cancel_returns_true_for_active_download() {
        let tracker = DownloadTracker::new();
        let token = tracker.start("test-model".to_string()).await;
        assert!(!token.is_cancelled());

        let cancelled = tracker.cancel("test-model").await;
        assert!(cancelled);
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancel_returns_false_for_unknown_download() {
        let tracker = DownloadTracker::new();
        let cancelled = tracker.cancel("unknown-model").await;
        assert!(!cancelled);
    }
}
