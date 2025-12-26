# Developer Guide

Welcome, contributor! This guide will help you extend gibb.eri.sh.

## Prerequisites

- **Rust** (stable, 1.75+)
- **Node.js** (20+)
- **macOS** (for now—Linux/Windows coming)

## Quick Start

```bash
# Clone
git clone https://github.com/mpuig/gibb.eri.sh
cd gibb.eri.sh

# Install frontend dependencies
cd apps/desktop && npm install

# Run in development mode
npm run tauri dev
```

## Project Structure

```
gibb.eri.sh/
├── apps/
│   └── desktop/          # Tauri app
│       ├── src/          # React frontend
│       └── src-tauri/    # Rust backend
├── crates/               # Pure Rust libraries
├── plugins/              # Tauri plugin adapters
├── scripts/              # Build & conversion tools
└── docs/                 # This documentation
```

## Development Workflow

### Making Changes

1. **Pure logic?** → Edit in `crates/`
2. **UI interaction?** → Edit in `plugins/`
3. **Frontend?** → Edit in `apps/desktop/src/`

### Testing

```bash
# Run all Rust tests
cargo test --workspace

# Run a specific crate's tests
cargo test -p gibberish-bus
```

### Building

```bash
# Debug build
cd apps/desktop && npm run tauri dev

# Release build
npm run tauri build
```

## Guides

- **[Adding Features](./adding-features.md)** — The proper way to extend functionality
- **[Adding Languages](./adding-languages.md)** — Support new languages via NeMo CTC
- **[Headless Engine](./headless-engine.md)** — Use the core without UI

## Code Style

### Rust

- Use `rustfmt` (default settings)
- Prefer `Result<T>` over panics
- Document public APIs with `///`

### TypeScript

- Use Prettier (default settings)
- Prefer functional components with hooks
- Type everything (no `any`)

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/mpuig/gibb.eri.sh/issues)
- **Discussions**: [GitHub Discussions](https://github.com/mpuig/gibb.eri.sh/discussions)
