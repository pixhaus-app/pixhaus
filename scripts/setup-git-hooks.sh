#!/usr/bin/env bash
# Wire .githooks/ as the project hooks directory.
# Idempotent.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d .githooks ]; then
    echo "error: .githooks directory missing" >&2
    exit 1
fi

git config core.hooksPath .githooks

# Ensure scripts are executable on platforms that respect the bit.
chmod +x .githooks/* 2>/dev/null || true

echo "Git hooks configured. core.hooksPath = $(git config --get core.hooksPath)"
