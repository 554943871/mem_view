# Privacy

memView is a local-only reader for Markdown memory repositories.

## Local Data Access

memView reads files only from the local folder you choose in the app. It scans
Markdown files to build the file tree, extract document titles, and detect
Mermaid diagrams.

## No Network Sync

memView does not upload your memory repository, document contents, paths, or
settings to a server.

The app currently has no telemetry, analytics, crash reporting, or account
login.

## Local Settings

memView stores a small amount of local app state:

- selected language
- last selected memory repository path

These settings are stored locally by the app runtime and can be changed from
the app UI.

## Read-Only Behavior

memView is designed as a read-only viewer. It does not edit, create, delete, or
move files in the selected memory repository.

## Release Note

GitHub release builds are distributed as downloadable macOS dmg files. If a
build is not signed and notarized with an Apple Developer ID, macOS Gatekeeper
may warn before opening it.
