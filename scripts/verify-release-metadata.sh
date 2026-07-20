#!/usr/bin/env bash
set -euo pipefail
workspace_version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
tauri_version=$(node -p "require('./crates/ascent-app/tauri.conf.json').version")
app_version=$(node -p "require('./app/package.json').version")
docs_version=$(tr -d '[:space:]' < docs/offline/VERSION)
test "$workspace_version" = "$tauri_version"
test "$workspace_version" = "$app_version"
test "$workspace_version" = "$docs_version"
grep -q "version $workspace_version" docs/offline/README.md
cargo metadata --no-deps --format-version 1 | jq -e --arg version "$workspace_version" \
  '[.packages[] | select(.name != "xtask") | .version] | all(. == $version)' >/dev/null
if test "${GITHUB_REF_TYPE:-}" = "tag"; then
  test "${GITHUB_REF_NAME#v}" = "$workspace_version"
fi
echo "release metadata agrees at $workspace_version"
