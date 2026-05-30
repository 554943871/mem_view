# Changelog

## v0.2.23

- Removed the startup system capability precheck and permission dialog.

## v0.2.22

- Fixed a macOS Screen Recording permission false negative by falling back to a tiny real capture probe when the system preflight result is stale.
- Added zoom and copy-image controls for Mermaid diagrams rendered inside HTML documents.

## v0.2.21

- Added startup system capability checks for macOS Screen Recording, clipboard availability, and GitHub network reachability.
- Added a system capability dialog that can request Screen Recording permission, open macOS privacy settings, recheck status, and prompt for relaunch when needed.

## v0.2.20

- Added HTML as a first-class document type alongside Markdown, including repository scanning, standalone file opening, local resource loading, outline extraction, find, link navigation, and annotation support.
- Added document last-updated time in the reader header.
- Added a copy button for the current document's full local path.
- Preserved each memory repository's expanded folders, open document, and reader state when switching repositories.

## v0.2.19

- Rebuilt the macOS arm64 package after the repository-state recovery fixes.
- Kept the GitHub release separate from the online updater until the updater signing key is available.

## v0.2.18

- Added a header action to copy the current document's full local file path.
- Remembered each memory repository's open document, folder expansion state, and reader scroll position when switching repositories.
- Added recovery guards so failed repository switches keep the previous view and startup can reopen the most recent repository.

## v0.2.17

- Changed annotation export to create a temporary bundle directory with a README, structured JSON, and image evidence.
- Added per-annotation visual evidence metadata and macOS region screenshots as an auxiliary source for Markdown and Mermaid annotation handling.
- Updated the copied annotation prompt to point agents at the generated README first.

## v0.2.16

- Moved annotation export work requirements into the temporary JSON bundle.
- Shortened the copied annotation prompt so it only points agents to the bundle.

## v0.2.15

- Fixed local Markdown image rendering for relative image paths in memView.
- Enabled the Tauri asset protocol so bundled app windows can display local SVG wireframes and other referenced images.

## v0.2.14

- Added Git pull before repository re-indexing from the sidebar refresh button.
- Updated documentation to describe Pull & Refresh as the only repository write path.

## v0.2.13

- Updated the annotation export prompt to distinguish document edits from document questions.
- Instructed Codex to avoid modifying files for question-only or unclear annotation notes.

## v0.2.12

- Added AI-driven annotation export for Markdown documents.
- Added source-aware annotation capture with visual boxes, editable notes, and page-scoped export.
- Added temporary JSON annotation bundles and copied Codex prompts for document updates.

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
