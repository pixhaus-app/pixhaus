#!/usr/bin/env bash
# Post-edit hook: format and type-check the file Claude just touched.
# Receives Claude Code PostToolUse JSON on stdin (Edit / Write tool calls).
#
# Goal: surface format and type errors immediately so they go back into
# Claude's next turn. Keep it fast — sub-second on the second run.

set -uo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Pull file_path out of the PostToolUse JSON payload on stdin.
# Fall back to $1 for manual invocation: `bash scripts/post-edit.sh <file>`.
JSON_INPUT=""
if [ ! -t 0 ]; then
    JSON_INPUT="$(cat || true)"
fi

extract_file_path() {
    if command -v python >/dev/null 2>&1; then
        printf '%s' "$JSON_INPUT" | python -c "
import json, sys
try:
    data = json.load(sys.stdin)
    print(data.get('tool_input', {}).get('file_path', ''))
except Exception:
    pass
" 2>/dev/null
    elif command -v jq >/dev/null 2>&1; then
        printf '%s' "$JSON_INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null
    fi
}

FILE_PATH="${1:-}"
if [ -z "$FILE_PATH" ] && [ -n "$JSON_INPUT" ]; then
    FILE_PATH="$(extract_file_path)"
fi

if [ -z "$FILE_PATH" ]; then
    exit 0
fi

# Normalize: convert backslashes (Windows) to forward slashes for consistency.
FILE_PATH="${FILE_PATH//\\//}"

# Skip files we don't own.
case "$FILE_PATH" in
    */target/*|*/node_modules/*|*/dist/*|*/.git/*) exit 0 ;;
esac

is_rust=false
is_ts=false
case "$FILE_PATH" in
    *.rs)             is_rust=true ;;
    *.ts|*.tsx)       is_ts=true ;;
    *)                exit 0 ;;
esac

if $is_rust; then
    if ! command -v cargo >/dev/null 2>&1; then
        echo "post-edit: cargo not on PATH; skipping Rust checks." >&2
        exit 0
    fi

    cargo fmt --manifest-path Cargo.toml -- "$FILE_PATH" 2>/dev/null || true

    CRATE="$(bash scripts/find-crate-for-file.sh "$FILE_PATH" 2>/dev/null || true)"
    if [ -n "$CRATE" ]; then
        echo "post-edit: cargo clippy --tests -p $CRATE -- -D warnings" >&2
        if ! cargo clippy --tests -p "$CRATE" -- -D warnings 2>&1; then
            echo "post-edit: cargo clippy failed in crate $CRATE" >&2
            exit 0
        fi
    else
        echo "post-edit: could not locate owning crate for $FILE_PATH" >&2
    fi
fi

if $is_ts; then
    if ! command -v pnpm >/dev/null 2>&1; then
        echo "post-edit: pnpm not on PATH; skipping TS checks." >&2
        exit 0
    fi

    (cd ui && pnpm prettier --write "$FILE_PATH" 2>/dev/null || true)
    echo "post-edit: tsc --noEmit (ui)" >&2
    if ! (cd ui && pnpm tsc --noEmit) 2>&1; then
        echo "post-edit: tsc reported errors" >&2
        exit 0
    fi
fi

exit 0
