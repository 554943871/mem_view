# memView

[English](README.md) | [简体中文](README.zh-CN.md)

memView is a minimal, read-only macOS app for browsing local Markdown memory
repositories. It is built for memory repos that contain many `.md` files and
Mermaid diagrams, with a focus on fast local viewing rather than editing.

The current MVP is tuned for:

```text
/Users/god/project/easy-kid-mem
```

## Download

The first packaged macOS build is available from GitHub Releases:

[Download memView v0.1.1](https://github.com/554943871/mem_view/releases/tag/v0.1.1)

Direct dmg asset:

[memView_0.1.1_x64.dmg](https://github.com/554943871/mem_view/releases/download/v0.1.1/memView_0.1.1_x64.dmg)

## Features

- Local-only macOS desktop app, packaged with Tauri.
- Read-only Markdown browsing. It does not modify the memory repository.
- Scans the memory repo and shows a file tree.
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

Build the macOS app and dmg:

```bash
npm run build
```

Build outputs:

```text
src-tauri/target/release/bundle/macos/memView.app
src-tauri/target/release/bundle/dmg/memView_0.1.1_x64.dmg
```

## Repository Path

The MVP currently uses a fixed local memory repo path in two places:

- `src/App.svelte`
- `src-tauri/src/lib.rs`

To point memView at another memory repo, update:

```ts
const repoPath = "/Users/god/project/easy-kid-mem";
```

and:

```rust
const DEFAULT_REPO: &str = "/Users/god/project/easy-kid-mem";
```

Future versions should move this into a local preference or folder picker.

## Project Structure

```text
.
+-- src/                 # Svelte frontend
+-- src-tauri/           # Tauri and Rust backend
+-- package.json         # npm scripts and frontend dependencies
+-- vite.config.ts       # Vite config
+-- README.md
```

## MVP Notes

- macOS-focused.
- Read-only by design.
- The memory repo path is fixed for now.
- The app is optimized for local Markdown and Mermaid memory libraries, not for
  general-purpose note editing.
