# Design Principles

gibb.eri.sh is designed around three constraints:

1. **[Privacy First](./privacy.md)** — Audio processed locally, never uploaded
2. **[Low Latency](./latency.md)** — Time-to-first-token < 200ms
3. **[Rust + Tauri](./stack.md)** — Native performance, minimal runtime overhead

## Trade-offs

| Constraint | Trade-off |
|------------|-----------|
| Local-only processing | Requires ~500MB model download |
| Low latency | Higher CPU usage for streaming |
| Native code | Rust learning curve for contributors |
