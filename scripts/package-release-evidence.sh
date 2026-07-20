#!/usr/bin/env bash
set -euo pipefail
platform=$1
output=$2
mkdir -p "$output"
git rev-parse HEAD > "$output/source-commit.txt"
rustc --version --verbose > "$output/rust-toolchain.txt"
node --version > "$output/node-version.txt"
cargo tauri --version > "$output/tauri-cli-version.txt"
find target/release/bundle -type f -print0 | sort -z | xargs -0 shasum -a 256 > "$output/SHA256SUMS.unsigned-$platform"
cp docs/VERIFY_RELEASE.md "$output/VERIFY_RELEASE.md"
cp docs/offline/VERSION "$output/docs-version.txt"
shasum -a 256 docs/offline/VERSION docs/offline/README.md docs/VERIFY_RELEASE.md docs/EVIDENCE.md docs/CREDIBILITY.md docs/CROSS_VALIDATION.md docs/JOURNAL_FORMAT.md docs/EXPORTS.md docs/SIXDOF_DERIVATION.md > "$output/OFFLINE_DOCS_SHA256SUMS"
find . -type f -name '*.cdx.json' -exec cp {} "$output/" \;
