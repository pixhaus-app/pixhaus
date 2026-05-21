#!/usr/bin/env bash
# Pre-PR gate: run before opening a pull request.
# Heavier than the pre-commit hook — adds the full test suite and build.
# CI runs the same checks; running this locally first saves a PR round-trip.
#
# Exits non-zero on the first failure. Fix and re-run.

set -uo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

failed=0
section() { printf '\n==> %s\n' "$1"; }
fail()    { failed=1; printf '    FAILED: %s\n' "$1" >&2; }

if command -v cargo >/dev/null 2>&1; then
    section "cargo fmt --check"
    cargo fmt --all -- --check || fail "rustfmt (run: cargo fmt --all)"

    section "cargo clippy --all-targets -D warnings"
    cargo clippy --workspace --all-targets -- -D warnings \
        || fail "clippy (fix warnings or annotate)"

    section "cargo test --workspace"
    if command -v cargo-nextest >/dev/null 2>&1; then
        cargo nextest run --workspace --no-fail-fast || fail "tests"
    else
        cargo test --workspace --no-fail-fast || fail "tests"
    fi

    section "cargo doc --no-deps -D warnings"
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --document-private-items \
        || fail "rustdoc (broken intra-doc link or missing docs)"

    if command -v cargo-deny >/dev/null 2>&1; then
        section "cargo deny check"
        cargo deny check --config .cargo/deny.toml \
            || fail "cargo-deny (dependency, license, or advisory violation)"
    else
        echo "pre-pr: cargo-deny not installed; install with cargo install cargo-deny" >&2
    fi
else
    echo "pre-pr: cargo not on PATH; Rust checks skipped" >&2
    failed=1
fi

if command -v typos >/dev/null 2>&1; then
    section "typos"
    typos || fail "typos (run: typos --write-changes)"
else
    echo "pre-pr: typos not installed; install with cargo install typos-cli" >&2
fi

if command -v pnpm >/dev/null 2>&1 && [ -d ui ]; then
    section "pnpm typecheck"
    pnpm typecheck || fail "pnpm typecheck"

    section "pnpm lint"
    pnpm lint || fail "pnpm lint"

    section "pnpm format:check"
    pnpm format:check || fail "pnpm format:check (run: pnpm format)"

    section "pnpm test"
    pnpm test || fail "pnpm test"

    section "pnpm ui:build"
    pnpm ui:build || fail "pnpm ui:build"
else
    echo "pre-pr: pnpm not on PATH or ui/ missing; UI checks skipped" >&2
    failed=1
fi

echo ""
if [ "$failed" -eq 0 ]; then
    echo "pre-pr: ok — ready to open the PR"
    exit 0
else
    echo "pre-pr: failures above; fix them before opening the PR" >&2
    exit 1
fi
