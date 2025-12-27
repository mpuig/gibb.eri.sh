//! SKILL.md parser and executor for user-defined tools.
//!
//! This crate provides:
//! - Parsing of SKILL.md files (YAML frontmatter + Markdown body)
//! - Safe command execution (program + args, no shell)
//! - Output capture with truncation
//!
//! # Example
//!
//! ```ignore
//! use gibberish_skills::{parse_skill, execute_tool};
//!
//! let skill = parse_skill(Path::new("skills/git/SKILL.md"))?;
//! let result = execute_tool(&skill.tools[0], json!({})).await?;
//! ```

mod error;
mod executor;
mod parser;
mod types;

pub use error::{SkillError, SkillResult};
pub use executor::{execute_command, execute_tool, CommandOutput, ExecutorConfig};
pub use parser::parse_skill;
pub use types::{
    ArgFragment, CommandTemplate, Mode, ParameterDefinition, ParameterType, SkillDefinition,
    ToolDefinition,
};
