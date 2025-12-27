//! Generic tool wrapper for skill-defined tools.
//!
//! Wraps a `ToolDefinition` from a SKILL.md file and implements
//! the `Tool` trait for use in the action router.

use crate::tools::{Tool, ToolContext, ToolError, ToolResult};
use gibberish_context::Mode;
use async_trait::async_trait;
use gibberish_skills::{
    execute_tool, ExecutorConfig, ParameterType, SkillDefinition,
    ToolDefinition as SkillToolDefinition,
};
use serde_json::json;
use std::sync::Arc;

/// Convert skills::Mode to context::Mode.
fn convert_mode(mode: gibberish_skills::Mode) -> Mode {
    match mode {
        gibberish_skills::Mode::Global => Mode::Global,
        gibberish_skills::Mode::Dev => Mode::Dev,
        gibberish_skills::Mode::Meeting => Mode::Meeting,
        gibberish_skills::Mode::Writer => Mode::Writer,
    }
}

/// Generic tool that wraps a skill-defined tool.
///
/// Uses `Box::leak` to create static strings required by the Tool trait.
/// This is acceptable because skills are long-lived and reloading
/// creates new GenericSkillTool instances.
pub struct GenericSkillTool {
    /// Leaked static tool name.
    pub(crate) name: &'static str,

    /// Leaked static description.
    pub(crate) description: &'static str,

    /// Leaked static event name for results.
    pub(crate) event_name: &'static str,

    /// Leaked static modes slice.
    pub(crate) modes: &'static [Mode],

    /// Whether this tool is read-only.
    pub(crate) read_only: bool,

    /// Whether this tool always requires approval.
    #[allow(dead_code)]
    pub(crate) always_ask: bool,

    /// Execution timeout in seconds.
    pub(crate) timeout_secs: u32,

    /// Tool definition for schema and command execution.
    pub(crate) tool_def: Arc<SkillToolDefinition>,

    /// Skill name for grouping.
    pub(crate) skill_name: String,
}

impl GenericSkillTool {
    /// Create a new generic skill tool from a skill and tool definition.
    pub fn new(skill: &SkillDefinition, tool: &SkillToolDefinition) -> Self {
        // Leak strings to get &'static str
        let name: &'static str = Box::leak(tool.name.clone().into_boxed_str());
        let description: &'static str = Box::leak(tool.description.clone().into_boxed_str());
        let event_name: &'static str =
            Box::leak(format!("tools:skill:{}", tool.name).into_boxed_str());

        // Convert and leak modes
        let modes_vec: Vec<Mode> = skill.modes.iter().map(|m| convert_mode(*m)).collect();
        let modes: &'static [Mode] = Box::leak(modes_vec.into_boxed_slice());

        Self {
            name,
            description,
            event_name,
            modes,
            read_only: skill.read_only,
            always_ask: skill.always_ask,
            timeout_secs: skill.timeout,
            tool_def: Arc::new(tool.clone()),
            skill_name: skill.name.clone(),
        }
    }

    /// Create all tools from a skill definition.
    pub fn from_skill(skill: &SkillDefinition) -> Vec<Self> {
        skill
            .tools
            .iter()
            .map(|tool| Self::new(skill, tool))
            .collect()
    }

    /// Build JSON schema for the tool's parameters.
    fn build_schema(&self) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for param in &self.tool_def.parameters {
            let type_str = match param.param_type {
                ParameterType::String => "string",
                ParameterType::Integer => "integer",
                ParameterType::Number => "number",
                ParameterType::Boolean => "boolean",
                ParameterType::Array => "array",
            };

            let mut prop = json!({
                "type": type_str,
                "description": param.description,
            });

            if let Some(ref default) = param.default {
                prop["default"] = default.clone();
            }

            if param.param_type == ParameterType::Array {
                prop["items"] = json!({"type": "string"});
            }

            properties.insert(param.name.clone(), prop);

            if param.required {
                required.push(serde_json::Value::String(param.name.clone()));
            }
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": required,
        })
    }

    /// Get the skill name this tool belongs to.
    pub fn skill_name(&self) -> &str {
        &self.skill_name
    }
}

#[async_trait]
impl Tool for GenericSkillTool {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn modes(&self) -> &'static [Mode] {
        self.modes
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn args_schema(&self) -> serde_json::Value {
        self.build_schema()
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let config = ExecutorConfig {
            timeout_secs: self.timeout_secs,
            ..Default::default()
        };

        let output = execute_tool(&self.tool_def, args, &config)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolResult {
            event_name: self.event_name,
            payload: output.to_json(),
            cache_key: None,
            cooldown_key: if self.read_only {
                None
            } else {
                Some(format!("skill:{}:{}", self.skill_name, self.name))
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gibberish_skills::parse_skill_content;
    use std::path::Path;

    const TEST_SKILL: &str = r#"---
name: test
version: 1.0.0
description: Test skill
modes: [Global, Dev]
read_only: true
---

### echo_test

Echo back the input.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| message | string | yes | Message to echo |

#### Command

```bash
echo {{message}}
```
"#;

    #[test]
    fn test_create_from_skill() {
        let skill = parse_skill_content(TEST_SKILL, Path::new("test.md")).unwrap();
        let tools = GenericSkillTool::from_skill(&skill);

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name(), "echo_test");
        assert_eq!(tools[0].skill_name(), "test");
        assert!(tools[0].is_read_only());
    }

    #[test]
    fn test_schema_generation() {
        let skill = parse_skill_content(TEST_SKILL, Path::new("test.md")).unwrap();
        let tools = GenericSkillTool::from_skill(&skill);
        let schema = tools[0].args_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["message"].is_object());
        assert_eq!(schema["properties"]["message"]["type"], "string");
        assert_eq!(schema["required"][0], "message");
    }

    #[test]
    fn test_mode_conversion() {
        let skill = parse_skill_content(TEST_SKILL, Path::new("test.md")).unwrap();
        let tools = GenericSkillTool::from_skill(&skill);
        let modes = tools[0].modes();

        assert_eq!(modes.len(), 2);
        assert!(modes.contains(&Mode::Global));
        assert!(modes.contains(&Mode::Dev));
    }
}
