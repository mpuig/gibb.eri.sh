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

## Implicit Referencing ("The Magic Word: This")

Because gibb.eri.sh knows your context, you can use **Deictic references**.

- **User says:** "Summarize *this*."
- **LLM sees:** `{"tool": "summarize", "source": "active_selection"}`
- **Context Engine:**
    1. Checks active app (e.g., Chrome).
    2. Grabs currently selected text (via Accessibility API).
    3. Feeds text to the tool.

We also support **"what I just said"**:
- **User says:** "Create a todo from *what I just said*."
- **System:** Grabs the last 30 seconds of transcript history.

This allows generic commands to work across any application without specific integrations.

## Available Tools

### Global
- **System Control**: Volume, Mute, Media keys.
- **App Launcher**: Opens applications.
- **Web Search**: Knowledge lookups (Wikipedia by default, extensible to other sources).

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