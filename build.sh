#!/bin/bash
# Build ScribeBuddy, sign it, and strip the quarantine flag.
# Run with --install to also copy it to /Applications.
#
# Requires a "ScribeBuddy Dev" code-signing certificate in your Keychain.
# See README.md → Building → Step 1 for the one-time setup.
set -e

IDENTITY="${SIGNING_IDENTITY:-ScribeBuddy Dev}"
BUNDLE="target/release/bundle/macos/ScribeBuddy.app"

cargo tauri build

echo "→ Signing ($IDENTITY)…"
codesign --force --deep --sign "$IDENTITY" "$BUNDLE"
xattr -rd com.apple.quarantine "$BUNDLE" 2>/dev/null || true

if [[ "$1" == "--install" ]]; then
    cp -R "$BUNDLE" /Applications/ScribeBuddy.app
    xattr -rd com.apple.quarantine /Applications/ScribeBuddy.app 2>/dev/null || true
    echo "✓ Installed: /Applications/ScribeBuddy.app"
else
    echo "✓ Built: $BUNDLE"
fi
