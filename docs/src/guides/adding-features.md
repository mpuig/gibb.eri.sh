# Adding Features

gibb.eri.sh is designed to be extensible. Depending on what you want to add, you have two paths: **Agent Skills** or **Native Plugins**.

## Which path should I take?

| Goal | Path |
| :--- | :--- |
| Add a tool (Git, Jira, Docker, Scripts) | **Agent Skill** (Recommended) |
| Add a new audio processor or OS sensor | **Native Crate/Plugin** |
| Change the core STT/LLM logic | **Native Crate** |

---

## 1. The Easy Way: Agent Skills
If your feature involves running a CLI command or a script, **do not write Rust**. Use a [Skill Pack](../features/skills.md). It's faster, safer, and doesn't require recompiling the app.

---

## 2. The Native Way: Plugins
Use this for features that need low-level OS access or high-performance data processing.

### The Golden Rule
> **Domain logic in `crates/`. Tauri glue in `plugins/`.**

Never put business logic in plugins. Plugins are thin adapters that translate between Rust and JavaScript.

### Step-by-Step Example: Word Counter
Let's add a "Native" feature that counts words in real-time.

#### Step 1: Create the Domain Crate
```bash
cd crates
cargo new --lib wordcount
```

`crates/wordcount/src/lib.rs`:
```rust
pub struct WordCounter {
    total: usize,
}

impl WordCounter {
    pub fn new() -> Self { Self { total: 0 } }
    pub fn add(&mut self, text: &str) -> usize {
        self.total += text.split_whitespace().count();
        self.total
    }
}
```

#### Step 2: Create the Tauri Plugin
```bash
cd plugins
cargo new --lib wordcount
```

`plugins/wordcount/src/lib.rs`:
```rust
use gibberish_events::event_names::STT_STREAM_COMMIT;
use gibberish_events::StreamCommitEvent;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("wordcount")
        .setup(|app, _api| {
            app.manage(Mutex::new(WordCounter::new()));

            // Listen for events using the shared contract
            app.listen_any(STT_STREAM_COMMIT, move |event| {
                if let Ok(payload) = serde_json::from_str::<StreamCommitEvent>(event.payload()) {
                    // Logic here...
                }
            });
            Ok(())
        })
        .build()
}
```

## Testing Tips

### Dependency Injection
Don't use `std::process::Command` directly in your crates. Use the `SystemEnvironment` trait from `plugins/tools`. This allows you to mock OS calls in unit tests without actually executing code on the host.

### Shared Events
Always use the `gibberish-events` crate for inter-plugin communication. This prevents runtime "stringly-typed" errors.