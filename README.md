# gibb.eri.sh

**Local-first voice AI for macOS. It listens, it types, it runs scripts.**

> "I build voice bots for a living. I decided to build the one I actually wanted to use."

## The Pitch

I build voice bots for a living. Most of them are slow, privacy-invasive, and fragile. I decided to build the one I actually wanted to use.

gibb.eri.sh is a **Context-Aware Voice OS**. It runs entirely on `localhost`.
It doesn't just transcribe speech; it detects what you're doing (coding in VS Code, meeting in Zoom) and executes relevant tools.

**Tech Stack:**
*   **Rust:** For the heavy lifting (Audio, Inference, State).
*   **Tauri v2:** For the UI (lightweight, secure).
*   **ONNX Runtime:** For local model inference (Sherpa, FunctionGemma).
*   **Agent Skills:** For extensibility (drop a Markdown file, get a new tool).

## What it does (v0.9.0)

1.  **Transcribes in Real-Time:** <45ms latency. Words appear as you speak.
2.  **Understands Context:** "Summarize *this*" reads your active window's selection.
3.  **Executes Skills:** "Undo last commit" runs `git reset`.
4.  **Respects Privacy:** Audio never leaves your machine. Models run on the Apple Neural Engine.

## Who is this for?

Right now? **Developers and Power Users.**
If you know what `git status` means and you're comfortable with a terminal, this is for you.
If you want a polished consumer product, wait for v1.0.

## Installation

```bash
# Clone the repo
git clone https://github.com/mpuig/gibb.eri.sh
cd gibb.eri.sh

# Install dependencies (Node + Rust required)
npm install

# Run dev mode
npm run tauri dev
```

## The Architecture

It's a Modular Monolith. We use a **Zero-Copy Audio Bus** to stream mic data to multiple consumers (VAD, STT, Visualizer) without locking.

*   **The Ears:** Sherpa-ONNX (Streaming) + Parakeet (Batch).
*   **The Brain:** Google FunctionGemma (Router).
*   **The Hands:** Agent Skills (Bash/Python scripts defined in Markdown).

[Read the Architecture Docs](./docs/src/introduction.md)

## License

MIT. Hack on it.