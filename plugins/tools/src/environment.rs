//! System environment abstraction for testability.
//!
//! Abstracts OS capabilities behind a trait, allowing tools to be unit-tested
//! without actually executing system commands.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

/// Result type for command execution.
pub type CommandResult = Result<CommandOutput, CommandError>;

/// Output from a system command.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// Error during command execution.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CommandError {
    #[error("command not found: {0}")]
    NotFound(String),

    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),
}

/// Abstraction over system capabilities for testability.
///
/// This trait allows tools to execute OS commands without being tightly coupled
/// to `std::process::Command` or `tokio::process::Command`.
pub trait SystemEnvironment: Send + Sync {
    /// Get the HTTP client for network requests.
    fn http_client(&self) -> &reqwest::Client;

    /// Get the current working directory.
    fn current_dir(&self) -> PathBuf;

    /// Get the user's home directory.
    fn home_dir(&self) -> Option<PathBuf>;

    /// Execute a system command asynchronously.
    fn execute_command(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = CommandResult> + Send + '_>>;

    /// Check if a path is safe (under home directory).
    fn is_safe_path(&self, path: &str) -> bool {
        let p = std::path::Path::new(path);
        if !p.is_absolute() {
            return true;
        }
        if let Some(home) = self.home_dir() {
            return p.starts_with(&home);
        }
        false
    }
}

/// Production implementation using real system calls.
pub struct RealSystemEnvironment {
    client: reqwest::Client,
}

impl RealSystemEnvironment {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

impl SystemEnvironment for RealSystemEnvironment {
    fn http_client(&self) -> &reqwest::Client {
        &self.client
    }

    fn current_dir(&self) -> PathBuf {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    fn home_dir(&self) -> Option<PathBuf> {
        dirs::home_dir()
    }

    fn execute_command(
        &self,
        program: &str,
        args: &[&str],
        cwd: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = CommandResult> + Send + '_>> {
        let program = program.to_string();
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let cwd = cwd.map(String::from);

        Box::pin(async move {
            let mut cmd = tokio::process::Command::new(&program);
            cmd.args(&args);

            if let Some(dir) = &cwd {
                cmd.current_dir(dir);
            }

            let output = cmd.output().await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    CommandError::NotFound(program.clone())
                } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                    CommandError::PermissionDenied(program.clone())
                } else {
                    CommandError::ExecutionFailed(e.to_string())
                }
            })?;

            Ok(CommandOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                success: output.status.success(),
            })
        })
    }
}

#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Mock implementation for testing.
    pub struct MockSystemEnvironment {
        client: reqwest::Client,
        home: PathBuf,
        cwd: PathBuf,
        /// Predefined responses: (program, args) -> result
        responses: Mutex<HashMap<String, CommandResult>>,
    }

    impl MockSystemEnvironment {
        pub fn new() -> Self {
            Self {
                client: reqwest::Client::new(),
                home: PathBuf::from("/Users/test"),
                cwd: PathBuf::from("/Users/test/project"),
                responses: Mutex::new(HashMap::new()),
            }
        }

        /// Add a mock response for a command.
        pub fn mock_command(&self, program: &str, response: CommandResult) {
            self.responses
                .lock()
                .unwrap()
                .insert(program.to_string(), response);
        }
    }

    impl Default for MockSystemEnvironment {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SystemEnvironment for MockSystemEnvironment {
        fn http_client(&self) -> &reqwest::Client {
            &self.client
        }

        fn current_dir(&self) -> PathBuf {
            self.cwd.clone()
        }

        fn home_dir(&self) -> Option<PathBuf> {
            Some(self.home.clone())
        }

        fn execute_command(
            &self,
            program: &str,
            _args: &[&str],
            _cwd: Option<&str>,
        ) -> Pin<Box<dyn Future<Output = CommandResult> + Send + '_>> {
            let response = self
                .responses
                .lock()
                .unwrap()
                .get(program)
                .cloned()
                .unwrap_or_else(|| {
                    Ok(CommandOutput {
                        stdout: String::new(),
                        stderr: String::new(),
                        success: true,
                    })
                });

            Box::pin(async move { response })
        }
    }
}
