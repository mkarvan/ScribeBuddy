# ScribeBuddy

macOS meeting transcription tool that transcribes Zoom, MS Teams, and Google Meet in real-time using local Whisper models with Metal GPU acceleration.

## Features

- **Zero-setup audio capture** — ScreenCaptureKit (macOS 13+) captures meeting app audio with one permission prompt. No driver install needed.
- **Local Whisper inference** — Runs entirely on-device. `small` model (~461MB). Metal-accelerated on Apple Silicon, CPU fallback on Intel.
- **Real-time transcription** — ~4s end-to-end latency with 3-second audio chunks. RMS silence detection prevents hallucination.
- **Speaker labels** — "You" (local mic) and "Remote" (meeting audio) tracked independently.
- **Markdown export** — Timestamped transcript exported to `.md` files.
- **System tray** — Lives in the menu bar, shows recording state.

## Requirements

- macOS 13.0+ (ScreenCaptureKit) or macOS 12 (BlackHole fallback)
- Apple Silicon recommended (Metal GPU) — Intel supported via CPU fallback
- ~500MB disk for the default `small` Whisper model

## Quick Start

```bash
# Clone and build
git clone https://github.com/yourname/scribebuddy
cd ScribeBuddy

# Install dependencies
cargo build --release

# Run
cargo run --release
```

On first launch, select a model size and click the download button. Then select your meeting app, hit Start, and join your meeting.

## Architecture

```
[Microphone] ──► cpal ──► Ring Buffer ──┐
                                        ├──► AudioProcessor ──► WhisperEngine ──► Transcript
[Meeting App] ──► SCK ──► Ring Buffer ──┘
```

- `scribebuddy-core` — Platform-independent library with audio capture, Whisper transcription, session management, and Markdown export
- `src-tauri` — Tauri 2 macOS app with 16 commands, system tray, and Channels API
- `frontend` — Web UI (HTML/CSS/JS) with live transcript rendering, model download progress, and export dialogs

## Project Structure

```
├── Cargo.toml                  # Workspace root
├── crates/scribebuddy-core/    # Core library (no Tauri deps, fully testable)
│   └── src/
│       ├── audio/              # AudioSource trait, SCK, cpal, resampler, processor
│       ├── transcription/      # WhisperEngine, ModelManager, TranscriptSegment
│       ├── session/            # SessionManager state machine, TranscriptAccumulator
│       └── export/             # Markdown exporter
├── src-tauri/                  # Tauri 2 application
│   └── src/                    # main.rs, lib.rs, commands.rs, state.rs, tray.rs
└── frontend/                   # Web frontend
    └── src/                    # main.js, transcript.js, controls.js, settings.js
```

## Build & Test

```bash
cargo check --workspace     # Type-check everything
cargo test -p scribebuddy-core  # Run core library tests (9 tests)
cargo build --release       # Production build
```

## Export Format

```markdown
# Meeting Transcript

**Date:** 2026-05-02
**Duration:** 00:45:12

---

**[00:00:05] You:**
Hello, thanks for joining the call today.

**[00:00:12] Remote:**
Hi, glad to be here. Let me share my screen.

---
*Transcribed by ScribeBuddy*
```

## Stack

| Crate | Purpose |
|-------|---------|
| whisper-rs 0.16 | Whisper.cpp bindings (Metal) |
| screencapturekit 0.3 | macOS system audio capture |
| cpal 0.15 | Microphone capture |
| rubato 0.14 | Audio resampling to 16kHz |
| ringbuf 0.4 | Lock-free SPSC ring buffers |
| crossbeam-channel 0.5 | Thread communication |
| Tauri 2 | App framework + system tray |
| tauri-plugin-dialog 2 | Native save dialogs |

## License

MIT
