#!/usr/bin/env node
// Cross-platform hook dispatcher.
//
// Picks scripts/<task>.ps1 on Windows and scripts/<task>.sh on *nix, forwards
// argv tail and stdin/stdout/stderr, and exits with the child's code.
//
// Usage: node scripts/run.mjs <task> [args...]
//   <task>  one of the entries in ALLOWED_TASKS below.
//
// Why this exists: .claude/settings.json hooks and .githooks/pre-commit need
// a single command line that works on every developer OS. Node is already
// required (pnpm), so we route through it instead of branching in shell.

import { spawn, spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPTS_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPTS_DIR, "..");
const isWindows = process.platform === "win32";

// Explicit allowlist of permitted task names. Keep in sync with the
// scripts/<task>.{sh,ps1} files on disk. Using a closed set (rather than a
// regex + filesystem lookup) makes the safety property locally verifiable
// and silences CodeQL's user-controlled-path / bypass warnings on the
// $task argument.
const ALLOWED_TASKS = new Set([
  "bootstrap",
  "claim-next-task",
  "dispatch",
  "doctor",
  "export-icons",
  "fan-out-bedrock",
  "finalize-task",
  "find-crate-for-file",
  "gen-updater-key",
  "generate-samples",
  "install-tools",
  "new-worktree",
  "post-edit",
  "pre-commit",
  "pre-pr",
  "ralph",
  "setup-e2e",
  "setup-git-hooks",
]);

const [, , task, ...rest] = process.argv;

if (!task || !ALLOWED_TASKS.has(task)) {
  const list = [...ALLOWED_TASKS].join("|");
  console.error(`usage: node scripts/run.mjs <${list}> [args...]`);
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
