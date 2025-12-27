# Agentic Tools

gibb.eri.sh doesn't just transcribe—it *understands*. And crucially, it understands *context*.

## The Concept

A local LLM monitors your speech for **intents**. But unlike dumb assistants, gibb.eri.sh changes its capabilities based on what you are doing.

## Contextual Modes

The available tools change dynamically based on your environment.

### 1. Global Mode (Default)
Always available.
- **Tools:** `web_search`, `app_launcher`, `system_control`
- **Example:** "Open Figma", "Turn up the volume", "What is quantum computing"

### 2. Meeting Mode
Triggered when: A meeting app (Zoom, Teams, Slack) is using the microphone.
- **Tools:** `transcript_marker`, `add_todo`
- **Example:** "Flag this as important", "Add action item for Marc"

### 3. Dev Mode
Triggered when: An IDE (VS Code, IntelliJ, Terminal) is the active window.
- **Tools:** `git_voice`, `file_finder`
- **Example:** "Undo last commit", "Find the user struct"

## How It Works

### The Pipeline

```
Context Engine ─▶ [State: Dev Mode]
                        │
                        ▼
User Speech ───▶ [Router] ───▶ Tool Registry (Filter: Dev + Global)
                                        │
                                        ▼
                                [FunctionGemma LLM]
                                (Only sees ~5 relevant tools)
                                        │
                                        ▼
                                [Executor] ─▶ git_voice
```

### Event-Driven Architecture

The Tools plugin listens for `stt:stream_commit` events and combines them with the latest `ContextState`:

```rust
// plugins/tools/src/router.rs

// 1. Get current mode (e.g., Dev)
let mode = state.context.effective_mode();

// 2. Filter registry
let tools = registry.tools_for_mode(mode);

// 3. Build system prompt with ONLY those tools
let prompt = build_prompt(tools);

// 4. Run Inference
let result = llm.infer(prompt, user_text);
```

### Why Dynamic Filtering?

1.  **Accuracy:** The LLM isn't confused by "Book a flight" when you're trying to "Book a meeting room". Smaller search space = fewer hallucinations.
2.  **Performance:** Less text in the system prompt = faster inference.
3.  **Safety:** Destructive tools (like `git reset`) are only exposed when you are explicitly focusing on your code editor.

## Context Injection

The LLM doesn't just see your command—it sees your **environment**. Before every inference, we inject a context snapshot:

```
Current Context:
Mode: Dev
Active App: VS Code
Clipboard: "RuntimeError: Connection refused at port 8080"
Date: 2025-12-27
```

This enables **implicit referencing**:

| You say | LLM infers |
|---------|-----------|
| "Search this error" | `web_search{query: "RuntimeError: Connection refused"}` |
| "Open that app" | Resolves from active window context |
| "What does this mean?" | Uses clipboard or selection |

### What Gets Injected

- **Mode**: Current mode (Global, Dev, Meeting)
- **Active App**: Name of the focused application
- **Clipboard**: First ~200 chars of clipboard text
- **Selection**: Currently selected text (via Accessibility API)
- **Date**: Current date (for scheduling-aware commands)

### The Magic Word: "This"

Because gibb.eri.sh knows your context, you can use **deictic references**:

- **User says:** "Summarize *this*."
- **Context Engine:**
    1. Checks active app (e.g., Chrome).
    2. Grabs currently selected text (via Accessibility API).
    3. The LLM sees this in the context and fills the argument automatically.

We also support **"what I just said"**:
- **User says:** "Create a todo from *what I just said*."
- **System:** Grabs the last 30 seconds of transcript history.

This allows generic commands to work across any application without specific integrations.

## Feedback Loop

Tools don't just execute—they **respond**. After a tool runs, the result is fed back to the LLM for summarization.

### The Flow

```
User: "What is quantum computing?"
      │
      ▼
[FunctionGemma] → web_search{query: "quantum computing"}
      │
      ▼
[Wikipedia API] → {title: "Quantum computing", summary: "...uses qubits..."}
      │
      ▼
[FunctionGemma] → "Quantum computing uses qubits instead of classical bits,
                   enabling exponential speedups for certain problems."
      │
      ▼
[UI] → Displays summary (or speaks via TTS)
```

### Why This Matters

1. **Accessibility**: You don't have to read raw JSON or API responses.
2. **Natural Language**: Results are summarized conversationally.
3. **Composability**: The model can chain thoughts based on results.

## Available Tools

### Global
- **System Control**: Volume, Mute, Media keys.
- **App Launcher**: Opens applications.
- **Web Search**: Knowledge lookups (Wikipedia by default, extensible to other sources).
- **The Typer**: Voice-controlled typing.
    - **Smart Injection**: Types short phrases char-by-char for natural interaction.
    - **Transparent Paste**: For long text blocks, it saves your current clipboard, pastes the content instantly via `Cmd+V`, and restores your original clipboard after a short delay.
    - **Context Awareness**: "Paste *this* here" knows to use the active selection as the source.

### Meeting
- **Transcript Marker**: Inserts `[FLAG]` or `[TODO]` tags into the transcript file.
- **Add Todo**: Appends a line to your daily notes.

### Development
- **Git Voice**: Wraps common git commands.
- **File Finder**: Uses `mdfind` (Spotlight) to locate files in the current project context.

## Adding Custom Tools

Tools are defined in `plugins/tools/src/tools/` and must implement `is_available_in(mode)`:

```rust
impl Tool for GitVoiceTool {
    fn name(&self) -> &'static str { "git_voice" }

    fn modes(&self) -> &'static [Mode] {
        &[Mode::Dev]
    }

    // ...
}
```