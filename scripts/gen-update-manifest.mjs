#!/usr/bin/env node
/**
 * gen-update-manifest.mjs
 *
 * Generates the `latest.json` update manifest consumed by tauri-plugin-updater.
 * Runs as part of the `publish` CI job after all platform builds have uploaded
 * their signed artifacts to the draft GitHub release.
 *
 * Usage:
 *   node scripts/gen-update-manifest.mjs \
 *     --tag v0.2.0 \
 *     --version 0.2.0 \
 *     --artifacts-dir ./release-artifacts \
 *     --output ./latest.json
 *
 * Expected artifacts directory layout (produced by tauri-action):
 *   pixhaus_0.2.0_amd64.AppImage
 *   pixhaus_0.2.0_amd64.AppImage.tar.gz
 *   pixhaus_0.2.0_amd64.AppImage.tar.gz.sig
 *   pixhaus_0.2.0_x64_en-US.msi
 *   pixhaus_0.2.0_x64_en-US.msi.zip
 *   pixhaus_0.2.0_x64_en-US.msi.zip.sig
 *   pixhaus_0.2.0_x64-setup.exe
 *   pixhaus_0.2.0_x64-setup.nsis.zip
 *   pixhaus_0.2.0_x64-setup.nsis.zip.sig
 *   pixhaus_0.2.0_x64.dmg
 *   pixhaus_0.2.0_x64.dmg.tar.gz
 *   pixhaus_0.2.0_x64.dmg.tar.gz.sig
 *   pixhaus_0.2.0_aarch64.dmg
 *   pixhaus_0.2.0_aarch64.dmg.tar.gz
 *   pixhaus_0.2.0_aarch64.dmg.tar.gz.sig
 *
 * The .sig files contain the minisign signature that the updater plugin
 * verifies against the pubkey in tauri.conf.json.
 */

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join, basename } from "node:path";
import { parseArgs } from "node:util";

const { values: args } = parseArgs({
  options: {
    tag: { type: "string" },
    version: { type: "string" },
    "artifacts-dir": { type: "string" },
    output: { type: "string" },
  },
});

const tag = args["tag"];
const version = args["version"];
const artifactsDir = args["artifacts-dir"];
const outputPath = args["output"];

if (!tag || !version || !artifactsDir || !outputPath) {
  console.error(
    "Usage: gen-update-manifest.mjs --tag <tag> --version <ver> --artifacts-dir <dir> --output <path>"
  );
  process.exit(1);
}

const REPO = "pixhaus-app/pixhaus";
const BASE_URL = `https://github.com/${REPO}/releases/download/${tag}`;

// Maps a file name pattern to a tauri platform key.
// tauri-plugin-updater platform identifiers:
//   linux-x86_64, windows-x86_64, darwin-x86_64, darwin-aarch64
/** @type {Array<{ pattern: RegExp; platform: string; prefer: "tar.gz" | "zip" }>} */
const PLATFORM_MAP = [
  {
    pattern: /amd64\.AppImage\.tar\.gz\.sig$/,
    platform: "linux-x86_64",
    bundleExtension: "AppImage.tar.gz",
  },
  {
    // Prefer MSI over NSIS for Windows updates (smaller delta, silent install).
    pattern: /x64_en-US\.msi\.zip\.sig$/,
    platform: "windows-x86_64",
    bundleExtension: "msi.zip",
  },
  {
    // Fallback: NSIS if MSI .sig is absent.
    pattern: /x64-setup\.nsis\.zip\.sig$/,
    platform: "windows-x86_64",
    bundleExtension: "nsis.zip",
    fallback: true,
  },
  {
    pattern: /_x64\.dmg\.tar\.gz\.sig$/,
    platform: "darwin-x86_64",
    bundleExtension: "dmg.tar.gz",
  },
  {
    pattern: /aarch64\.dmg\.tar\.gz\.sig$/,
    platform: "darwin-aarch64",
    bundleExtension: "dmg.tar.gz",
  },
];

/** @type {Record<string, { signature: string; url: string }>} */
const platforms = {};

const files = readdirSync(artifactsDir);

for (const entry of PLATFORM_MAP) {
  const sigFile = files.find((f) => entry.pattern.test(f));
  if (!sigFile) {
    if (!entry.fallback) {
      console.warn(`WARNING: no .sig file found for pattern ${entry.pattern}`);
    }
    continue;
  }

  // Skip fallback platforms already populated by a preferred entry.
  if (entry.fallback && platforms[entry.platform]) continue;

  const signature = readFileSync(join(artifactsDir, sigFile), "utf8").trim();

  // Derive the bundle filename from the .sig filename.
  const bundleFile = sigFile.replace(/\.sig$/, "");
  if (!files.includes(bundleFile)) {
    console.warn(`WARNING: ${bundleFile} not found alongside ${sigFile}`);
    continue;
  }

  platforms[entry.platform] = {
    signature,
    url: `${BASE_URL}/${bundleFile}`,
  };

  console.log(`  ${entry.platform}: ${bundleFile}`);
}

if (Object.keys(platforms).length === 0) {
  console.error("ERROR: no platform artifacts found — aborting");
  process.exit(1);
}

const pubDate = new Date().toISOString();

const manifest = {
  version,
  notes: `See https://github.com/${REPO}/releases/tag/${tag} for release notes.`,
  pub_date: pubDate,
  platforms,
};

writeFileSync(outputPath, JSON.stringify(manifest, null, 2) + "\n", "utf8");
console.log(`\nWrote ${outputPath} with ${Object.keys(platforms).length} platform(s).`);
