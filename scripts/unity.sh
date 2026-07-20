#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "$0")/.." && pwd)"
version="$(awk '/m_EditorVersion:/{print $2}' "$project_root/visualizer/AscentUnity/ProjectSettings/ProjectVersion.txt")"
editor="/Applications/Unity/Hub/Editor/$version/Unity.app/Contents/MacOS/Unity"
if [[ ! -x "$editor" ]]; then
  echo "Unity editor $version is not installed through Unity Hub" >&2
  exit 2
fi
exec "$editor" -projectPath "$project_root/visualizer/AscentUnity" "$@"
