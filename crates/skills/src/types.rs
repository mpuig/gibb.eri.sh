//! Core types for skill definitions.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Context mode for skill availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Mode {
    Global,
    Dev,
    Meeting,
    Writer,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Global
    }
}

/// Parsed SKILL.md file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    /// Skill identifier (must match directory name).
    pub name: String,

    /// Semver version.
    pub version: String,

    /// Description of what the skill does and when to use it.
    pub description: String,

    /// Skill author.
    #[serde(default)]
    pub author: Option<String>,

    /// Context modes where this skill is available.
    #[serde(default = "default_modes")]
    pub modes: Vec<Mode>,

    /// If true, can auto-run without approval.
    #[serde(default)]
    pub read_only: bool,

    /// Always require approval, even if read_only.
    #[serde(default)]
    pub always_ask: bool,

    /// Max execution time in seconds.
    #[serde(default = "default_timeout")]
    pub timeout: u32,

    /// Whether the skill requires network access.
    #[serde(default)]
    pub network: bool,

    /// Tools defined in this skill.
    #[serde(skip)]
    pub tools: Vec<ToolDefinition>,

    /// Path to the SKILL.md file.
    #[serde(skip)]
    pub source_path: PathBuf,
}

fn default_modes() -> Vec<Mode> {
    vec![Mode::Global]
}

fn default_timeout() -> u32 {
    30
}

/// A single tool within a skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (e.g., "git_status").
    pub name: String,

    /// Description of what this tool does.
    pub description: String,

    /// Tool parameters.
    pub parameters: Vec<ParameterDefinition>,

    /// Command template to execute.
    pub command: CommandTemplate,

    /// Few-shot examples for FunctionGemma prompts.
    #[serde(default)]
    pub examples: Vec<String>,
}

/// Tool parameter definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDefinition {
    /// Parameter name.
    pub name: String,

    /// Parameter type.
    #[serde(rename = "type")]
    pub param_type: ParameterType,

    /// Whether this parameter is required.
    pub required: bool,

    /// Description of the parameter.
    pub description: String,

    /// Default value (for optional parameters).
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

/// Parameter types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterType {
    String,
    Integer,
    Number,
    Boolean,
    Array,
}

impl Default for ParameterType {
    fn default() -> Self {
        Self::String
    }
}

/// Parsed command template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandTemplate {
    /// Program to execute.
    pub program: String,

    /// Argument fragments (literals and variables).
    pub args: Vec<ArgFragment>,
}

/// Argument fragment in a command template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArgFragment {
    /// Literal string value.
    Literal(String),

    /// Variable interpolation with optional default.
    Variable {
        /// Variable name.
        name: String,
        /// Default value if not provided.
        default: Option<String>,
        /// Flag format (for boolean params, e.g., "--staged").
        flag: Option<String>,
    },
}

impl SkillDefinition {
    /// Check if this skill is available in the given mode.
    pub fn is_available_in(&self, mode: Mode) -> bool {
        self.modes.contains(&mode)
    }

    /// Get a tool by name.
    pub fn get_tool(&self, name: &str) -> Option<&ToolDefinition> {
        self.tools.iter().find(|t| t.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_modes() {
        let modes = default_modes();
        assert_eq!(modes, vec![Mode::Global]);
    }

    #[test]
    fn test_mode_availability() {
        let skill = SkillDefinition {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            author: None,
            modes: vec![Mode::Dev, Mode::Global],
            read_only: false,
            always_ask: false,
            timeout: 30,
            network: false,
            tools: vec![],
            source_path: PathBuf::new(),
        };

        assert!(skill.is_available_in(Mode::Dev));
        assert!(skill.is_available_in(Mode::Global));
        assert!(!skill.is_available_in(Mode::Meeting));
    }
}
