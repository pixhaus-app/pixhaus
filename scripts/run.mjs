#!/usr/bin/env node
// Cross-platform hook dispatcher.
//
// Picks scripts/<task>.ps1 on Windows and scripts/<task>.sh on *nix, forwards
// argv tail and stdin/stdout/stderr, and exits with the child's code.
//
// Usage: node scripts/run.mjs <task> [args...]
//   <task>  one of: post-edit | pre-commit | claim-next-task | finalize-task |
//                   find-crate-for-file | ralph | install-tools |
//                   setup-git-hooks | new-worktree | bootstrap | doctor |
//                   dispatch | fan-out-bedrock | pre-pr
//
// Why this exists: .claude/settings.json hooks and .githooks/pre-commit need
// a single command line that works on every developer OS. Node is already
// required (pnpm), so we route through it instead of branching in shell.

import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPTS_DIR, "..");
const isWindows = process.platform === "win32";

const [, , task, ...rest] = process.argv;

if (!task) {
  console.error("usage: node scripts/run.mjs <task> [args...]");
  process.exit(2);
}

if (!/^[a-z0-9][a-z0-9_-]*$/i.test(task)) {
  console.error(`run.mjs: invalid task name: ${task}`);
  process.exit(2);
}

function pickPwsh() {
  // Prefer pwsh (PowerShell 7+); fall back to Windows PowerShell 5.1.
  const probe = spawnSync("pwsh", ["-NoProfile", "-Command", "$Host.Name"], {
    stdio: "ignore",
    shell: false,
    windowsHide: true,
  });
  return probe.error ? "powershell.exe" : "pwsh";
}

const ext = isWindows ? "ps1" : "sh";
const scriptPath = join(SCRIPTS_DIR, `${task}.${ext}`);

if (!existsSync(scriptPath)) {
  console.error(`run.mjs: missing ${scriptPath}`);
  process.exit(2);
}

const cmd = isWindows ? pickPwsh() : "bash";
const cmdArgs = isWindows
  ? ["-NoProfile", "-NoLogo", "-ExecutionPolicy", "Bypass", "-File", scriptPath, ...rest]
  : [scriptPath, ...rest];

const child = spawn(cmd, cmdArgs, {
  cwd: REPO_ROOT,
  stdio: "inherit",
  windowsHide: true,
});

child.on("error", (err) => {
  console.error(`run.mjs: failed to spawn ${cmd}: ${err.message}`);
  process.exit(127);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.exit(1);
  }
  process.exit(code ?? 1);
});
