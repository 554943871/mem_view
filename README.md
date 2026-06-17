# memView

[English](README.md) | [简体中文](README.zh-CN.md)

memView is a minimal macOS app for browsing and manually refreshing local
Markdown memory repositories. It is built for memory repos that contain many
`.md` files and Mermaid diagrams, with a focus on fast local viewing rather
than editing.

memView does not hard-code a memory repository. On first launch, choose a local
folder to open. The selected folder is saved locally and reopened next time.

## Download

The latest packaged macOS build is available from GitHub Releases:

[Download memView v0.2.41](https://github.com/554943871/mem_view/releases/tag/v0.2.41)

Direct dmg asset:

[memView_0.2.41_arm64.dmg](https://github.com/554943871/mem_view/releases/download/v0.2.41/memView_0.2.41_arm64.dmg)

## Important: macOS Gatekeeper Notice

Current public builds are macOS arm64 builds with ad-hoc signing, but they are
not notarized yet. Apple Developer ID signing and notarization are still
required before calling the app a fully polished stable macOS distribution.

If macOS says the downloaded dmg or app cannot be opened because Apple cannot
check it for malicious software, only proceed if you trust this repository and
the downloaded release asset. You can remove the quarantine flag from the dmg:

```bash
xattr -d com.apple.quarantine /path/to/memView_0.2.41_arm64.dmg
open /path/to/memView_0.2.41_arm64.dmg
```

If you already copied the app into Applications or another folder, remove
quarantine from the app bundle instead:

```bash
xattr -dr com.apple.quarantine /path/to/memView.app
```

## Features

- Local-only macOS desktop app, packaged with Tauri.
- Markdown browsing with an explicit Pull & Refresh action for Git repositories.
- The Pull & Refresh button runs `git pull --ff-only` before re-indexing files.
- Choose a local memory repository folder on first launch.
- Reopen the last selected memory repository on later launches.
- Scans the memory repo and shows documents plus source-material attachments in a file tree.
- Renders Markdown content with Mermaid diagrams.
- Shows file metadata and a simple read chain panel.
- Search by document title, path, and kind.
- Bilingual UI with English and Simplified Chinese support.
- Mermaid detail viewer:
  - top-right enlarge button for each diagram
  - solid white background
  - default fit-to-view layout after trimming Mermaid's internal empty bounds
  - mouse wheel zoom around the pointer
  - drag to pan
  - copy diagrams to the system image clipboard
  - `Fit`, `+`, `-`, and close controls

## Tech Stack

| Part | Technology | Purpose |
| --- | --- | --- |
| Desktop shell | Tauri 2 | Native macOS app packaging and Rust command bridge |
| Backend | Rust | Local filesystem scan, Markdown file reading, path safety checks |
| Frontend | Svelte + TypeScript | Minimal reader UI and interaction state |
| Build tool | Vite | Frontend development server and production build |
| Markdown | markdown-it | Markdown-to-HTML rendering |
| Diagrams | Mermaid | Mermaid diagram rendering |

## Development

Install dependencies:

```bash
npm install
```

Run the app in development mode:

```bash
npm run dev
```

Build the frontend only:

```bash
npm run build:web
```

Run Rust tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Run all checks:

```bash
npm run check
```

Build the macOS app and dmg:

```bash
npm run build
```

Build only the app and signed updater package:

```bash
npm run build:updater
```

Generate the GitHub Release updater metadata:

```bash
npm run release:latest-json
```

Verify the macOS release signature before uploading:

```bash
npm run verify:mac-release -- src-tauri/target/release/bundle/macos/memView.app
```

Build outputs:

```text
src-tauri/target/release/bundle/macos/memView.app
src-tauri/target/release/bundle/dmg/memView_0.2.0_x64.dmg
dist-release/latest.json
dist-release/memView_<version>_arm64.dmg
dist-release/memView_<version>_arm64.app.tar.gz
```

The generated dmg name may include `x64` when the local Node.js runtime is
x64. Check the actual app binary with `file`; current releases are uploaded as
`arm64` assets when the binary is arm64.

Online updates use the Tauri updater and the GitHub Release asset
`latest.json`; no separate update server is required.

## Repository Path

memView opens the repository folder chosen in the app. The path is persisted in
local app storage, not written into the memory repository. You can change it
from the repository picker in the sidebar.

## Project Structure

```text
.
+-- src/                 # Svelte frontend
+-- src-tauri/           # Tauri and Rust backend
+-- docs/                # Release checklist and packaging notes
+-- package.json         # npm scripts and frontend dependencies
+-- vite.config.ts       # Vite config
+-- README.md
```

## Privacy and License

- Privacy: [PRIVACY.md](PRIVACY.md)
- License: [MIT](LICENSE)
- Release checklist: [docs/RELEASE.md](docs/RELEASE.md)

## MVP Notes

- macOS-focused.
- Browsing-focused by design; the only repository write path is the explicit
  Pull & Refresh action.
- The memory repo path is user-selected and stored locally.
- The app is optimized for local Markdown and Mermaid memory libraries, not for
  general-purpose note editing.
