# Verify an Ascent release

Ascent publishes platform-signed installers, an independently signed update artifact, SHA-256 checksums, a CycloneDX SBOM, and a GitHub build-provenance attestation for each protected release. Signed installers are not claimed to be byte-identical: Apple and Windows signatures and trusted timestamps intentionally vary. The unsigned source commit, pinned toolchain record, SBOM, and unsigned-payload checksum manifest establish the repeatable build inputs.

1. Verify the GitHub attestation with `gh attestation verify <artifact> --repo roshanshah11/ascent`.
2. Verify `shasum -a 256 -c SHA256SUMS` on macOS or `Get-FileHash -Algorithm SHA256` on Windows.
3. On macOS, run `codesign --verify --deep --strict Ascent.app`, `spctl --assess --type execute Ascent.app`, and `xcrun stapler validate Ascent.app`.
4. On Windows, open the installer signature properties or run `signtool verify /pa /all <installer>`.
5. Confirm the app, mission-review bundle schema, and bundled offline documentation versions match the release manifest. Open the golden `.ascent-review` bundle with networking disabled; its member hashes and journal replay must verify before the review is shown.

Signing credentials live only in protected `release-macos` and `release-windows` environments. Pull requests and fork jobs run verification only and cannot access those secrets.
