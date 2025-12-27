# gibb.eri.sh

> **"Most voice bots suck. I decided to build the one I actually wanted to use."**

This is the documentation for **gibb.eri.sh** (v0.9.0), a local-first Voice OS for macOS.

## The Story

I build voice bots professionally. I've seen the sausage made:
-   **Latency:** Sending audio to the cloud takes 500ms minimum.
-   **Privacy:** Your voice data is training someone else's model.
-   **Context:** Cloud bots don't know you're looking at VS Code.

I wanted a tool that felt instant, respected my privacy, and could actually *do things* on my computer. Since it didn't exist, I built it.

## What is it?

It's a desktop app that sits in your menu bar. It listens (when you tell it to), transcribes in real-time, and executes **Skills**.

### Key Capabilities

1.  **Context Awareness:** It polls the OS to know what app is focused. If you're in a terminal, it enables Git tools. If you're in Zoom, it enables Note-taking tools.
2.  **Zero Latency:** We use a custom **Zero-Copy Audio Bus** in Rust to stream microphone data directly to local ONNX models.
3.  **Agent Skills:** You can extend the capabilities by dropping a `SKILL.md` file into a folder. It supports Bash, Python, and Node scripts.

## Who is this for?

**Developers and Hackers.**
This is 0.9.0 software. It's powerful, but it assumes you know what a "terminal" is. Ideally, it will become useful for everyone, but right now, it's a power tool.

## Tech Stack

| Component | Tech | Why? |
|-----------|------|------|
| **Core** | Rust | Memory safety, threading, no GC pauses. |
| **UI** | Tauri + React | HTML/CSS is flexible, Electron is too heavy. |
| **STT** | Sherpa-ONNX | Best streaming accuracy on Apple Silicon. |
| **Reasoning** | FunctionGemma | Optimized for tool calling, runs locally. |

## Next Steps

- [Read the Philosophy](./philosophy/README.md)
- [Check out the Features](./features/README.md)
- [Build a Skill](./guides/adding-features.md)