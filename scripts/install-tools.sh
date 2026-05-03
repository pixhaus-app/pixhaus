#!/usr/bin/env bash
# Install the agent-side dev tools Pixhaus expects.
# Idempotent — re-runs are cheap once everything is installed.

set -euo pipefail

echo "==> Installing cargo dev tools"

CARGO_TOOLS=(
    "cargo-nextest"
    "cargo-deny"
    "cargo-audit"
    "cargo-machete"
    "cargo-watch"
    "typos-cli"
    "bacon"
)

for tool in "${CARGO_TOOLS[@]}"; do
    echo "  - $tool"
    cargo install --locked "$tool" || echo "    (skipped: $tool already installed or unavailable)"
done

echo "==> Installing Tauri CLI"
cargo install --locked tauri-cli --version "^2.0.0" \
    || echo "  (skipped: tauri-cli already installed or unavailable)"

echo "==> Ensuring pnpm is available"
if ! command -v pnpm >/dev/null 2>&1; then
    if command -v corepack >/dev/null 2>&1; then
        echo "  - enabling pnpm via corepack"
        corepack enable pnpm
        corepack prepare pnpm@latest --activate
    elif command -v npm >/dev/null 2>&1; then
        echo "  - installing pnpm via npm"
        npm install -g pnpm
    else
        echo "  ! neither corepack nor npm found; install Node 20+ then re-run"
        exit 1
    fi
fi

echo "==> Done."
