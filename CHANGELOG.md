# Changelog

## v0.2.6

- Fixed Find so it consistently shows the current and total match count.
- Show `0 / 0` when a query has no matches.

## v0.2.5

- Added Find in the currently open Markdown document.
- Added match highlighting, match count, previous/next navigation, and keyboard shortcuts.

## v0.2.4

- Added GitHub Release based online updates through Tauri updater.
- Added startup update checks with an explicit user-triggered update button.
- Added release scripts for signed updater packages and `latest.json`.

## v0.2.1

- Replaced the default placeholder app icon with a polished memView icon.
- Added release-side macOS signature verification notes from the v0.2.0 repack.

## v0.2.0

- Added selectable memory repository support.
- The app no longer hard-codes a repository path.
- Added native macOS folder picker and manual path input.
- Reopens the last selected repository on launch.
- Added stable bundle identifier and generated app icon assets.
- Renamed release asset to `arm64` to match the current macOS build.
- Replaced machine-specific Rust tests with a generated temporary test repo.
- Added privacy notes, release checklist, and MIT license.

## v0.1.2

- Added selectable memory repository support.
- Added native folder picker permission.
- Updated English and Chinese README files.

## v0.1.1

- Added Simplified Chinese and English UI switching.
- Added Chinese README.

## v0.1.0

- Initial memView MVP.
- Local Markdown repository scanning.
- Markdown and Mermaid rendering.
- Mermaid detail viewer with zoom and pan.
