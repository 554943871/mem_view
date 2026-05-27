# Changelog

## v0.2.11

- Restored automatic update checks when the app starts.
- Kept startup update checks quiet unless a new version is available.

## v0.2.10

- Added browser-style back and forward navigation controls in the titlebar.
- Disabled history arrows when no previous or next view is available.
- Fixed the sidebar selected-document highlight after following links between files.

## v0.2.9

- Moved global toolbar actions into the macOS titlebar row.
- Refined the overlay titlebar spacing and restored native dragging in the top region.
- Reworked update checking into a modal flow with release notes, install confirmation, and progress.
- Made the no-update result more visible with a top-centered tip and animated checking state.
- Updated the sidebar repository header with a clearer quick-switch control and separate open-new-repo action.

## v0.2.8

- Added Mermaid diagram image copying to the system clipboard.
- Added copy controls in both inline diagrams and the enlarged diagram viewer.
- Improved Mermaid export rendering with white backgrounds and preserved text/style output.

## v0.2.7

- Keep the current Markdown document open when refreshing the repository index.
- Restore the reader scroll position after refresh when the same document still exists.
- Fall back to the default README only when the previously open document is gone.
- Changed the update check toolbar icon so it no longer looks like a download action.

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
