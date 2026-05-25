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

- Verify that the app bundle signature is structurally valid before uploading:

```bash
npm run verify:mac-release -- src-tauri/target/release/bundle/macos/memView.app
npm run verify:mac-release -- src-tauri/target/release/bundle/dmg/memView_<version>_arm64.dmg
```

Do not publish a dmg if verification reports a `Codesign Error`. macOS may show
that broken bundle as damaged even when the dmg checksum is valid.

## macOS Signing and Notarization

For a public stable release, sign and notarize with an Apple Developer ID.

This machine currently has no valid code signing identities, so local builds are
ad-hoc signed unless a Developer ID certificate is installed. The Tauri config
sets `bundle.macOS.signingIdentity` to `-` so local macOS bundles receive a
complete ad-hoc signature instead of relying on the linker signature of the
main executable only.

Check identities:

```bash
security find-identity -v -p codesigning
```

Ad-hoc signing only fixes bundle signature integrity. It does not notarize the
app. For a browser-downloaded public dmg that opens without Gatekeeper override,
use a Developer ID Application certificate and notarization. Replace
`bundle.macOS.signingIdentity` or override it with the Developer ID identity for
that build, configure Apple's notarization credentials, then run:

```bash
npm run verify:mac-release -- --gatekeeper src-tauri/target/release/bundle/dmg/memView_<version>_arm64.dmg
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
