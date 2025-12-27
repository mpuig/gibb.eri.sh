//! Generic tool wrapper for skill-defined tools.
//!
//! Wraps a `ToolDefinition` from a SKILL.md file and implements
//! the `Tool` trait for use in the action router.

use crate::tools::{Tool, ToolContext, ToolError, ToolResult};
use async_trait::async_trait;
use gibberish_context::Mode;
use gibberish_skills::{
    execute_tool, ExecutorConfig, ParameterType, SkillDefinition,
    ToolDefinition as SkillToolDefinition,
};
use serde_json::json;
use std::borrow::Cow;
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
/// Stores owned data to avoid memory leaks.
pub struct GenericSkillTool {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) event_name: String,
    pub(crate) modes: Vec<Mode>,
    pub(crate) read_only: bool,
    pub(crate) timeout_secs: u32,
    pub(crate) tool_def: Arc<SkillToolDefinition>,
    pub(crate) skill_name: String,
}

impl GenericSkillTool {
    /// Create a new generic skill tool from a skill and tool definition.
    pub fn new(skill: &SkillDefinition, tool: &SkillToolDefinition) -> Self {
        let name = tool.name.clone();
        let description = tool.description.clone();
        let event_name = format!("tools:skill:{}", tool.name);
        let modes: Vec<Mode> = skill.modes.iter().map(|m| convert_mode(*m)).collect();

        Self {
            name,
            description,
            event_name,
            modes,
            read_only: skill.read_only,
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
    fn name(&self) -> Cow<'static, str> {
        Cow::Owned(self.name.clone())
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Owned(self.description.clone())
    }

    fn modes(&self) -> Cow<'static, [Mode]> {
        Cow::Owned(self.modes.clone())
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
            event_name: Cow::Owned(self.event_name.clone()),
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
    use crate::environment::mock::MockSystemEnvironment;
    use gibberish_skills::parse_skill_content;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;

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
    fn test_mode_conversion() {
        let skill = parse_skill_content(TEST_SKILL, Path::new("test.md")).unwrap();
        let tools = GenericSkillTool::from_skill(&skill);
        let modes = tools[0].modes();

        assert_eq!(modes.len(), 2);
        assert!(modes.contains(&Mode::Global));
        assert!(modes.contains(&Mode::Dev));
    }

    #[tokio::test]
    async fn test_execute_tool() {
        let skill = parse_skill_content(TEST_SKILL, Path::new("test.md")).unwrap();
        let tools = GenericSkillTool::from_skill(&skill);
        let tool = &tools[0];

        let args = json!({
            "message": "hello world"
        });

        // Setup dummy context (not used by GenericSkillTool currently)
        let env = Arc::new(MockSystemEnvironment::default());
        let ctx = ToolContext::with_abort(
            env,
            "en".to_string(),
            Arc::new(AtomicBool::new(false)),
        );

        let result = tool.execute(&args, &ctx).await.unwrap();
        
        // The echo command output includes a newline
        let output_str = result.payload["output"].as_str().unwrap();
        assert!(output_str.trim() == "hello world", "Output was: {:?}", output_str);
        
        assert_eq!(result.event_name, "tools:skill:echo_test");
        assert!(result.payload["success"].as_bool().unwrap());
    }
}
