//! Error types for skill parsing and execution.

use std::path::PathBuf;
use thiserror::Error;

/// Result type for skill operations.
pub type SkillResult<T> = Result<T, SkillError>;

/// Errors that can occur during skill parsing or execution.
#[derive(Debug, Error)]
pub enum SkillError {
    /// Failed to read skill file.
    #[error("Failed to read skill file '{path}': {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Invalid YAML frontmatter.
    #[error("Invalid YAML frontmatter in '{path}': {message}")]
    InvalidFrontmatter { path: PathBuf, message: String },

    /// Missing required frontmatter field.
    #[error("Missing required field '{field}' in '{path}'")]
    MissingField { path: PathBuf, field: String },

    /// No tool definitions found.
    #[error("No tool definitions found in '{path}'")]
    NoTools { path: PathBuf },

    /// Invalid tool definition.
    #[error("Invalid tool '{tool}' in '{path}': {message}")]
    InvalidTool {
        path: PathBuf,
        tool: String,
        message: String,
    },

    /// Duplicate tool name.
    #[error("Duplicate tool name '{tool}' in '{path}'")]
    DuplicateTool { path: PathBuf, tool: String },

    /// Invalid command template.
    #[error("Invalid command in tool '{tool}': {message}")]
    InvalidCommand { tool: String, message: String },

    /// Missing required parameter during execution.
    #[error("Missing required parameter '{param}' for tool '{tool}'")]
    MissingParameter { tool: String, param: String },

    /// Invalid parameter value.
    #[error("Invalid value for parameter '{param}': expected {expected}")]
    InvalidParameterValue { param: String, expected: String },

    /// Command execution failed.
    #[error("Command execution failed: {message}")]
    ExecutionFailed { message: String },

    /// Command timed out.
    #[error("Command timed out after {seconds}s")]
    Timeout { seconds: u32 },

    /// Command not found.
    #[error("Command not found: {program}")]
    CommandNotFound { program: String },

    /// Permission denied.
    #[error("Permission denied: {path}")]
    PermissionDenied { path: String },
}
