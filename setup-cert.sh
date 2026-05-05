#!/bin/bash
# Creates a local self-signed code-signing certificate in your login Keychain.
# Run this once; build.sh calls it automatically if the cert is missing.
#
# macOS will ask for your login password once to add the trust setting.
# That prompt is enforced by the OS and cannot be skipped.
set -e

CERT_NAME="ScribeBuddy Dev"

if security find-identity -v -p codesigning 2>/dev/null | grep -q "$CERT_NAME"; then
    echo "✓ Certificate '$CERT_NAME' already exists — nothing to do."
    exit 0
fi

echo "Creating signing certificate '$CERT_NAME'…"
echo "(macOS will ask for your login password once to set the trust — this is normal)"
echo ""

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Write an OpenSSL config that marks the cert for code signing.
cat > "$TMP/cert.conf" << 'CONF'
[req]
distinguished_name = dn
x509_extensions    = ext
prompt             = no

[dn]
CN = ScribeBuddy Dev

[ext]
keyUsage             = critical, digitalSignature
extendedKeyUsage     = critical, codeSigning
basicConstraints     = critical, CA:true
subjectKeyIdentifier = hash
CONF

# Generate key + self-signed certificate (10-year validity).
openssl genrsa -out "$TMP/key.pem" 2048 2>/dev/null
openssl req -new -x509 \
    -key "$TMP/key.pem" \
    -out "$TMP/cert.pem" \
    -days 3650 \
    -config "$TMP/cert.conf" 2>/dev/null

# Bundle into PKCS12 so `security import` can ingest both key and cert.
openssl pkcs12 -export \
    -out "$TMP/cert.p12" \
    -inkey "$TMP/key.pem" \
    -in  "$TMP/cert.pem" \
    -passout pass:tmp 2>/dev/null

# Import into the login keychain.
# -A lets codesign use the key without a per-use prompt.
security import "$TMP/cert.p12" \
    -k ~/Library/Keychains/login.keychain-db \
    -P tmp \
    -A \
    -f pkcs12 2>/dev/null

# Mark the certificate as trusted for code signing.
# This is the step that triggers the macOS password prompt.
security add-trusted-cert \
    -d \
    -r trustRoot \
    -k ~/Library/Keychains/login.keychain-db \
    "$TMP/cert.pem"

echo ""
echo "✓ Done. '$CERT_NAME' is now trusted for code signing."
