#!/usr/bin/env bash
# Pre-commit: gate formatting, lints, typos.
# Failures here block the commit. Fix the underlying issue rather than
# bypassing with --no-verify.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

fail() {
    echo ""
    echo "pre-commit: $1" >&2
    echo "pre-commit: blocked. Fix the issue above and try again." >&2
    exit 1
}

if command -v cargo >/dev/null 2>&1; then
    echo "pre-commit: cargo fmt --check"
    cargo fmt --all -- --check || fail "rustfmt check failed (run: cargo fmt --all)"

    echo "pre-commit: cargo clippy"
    cargo clippy --workspace --all-targets -- -D warnings \
        || fail "clippy reported warnings"

    if command -v cargo-deny >/dev/null 2>&1; then
        echo "pre-commit: cargo deny check"
        cargo deny check --config .cargo/deny.toml \
            || fail "cargo-deny: dependency, license, or advisory violation"
    else
        echo "pre-commit: cargo-deny not installed; skipping (cargo install cargo-deny)" >&2
    fi
else
    echo "pre-commit: cargo not on PATH; skipping Rust checks" >&2
fi

if command -v typos >/dev/null 2>&1; then
    echo "pre-commit: typos"
    typos || fail "typos found (run: typos --write-changes)"
else
    echo "pre-commit: typos not installed; skipping" >&2
fi

# Call the same scripts CI runs (pnpm -r format:check / lint) so the local
# gate covers the exact glob CI does - including tests/, which the old
# hand-rolled src/** globs missed and let unformatted test files reach CI.
if command -v pnpm >/dev/null 2>&1 && [ -d ui ]; then
    echo "pre-commit: pnpm format:check"
    pnpm format:check || fail "prettier check failed (run: pnpm format)"

    echo "pre-commit: pnpm lint"
    pnpm lint || fail "eslint reported errors"
else
    echo "pre-commit: pnpm not on PATH or ui/ missing; skipping TS checks" >&2
fi

echo "pre-commit: ok"
