# Adding Features

This guide walks through the proper way to add new functionality to gibb.eri.sh.

## The Golden Rule

> **Domain logic in `crates/`. Tauri glue in `plugins/`.**

Never put business logic in plugins. Plugins are thin adapters.

## Step-by-Step Example: Adding a Word Counter

Let's add a feature that counts words in real-time.

### Step 1: Create the Crate

```bash
cd crates
cargo new --lib gibberish-wordcount
```

Edit `crates/gibberish-wordcount/Cargo.toml`:

```toml
[package]
name = "gibberish-wordcount"
version = "0.1.0"
edition = "2021"

[dependencies]
# Keep dependencies minimal
```

### Step 2: Implement the Logic

`crates/gibberish-wordcount/src/lib.rs`:

```rust
/// Counts words in a string, handling edge cases.
pub fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Tracks running word count across multiple segments.
pub struct WordCounter {
    total: usize,
}

impl WordCounter {
    pub fn new() -> Self {
        Self { total: 0 }
    }

    pub fn add(&mut self, text: &str) -> usize {
        let count = count_words(text);
        self.total += count;
        self.total
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn reset(&mut self) {
        self.total = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_simple_sentence() {
        assert_eq!(count_words("hello world"), 2);
    }

    #[test]
    fn handles_extra_whitespace() {
        assert_eq!(count_words("  hello   world  "), 2);
    }

    #[test]
    fn tracks_running_total() {
        let mut counter = WordCounter::new();
        assert_eq!(counter.add("hello world"), 2);
        assert_eq!(counter.add("foo bar baz"), 5);
        counter.reset();
        assert_eq!(counter.total(), 0);
    }
}
```

### Step 3: Add to Workspace

Edit root `Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing crates
    "crates/gibberish-wordcount",
]
```

### Step 4: Create the Plugin

```bash
cd plugins
cargo new --lib tauri-plugin-wordcount
```

Edit `plugins/tauri-plugin-wordcount/Cargo.toml`:

```toml
[package]
name = "tauri-plugin-wordcount"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = { version = "2", features = ["plugin"] }
gibberish-wordcount = { path = "../../crates/gibberish-wordcount" }
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["sync"] }
```

### Step 5: Implement the Plugin

`plugins/tauri-plugin-wordcount/src/lib.rs`:

```rust
use gibberish_wordcount::WordCounter;
use std::sync::Mutex;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime, State,
};

struct WordCountState(Mutex<WordCounter>);

#[tauri::command]
fn get_count(state: State<WordCountState>) -> usize {
    state.0.lock().unwrap().total()
}

#[tauri::command]
fn reset_count(state: State<WordCountState>) {
    state.0.lock().unwrap().reset();
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("wordcount")
        .setup(|app, _api| {
            app.manage(WordCountState(Mutex::new(WordCounter::new())));

            // Listen for transcript commits
            let app_handle = app.clone();
            app.listen_global("stt:stream_commit", move |event| {
                if let Some(payload) = event.payload() {
                    if let Ok(segment) = serde_json::from_str::<Segment>(payload) {
                        let state = app_handle.state::<WordCountState>();
                        let total = state.0.lock().unwrap().add(&segment.text);
                        let _ = app_handle.emit("wordcount:update", total);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_count, reset_count])
        .build()
}

#[derive(serde::Deserialize)]
struct Segment {
    text: String,
}
```

### Step 6: Define Permissions

Create `plugins/tauri-plugin-wordcount/permissions/default.json`:

```json
{
  "default": {
    "permissions": ["wordcount:get_count", "wordcount:reset_count"]
  }
}
```

### Step 7: Register the Plugin

In `apps/desktop/src-tauri/src/lib.rs`:

```rust
pub fn run() {
    tauri::Builder::default()
        // ... existing plugins
        .plugin(tauri_plugin_wordcount::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Step 8: Consume in Frontend

```typescript
import { listen, invoke } from '@tauri-apps/api';

function WordCountDisplay() {
    const [count, setCount] = useState(0);

    useEffect(() => {
        // Get initial count
        invoke<number>('plugin:wordcount|get_count').then(setCount);

        // Listen for updates
        const unlisten = listen<number>('wordcount:update', (event) => {
            setCount(event.payload);
        });

        return () => { unlisten.then(f => f()); };
    }, []);

    return <div>Words: {count}</div>;
}
```

## Testing Tips

### Dependency Injection
Don't use `std::process::Command` directly. Use the `SystemEnvironment` trait so you can mock OS calls.

```rust
// Good
fn execute(&self, env: &dyn SystemEnvironment) {
    env.execute_command("git", &["status"])
}

// Bad
fn execute(&self) {
    std::process::Command::new("git").arg("status")
}
```

## Checklist

Before submitting a PR:

- [ ] Logic is in `crates/`, not `plugins/`
- [ ] Crate has unit tests
- [ ] Plugin has minimal dependencies
- [ ] Permissions are defined
- [ ] No `unwrap()` in production code
- [ ] Public APIs are documented
