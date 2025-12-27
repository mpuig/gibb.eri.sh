//! SKILL.md parser.
//!
//! Parses YAML frontmatter and Markdown tool definitions.

use crate::error::{SkillError, SkillResult};
use crate::types::{
    ArgFragment, CommandTemplate, ParameterDefinition, ParameterType, SkillDefinition,
    ToolDefinition,
};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

/// Parse a SKILL.md file into a SkillDefinition.
pub fn parse_skill(path: &Path) -> SkillResult<SkillDefinition> {
    let content = std::fs::read_to_string(path).map_err(|e| SkillError::ReadFile {
        path: path.to_path_buf(),
        source: e,
    })?;

    parse_skill_content(&content, path)
}

/// Parse SKILL.md content (for testing without filesystem).
pub fn parse_skill_content(content: &str, path: &Path) -> SkillResult<SkillDefinition> {
    // Extract YAML frontmatter
    let (frontmatter, body) = extract_frontmatter(content, path)?;

    // Parse frontmatter into SkillDefinition
    let mut skill: SkillDefinition =
        serde_yml::from_str(&frontmatter).map_err(|e| SkillError::InvalidFrontmatter {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

    // Validate required fields
    if skill.name.is_empty() {
        return Err(SkillError::MissingField {
            path: path.to_path_buf(),
            field: "name".to_string(),
        });
    }
    if skill.version.is_empty() {
        return Err(SkillError::MissingField {
            path: path.to_path_buf(),
            field: "version".to_string(),
        });
    }
    if skill.description.is_empty() {
        return Err(SkillError::MissingField {
            path: path.to_path_buf(),
            field: "description".to_string(),
        });
    }

    // Parse tool definitions from body
    skill.tools = parse_tools(&body, path)?;
    skill.source_path = path.to_path_buf();

    if skill.tools.is_empty() {
        return Err(SkillError::NoTools {
            path: path.to_path_buf(),
        });
    }

    // Check for duplicate tool names
    let mut seen = HashSet::new();
    for tool in &skill.tools {
        if !seen.insert(&tool.name) {
            return Err(SkillError::DuplicateTool {
                path: path.to_path_buf(),
                tool: tool.name.clone(),
            });
        }
    }

    Ok(skill)
}

/// Extract YAML frontmatter from content.
fn extract_frontmatter(content: &str, path: &Path) -> SkillResult<(String, String)> {
    let content = content.trim();

    if !content.starts_with("---") {
        return Err(SkillError::InvalidFrontmatter {
            path: path.to_path_buf(),
            message: "Missing opening '---'".to_string(),
        });
    }

    let after_start = &content[3..];
    let end_pos = after_start.find("\n---").ok_or_else(|| SkillError::InvalidFrontmatter {
        path: path.to_path_buf(),
        message: "Missing closing '---'".to_string(),
    })?;

    let frontmatter = after_start[..end_pos].trim().to_string();
    let body = after_start[end_pos + 4..].trim().to_string();

    Ok((frontmatter, body))
}

/// Parse tool definitions from Markdown body.
fn parse_tools(body: &str, path: &Path) -> SkillResult<Vec<ToolDefinition>> {
    let mut tools = Vec::new();

    // Split by H3 headers (### tool_name)
    let tool_re = Regex::new(r"(?m)^### +(\w+)\s*$").unwrap();
    let sections: Vec<_> = tool_re.find_iter(body).collect();

    for (i, section_match) in sections.iter().enumerate() {
        let tool_name = tool_re
            .captures(section_match.as_str())
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        // Get content until next section or end
        let start = section_match.end();
        let end = sections.get(i + 1).map(|m| m.start()).unwrap_or(body.len());
        let section_content = &body[start..end];

        let tool = parse_tool_section(&tool_name, section_content, path)?;
        tools.push(tool);
    }

    Ok(tools)
}

/// Parse a single tool section.
fn parse_tool_section(name: &str, content: &str, _path: &Path) -> SkillResult<ToolDefinition> {
    // Extract description (text before first H4)
    let description = extract_description(content);

    // Extract parameters table
    let parameters = extract_parameters(content, name)?;

    // Extract command block
    let command = extract_command(content, name)?;

    Ok(ToolDefinition {
        name: name.to_string(),
        description,
        parameters,
        command,
    })
}

/// Extract tool description (text before first ####).
fn extract_description(content: &str) -> String {
    let lines: Vec<_> = content
        .lines()
        .take_while(|line| !line.starts_with("####"))
        .filter(|line| !line.trim().is_empty() && !line.starts_with("---"))
        .collect();

    lines.join(" ").trim().to_string()
}

/// Extract parameters from Markdown table.
fn extract_parameters(content: &str, _tool: &str) -> SkillResult<Vec<ParameterDefinition>> {
    let mut params = Vec::new();

    // Find Parameters section
    let params_section = content
        .find("#### Parameters")
        .map(|pos| &content[pos..])
        .unwrap_or("");

    if params_section.contains("None.") || params_section.contains("None") {
        return Ok(params);
    }

    // Parse Markdown table
    // | Name | Type | Required | Description |
    let table_re = Regex::new(r"(?m)^\|\s*(\w+)\s*\|\s*(\w+)\s*\|\s*(yes|no)\s*\|\s*(.+?)\s*\|$")
        .unwrap();

    for cap in table_re.captures_iter(params_section) {
        let name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let type_str = cap.get(2).map(|m| m.as_str()).unwrap_or("string");
        let required_str = cap.get(3).map(|m| m.as_str()).unwrap_or("no");
        let description = cap.get(4).map(|m| m.as_str()).unwrap_or("");

        // Skip header row
        if name == "Name" || name == "---" || name.contains('-') {
            continue;
        }

        let param_type = match type_str.to_lowercase().as_str() {
            "string" => ParameterType::String,
            "integer" | "int" => ParameterType::Integer,
            "number" | "float" => ParameterType::Number,
            "boolean" | "bool" => ParameterType::Boolean,
            "array" => ParameterType::Array,
            _ => ParameterType::String,
        };

        params.push(ParameterDefinition {
            name: name.to_string(),
            param_type,
            required: required_str == "yes",
            description: description.to_string(),
            default: None,
        });
    }

    Ok(params)
}

/// Extract command from fenced code block.
fn extract_command(content: &str, tool: &str) -> SkillResult<CommandTemplate> {
    // Find ```bash block
    let code_re = Regex::new(r"```(?:bash|sh)\s*\n([\s\S]*?)\n```").unwrap();

    let command_str = code_re
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim())
        .ok_or_else(|| SkillError::InvalidCommand {
            tool: tool.to_string(),
            message: "No ```bash or ```sh code block found".to_string(),
        })?;

    parse_command_template(command_str, tool)
}

/// Tokenize a command string, respecting quoted strings.
/// Handles single quotes, double quotes, and escaped characters.
fn tokenize_command(command: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while let Some(c) = chars.next() {
        match c {
            '\\' if !in_single_quote => {
                // Escape next character
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '\'' if !in_double_quote => {
                in_single_quote = !in_single_quote;
            }
            '"' if !in_single_quote => {
                in_double_quote = !in_double_quote;
            }
            ' ' | '\t' if !in_single_quote && !in_double_quote => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Parse a command string into program + args with interpolation.
fn parse_command_template(command: &str, tool: &str) -> SkillResult<CommandTemplate> {
    let parts = tokenize_command(command);

    if parts.is_empty() {
        return Err(SkillError::InvalidCommand {
            tool: tool.to_string(),
            message: "Empty command".to_string(),
        });
    }

    let program = parts[0].to_string();
    let mut args = Vec::new();

    // Variable pattern: {{name}}, {{name:default}}, {{name:--flag}}
    let var_re = Regex::new(r"\{\{(\w+)(?::([^}]+))?\}\}").unwrap();

    for part in parts.iter().skip(1) {
        if let Some(cap) = var_re.captures(part) {
            let var_name = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let modifier = cap.get(2).map(|m| m.as_str());

            // Check if it's a flag (starts with -)
            let (default, flag) = match modifier {
                Some(m) if m.starts_with('-') => (None, Some(m.to_string())),
                Some(m) => (Some(m.to_string()), None),
                None => (None, None),
            };

            // Check if part has literal prefix/suffix
            let full_match = cap.get(0).map(|m| m.as_str()).unwrap_or("");
            if part != full_match {
                // Has literal parts, e.g., "--count={{count}}"
                let prefix = part.split("{{").next().unwrap_or("");
                if !prefix.is_empty() {
                    args.push(ArgFragment::Literal(prefix.to_string()));
                }
                args.push(ArgFragment::Variable {
                    name: var_name.to_string(),
                    default,
                    flag,
                });
            } else {
                args.push(ArgFragment::Variable {
                    name: var_name.to_string(),
                    default,
                    flag,
                });
            }
        } else {
            args.push(ArgFragment::Literal(part.to_string()));
        }
    }

    Ok(CommandTemplate { program, args })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SKILL: &str = r#"---
name: git
version: 1.0.0
description: Git operations for checking status.
modes: [Dev, Global]
read_only: true
---

### git_status

Show the current git status.

#### Parameters

None.

#### Command

```bash
git status --short
```

### git_log

Show commit history.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| count | integer | no | Number of commits |

#### Command

```bash
git log --oneline -n {{count:10}}
```
"#;

    #[test]
    fn test_parse_skill() {
        let skill = parse_skill_content(SAMPLE_SKILL, Path::new("test.md")).unwrap();

        assert_eq!(skill.name, "git");
        assert_eq!(skill.version, "1.0.0");
        assert!(skill.read_only);
        assert_eq!(skill.tools.len(), 2);
    }

    #[test]
    fn test_parse_tool_no_params() {
        let skill = parse_skill_content(SAMPLE_SKILL, Path::new("test.md")).unwrap();
        let tool = skill.get_tool("git_status").unwrap();

        assert_eq!(tool.name, "git_status");
        assert!(tool.parameters.is_empty());
        assert_eq!(tool.command.program, "git");
    }

    #[test]
    fn test_parse_tool_with_params() {
        let skill = parse_skill_content(SAMPLE_SKILL, Path::new("test.md")).unwrap();
        let tool = skill.get_tool("git_log").unwrap();

        assert_eq!(tool.parameters.len(), 1);
        assert_eq!(tool.parameters[0].name, "count");
        assert_eq!(tool.parameters[0].param_type, ParameterType::Integer);
        assert!(!tool.parameters[0].required);
    }

    #[test]
    fn test_parse_command_with_default() {
        let cmd = parse_command_template("git log -n {{count:10}}", "test").unwrap();

        assert_eq!(cmd.program, "git");
        assert_eq!(cmd.args.len(), 3);

        match &cmd.args[2] {
            ArgFragment::Variable { name, default, .. } => {
                assert_eq!(name, "count");
                assert_eq!(default.as_deref(), Some("10"));
            }
            _ => panic!("Expected variable"),
        }
    }

    #[test]
    fn test_missing_frontmatter() {
        let content = "# No frontmatter\nJust markdown.";
        let result = parse_skill_content(content, Path::new("test.md"));

        assert!(matches!(result, Err(SkillError::InvalidFrontmatter { .. })));
    }

    #[test]
    fn test_missing_tools() {
        let content = r#"---
name: empty
version: 1.0.0
description: No tools
---

Just some text, no tools.
"#;
        let result = parse_skill_content(content, Path::new("test.md"));

        assert!(matches!(result, Err(SkillError::NoTools { .. })));
    }

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize_command("echo hello world");
        assert_eq!(tokens, vec!["echo", "hello", "world"]);
    }

    #[test]
    fn test_tokenize_double_quotes() {
        let tokens = tokenize_command(r#"echo "hello world" foo"#);
        assert_eq!(tokens, vec!["echo", "hello world", "foo"]);
    }

    #[test]
    fn test_tokenize_single_quotes() {
        let tokens = tokenize_command("echo 'hello world' foo");
        assert_eq!(tokens, vec!["echo", "hello world", "foo"]);
    }

    #[test]
    fn test_tokenize_escaped_space() {
        let tokens = tokenize_command(r"echo hello\ world foo");
        assert_eq!(tokens, vec!["echo", "hello world", "foo"]);
    }

    #[test]
    fn test_tokenize_mixed_quotes() {
        let tokens = tokenize_command(r#"cmd "arg with 'nested'" 'and "this"'"#);
        assert_eq!(tokens, vec!["cmd", "arg with 'nested'", r#"and "this""#]);
    }

    #[test]
    fn test_tokenize_with_variable() {
        let tokens = tokenize_command(r#"echo "{{message}}" --flag"#);
        assert_eq!(tokens, vec!["echo", "{{message}}", "--flag"]);
    }

    #[test]
    fn test_parse_command_with_quoted_arg() {
        let cmd = parse_command_template(r#"echo "hello world" {{name}}"#, "test").unwrap();
        assert_eq!(cmd.program, "echo");
        assert_eq!(cmd.args.len(), 2);
        match &cmd.args[0] {
            ArgFragment::Literal(s) => assert_eq!(s, "hello world"),
            _ => panic!("Expected literal"),
        }
    }
}
