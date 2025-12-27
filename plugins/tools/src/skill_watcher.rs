//! File watcher for skill hot reloading.
//!
//! Watches skill directories for changes to SKILL.md files and
//! triggers automatic reloading.

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Callback for skill reload events.
pub type ReloadCallback = Box<dyn Fn() + Send + Sync + 'static>;

/// Skill file watcher for hot reloading.
pub struct SkillWatcher {
    /// The watcher instance (kept alive).
    _watcher: RecommendedWatcher,
    /// Paths being watched.
    watched_paths: Vec<PathBuf>,
}

impl SkillWatcher {
    /// Create a new skill watcher that monitors the given directories.
    ///
    /// The callback is invoked when any SKILL.md file changes.
    pub fn new<F>(directories: Vec<PathBuf>, callback: F) -> Result<Self, SkillWatcherError>
    where
        F: Fn() + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |result: Result<Event, notify::Error>| {
                if let Err(e) = tx.send(result) {
                    error!(error = %e, "Failed to send watcher event");
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .map_err(|e| SkillWatcherError::WatcherCreation(e.to_string()))?;

        // Watch each directory
        let mut watched_paths = Vec::new();
        for dir in &directories {
            if dir.exists() {
                match watcher.watch(dir, RecursiveMode::Recursive) {
                    Ok(()) => {
                        info!(path = %dir.display(), "Watching skill directory");
                        watched_paths.push(dir.clone());
                    }
                    Err(e) => {
                        warn!(path = %dir.display(), error = %e, "Failed to watch skill directory");
                    }
                }
            } else {
                debug!(path = %dir.display(), "Skill directory does not exist, skipping");
            }
        }

        // Spawn thread to handle events
        let callback = Box::new(callback);
        thread::spawn(move || {
            Self::event_loop(rx, callback);
        });

        Ok(Self {
            _watcher: watcher,
            watched_paths,
        })
    }

    /// Event processing loop.
    fn event_loop(rx: mpsc::Receiver<Result<Event, notify::Error>>, callback: ReloadCallback) {
        // Debounce: track last reload time
        let mut last_reload = std::time::Instant::now();
        let debounce_duration = Duration::from_millis(500);

        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    // Check if any affected path is a SKILL.md file
                    let is_skill_change = event.paths.iter().any(|p| {
                        p.file_name()
                            .map(|n| n == "SKILL.md")
                            .unwrap_or(false)
                    });

                    if is_skill_change {
                        // Debounce: only reload if enough time has passed
                        let now = std::time::Instant::now();
                        if now.duration_since(last_reload) >= debounce_duration {
                            info!(paths = ?event.paths, "SKILL.md changed, triggering reload");
                            callback();
                            last_reload = now;
                        } else {
                            debug!("Skipping reload due to debounce");
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!(error = %e, "File watcher error");
                }
                Err(e) => {
                    // Channel closed, watcher was dropped
                    debug!(error = %e, "Watcher channel closed");
                    break;
                }
            }
        }
    }

    /// Get the paths being watched.
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched_paths
    }
}

impl std::fmt::Debug for SkillWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillWatcher")
            .field("watched_paths", &self.watched_paths)
            .finish()
    }
}

/// Get the directories to watch for skills.
pub fn get_skill_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Bundled skills directory
    let cwd_skills = PathBuf::from("skills");
    if cwd_skills.exists() {
        dirs.push(cwd_skills);
    }

    // User skills directory
    if let Some(config_dir) = dirs::config_dir() {
        let user_skills = config_dir.join("gibberish").join("skills");
        dirs.push(user_skills); // Add even if doesn't exist (might be created later)
    }

    dirs
}

/// Error types for skill watcher.
#[derive(Debug, thiserror::Error)]
pub enum SkillWatcherError {
    #[error("Failed to create watcher: {0}")]
    WatcherCreation(String),

    #[error("Failed to watch path: {0}")]
    WatchPath(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_skill_directories() {
        let dirs = get_skill_directories();
        // Should return at least the user skills directory
        assert!(!dirs.is_empty() || std::env::var("HOME").is_err());
    }
}
