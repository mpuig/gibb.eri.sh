//! JSON-based tool pack definitions.
//!
//! Tool packs are lightweight JSON files that define voice-activated tools
//! using shell command templates. This is the primary format for extending
//! gibb.eri.sh with new tools.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use gibberish_context::Mode;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{debug, error, warn};

use crate::tools::{Tool, ToolContext, ToolDefinition, ToolError, ToolResult};

/// A tool pack loaded from a .tool.json file.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolPack {
    /// Tool identifier (snake_case).
    pub name: String,

    /// Human description for FunctionGemma context.
    pub description: String,

    /// Semantic version.
    #[serde(default)]
    pub version: Option<String>,

    /// Context modes: Global, Coding, Dictation, Meeting.
    #[serde(default)]
    pub modes: Vec<String>,

    /// Few-shot examples for FunctionGemma.
    #[serde(default)]
    pub examples: Vec<String>,

    /// Parameter definitions.
    #[serde(default)]
    pub parameters: HashMap<String, ParamDef>,

    /// Command to execute.
    pub command: CommandDef,

    /// Execution policy.
    #[serde(default)]
    pub policy: PolicyDef,

    /// Output handling.
    #[serde(default)]
    pub output: OutputDef,
}

/// Parameter definition.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParamDef {
    /// Parameter type: string, number, boolean.
    #[serde(rename = "type")]
    pub param_type: String,

    /// Whether the parameter is required.
    #[serde(default)]
    pub required: bool,

    /// Human description.
    #[serde(default)]
    pub description: Option<String>,

    /// Allowed values for enum types.
    #[serde(default, rename = "enum")]
    pub enum_values: Option<Vec<String>>,

    /// Default value.
    #[serde(default)]
    pub default: Option<serde_json::Value>,

    /// Source for auto-injection: context.url, context.selection, transcript, etc.
    #[serde(default)]
    pub source: Option<String>,
}

/// Command definition using program + args (no shell injection).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CommandDef {
    /// Program to execute (e.g., "npx", "git", "gemini").
    pub program: String,

    /// Arguments with template variables (e.g., ["{{url}}", "--length", "{{length}}"]).
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory (supports {{context.cwd}}).
    #[serde(default)]
    pub cwd: Option<String>,
}

/// Execution policy.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PolicyDef {
    /// Tool doesn't modify system state.
    #[serde(default = "default_true")]
    pub read_only: bool,

    /// Needs internet access.
    #[serde(default)]
    pub requires_network: bool,

    /// Max execution time in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Ask user before executing.
    #[serde(default)]
    pub confirm_before_run: bool,

    /// Non-error exit codes (default: [0]).
    #[serde(default = "default_exit_codes")]
    pub allowed_exit_codes: Vec<i32>,
}

fn default_true() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

fn default_exit_codes() -> Vec<i32> {
    vec![0]
}

impl Default for PolicyDef {
    fn default() -> Self {
        Self {
            read_only: true,
            requires_network: false,
            timeout_secs: 30,
            confirm_before_run: false,
            allowed_exit_codes: vec![0],
        }
    }
}

/// Output handling configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputDef {
    /// Expected output format: text, json, markdown.
    #[serde(default = "default_format")]
    pub format: String,

    /// Read result aloud via TTS.
    #[serde(default = "default_true")]
    pub speak: bool,

    /// Truncate spoken output at this length.
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,

    /// For JSON output, extract this field.
    #[serde(default)]
    pub extract_field: Option<String>,

    /// Template for output (e.g., "Summary: {{output}}").
    #[serde(default)]
    pub template: Option<String>,
}

fn default_format() -> String {
    "text".to_string()
}

fn default_max_chars() -> usize {
    1000
}

impl Default for OutputDef {
    fn default() -> Self {
        Self {
            format: "text".to_string(),
            speak: true,
            max_chars: 1000,
            extract_field: None,
            template: None,
        }
    }
}

