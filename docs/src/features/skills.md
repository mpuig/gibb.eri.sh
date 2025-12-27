# Agent Skills

**Extend gibb.eri.sh with Bash, Python, or Node.js.**

The "Hands" of the Voice OS are extensible. We use the [Agent Skills](https://github.com/agentskills/agentskills) standard (`SKILL.md`) to let you add new tools without writing Rust.

## How it works

1.  **Drop a file:** Put a `SKILL.md` file in `~/Library/Application Support/sh.eri.gibb/skills/`.
2.  **Define the tool:** Describe what it does and the command to run.
3.  **Speak:** The LLM sees your new tool and uses it when relevant.

## Example: Summarizer Skill

Create `skills/summarize/SKILL.md`:

```markdown
---
name: super_summarizer
version: 1.0.0
description: Extract and summarize content from URLs.
---

## Tools

### extract_content

Extracts clean text from a URL.

**Command:**
```bash
npx @steipete/summarize {{source}} --extract-only
```

**Parameters:**
- `source` (string, required): The URL.
```

## The Spec

We support a strict subset of the Agent Skills standard for safety.

### File Format
- **Frontmatter:** YAML with `name` and `description`.
- **Tool Blocks:** Markdown sections defining the tool name, description, command, and parameters.

### Execution Model
- **No Shell:** We execute the binary directly (`program` + `args`). No `sh -c`.
- **Interpolation:** `{{param}}` in the command block is replaced by the JSON argument from the LLM.

### Context Awareness
You can restrict skills to specific modes by adding a `modes` field to the frontmatter:

```yaml
modes: [Dev, Global]
```
