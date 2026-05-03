#!/usr/bin/env bash
# Bootstrap: idempotent first-time setup. Re-runnable any time.
#
# Steps: install dev tools, wire git hooks, fetch deps, build the ui/dist
# stub for tauri::generate_context!, verify the workspace compiles, run
# typecheck and tests, end with doctor.
#
# Exits non-zero on the first failing step with a fix hint.

set -uo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

step()  { printf '\n==> %s\n' "$1"; }
abort() { printf '\nbootstrap failed at: %s\n' "$1" >&2; printf 'fix: %s\n' "$2" >&2; exit 1; }

DISPATCH="node scripts/run.mjs"

step "1/9  install dev tools"
$DISPATCH install-tools || abort "install-tools" "see scripts/install-tools.sh output above; rerun bootstrap once fixed"

step "2/9  wire git hooks"
$DISPATCH setup-git-hooks || abort "setup-git-hooks" "rerun: node scripts/run.mjs setup-git-hooks"

step "3/9  pnpm install"
pnpm install || abort "pnpm install" "check pnpm-lock.yaml and the network; pnpm install --force may help"

step "4/9  cargo fetch"
cargo fetch || abort "cargo fetch" "check Cargo.lock and the network"

step "5/9  ui/dist stub"
if [ ! -f ui/dist/index.html ]; then
    mkdir -p ui/dist
    printf '<!doctype html><title>stub</title>' > ui/dist/index.html
    echo "    wrote ui/dist/index.html (placeholder for tauri::generate_context!)"
else
    echo "    ui/dist/index.html already present; skipping"
fi

step "6/9  cargo check --workspace --all-targets"
cargo check --workspace --all-targets || abort "cargo check" "fix the compile errors above"

step "7/9  pnpm typecheck"
pnpm typecheck || abort "pnpm typecheck" "fix the TypeScript errors above"

step "8/9  pnpm test"
pnpm test || abort "pnpm test" "fix the failing test(s) above"

step "9/9  doctor"
$DISPATCH doctor
exit $?
