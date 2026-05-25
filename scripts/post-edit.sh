#!/usr/bin/env bash
# Post-edit hook: format and lint the Rust file Claude just touched.
# Receives Claude Code PostToolUse JSON on stdin (Edit / Write tool calls).
#
# Goal: surface format and clippy errors immediately so they go back into
# Claude's next turn. Rust-only, no Node. Mirror of scripts/post-edit.ps1.

set -uo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

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

[ -z "$FILE_PATH" ] && exit 0

# Normalize backslashes (Windows) to forward slashes.
FILE_PATH="${FILE_PATH//\\//}"

case "$FILE_PATH" in
    */target/*|*/.git/*) exit 0 ;;
esac

case "$FILE_PATH" in
    *.rs) ;;
    *) exit 0 ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
    echo "post-edit: cargo not on PATH; skipping Rust checks." >&2
    exit 0
fi

# Walk up from the edited file to the nearest Cargo.toml that declares a
# [package], so fmt/clippy scope to the owning crate rather than the whole
# workspace. Fall back to workspace-wide when none is found (virtual manifest).
find_package_manifest() {
    local dir
    dir="$(cd -- "$(dirname -- "$1")" 2>/dev/null && pwd)" || return
    while [ -n "$dir" ]; do
        if [ -f "$dir/Cargo.toml" ] && grep -qE '^\s*\[package\]' "$dir/Cargo.toml"; then
            printf '%s' "$dir/Cargo.toml"
            return
        fi
        local parent
        parent="$(dirname -- "$dir")"
        [ "$parent" = "$dir" ] && break
        dir="$parent"
    done
}

MANIFEST="$(find_package_manifest "$FILE_PATH")"

if [ -n "$MANIFEST" ]; then
    echo "post-edit: cargo fmt --manifest-path $MANIFEST" >&2
    cargo fmt --manifest-path "$MANIFEST" 2>&1 || echo "post-edit: cargo fmt failed for $MANIFEST" >&2

    echo "post-edit: cargo clippy --manifest-path $MANIFEST --tests -- -D warnings" >&2
    cargo clippy --manifest-path "$MANIFEST" --tests -- -D warnings 2>&1 \
        || echo "post-edit: cargo clippy reported problems for $MANIFEST" >&2
else
    echo "post-edit: no owning crate found; running cargo fmt --all" >&2
    cargo fmt --all 2>&1 || echo "post-edit: cargo fmt --all failed" >&2
fi

exit 0
