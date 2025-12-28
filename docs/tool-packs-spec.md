# Tool Packs Specification

> Lightweight JSON-based tool definitions for voice-driven CLI orchestration.

## Overview

Tool Packs replace the SKILL.md format with a simpler JSON approach that:
- Requires zero new dependencies (just `serde_json`)
- Maps directly to FunctionGemma few-shot format
- Supports both simple CLI tools and agentic AI CLIs
- Uses safe `program + args[]` execution (no shell injection)

## File Format

Each tool is a `.tool.json` file in the tools directory:

```
~/.config/gibb.eri.sh/tools/
  summarize.tool.json
  linear.tool.json
  git.tool.json
```

### Schema

```json
{
  "$schema": "https://gibb.eri.sh/schemas/tool-pack.v1.json",
  "name": "tool_name",
  "description": "Human description for FunctionGemma context",
  "version": "1.0.0",

  "modes": ["Global"],

  "examples": [
    "User: summarize this\ncall:tool_name{url:<escape>{{url}}<escape>}",
    "User: what is this about\ncall:tool_name{url:<escape>{{url}}<escape>}"
  ],

  "parameters": {
    "url": {
      "type": "string",
      "required": true,
      "description": "URL to process",
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
    "timeout_secs": 60,
    "confirm_before_run": false
  },

  "output": {
    "format": "text",
    "speak": true,
    "max_chars": 500
  }
}
```

## Field Reference

### Core Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Tool identifier (snake_case, used in function calls) |
| `description` | string | yes | Human description injected into prompt |
| `version` | string | no | Semantic version for the tool pack |
| `modes` | string[] | no | Context modes: `Global`, `Coding`, `Dictation` |

### Examples

Few-shot examples for FunctionGemma. Format matches the model's training:

```json
"examples": [
  "User: summarize this webpage\ncall:summarize_url{url:<escape>{{url}}<escape>}",
  "User: tl;dr this article\ncall:summarize_url{url:<escape>{{url}}<escape>}"
]
```

The `{{url}}` placeholder shows where context injection happens. At runtime, these become concrete examples with actual URLs from context.

### Parameters

Declare expected parameters with types and sources:

```json
"parameters": {
  "url": {
    "type": "string",
    "required": true,
    "source": "context.url"
  },
  "query": {
    "type": "string",
    "required": true,
    "source": "transcript"
  },
  "format": {
    "type": "string",
    "enum": ["json", "text", "markdown"],
    "default": "text"
  }
}
```

**Source types:**
- `context.url` - URL from active browser/clipboard
- `context.selection` - Selected text
- `context.clipboard` - Clipboard content
- `context.file` - Active file path
- `transcript` - Extracted from voice command
- `literal` - Hardcoded value

### Command

Safe execution using program + args (no shell):

```json
"command": {
  "program": "npx",
  "args": ["-y", "@steipete/summarize", "{{url}}", "--length", "{{length}}"],
  "env": {
    "NODE_ENV": "production"
  },
  "cwd": "{{context.cwd}}"
}
```

**Template variables:**
- `{{param_name}}` - Parameter value
- `{{param:default}}` - Parameter with default
- `{{context.X}}` - Context values (url, selection, clipboard, cwd, file)

### Policy

Control execution behavior:

```json
"policy": {
  "read_only": true,
  "requires_network": true,
  "timeout_secs": 60,
  "confirm_before_run": false,
  "allowed_exit_codes": [0, 1]
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `read_only` | true | Tool doesn't modify system state |
| `requires_network` | false | Needs internet access |
| `timeout_secs` | 30 | Max execution time |
| `confirm_before_run` | false | Ask user before executing |
| `allowed_exit_codes` | [0] | Non-error exit codes |

### Output

Control result handling:

```json
"output": {
  "format": "json",
  "speak": true,
  "max_chars": 500,
  "extract_field": "summary",
  "template": "Here's the summary: {{output}}"
}
```

| Field | Default | Description |
|-------|---------|-------------|
| `format` | "text" | Expected output: `text`, `json`, `markdown` |
| `speak` | true | Read result aloud via TTS |
| `max_chars` | 1000 | Truncate spoken output |
| `extract_field` | null | For JSON output, extract this field |
| `template` | null | Format output before speaking |

## Example Tool Packs

### Summarizer

```json
{
  "name": "summarize_url",
  "description": "Summarize a webpage, article, or YouTube video",
  "examples": [
    "User: summarize this\ncall:summarize_url{url:<escape>{{url}}<escape>}",
    "User: what is this page about\ncall:summarize_url{url:<escape>{{url}}<escape>}",
    "User: give me a tl;dr\ncall:summarize_url{url:<escape>{{url}}<escape>}"
  ],
  "parameters": {
    "url": { "type": "string", "required": true, "source": "context.url" }
  },
  "command": {
    "program": "npx",
    "args": ["-y", "@steipete/summarize", "{{url}}", "--length", "medium"]
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
}
```

### Linear Issue Creator

```json
{
  "name": "create_linear_issue",
  "description": "Create a new issue in Linear project tracker",
  "examples": [
    "User: create an issue for fixing the login bug\ncall:create_linear_issue{title:<escape>Fix login bug<escape>}",
    "User: add a task to implement dark mode\ncall:create_linear_issue{title:<escape>Implement dark mode<escape>}"
  ],
  "parameters": {
    "title": { "type": "string", "required": true, "source": "transcript" },
    "team": { "type": "string", "default": "ENG" }
  },
  "command": {
    "program": "linear",
    "args": ["issue", "create", "--title", "{{title}}", "--team", "{{team}}"]
  },
  "policy": {
    "read_only": false,
    "requires_network": true,
    "confirm_before_run": true
  },
  "output": {
    "format": "json",
    "extract_field": "url",
    "template": "Created issue: {{output}}"
  }
}
```

### Agentic Code Explainer

```json
{
  "name": "explain_code",
  "description": "Use Gemini to explain selected code",
  "examples": [
    "User: explain this code\ncall:explain_code{code:<escape>{{selection}}<escape>}",
    "User: what does this function do\ncall:explain_code{code:<escape>{{selection}}<escape>}"
  ],
  "parameters": {
    "code": { "type": "string", "required": true, "source": "context.selection" }
  },
  "command": {
    "program": "gemini",
    "args": ["-p", "Explain this code concisely:\n\n{{code}}", "--output-format", "text"]
  },
  "policy": {
    "read_only": true,
    "requires_network": true,
    "timeout_secs": 30,
    "confirm_before_run": true
  },
  "output": {
    "speak": true,
    "max_chars": 300
  }
}
```

### Git Commit

```json
{
  "name": "git_commit",
  "description": "Commit staged changes with a message",
  "modes": ["Coding"],
  "examples": [
    "User: commit with message fix typo\ncall:git_commit{message:<escape>fix typo<escape>}",
    "User: commit these changes\ncall:git_commit{message:<escape>{{transcript}}<escape>}"
  ],
  "parameters": {
    "message": { "type": "string", "required": true, "source": "transcript" }
  },
  "command": {
    "program": "git",
    "args": ["commit", "-m", "{{message}}"],
    "cwd": "{{context.cwd}}"
  },
  "policy": {
    "read_only": false,
    "confirm_before_run": true
  }
}
```

## Discovery & Loading

### Directory Structure

```
~/.config/gibb.eri.sh/
  tools/
    summarize.tool.json
    linear.tool.json
    git.tool.json
  tools.d/                    # Optional: grouped tools
    productivity/
      calendar.tool.json
      todo.tool.json
```

### Loading Process

1. Glob `tools/*.tool.json` and `tools.d/**/*.tool.json`
2. Parse each JSON file
3. Validate against schema
4. Build `ToolDefinition` + `ToolPolicy` for router
5. Generate few-shot examples for FunctionGemma prompt

### Runtime Flow

```
Voice: "summarize this webpage"
    ↓
FunctionGemma (with few-shot examples from tool packs)
    ↓
Parses: call:summarize_url{url:<escape>https://...<escape>}
    ↓
Router looks up "summarize_url" in tool registry
    ↓
Substitutes {{url}} in command args
    ↓
Executes: npx -y @steipete/summarize https://... --length medium
    ↓
Captures output, optionally speaks via TTS
```

## Migration from SKILL.md

SKILL.md files can be converted to tool packs:

```bash
# Future CLI command
gibb tools convert skills/summarize/SKILL.md --output tools/
```

Or keep both formats - the loader can support:
1. `*.tool.json` - Native tool pack format
2. `SKILL.md` - Converted on load to internal format

## Rust Implementation Sketch

```rust
#[derive(Debug, Deserialize)]
pub struct ToolPack {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub modes: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub parameters: HashMap<String, ParamDef>,
    pub command: CommandDef,
    #[serde(default)]
    pub policy: PolicyDef,
    #[serde(default)]
    pub output: OutputDef,
}

#[derive(Debug, Deserialize)]
pub struct CommandDef {
    pub program: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PolicyDef {
    #[serde(default = "default_true")]
    pub read_only: bool,
    #[serde(default)]
    pub requires_network: bool,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub confirm_before_run: bool,
}

impl ToolPack {
    /// Load all tool packs from directory
    pub fn load_all(dir: &Path) -> Vec<Self> {
        glob(&dir.join("*.tool.json"))
            .filter_map(|path| Self::load(&path).ok())
            .collect()
    }

    /// Convert to internal ToolDefinition for router
    pub fn to_tool_definition(&self) -> ToolDefinition {
        // ...
    }

    /// Generate few-shot examples with context substitution
    pub fn generate_examples(&self, context: &Context) -> Vec<String> {
        self.examples.iter()
            .map(|ex| substitute_context(ex, context))
            .collect()
    }
}
```

## Open Questions

1. **Bundled vs User tools**: Ship defaults in app bundle, user overrides in config?
2. **Tool namespacing**: `linear:create_issue` vs flat `create_linear_issue`?
3. **Chaining**: Can one tool call another? (Probably not for v1)
4. **Secrets**: How to handle API keys? Environment variables only?
5. **Validation**: Runtime validation of parameters before execution?