impl ToolPack {
    /// Parse a tool pack from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Parse modes into Mode enum values.
    fn parsed_modes(&self) -> Vec<Mode> {
        self.modes
            .iter()
            .filter_map(|s| match s.to_lowercase().as_str() {
                "global" => Some(Mode::Global),
                "coding" | "dev" => Some(Mode::Dev),
                "writer" | "writing" | "dictation" => Some(Mode::Writer),
                "meeting" | "call" => Some(Mode::Meeting),
                _ => {
                    warn!(mode = %s, "Unknown mode in tool pack");
                    None
                }
            })
            .collect()
    }

    /// Build JSON schema for arguments.
    fn build_args_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, def) in &self.parameters {
            let mut prop = serde_json::Map::new();
            prop.insert("type".to_string(), serde_json::json!(def.param_type));

            if let Some(desc) = &def.description {
                prop.insert("description".to_string(), serde_json::json!(desc));
            }

            if let Some(values) = &def.enum_values {
                prop.insert("enum".to_string(), serde_json::json!(values));
            }

            if let Some(default) = &def.default {
                prop.insert("default".to_string(), default.clone());
            }

            properties.insert(name.clone(), serde_json::Value::Object(prop));

            if def.required {
                required.push(name.clone());
            }
        }

        serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }

    /// Generate a selection hint from examples.
    fn generate_selection_hint(&self) -> Option<String> {
        if self.examples.is_empty() {
            return None;
        }

        // Extract trigger phrases from examples
        let triggers: Vec<&str> = self
            .examples
            .iter()
            .filter_map(|ex| {
                // Extract "User: <phrase>" from example
                ex.strip_prefix("User: ")
                    .and_then(|s| s.lines().next())
                    .map(|s| s.trim())
            })
            .take(3)
            .collect();

        if triggers.is_empty() {
            None
        } else {
            Some(triggers.join("/"))
        }
    }
}

/// Wrapper to make ToolPack implement the Tool trait.
pub struct ToolPackTool {
    pack: Arc<ToolPack>,
}

impl ToolPackTool {
    pub fn new(pack: ToolPack) -> Self {
        Self {
            pack: Arc::new(pack),
        }
    }

    pub fn from_arc(pack: Arc<ToolPack>) -> Self {
        Self { pack }
    }

