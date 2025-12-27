//! Safe command executor for skills.
//!
//! Executes commands as program + args (no shell) with:
//! - Timeout handling
//! - Output truncation
//! - Process tree cleanup (kill_tree)
//! - PID tracking

use crate::error::{SkillError, SkillResult};
use crate::types::{ArgFragment, CommandTemplate, ToolDefinition};
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::debug;

/// Configuration for command execution.
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Maximum execution time.
    pub timeout_secs: u32,

    /// Maximum output size for head (bytes).
    pub head_size: usize,

    /// Maximum output size for tail (bytes).
    pub tail_size: usize,

    /// Working directory (None = inherit).
    pub working_dir: Option<std::path::PathBuf>,

    /// Environment variables to inject.
    pub env: HashMap<String, String>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 30,
            head_size: 2048,
            tail_size: 2048,
            working_dir: None,
            env: HashMap::new(),
        }
    }
}

/// Output from command execution.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Whether the command succeeded (exit code 0).
    pub success: bool,

    /// Exit code.
    pub exit_code: i32,

    /// Captured output (stdout + stderr merged).
    pub output: String,

    /// Whether output was truncated.
    pub truncated: bool,

    /// Execution duration in milliseconds.
    pub duration_ms: u64,

    /// Process ID (for tracking/cancellation).
    pub pid: Option<u32>,
}

impl CommandOutput {
    /// Convert to JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "success": self.success,
            "exit_code": self.exit_code,
            "output": self.output,
            "truncated": self.truncated,
            "duration_ms": self.duration_ms,
        });

        // Try to parse output as JSON
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&self.output) {
            obj["parsed"] = parsed;
        }

        if !self.success {
            obj["error"] = serde_json::json!(format!("Command failed with exit code {}", self.exit_code));
        }

        obj
    }
}

/// Kill a process and all its children.
///
/// On Unix, this sends SIGTERM to the process group, then SIGKILL if needed.
/// On other platforms, falls back to regular kill.
#[cfg(unix)]
pub fn kill_tree(pid: u32) {
    use std::process::Command as StdCommand;

    debug!(pid, "Killing process tree");

    // First try SIGTERM to the process group (negative PID)
    let pgid = pid as i32;

    // pkill by process group
    let _ = StdCommand::new("pkill")
        .arg("-TERM")
        .arg("-g")
        .arg(pgid.to_string())
        .status();

    // Give processes a moment to clean up
    std::thread::sleep(Duration::from_millis(100));

    // Force kill with SIGKILL if still running
    let _ = StdCommand::new("pkill")
        .arg("-KILL")
        .arg("-g")
        .arg(pgid.to_string())
        .status();

    debug!(pid, "Process tree killed");
}

#[cfg(not(unix))]
pub fn kill_tree(pid: u32) {
    debug!(pid, "Killing process (non-Unix fallback)");
    // On non-Unix, we can't easily kill process trees
    // The process should be killed by kill_on_drop anyway
}

/// Create a new process group for the child process (Unix only).
///
/// This allows us to kill all child processes with kill_tree.
#[cfg(unix)]
fn configure_process_group(cmd: &mut Command) {
    // Create a new process group with the child as leader
    // SAFETY: pre_exec is safe as long as we don't call async code
    unsafe {
        cmd.pre_exec(|| {
            // setpgid(0, 0) creates a new process group with the child's PID
            libc::setpgid(0, 0);
            Ok(())
        });
    }
}

#[cfg(not(unix))]
fn configure_process_group(_cmd: &mut Command) {
    // Process groups not supported on non-Unix
}

/// Execute a tool with the given arguments.
pub async fn execute_tool(
    tool: &ToolDefinition,
    args: &serde_json::Value,
    config: &ExecutorConfig,
) -> SkillResult<CommandOutput> {
    // Interpolate arguments into command
    let (program, cmd_args) = interpolate_command(&tool.command, args, tool)?;

    execute_command(&program, &cmd_args, config).await
}

