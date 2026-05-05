#!/bin/bash
# Build ScribeBuddy, sign it, and strip the quarantine flag.
#
#   ./build.sh           — build to target/release/bundle/macos/
#   ./build.sh --install — build + copy to /Applications
#
# On the very first run this script will create a local signing certificate
# (requires your login password once). After that, no prompts.
set -e

CERT_NAME="ScribeBuddy Dev"
BUNDLE="target/release/bundle/macos/ScribeBuddy.app"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ── One-time certificate setup ────────────────────────────────────────────────
if ! security find-identity -v -p codesigning 2>/dev/null | grep -q "$CERT_NAME"; then
    "$SCRIPT_DIR/setup-cert.sh"
fi

# ── Build (runs the test suite first via beforeBuildCommand) ──────────────────
cargo tauri build

# ── Sign and strip quarantine ─────────────────────────────────────────────────
echo "→ Signing…"
codesign --force --deep --sign "$CERT_NAME" "$BUNDLE"
xattr -rd com.apple.quarantine "$BUNDLE" 2>/dev/null || true

# ── Optional: install to /Applications ───────────────────────────────────────
if [[ "$1" == "--install" ]]; then
    cp -R "$BUNDLE" /Applications/ScribeBuddy.app
    xattr -rd com.apple.quarantine /Applications/ScribeBuddy.app 2>/dev/null || true
    echo "✓ Installed: /Applications/ScribeBuddy.app"
else
    echo "✓ Built: $BUNDLE"
fi
