#!/usr/bin/env node
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tauriConfig = JSON.parse(readFileSync(path.join(root, "src-tauri/tauri.conf.json"), "utf8"));
const version = tauriConfig.version;
const productName = tauriConfig.productName;
const githubRepo = "554943871/mem_view";
const releaseNotes = readReleaseNotes(version);
const appBinary = path.join(
  root,
  "src-tauri/target/release/bundle/macos",
  `${productName}.app/Contents/MacOS/mem-view`
);
const binaryDescription = existsSync(appBinary)
  ? execFileSync("file", [appBinary], { encoding: "utf8" })
  : "";
const arch = binaryDescription.includes("arm64")
  ? "aarch64"
  : binaryDescription.includes("x86_64")
    ? "x86_64"
    : process.arch === "arm64"
      ? "aarch64"
      : process.arch === "x64"
        ? "x86_64"
        : process.arch;
const platform = `darwin-${arch}`;
const archLabel = arch === "aarch64" ? "arm64" : arch;
const updaterBundle = path.join(
  root,
  "src-tauri/target/release/bundle/macos",
  `${productName}.app.tar.gz`
);
const signatureFile = `${updaterBundle}.sig`;
const dmgFile = path.join(
  root,
  "src-tauri/target/release/bundle/dmg",
  `${productName}_${version}_${archLabel}.dmg`
);

if (!existsSync(updaterBundle)) {
  throw new Error(`Missing updater bundle: ${updaterBundle}`);
}

if (!existsSync(signatureFile)) {
  throw new Error(`Missing updater signature: ${signatureFile}`);
}

const releaseDir = path.join(root, "dist-release");
const assetName = `${productName}_${version}_${archLabel}.app.tar.gz`;
const signatureName = `${assetName}.sig`;
const assetUrl = `https://github.com/${githubRepo}/releases/download/v${version}/${assetName}`;
const signature = readFileSync(signatureFile, "utf8").trim();
const latest = {
  version,
  notes: releaseNotes,
  pub_date: new Date().toISOString(),
  platforms: {
    [platform]: {
      signature,
      url: assetUrl
    }
  }
};

mkdirSync(releaseDir, { recursive: true });
copyFileSync(updaterBundle, path.join(releaseDir, assetName));
copyFileSync(signatureFile, path.join(releaseDir, signatureName));
if (existsSync(dmgFile)) {
  copyFileSync(dmgFile, path.join(releaseDir, path.basename(dmgFile)));
}
writeFileSync(path.join(releaseDir, "latest.json"), `${JSON.stringify(latest, null, 2)}\n`);

console.log(`Wrote ${path.relative(root, path.join(releaseDir, "latest.json"))}`);
const uploadNames = [assetName, signatureName, "latest.json"];
if (existsSync(dmgFile)) {
  uploadNames.unshift(path.basename(dmgFile));
}
console.log(`Upload ${uploadNames.join(", ")} to GitHub release v${version}.`);

function readReleaseNotes(releaseVersion) {
  const changelogPath = path.join(root, "CHANGELOG.md");
  if (!existsSync(changelogPath)) {
    return "";
  }

  const changelog = readFileSync(changelogPath, "utf8");
  const headingPattern = new RegExp(`^##\\s+v?${escapeRegExp(releaseVersion)}\\s*$`, "m");
  const heading = headingPattern.exec(changelog);
  if (!heading || heading.index === undefined) {
    return "";
  }

  const notesStart = heading.index + heading[0].length;
  const remaining = changelog.slice(notesStart);
  const nextHeadingIndex = remaining.search(/^##\s+/m);
  return remaining
    .slice(0, nextHeadingIndex === -1 ? undefined : nextHeadingIndex)
    .trim();
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
