# ScribeBuddy

macOS meeting transcription tool. Captures Zoom, Teams, Google Meet, and any browser tab in real-time using a local Whisper model — no data leaves your machine.

## Requirements

- macOS 15 (Sequoia) or later
- Xcode 16+ Command Line Tools — `xcode-select --install`
- Rust — [rustup.rs](https://rustup.rs)
- Tauri CLI — `cargo install tauri-cli --version "^2.0"`

## Building

### Step 1 — Clone and build

```bash
git clone https://github.com/mkarvan/ScribeBuddy.git
cd ScribeBuddy
./build.sh
```

On the **first run**, `build.sh` automatically creates a local code-signing certificate in your Keychain so macOS retains the Screen Recording permission across rebuilds. macOS will ask for your **login password once** to set the trust — that's normal and only happens this one time.

After that, every subsequent `./build.sh` builds silently with no prompts.

To build and copy straight to `/Applications`:

```bash
./build.sh --install
```

### Step 2 — First launch

The app is signed with a local certificate, not notarized by Apple, so Gatekeeper blocks it the first time:

1. Double-click **ScribeBuddy.app** — dismiss the "cannot be opened" dialog
2. Open **System Settings → Privacy & Security**
3. Scroll down and click **Open Anyway**

This only happens once.

### Step 3 — Grant permissions

On first launch macOS will ask for:

- **Microphone** — to capture your voice
- **Screen & System Audio Recording** — to capture meeting audio (no screen content is recorded)

Click **Allow** for both.

### Step 4 — Download a Whisper model

1. In the settings bar, choose a **Model** size (`small` is a good default — 461 MB, ~4 s latency)
2. Click the **↙ Download** button and wait for it to complete

Models are stored in `~/Library/Application Support/ai.scribebuddy.app/` and reused across launches.

## Development

```bash
# Hot-reload dev window (no signing needed)
cargo tauri dev

# Run the test suite
cargo test -p scribebuddy-core

# Type-check without a full build
cargo check --workspace
```

The test suite runs automatically before every `./build.sh` via `beforeBuildCommand` in `tauri.conf.json`.

## Troubleshooting

### Screen Recording permission resets after every rebuild

This means the app was built without the signing certificate (e.g. with `cargo tauri build` directly instead of `./build.sh`). Always use `./build.sh` — it signs the app with a stable local identity so macOS retains the permission.

If the permission is stuck in a denied state, reset it and re-grant:

```bash
tccutil reset ScreenCapture ai.scribebuddy.app
```

Then relaunch and click **Allow** when prompted.

### App isn't listed in Screen & System Audio Recording settings

Start the app first (so macOS registers it), then open **System Settings → Privacy & Security → Screen & System Audio Recording**.

### Microphone not captured

Open **System Settings → Privacy & Security → Microphone** and confirm ScribeBuddy is enabled.

### "Apple cannot verify" on every launch

You're running the app without signing it first. Always use `./build.sh` — it signs the app automatically.

## Architecture

```
[Microphone]   ──► cpal (ring buffer) ──┐
                                        ├──► AudioProcessor ──► Transcriber ──► Transcript
[Meeting App]  ──► ScreenCaptureKit ────┘        │
                   (48 kHz stereo → mono)    rubato resampler
                                             (source Hz → 16 kHz)
                                                  │
                                             WhisperEngine
                                             (Metal / CPU)
```

- `AudioProcessor::run_with<T: Transcriber>` is generic over the transcription engine — `WhisperEngine` in production, `MockTranscriber` in tests
- Session lifecycle (`Idle → Recording → Paused → Stopped`) is owned by `SessionManager`; Tauri commands are thin wrappers
- All file writes use write-to-temp-then-rename for crash safety

## Stack

| Crate | Purpose |
|-------|---------|
| whisper-rs 0.16 | Whisper.cpp bindings with Metal acceleration |
| screencapturekit 0.3 | macOS system audio capture |
| cpal 0.15 | Microphone capture |
| rubato 0.14 | Audio resampling to 16 kHz |
| ringbuf 0.4 | Lock-free SPSC ring buffers |
| Tauri 2 | App framework, system tray, native dialogs |

## License

MIT
