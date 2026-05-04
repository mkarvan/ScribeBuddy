# ScribeBuddy

macOS meeting transcription tool that transcribes Zoom, Teams, and Google Meet in real-time using local Whisper models with Metal GPU acceleration.

## Features

- **Zero-setup audio capture** — ScreenCaptureKit captures meeting app audio with one permission prompt. No driver install needed.
- **Local Whisper inference** — Runs entirely on-device. No data leaves your machine. Metal-accelerated on Apple Silicon, CPU fallback on Intel.
- **Real-time transcription** — ~4 s end-to-end latency with 3-second audio chunks. RMS silence gate prevents hallucination on quiet audio.
- **Speaker labels** — "You" (local mic) and "Remote" (meeting audio) tracked independently.
- **Markdown export** — Timestamped transcript exported to `.md` files via native save dialog.
- **Auto-save** — Transcripts saved automatically to `~/Library/Application Support/ai.scribebuddy.app/sessions/` on stop.
- **Settings persistence** — Model size, language, chunk duration, and target app saved across restarts.
- **System tray** — Lives in the menu bar; shows recording state.

## Requirements

- **macOS 15.0 or later** (Sequoia+) — required for ScreenCaptureKit audio and Metal resource sets used by Whisper
- **Apple Silicon recommended** — Metal GPU acceleration; Intel supported via CPU fallback
- **Xcode 16+** with Command Line Tools (`xcode-select --install`)
- **Rust** — install via [rustup.rs](https://rustup.rs)
- **~500 MB** disk space for the default `small` Whisper model

## Building the macOS App

### 1. Install the Tauri CLI

```bash
cargo install tauri-cli --version "^2.0"
```

### 2. Clone and build

```bash
git clone https://github.com/mkarvan/ScribeBuddy.git
cd ScribeBuddy
cargo tauri build
```

`MACOSX_DEPLOYMENT_TARGET=15.0` is set automatically via `.cargo/config.toml` — no manual env vars needed.

The build produces:

```
target/release/bundle/macos/ScribeBuddy.app   ← drag to /Applications
```

Build time is roughly 3–5 minutes on Apple Silicon (Whisper compiles from source with Metal support).

### 3. First launch — bypass Gatekeeper

The app is unsigned, so macOS will block it the first time:

1. Double-click **ScribeBuddy.app** → "Apple cannot verify..."
2. Open **System Settings → Privacy & Security**
3. Scroll down and click **Open Anyway**

After that it launches normally every time.

### 4. Grant permissions

On first run, macOS will ask for:
- **Microphone** — required to capture your voice
- **Screen Recording** — required for ScreenCaptureKit to capture meeting audio (no screen content is recorded)

## Development

```bash
# Type-check everything without a full build
cargo check --workspace

# Run all tests (34 unit + integration tests, no model needed)
cargo test -p scribebuddy-core

# Dev mode with hot-reload (opens the app window directly)
cargo tauri dev
```

> Integration tests in `crates/scribebuddy-core/tests/pipeline.rs` inject a `MockTranscriber` so they run without a real Whisper model.

## First Run

1. Open ScribeBuddy from `/Applications` (or the `.app` bundle)
2. In Settings, choose a **model size** (start with `small` — best speed/accuracy tradeoff)
3. Click **Download Model** and wait for it to complete
4. Select your **meeting app** from the dropdown (e.g. Google Chrome, Zoom)
5. Click **Start** before joining your meeting
6. When done, click **Stop** — transcript is auto-saved and available for Markdown export

## Architecture

```
[Microphone]   ──► cpal (SPSC ring buffer) ──┐
                                             ├──► AudioProcessor ──► Transcriber ──► Transcript
[Meeting App]  ──► ScreenCaptureKit ─────────┘        │
                   (48 kHz stereo → mono)         rubato resampler
                                                  (source Hz → 16 kHz)
                                                       │
                                                  WhisperEngine
                                                  (Metal / CPU)
```

**Key design points:**
- `AudioProcessor::run_with<T: Transcriber>` is generic over the transcription engine — `WhisperEngine` in production, `MockTranscriber` in tests
- Session lifecycle (`Idle → Recording → Paused → Stopped`) is owned entirely by `SessionManager`; Tauri commands are thin wrappers
- All file writes (config, transcripts) use write-to-temp-then-rename for crash safety

## Project Structure

```
├── .cargo/config.toml            # MACOSX_DEPLOYMENT_TARGET=15.0
├── Cargo.toml                    # Workspace root
├── crates/scribebuddy-core/      # Platform-independent core library
│   ├── src/
│   │   ├── audio/
│   │   │   ├── capture.rs        # AudioSource trait, ring buffer helpers
│   │   │   ├── cpal_capture.rs   # Microphone via CPAL
│   │   │   ├── screencapturekit.rs  # System audio via SCK
│   │   │   ├── resampler.rs      # rubato SincFixedIn wrapper
│   │   │   └── processor.rs      # AudioProcessor, run_with<T: Transcriber>
│   │   ├── transcription/
│   │   │   ├── mod.rs            # Transcriber trait
│   │   │   ├── engine.rs         # WhisperEngine (implements Transcriber)
│   │   │   ├── model.rs          # ModelManager — download, path resolution
│   │   │   └── segment.rs        # TranscriptSegment helpers
│   │   ├── session/
│   │   │   ├── manager.rs        # SessionManager state machine
│   │   │   └── transcript.rs     # TranscriptAccumulator
│   │   ├── export/
│   │   │   └── markdown.rs       # Markdown exporter
│   │   └── lib.rs                # Public types: SessionConfig, Speaker, ModelSize …
│   └── tests/
│       └── pipeline.rs           # Integration tests (silence gate, segment flow)
├── src-tauri/                    # Tauri 2 application shell
│   ├── icons/                    # App icons (SVG source + PNG exports)
│   ├── src/
│   │   ├── commands.rs           # 17 Tauri commands
│   │   ├── state.rs              # AppState (config, session, accumulated segments)
│   │   ├── tray.rs               # System tray setup
│   │   └── lib.rs                # Tauri builder, window events
│   ├── Entitlements.plist        # Microphone + network entitlements
│   └── tauri.conf.json           # Bundle config, macOS minimum 15.0
└── frontend/                     # Web UI (vanilla HTML/CSS/JS, no framework)
    └── src/
        ├── main.js               # App bootstrap, Tauri event listeners
        ├── transcript.js         # Live transcript rendering
        ├── controls.js           # Record/pause/stop controls
        └── settings.js           # Settings panel, model download UI
```

## Stack

| Crate / Library | Purpose |
|-----------------|---------|
| whisper-rs 0.16 | Whisper.cpp bindings with Metal acceleration |
| screencapturekit 0.3 | macOS system audio capture (ScreenCaptureKit) |
| cpal 0.15 | Cross-platform microphone capture |
| rubato 0.14 | High-quality audio resampling (source → 16 kHz) |
| ringbuf 0.4 | Lock-free SPSC ring buffers between audio threads |
| crossbeam-channel 0.5 | Thread-safe segment/error channels |
| parking_lot 0.12 | Mutex/RwLock for shared state |
| Tauri 2 | App framework, system tray, native dialogs |
| tauri-plugin-dialog 2 | Native save-file dialog for Markdown export |

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

## License

MIT
