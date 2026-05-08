// WebdriverIO configuration for Pixhaus end-to-end tests.
//
// Drives a real Pixhaus binary through tauri-driver. Each spec opens a
// fresh WebView, exercises UI flows, and asserts on DOM state, the
// __pixhaus_debug__ surface, and __pixhaus_ipc_log__.
//
// Prerequisites (one-time setup):
//   1. cargo install tauri-driver --locked
//   2. Windows: install Microsoft Edge Driver matching your Edge version
//      (https://developer.microsoft.com/en-us/microsoft-edge/tools/webdriver/)
//      Linux: install webkit2gtk-driver via your package manager
//      macOS: not supported by tauri-driver
//   3. Build the binary:  pnpm tauri:build:debug
//
// Then run:  pnpm e2e   (from the repo root)

import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";
import { spawn, type ChildProcess } from "node:child_process";

// ── Binary discovery ──────────────────────────────────────────────────────────

const REPO_ROOT = resolve(import.meta.dirname, "..", "..");
const BINARY_PATH = resolve(
  REPO_ROOT,
  "target",
  "debug",
  process.platform === "win32" ? "pixhaus.exe" : "pixhaus",
);

const TAURI_DRIVER_PATH = resolve(
  homedir(),
  ".cargo",
  "bin",
  process.platform === "win32" ? "tauri-driver.exe" : "tauri-driver",
);

// ── tauri-driver lifecycle ────────────────────────────────────────────────────
//
// tauri-driver is a long-running WebDriver server. We spawn it before each
// session and kill it after. WDIO's beforeSession/afterSession hooks line
// this up with the page-load / page-close lifecycle.

let tauriDriver: ChildProcess | null = null;

// ── Config ────────────────────────────────────────────────────────────────────

export const config: WebdriverIO.Config = {
  runner: "local",
  tsConfigPath: "./tsconfig.json",

  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,

  capabilities: [
    // The "tauri:options" capability is consumed by tauri-driver and
    // translated to the underlying msedgedriver / webkitwebdriver call.
    // It's not in WebdriverIO.Capabilities, so we cast through unknown.
    {
      maxInstances: 1,
      browserName: "wry",
      "tauri:options": {
        application: BINARY_PATH,
      },
    } as unknown as WebdriverIO.Capabilities,
  ],

  logLevel: "warn",
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 30000,
  connectionRetryCount: 1,

  // tauri-driver listens on 127.0.0.1:4444 by default.
  hostname: "127.0.0.1",
  port: 4444,

  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },

  // ── Hooks ───────────────────────────────────────────────────────────────────

  onPrepare(): void {
    if (!existsSync(BINARY_PATH)) {
      throw new Error(
        `[e2e] Pixhaus binary not found at ${BINARY_PATH}\n` +
          `       Build it first:  pnpm tauri:build:debug`,
      );
    }
    if (!existsSync(TAURI_DRIVER_PATH)) {
      throw new Error(
        `[e2e] tauri-driver not found at ${TAURI_DRIVER_PATH}\n` +
          `       Install it:  cargo install tauri-driver --locked`,
      );
    }
  },

  beforeSession(): void {
    tauriDriver = spawn(TAURI_DRIVER_PATH, [], {
      stdio: ["ignore", "inherit", "inherit"],
    });
    tauriDriver.on("error", (err) => {
      console.error("[e2e] tauri-driver error:", err);
    });
  },

  afterSession(): void {
    if (tauriDriver !== null) {
      tauriDriver.kill();
      tauriDriver = null;
    }
  },
};
