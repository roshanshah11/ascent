#!/usr/bin/env bash
set -euo pipefail
keychain="$RUNNER_TEMP/ascent-signing.keychain-db"
password=$(openssl rand -hex 24)
security create-keychain -p "$password" "$keychain"
security set-keychain-settings -lut 21600 "$keychain"
security unlock-keychain -p "$password" "$keychain"
printf '%s' "$APPLE_CERTIFICATE_P12" | base64 --decode > "$RUNNER_TEMP/certificate.p12"
security import "$RUNNER_TEMP/certificate.p12" -P "$APPLE_CERTIFICATE_PASSWORD" -A -t cert -f pkcs12 -k "$keychain"
security set-key-partition-list -S apple-tool:,apple: -k "$password" "$keychain"
security list-keychains -d user -s "$keychain"
