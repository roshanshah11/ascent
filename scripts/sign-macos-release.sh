#!/usr/bin/env bash
set -euo pipefail
payload=$1
mkdir -p "$payload/signed"
app=$(find "$payload" -type d -name '*.app' -print -quit)
identity=$(security find-identity -v -p codesigning | sed -n '1s/.*"\(.*\)"/\1/p')
test -n "$app" && test -n "$identity"
codesign --force --deep --options runtime --timestamp --sign "$identity" "$app"
codesign --verify --deep --strict --verbose=2 "$app"
ditto -c -k --keepParent "$app" "$payload/signed/Ascent-macos.zip"
xcrun notarytool submit "$payload/signed/Ascent-macos.zip" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
xcrun stapler staple "$app"
xcrun stapler validate "$app"
# Rebuild the update archive after stapling so the shipped app contains the
# notarization ticket, then sign that exact archive independently for Tauri.
ditto -c -k --keepParent "$app" "$payload/signed/Ascent-macos.zip"
cargo tauri signer sign -k "$TAURI_SIGNING_PRIVATE_KEY" -p "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" "$payload/signed/Ascent-macos.zip"
hdiutil create -volname Ascent -srcfolder "$app" -ov -format UDZO "$payload/signed/Ascent-macos.dmg"
codesign --force --timestamp --sign "$identity" "$payload/signed/Ascent-macos.dmg"
xcrun notarytool submit "$payload/signed/Ascent-macos.dmg" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
xcrun stapler staple "$payload/signed/Ascent-macos.dmg"
xcrun stapler validate "$payload/signed/Ascent-macos.dmg"
find "$payload/release-evidence" -type f -exec cp {} "$payload/signed/" \;
find "$payload" -type f -name '*.cdx.json' -exec cp {} "$payload/signed/" \;
(
  cd "$payload/signed"
  for file in *; do
    case "$file" in SHA256SUMS*) continue ;; esac
    test -f "$file" && shasum -a 256 "$file"
  done
) > "$payload/signed/SHA256SUMS.macos"
