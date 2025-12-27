# SKILL.md Specification v1.0

> **Status**: Draft (Phase 0)
> **Author**: gibb.eri.sh team
> **Last Updated**: 2025-12-27

## Overview

A **Skill** is a user-defined tool described in a Markdown file. Skills extend gibb.eri.sh without requiring Rust code or recompilation.

```
~/.config/gibberish/skills/
├── git/
│   └── SKILL.md
├── summarize/
│   └── SKILL.md
└── my-custom-skill/
    └── SKILL.md
```

---

## 1. File Structure

### 1.1 Directory Layout

```
skill-name/
├── SKILL.md          # Required: Skill definition
├── scripts/          # Optional: Supporting scripts
│   └── helper.py
└── assets/           # Optional: Static files
    └── template.txt
```

### 1.2 Naming Constraints

- Directory name = skill name
- Lowercase alphanumeric and hyphens only: `[a-z0-9-]+`
- 1-64 characters
- Cannot start/end with hyphen
- No consecutive hyphens

**Valid**: `git-status`, `web-search`, `my-tool-v2`
**Invalid**: `-git`, `git-`, `my--tool`, `Git_Status`

---

## 2. SKILL.md Format

### 2.1 Structure

```markdown
---
# YAML Frontmatter (required)
name: skill-name
version: 1.0.0
description: What this skill does and when to use it.
---

# Markdown Body (tool definitions)
```

### 2.2 Frontmatter Fields

#### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Skill identifier. Must match directory name. |
| `version` | string | Semver version (e.g., `1.0.0`). |
| `description` | string | 1-256 chars. Describes purpose and trigger phrases. |

#### Optional Fields (Gibberish Extensions)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `author` | string | - | Skill author name/handle. |
| `modes` | array | `["Global"]` | Context modes: `Global`, `Dev`, `Meeting`. |
| `read_only` | boolean | `false` | If true, can auto-run without approval. |
| `always_ask` | boolean | `false` | Always require approval, even if read_only. |
| `timeout` | integer | `30` | Max execution time in seconds (1-300). |
| `network` | boolean | `false` | Declares if skill requires internet. Informational only. |

#### Example Frontmatter

```yaml
---
name: git-status
version: 1.0.0
description: Show git working tree status. Use when user asks about git state, uncommitted changes, or branch info.
author: gibb.eri.sh
modes: [Dev, Global]
read_only: true
timeout: 10
---
```

---

## 3. Tool Definitions

Each skill defines one or more **tools** in the Markdown body. Tools are parsed from H3 headers (`###`).

### 3.1 Tool Block Format

```markdown
### tool_name

Brief description of what this tool does.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| query | string | yes | The search query. |
| limit | integer | no | Max results (default: 10). |

#### Command

```bash
program arg1 arg2 {{query}} --limit={{limit}}
```
```

### 3.2 Tool Name Constraints

- Lowercase alphanumeric and underscores: `[a-z0-9_]+`
- 1-32 characters
- Must be unique within the skill

**Valid**: `git_status`, `search`, `run_tests`
**Invalid**: `git-status` (use underscore), `GitStatus`

### 3.3 Parameter Types

| Type | Description | JSON Mapping |
|------|-------------|--------------|
| `string` | Text value | `"value"` |
| `integer` | Whole number | `123` |
| `number` | Decimal number | `12.5` |
| `boolean` | True/false | `true` / `false` |
| `array` | List of strings | `["a", "b"]` |

### 3.4 Parameter Interpolation

Parameters are interpolated into the command using `{{name}}` syntax.

**Rules**:
1. Required parameters must be provided; missing = error.
2. Optional parameters with no value are omitted from the command.
3. Values are **shell-escaped** automatically (no injection possible).
4. Array values are joined with spaces: `["a", "b"]` → `a b`.

**Example**:
```yaml
# Parameters: { "query": "rust lang", "limit": 5 }
# Template:   curl "https://api.com/search?q={{query}}&n={{limit}}"
# Result:     curl "https://api.com/search?q=rust%20lang&n=5"
```

### 3.5 Command Execution Model

Commands are executed as **program + args**, never via shell.

```
# This template:
git log -n {{count}} --oneline

# Is parsed as:
program: "git"
args: ["log", "-n", "5", "--oneline"]

# NOT as:
sh -c "git log -n 5 --oneline"  # NEVER
```