    /// Substitute template variables in a string.
    fn substitute(&self, template: &str, args: &serde_json::Value) -> String {
        let mut result = template.to_string();

        // Substitute {{param}} and {{param:default}} patterns
        let re = regex::Regex::new(r"\{\{(\w+)(?::([^}]*))?\}\}").unwrap();

        result = re
            .replace_all(&result, |caps: &regex::Captures| {
                let param_name = &caps[1];
                let default = caps.get(2).map(|m| m.as_str());

                // Try to get value from args
                if let Some(value) = args.get(param_name) {
                    match value {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        _ => value.to_string(),
                    }
                } else if let Some(def) = default {
                    // Use default value
                    def.to_string()
                } else if let Some(param_def) = self.pack.parameters.get(param_name) {
                    // Use parameter default
                    param_def
                        .default
                        .as_ref()
                        .map(|v| match v {
                            serde_json::Value::String(s) => s.clone(),
                            _ => v.to_string(),
                        })
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            })
            .to_string();

        result
    }

    /// Build the command to execute.
    fn build_command(&self, args: &serde_json::Value) -> Command {
        let mut cmd = Command::new(&self.pack.command.program);

        // Substitute and add arguments
        for arg in &self.pack.command.args {
            let substituted = self.substitute(arg, args);
            if !substituted.is_empty() {
                cmd.arg(substituted);
            }
        }

        // Add environment variables
        for (key, value) in &self.pack.command.env {
            let substituted = self.substitute(value, args);
            cmd.env(key, substituted);
        }

        // Set working directory if specified
        if let Some(cwd) = &self.pack.command.cwd {
            let substituted = self.substitute(cwd, args);
            if !substituted.is_empty() {
                cmd.current_dir(substituted);
            }
        }

        cmd
    }

    /// Process command output according to output config.
    fn process_output(&self, stdout: &str) -> serde_json::Value {
        let output_config = &self.pack.output;

        // Parse JSON output if expected
        let mut output = if output_config.format == "json" {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(stdout) {
                // Extract field if specified
                if let Some(field) = &output_config.extract_field {
                    parsed.get(field).cloned().unwrap_or(serde_json::json!(stdout))
                } else {
                    parsed
                }
            } else {
                serde_json::json!(stdout)
            }
        } else {
            serde_json::json!(stdout)
        };

        // Apply template if specified
        if let Some(template) = &output_config.template {
            let output_str = match &output {
                serde_json::Value::String(s) => s.clone(),
                _ => output.to_string(),
            };
            let templated = template.replace("{{output}}", &output_str);
            output = serde_json::json!(templated);
        }

        // Truncate if needed
        if let serde_json::Value::String(s) = &output {
            if s.len() > output_config.max_chars {
                let truncated = &s[..output_config.max_chars];
                output = serde_json::json!(format!("{}...", truncated));
            }
        }

        output
    }
}

impl Clone for ToolPackTool {
    fn clone(&self) -> Self {
        Self {
            pack: Arc::clone(&self.pack),
        }
    }
}

#[async_trait]
impl Tool for ToolPackTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Owned(self.pack.name.clone())
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Owned(self.pack.description.clone())
    }

    fn selection_hint(&self) -> Option<Cow<'static, str>> {
        self.pack.generate_selection_hint().map(Cow::Owned)
    }

    fn owned_few_shot_examples(&self) -> Vec<String> {
        self.pack.examples.clone()
    }

    fn modes(&self) -> Cow<'static, [Mode]> {
        let modes = self.pack.parsed_modes();
        if modes.is_empty() {
            Cow::Borrowed(&[])
        } else {
            Cow::Owned(modes)
        }
    }

    fn is_read_only(&self) -> bool {
        self.pack.policy.read_only
    }

    fn args_schema(&self) -> serde_json::Value {
        self.pack.build_args_schema()
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.pack.name.clone(),
            description: self.pack.description.clone(),
            read_only: self.pack.policy.read_only,
            args_schema: self.pack.build_args_schema(),
        }
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        debug!(
            tool = %self.pack.name,
            args = %args,
            "Executing tool pack"
        );

        // Check for abort
        if ctx.is_aborted() {
            return Err(ToolError::ExecutionFailed("Aborted by user".to_string()));
        }

        // Build and execute command
        let mut cmd = self.build_command(args);

        // Set up output capture
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let start = std::time::Instant::now();

        // Execute with timeout
        let timeout = std::time::Duration::from_secs(self.pack.policy.timeout_secs);
        let output = tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| ToolError::ExecutionFailed("Command timed out".to_string()))?
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Check exit code
        let exit_code = output.status.code().unwrap_or(-1);
        if !self.pack.policy.allowed_exit_codes.contains(&exit_code) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!(
                tool = %self.pack.name,
                exit_code,
                stderr = %stderr,
                "Command failed"
            );
            return Err(ToolError::ExecutionFailed(format!(
                "Exit code {}: {}",
                exit_code, stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let processed = self.process_output(&stdout);

        debug!(
            tool = %self.pack.name,
            duration_ms,
            output_len = stdout.len(),
            "Tool pack execution completed"
        );

        Ok(ToolResult {
            event_name: Cow::Owned(format!("tools:{}", self.pack.name)),
            payload: serde_json::json!({
                "success": true,
                "output": processed,
                "duration_ms": duration_ms,
            }),
            cache_key: None,
            cooldown_key: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_tool_pack() {
        let json = r#"{
            "name": "test_tool",
            "description": "A test tool",
            "command": {
                "program": "echo",
                "args": ["hello"]
            }
        }"#;

        let pack = ToolPack::from_json(json).unwrap();
        assert_eq!(pack.name, "test_tool");
        assert_eq!(pack.description, "A test tool");
        assert_eq!(pack.command.program, "echo");
        assert_eq!(pack.command.args, vec!["hello"]);
        assert!(pack.policy.read_only);
        assert_eq!(pack.policy.timeout_secs, 30);
    }

    #[test]
    fn test_parse_full_tool_pack() {
        let json = r#"{
            "name": "summarize_url",
            "description": "Summarize a webpage",
            "version": "1.0.0",
            "modes": ["Global"],
            "examples": [
                "User: summarize this\ncall:summarize_url{url:<escape>{{url}}<escape>}"
            ],
            "parameters": {
                "url": {
                    "type": "string",
                    "required": true,
                    "source": "context.url"
                },
                "length": {
                    "type": "string",
                    "enum": ["short", "medium", "long"],
                    "default": "medium"
                }
            },
            "command": {
                "program": "npx",
                "args": ["-y", "@steipete/summarize", "{{url}}", "--length", "{{length}}"]
            },
            "policy": {
                "read_only": true,
                "requires_network": true,
                "timeout_secs": 120
            },
            "output": {
                "speak": true,
                "max_chars": 500
            }
        }"#;

        let pack = ToolPack::from_json(json).unwrap();
        assert_eq!(pack.name, "summarize_url");
        assert_eq!(pack.modes, vec!["Global"]);
        assert_eq!(pack.examples.len(), 1);
        assert!(pack.parameters.contains_key("url"));
        assert!(pack.parameters.get("url").unwrap().required);
        assert_eq!(pack.policy.timeout_secs, 120);
        assert!(pack.policy.requires_network);
    }

    #[test]
    fn test_substitute_variables() {
        let json = r#"{
            "name": "test",
            "description": "Test",
            "parameters": {
                "url": { "type": "string", "required": true },
                "length": { "type": "string", "default": "medium" }
            },
            "command": {
                "program": "echo",
                "args": ["{{url}}", "--length", "{{length:short}}"]
            }
        }"#;

        let pack = ToolPack::from_json(json).unwrap();
        let tool = ToolPackTool::new(pack);

        // Test with both params provided
        let args = serde_json::json!({
            "url": "https://example.com",
            "length": "long"
        });

        let result = tool.substitute("{{url}} {{length}}", &args);
        assert_eq!(result, "https://example.com long");

        // Test with default fallback
        let args = serde_json::json!({ "url": "https://example.com" });
        let result = tool.substitute("{{url}} {{length:short}}", &args);
        assert_eq!(result, "https://example.com short");
    }

    #[test]
    fn test_modes_parsing() {
        let json = r#"{
            "name": "test",
            "description": "Test",
            "modes": ["Global", "Dev", "invalid"],
            "command": { "program": "echo", "args": [] }
        }"#;

        let pack = ToolPack::from_json(json).unwrap();
        let modes = pack.parsed_modes();
        assert_eq!(modes.len(), 2);
        assert!(modes.contains(&Mode::Global));
        assert!(modes.contains(&Mode::Dev));
    }

    #[test]
    fn test_selection_hint_generation() {
        let json = r#"{
            "name": "test",
            "description": "Test",
            "examples": [
                "User: summarize this\ncall:test{}",
                "User: what is this about\ncall:test{}",
                "User: give me a tl;dr\ncall:test{}"
            ],
            "command": { "program": "echo", "args": [] }
        }"#;

        let pack = ToolPack::from_json(json).unwrap();
        let hint = pack.generate_selection_hint();
        assert_eq!(
            hint,
            Some("summarize this/what is this about/give me a tl;dr".to_string())
        );
    }
}
