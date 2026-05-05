# ── ScribeBuddy build targets ────────────────────────────────────────────────
#
# Usage:
#   make build           — test → build → sign → strip quarantine
#   make dev             — hot-reload dev window
#   make test            — run the core test suite only
#   make install         — copy signed .app to /Applications
#   make clean           — wipe cargo build artifacts
#
# One-time setup (creates a local code-signing certificate so macOS retains
# Screen & System Audio Recording permission across rebuilds):
#   make setup-cert
#
# The SIGNING_IDENTITY is stored in .signing_identity after setup-cert.
# Override at any time:  make build SIGNING_IDENTITY="My Cert Name"
# ─────────────────────────────────────────────────────────────────────────────

APP      := ScribeBuddy
BUNDLE   := target/release/bundle/macos/$(APP).app
IDENTITY_FILE := .signing_identity

# Load saved identity if it exists, otherwise fall back to ad-hoc (-).
# Ad-hoc works but does NOT persist TCC permissions across rebuilds.
ifneq ($(wildcard $(IDENTITY_FILE)),)
    SIGNING_IDENTITY := $(shell cat $(IDENTITY_FILE))
else
    SIGNING_IDENTITY := -
endif

.PHONY: build dev test install clean setup-cert

## Run tests, build, sign, and strip quarantine.
build:
	cargo tauri build
	@echo "→ Signing with identity: $(SIGNING_IDENTITY)"
	codesign --force --deep --sign "$(SIGNING_IDENTITY)" "$(BUNDLE)"
	xattr -rd com.apple.quarantine "$(BUNDLE)" 2>/dev/null || true
	@echo "✓ Built and signed: $(BUNDLE)"

## Hot-reload dev window (no signing needed).
dev:
	cargo tauri dev

## Run the core test suite.
test:
	cargo test -p scribebuddy-core

## Copy the signed app to /Applications.
install: build
	cp -R "$(BUNDLE)" /Applications/$(APP).app
	xattr -rd com.apple.quarantine /Applications/$(APP).app 2>/dev/null || true
	@echo "✓ Installed to /Applications/$(APP).app"

## Wipe build artifacts.
clean:
	cargo clean

## One-time setup: create a local self-signed code-signing certificate.
## This certificate gives the app a stable identity so macOS TCC retains
## the Screen & System Audio Recording permission across rebuilds.
setup-cert:
	@echo "Creating local code-signing certificate '$(APP) Dev' in Keychain…"
	@security find-certificate -c "$(APP) Dev" >/dev/null 2>&1 && \
		echo "Certificate '$(APP) Dev' already exists — skipping creation." || \
		security create-certificate \
			-p CodeSigning \
			-n "$(APP) Dev" \
			-t self-signed 2>/dev/null || \
		( echo "" && \
		  echo "  Automatic creation failed (expected on macOS 14+)." && \
		  echo "  Please create the certificate manually:" && \
		  echo "" && \
		  echo "  1. Open Keychain Access" && \
		  echo "  2. Menu → Certificate Assistant → Create a Certificate…" && \
		  echo "  3. Name:             $(APP) Dev" && \
		  echo "  4. Identity Type:    Self Signed Root" && \
		  echo "  5. Certificate Type: Code Signing" && \
		  echo "  6. Click Continue → Done" && \
		  echo "" && \
		  echo "  Then re-run: make setup-cert" && \
		  echo "" )
	@security find-identity -v -p codesigning | grep -q "$(APP) Dev" && \
		( echo "$(APP) Dev" > $(IDENTITY_FILE) && \
		  echo "✓ Saved signing identity to $(IDENTITY_FILE)" && \
		  echo "  Future 'make build' calls will use it automatically." ) || \
		echo "⚠ Certificate not found in codesigning identities yet."
