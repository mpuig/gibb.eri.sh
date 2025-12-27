# SKILL.md Specification v1.0

> **Status**: Draft (Phase 0)
> **Author**: gibb.eri.sh team
> **Last Updated**: 2025-12-27

## Overview

A **Skill** is a user-defined tool described in a Markdown file. Skills extend gibb.eri.sh without requiring Rust code or recompilation.

---

## 1. File Structure

### 1.1 Directory Layout

```
~/Library/Application Support/gibb.eri.sh/skills/
├── git/
│   └── SKILL.md
├── summarize/
│   └── SKILL.md
└── my-custom-skill/
    └── SKILL.md
```

**Platform Paths:**
- **macOS:** `~/Library/Application Support/gibb.eri.sh/skills/`
- **Linux:** `~/.local/share/gibb.eri.sh/skills/`
- **Windows:** `%APPDATA%\gibb.eri.sh\skills\`

---

## 2. Interpolation Syntax

Arguments in the command string are replaced by JSON values.

| Syntax | Behavior | Example |
| :--- | :--- | :--- |
| `{{key}}` | Replaced by `args["key"]`. Error if missing. | `echo {{msg}}` -> `echo hello` |
| `{{key:default}}` | Uses `default` if key is missing/null. | `git log -n {{count:10}}` -> `git log -n 10` |
| `{{key:--flag}}` | If boolean true, replaced by `--flag`. Else empty. | `git diff {{staged:--staged}}` |
| `{{key:.}}` | If missing, replaced by `.`. Useful for paths. | `ls {{path:.}}` -> `ls .` |

---

## 3. SKILL.md Format

### 3.1 Structure

```markdown
---
# YAML Frontmatter (required)
name: skill-name
version: 1.0.0
description: What this skill does and when to use it.
---

# Markdown Body (tool definitions)
```

### 3.2 Frontmatter Fields

#### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Skill identifier. Must match directory name. |
| `version` | string | Semver version (e.g., `1.0.0`). |
| `description` | string | 1-256 chars. Describes purpose and trigger phrases. |

#### Optional Fields (Gibberish Extensions)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `modes` | array | `["Global"]` | Context modes: `Global`, `Dev`, `Meeting`. |
| `read_only` | boolean | `false` | If true, can auto-run without approval. |
| `always_ask` | boolean | `false` | Always require approval, even if read_only. |
| `timeout` | integer | `30` | Max execution time in seconds (1-300). |

---

## 4. Tool Definitions

Each skill defines one or more **tools** in the Markdown body. Tools are parsed from H3 headers (`###`).

### 4.1 Tool Block Format

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

### 4.2 Command Execution Model

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

---

## 5. Security Model

### 5.1 Threat Model

Skills can execute **arbitrary code**. We mitigate, not eliminate, risk.

| Threat | Mitigation |
|--------|------------|
| Command injection | No shell execution; args are escaped |
| Infinite loops | Timeout with hard kill |
| Resource exhaustion | Output truncation; timeout |
| Secret exfiltration | Env var filtering; network flag is advisory |

### 5.2 Trust Levels

| Level | Description | Approval |
|-------|-------------|----------|
| **Bundled** | Ships with gibb.eri.sh | Auto-run if `read_only: true` |
| **User** | Added by user to skills dir | Requires first-run approval |

---

## 6. Complete Example: Git

```markdown
---
name: git
version: 1.0.0
description: Git operations.
modes: [Dev]
---

### git_status

Show the current git status with short format.

#### Parameters
(None)

#### Command
```bash
git status --short --branch
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