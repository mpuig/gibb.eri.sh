# Agentic Tools

A local LLM monitors transcribed text for commands. When it detects an intent, it executes an action.

```
You say: "Search Wikipedia for Rust programming"
                    ↓
            [FunctionGemma LLM]
                    ↓
    {"tool": "browser", "query": "Rust programming language"}
                    ↓
            [Tool Executor]
                    ↓
        Browser opens with Wikipedia results
```

## How It Works

### The Pipeline

1. **STT Commit**: Text is finalized by the transcription engine
2. **Router**: Lightweight check (regex/heuristics) flags potential intents
3. **LLM Parse**: FunctionGemma extracts structured data
4. **Executor**: Runs the appropriate tool

### Event-Driven Architecture

The Tools plugin listens for `stt:stream_commit` events:

```rust
// plugins/tools/src/lib.rs
app.listen_global("stt:stream_commit", move |event| {
    if let Some(text) = event.payload() {
        // Debounce to avoid LLM overload during rapid speech
        router.process(text);
    }
});
```

### The Router

Not every sentence needs LLM analysis. The router uses cheap heuristics first:

```rust
fn should_analyze(text: &str) -> bool {
    let triggers = ["search", "open", "find", "look up", "what is"];
    triggers.iter().any(|t| text.to_lowercase().contains(t))
}
```

If no trigger words are found, we skip the expensive LLM call.

### FunctionGemma

We use Google's [FunctionGemma](https://huggingface.co/google/functiongemma) model, quantized for efficiency:

| Property | Value |
|----------|-------|
| Size | ~200MB (int8) |
| Latency | ~100ms |
| Accuracy | High for simple intents |

The model is trained specifically for function calling, not general chat.

## Available Tools

### Wikipedia Search

```json
{
  "tool": "wikipedia",
  "action": "search",
  "query": "Rust programming language"
}
```

Opens the default browser with Wikipedia search results.

### More Coming Soon

- **Web Search**: General search queries
- **Calculator**: Math expressions
- **Timer**: "Set a timer for 5 minutes"
- **Notes**: "Remember to buy milk"

## Adding Custom Tools

Tools are defined in `plugins/tools/src/tools/`:

```rust
// tools/my_tool.rs
pub struct MyTool;

impl Tool for MyTool {
    fn name(&self) -> &str { "my_tool" }

    fn execute(&self, params: &Value) -> Result<String> {
        // Do something useful
        Ok("Done!".into())
    }
}
```

Register it in the tool registry:

```rust
registry.register(Box::new(MyTool));
```

## Privacy Considerations

All intent recognition happens **locally**:
- No cloud API calls
- No data leaves your device
- Works offline

The LLM sees only the committed transcript text, never raw audio.

## Debouncing

Rapid speech can generate many commits in quick succession. We debounce to avoid overwhelming the LLM:

```rust
const DEBOUNCE_MS: u64 = 500;

// Only process if 500ms have passed since last analysis
if now - last_analysis > DEBOUNCE_MS {
    router.process(text);
    last_analysis = now;
}
```
