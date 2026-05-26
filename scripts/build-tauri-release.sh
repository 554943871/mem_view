#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

default_key="$HOME/.tauri/memview-updater.key"
key_path="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$default_key}"

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" && -f "$key_path" ]]; then
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$key_path")"
fi

export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

if [[ "$#" -eq 0 ]]; then
  tauri build --bundles app
  "$repo_root/scripts/create-macos-dmg.sh"
else
  exec tauri build "$@"
fi
