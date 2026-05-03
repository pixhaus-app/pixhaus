#!/usr/bin/env bash
# Walk up from a file to the nearest Cargo.toml and print the crate name.
# Usage: bash scripts/find-crate-for-file.sh path/to/file.rs

set -uo pipefail

if [ $# -lt 1 ]; then
    echo "usage: find-crate-for-file.sh <file>" >&2
    exit 2
fi

INPUT="$1"
INPUT="${INPUT//\\//}"

if [ -d "$INPUT" ]; then
    DIR="$INPUT"
else
    DIR="$(dirname -- "$INPUT")"
fi

if [ ! -d "$DIR" ]; then
    exit 1
fi

DIR="$(cd -- "$DIR" 2>/dev/null && pwd)" || exit 1

while [ "$DIR" != "/" ] && [ -n "$DIR" ]; do
    if [ -f "$DIR/Cargo.toml" ]; then
        # Skip the workspace root (no [package] section).
        if grep -q '^\[package\]' "$DIR/Cargo.toml"; then
            NAME="$(grep -E '^name\s*=' "$DIR/Cargo.toml" | head -n1 | sed -E 's/^name\s*=\s*"(.*)"\s*$/\1/')"
            if [ -n "$NAME" ]; then
                echo "$NAME"
                exit 0
            fi
        fi
    fi
    PARENT="$(dirname -- "$DIR")"
    if [ "$PARENT" = "$DIR" ]; then
        break
    fi
    DIR="$PARENT"
done

exit 1