/// Execute a command directly.
pub async fn execute_command(
    program: &str,
    args: &[String],
    config: &ExecutorConfig,
) -> SkillResult<CommandOutput> {
    let start = std::time::Instant::now();

    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    // Set up process group for clean termination
    configure_process_group(&mut cmd);

    // Set working directory
    if let Some(ref dir) = config.working_dir {
        cmd.current_dir(dir);
    }

    // Set environment
    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    // Sanitize environment (remove secrets)
    for key in &[
        "GITHUB_TOKEN",
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "AWS_SECRET_ACCESS_KEY",
    ] {
        cmd.env_remove(key);
    }

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            SkillError::CommandNotFound {
                program: program.to_string(),
            }
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            SkillError::PermissionDenied {
                path: program.to_string(),
            }
        } else {
            SkillError::ExecutionFailed {
                message: e.to_string(),
            }
        }
    })?;

    // Track the PID for potential cancellation
    let pid = child.id();
    debug!(pid = ?pid, program, "Process spawned");

    // Capture output with timeout
    let timeout_duration = Duration::from_secs(config.timeout_secs as u64);

    let result = timeout(timeout_duration, async {
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let mut output = String::new();
        let max_size = config.head_size + config.tail_size;

        // Read stdout
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if output.len() < max_size {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&line);
                }
            }
        }

        // Read stderr
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if output.len() < max_size {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str(&line);
                }
            }
        }

        let status = child.wait().await;
        (output, status)
    })
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok((output, status)) => {
            let status = status.map_err(|e| SkillError::ExecutionFailed {
                message: e.to_string(),
            })?;

            let truncated = output.len() >= config.head_size + config.tail_size;
            let output = truncate_output(&output, config.head_size, config.tail_size);

            Ok(CommandOutput {
                success: status.success(),
                exit_code: status.code().unwrap_or(-1),
                output,
                truncated,
                duration_ms,
                pid,
            })
        }
        Err(_) => {
            // Timeout - kill the process tree
            if let Some(pid) = child.id() {
                debug!(pid, "Timeout reached, killing process tree");
                kill_tree(pid);
            }
            // Also call kill() to ensure cleanup
            let _ = child.kill().await;
            Err(SkillError::Timeout {
                seconds: config.timeout_secs,
            })
        }
    }
}

/// Interpolate variables into command template.
fn interpolate_command(
    template: &CommandTemplate,
    args: &serde_json::Value,
    tool: &ToolDefinition,
) -> SkillResult<(String, Vec<String>)> {
    let args_map = args.as_object();

    let mut cmd_args = Vec::new();

    for fragment in &template.args {
        match fragment {
            ArgFragment::Literal(s) => {
                cmd_args.push(s.clone());
            }
            ArgFragment::Variable {
                name,
                default,
                flag,
            } => {
                let value = args_map
                    .and_then(|m| m.get(name))
                    .or_else(|| {
                        // Check if parameter has default in definition
                        tool.parameters
                            .iter()
                            .find(|p| &p.name == name)
                            .and_then(|p| p.default.as_ref())
                    });

                match value {
                    Some(v) => {
                        // Handle boolean flags
                        if let Some(flag_str) = flag {
                            if v.as_bool().unwrap_or(false) {
                                cmd_args.push(flag_str.clone());
                            }
                        } else {
                            // Convert value to string
                            let str_val = match v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                serde_json::Value::Array(arr) => arr
                                    .iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(" "),
                                _ => v.to_string(),
                            };
                            cmd_args.push(str_val);
                        }
                    }
                    None => {
                        // Use default if available
                        if let Some(def) = default {
                            cmd_args.push(def.clone());
                        } else {
                            // Check if parameter is required
                            let is_required = tool
                                .parameters
                                .iter()
                                .find(|p| &p.name == name)
                                .map(|p| p.required)
                                .unwrap_or(false);

                            if is_required {
                                return Err(SkillError::MissingParameter {
                                    tool: tool.name.clone(),
                                    param: name.clone(),
                                });
                            }
                            // Optional parameter with no value - skip
                        }
                    }
                }
            }
        }
    }

    Ok((template.program.clone(), cmd_args))
}

/// Truncate output to head + tail with marker.
fn truncate_output(output: &str, head_size: usize, tail_size: usize) -> String {
    let total = head_size + tail_size;
    if output.len() <= total {
        return output.to_string();
    }

    let head = &output[..head_size];
    let tail = &output[output.len() - tail_size..];
    let truncated = output.len() - total;

    format!(
        "{}\n\n... [truncated {} bytes] ...\n\n{}",
        head, truncated, tail
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short() {
        let output = "short";
        let result = truncate_output(output, 100, 100);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_long() {
        let output = "a".repeat(500);
        let result = truncate_output(&output, 100, 100);
        assert!(result.contains("[truncated"));
        assert!(result.len() < 500);
    }

    #[tokio::test]
    async fn test_execute_echo() {
        let config = ExecutorConfig::default();
        let result = execute_command("echo", &["hello".to_string()], &config).await;

        let output = result.unwrap();
        assert!(output.success);
        assert_eq!(output.exit_code, 0);
        assert!(output.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let config = ExecutorConfig::default();
        let result =
            execute_command("nonexistent_command_12345", &[], &config).await;

        assert!(matches!(result, Err(SkillError::CommandNotFound { .. })));
    }

    #[tokio::test]
    async fn test_execute_timeout() {
        let config = ExecutorConfig {
            timeout_secs: 1,
            ..Default::default()
        };
        let result = execute_command("sleep", &["10".to_string()], &config).await;

        assert!(matches!(result, Err(SkillError::Timeout { .. })));
    }
}
