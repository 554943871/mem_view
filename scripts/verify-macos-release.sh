#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  scripts/verify-macos-release.sh [--gatekeeper] <path-to-app-or-dmg>

Checks that a macOS .app or .dmg contains a structurally valid code signature.
Use --gatekeeper for fully signed and notarized public releases.
USAGE
}

require_gatekeeper=0
target=""
mount_dir=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --gatekeeper)
      require_gatekeeper=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      if [[ -n "$target" ]]; then
        usage >&2
        exit 2
      fi
      target="$1"
      shift
      ;;
  esac
done

if [[ -z "$target" ]]; then
  usage >&2
  exit 2
fi

cleanup() {
  if [[ -n "$mount_dir" && -d "$mount_dir" ]]; then
    hdiutil detach "$mount_dir" >/dev/null 2>&1 || true
    rmdir "$mount_dir" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

if [[ "$target" == *.dmg ]]; then
  hdiutil verify "$target"
  mount_dir="$(mktemp -d "${TMPDIR:-/tmp}/memview-dmg.XXXXXX")"
  hdiutil attach -nobrowse -readonly -mountpoint "$mount_dir" "$target"
  app_path="$(find "$mount_dir" -maxdepth 1 -name '*.app' -type d -print -quit)"
  if [[ -z "$app_path" ]]; then
    echo "No .app bundle found in dmg: $target" >&2
    exit 1
  fi
elif [[ -d "$target" && "$target" == *.app ]]; then
  app_path="$target"
else
  echo "Expected a .app directory or .dmg file: $target" >&2
  exit 2
fi

echo "Verifying code signature: $app_path"
codesign --verify --deep --strict --verbose=4 "$app_path"
codesign -dv --verbose=4 "$app_path" 2>&1

if command -v syspolicy_check >/dev/null 2>&1; then
  echo "Running syspolicy_check distribution..."
  set +e
  syspolicy_output="$(syspolicy_check distribution "$app_path" 2>&1)"
  syspolicy_status=$?
  set -e
  printf '%s\n' "$syspolicy_output"

  if [[ $syspolicy_status -ne 0 ]]; then
    if grep -q "Codesign Error" <<<"$syspolicy_output"; then
      echo "Release failed: app bundle has a codesign error and may open as damaged." >&2
      exit "$syspolicy_status"
    fi

    if [[ $require_gatekeeper -eq 1 ]]; then
      echo "Release failed: Gatekeeper distribution checks did not pass." >&2
      exit "$syspolicy_status"
    fi

    echo "Gatekeeper distribution check did not fully pass."
    echo "This is expected for ad-hoc signed builds when the remaining issue is notarization."
  fi
fi
