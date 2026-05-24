# Release Checklist

Use this checklist before publishing a public macOS build.

## Required Checks

- `npm run check`
- `npm run build`
- Launch the generated app from `src-tauri/target/release/bundle/macos/memView.app`
- Open a memory repository with the folder picker
- Restart the app and confirm the last repository reopens
- Open a Mermaid diagram detail view and verify zoom/pan
- Verify the release asset architecture with:

```bash
file src-tauri/target/release/bundle/macos/memView.app/Contents/MacOS/mem-view
```

## macOS Signing and Notarization

For a public stable release, sign and notarize with an Apple Developer ID.

This machine currently has no valid code signing identities, so local builds are
ad-hoc signed unless a Developer ID certificate is installed.

Check identities:

```bash
security find-identity -v -p codesigning
```

## Release Assets

Current local builds on this machine are arm64 builds. Publish the dmg as:

```text
memView_<version>_arm64.dmg
```

If Intel macOS support is required, build and test an x64 or universal artifact
separately.

## Recommended GitHub Release Notes

Include:

- release highlights
- supported architecture
- whether the build is signed/notarized
- dmg SHA-256 checksum
- known limitations