**Implications**:
- No shell features: pipes (`|`), redirects (`>`), globbing (`*`), env vars (`$VAR`)
- No command chaining: `&&`, `||`, `;`
- Spaces in arguments are preserved (properly quoted)

**If you need shell features**, wrap in a script:
```markdown
#### Command

```bash
./scripts/complex-task.sh {{input}}
```
```

---

## 4. Output Handling

### 4.1 Output Capture

| Stream | Captured | Notes |
|--------|----------|-------|
| stdout | Yes | Primary output |
| stderr | Yes | Merged with stdout |
| exit code | Yes | 0 = success, non-zero = error |

### 4.2 Output Truncation

Large outputs are truncated to prevent memory issues and prompt bloat.

```
┌─────────────────────────────────────┐
│ HEAD: First 2KB of output           │
├─────────────────────────────────────┤
│ ... [truncated N bytes] ...         │
├─────────────────────────────────────┤
│ TAIL: Last 2KB of output            │
└─────────────────────────────────────┘
```

**Limits**:
- `HEAD_SIZE`: 2048 bytes
- `TAIL_SIZE`: 2048 bytes
- `MAX_TOTAL`: 4096 bytes (for prompt injection)

### 4.3 Output Format

Skill output is wrapped in a standard JSON envelope:

```json
{
  "success": true,
  "exit_code": 0,
  "output": "string content here...",
  "truncated": false,
  "duration_ms": 150
}
```

For errors:
```json
{
  "success": false,
  "exit_code": 1,
  "output": "error: not a git repository",
  "error": "Command failed with exit code 1",
  "duration_ms": 50
}
```

### 4.4 Structured Output (Optional)

If the skill outputs valid JSON to stdout, it's parsed and included:

```json
{
  "success": true,
  "exit_code": 0,
  "output": "{\"status\": \"ok\", \"count\": 5}",
  "parsed": {
    "status": "ok",
    "count": 5
  },
  "duration_ms": 200
}
```

---

## 5. Execution Environment

### 5.1 Working Directory

Commands execute with CWD set to:
1. **If in a git repo**: Repository root
2. **Otherwise**: User's home directory (`~`)

Rationale: Most skills operate on project files or user data.

### 5.2 Environment Variables

Skills inherit a **sanitized** subset of the parent environment:

**Inherited**:
- `PATH`
- `HOME`
- `USER`
- `LANG`, `LC_*`
- `TERM`

**Blocked** (security):
- `GITHUB_TOKEN`, `*_TOKEN`, `*_KEY`, `*_SECRET`
- `AWS_*`, `OPENAI_*`, `ANTHROPIC_*`

**Injected** by Gibberish:
- `GIBBERISH_SKILL_NAME`: Current skill name
- `GIBBERISH_SKILL_DIR`: Path to skill directory
- `GIBBERISH_MODE`: Current mode (Global/Dev/Meeting)

### 5.3 Timeout & Cancellation

- Default timeout: 30 seconds
- Configurable per-skill: 1-300 seconds
- On timeout/cancel: `SIGTERM`, then `SIGKILL` after 5s
- Child processes are killed via process group (`killpg`)

---

## 6. Security Model

### 6.1 Threat Model

Skills can execute **arbitrary code**. We mitigate, not eliminate, risk.

| Threat | Mitigation |
|--------|------------|
| Command injection | No shell execution; args are escaped |
| Infinite loops | Timeout with hard kill |
| Resource exhaustion | Output truncation; timeout |
| Secret exfiltration | Env var filtering; network flag is advisory |
| Malicious skill install | User must manually add skills (no auto-download) |

### 6.2 Trust Levels

| Level | Description | Approval |
|-------|-------------|----------|
| **Bundled** | Ships with gibb.eri.sh | Auto-run if `read_only: true` |
| **User** | Added by user to skills dir | Requires first-run approval |
| **Untrusted** | Downloaded from internet | Always ask + warning banner |

### 6.3 Approval Flow

1. **First execution** of a new skill: Show skill name, description, command preview. User approves or rejects.
2. **Subsequent executions**: Follow `read_only` / `always_ask` flags.
3. **Skill modification**: If SKILL.md changes, re-prompt for approval.

---

## 7. Integration with Gibberish

### 7.1 FunctionGemma Prompt Generation

