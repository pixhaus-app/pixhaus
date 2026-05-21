#!/usr/bin/env bash
# Create a new git worktree for parallel agent work.
#
# Usage: bash scripts/new-worktree.sh <name> [base-branch]
#   name          short slug, e.g. stream-s07
#   base-branch   defaults to main
#
# Worktrees live next to the repo at ../pixhaus-worktrees/<name>.

set -euo pipefail

NAME="${1:-}"
BASE="${2:-main}"

if [ -z "$NAME" ]; then
    echo "usage: new-worktree.sh <name> [base-branch]" >&2
    exit 2
fi

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

WORKTREE_PATH="../pixhaus-worktrees/$NAME"
BRANCH_NAME="feat/${NAME}"

if [ -d "$WORKTREE_PATH" ]; then
    echo "error: $WORKTREE_PATH already exists" >&2
    exit 1
fi

mkdir -p "../pixhaus-worktrees"

echo "==> Creating worktree at $WORKTREE_PATH on $BRANCH_NAME (base: $BASE)"
git worktree add "$WORKTREE_PATH" -b "$BRANCH_NAME" "$BASE"

# Wire the project hooks so the pre-commit gate fires even when `pnpm bootstrap`
# was never run in this checkout. core.hooksPath is resolved relative to each
# worktree's root, and every worktree carries its own .githooks/ checkout.
git -C "$WORKTREE_PATH" config core.hooksPath .githooks
chmod +x "$WORKTREE_PATH/.githooks/"* 2>/dev/null || true

echo ""
echo "Worktree ready."
echo "  path:   $WORKTREE_PATH"
echo "  branch: $BRANCH_NAME"
echo ""
echo "Next: bash scripts/ralph.sh $NAME"
