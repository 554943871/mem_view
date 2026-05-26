#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
product_name="$(node -p "require('${repo_root}/src-tauri/tauri.conf.json').productName")"
version="$(node -p "require('${repo_root}/src-tauri/tauri.conf.json').version")"
app_path="${repo_root}/src-tauri/target/release/bundle/macos/${product_name}.app"
binary_path="${app_path}/Contents/MacOS/mem-view"

if [[ ! -d "$app_path" ]]; then
  echo "Missing app bundle: $app_path" >&2
  exit 1
fi

binary_description="$(file "$binary_path")"
if [[ "$binary_description" == *"arm64"* ]]; then
  arch_label="arm64"
elif [[ "$binary_description" == *"x86_64"* ]]; then
  arch_label="x64"
else
  arch_label="$(uname -m)"
fi

dmg_dir="${repo_root}/src-tauri/target/release/bundle/dmg"
dmg_path="${dmg_dir}/${product_name}_${version}_${arch_label}.dmg"
staging_dir="$(mktemp -d "${TMPDIR:-/tmp}/memview-dmg.XXXXXX")"

cleanup() {
  rm -rf "$staging_dir"
}
trap cleanup EXIT

mkdir -p "$dmg_dir"
ditto "$app_path" "${staging_dir}/${product_name}.app"
ln -s /Applications "${staging_dir}/Applications"
hdiutil create -volname "$product_name" -srcfolder "$staging_dir" -ov -format UDZO "$dmg_path"

echo "Created $dmg_path"