Skills are included in the dynamic tool manifest:

```
You are a model that can do function calling with the following functions:

[Native Tools]
- typer: Type text at cursor
- web_search: Search the web

[Skills]
- git_status: Show git working tree status
- summarize_url: Summarize a webpage
```

### 7.2 Mode Filtering

Skills declare compatible modes in frontmatter:

```yaml
modes: [Dev]  # Only available when VS Code/terminal is focused
```

If omitted, defaults to `["Global"]` (always available).

### 7.3 Policy Integration

Skills generate `ToolPolicy` entries:

```rust
ToolPolicy {
    name: "git_status",
    read_only: true,
    always_ask: false,
    source: ToolSource::Skill("git"),
}
```

---

## 8. Complete Examples

### 8.1 Simple Skill: Git Status

```markdown
---
name: git
version: 1.0.0
description: Git operations. Use for checking status, viewing history, or seeing changes.
author: gibb.eri.sh
modes: [Dev, Global]
---

### git_status

Show the current git status with short format.

#### Parameters

None.

#### Command

```bash
git status --short --branch
```

### git_log

Show recent commit history.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| count | integer | no | Number of commits (default: 10) |

#### Command

```bash
git log --oneline -n {{count}}
```

### git_diff

Show uncommitted changes.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| staged | boolean | no | Show only staged changes |

#### Command

```bash
git diff {{staged:--staged}}
```
```

### 8.2 Skill with Network: URL Summarizer

```markdown
---
name: summarize
version: 1.0.0
description: Summarize webpages or text. Use when user wants a summary of a URL or article.
modes: [Global]
read_only: true
network: true
timeout: 60
---

### summarize_url

Fetch and summarize a webpage.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| url | string | yes | The URL to summarize |

#### Command

```bash
npx @anthropic-ai/summarize {{url}}
```
```

### 8.3 Skill with Script: Complex Workflow

```markdown
---
name: deploy
version: 1.0.0
description: Deploy to Vercel. Use when user says "deploy" or "ship it".
modes: [Dev]
read_only: false
always_ask: true
network: true
timeout: 120
---

### deploy_preview

Deploy a preview build to Vercel.

#### Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| message | string | no | Deployment message |

#### Command

```bash
./scripts/deploy.sh preview {{message}}
```
```

---

## 9. Validation Rules

### 9.1 Parser Errors (Fatal)

- Missing required frontmatter field
- Invalid YAML syntax
- No tool definitions found
- Duplicate tool names

### 9.2 Parser Warnings (Non-Fatal)

- Unknown frontmatter field (ignored)
- Tool with no parameters section (allowed)
- Very long description (truncated at 256 chars)

### 9.3 Runtime Errors

- Command not found (`ENOENT`)
- Permission denied (`EACCES`)
- Timeout exceeded
- Missing required parameter

---

## 10. Appendix: Rust Structs

```rust
/// Parsed SKILL.md file
pub struct SkillDefinition {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    pub modes: Vec<Mode>,
    pub read_only: bool,
    pub always_ask: bool,
    pub timeout_secs: u32,
    pub network: bool,
    pub tools: Vec<ToolDefinition>,
    pub source_path: PathBuf,
}

/// A single tool within a skill
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ParameterDefinition>,
    pub command: CommandTemplate,
}

/// Tool parameter
pub struct ParameterDefinition {
    pub name: String,
    pub param_type: ParameterType,
    pub required: bool,
    pub description: String,
    pub default: Option<serde_json::Value>,
}

/// Parsed command template
pub struct CommandTemplate {
    pub program: String,
    pub args: Vec<ArgFragment>,
}

/// Argument fragment (literal or interpolation)
pub enum ArgFragment {
    Literal(String),
    Variable { name: String, modifier: Option<String> },
}
```

---

## 11. Open Questions

> To be resolved before implementation:

1. **Boolean parameter syntax**: `{{staged:--staged}}` vs `{{#staged}}--staged{{/staged}}`?
2. **Default values**: In frontmatter or parameter table?
3. **Skill discovery**: Should we support a curated skill registry in the future?
4. **Skill dependencies**: Can a skill declare it needs `node`, `python`, etc.?
5. **Multi-command tools**: Should a single tool support multiple commands (fallback chain)?

---

## Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0-draft | 2025-12-27 | Initial specification |
