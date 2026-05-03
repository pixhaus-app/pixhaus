#!/usr/bin/env bash
# Atomically claim the next unclaimed task from work/queue.md.
#
# Lock strategy: mkdir is atomic on POSIX and Windows. We use a lock dir
# rather than `flock` so this works under Git Bash on Windows.
#
# Output on success (stdout):
#   line 1: TASK: <task-id>
#   line 2+: full brief text for the task
# Exit code:
#   0 = claimed
#   1 = no tasks available
#   2 = usage error

set -uo pipefail

WORKTREE_NAME="${1:-}"
if [ -z "$WORKTREE_NAME" ]; then
    echo "usage: claim-next-task.sh <worktree-name>" >&2
    exit 2
fi

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

QUEUE="work/queue.md"
LOCK_DIR=".queue.lock"

if [ ! -f "$QUEUE" ]; then
    echo "claim: $QUEUE missing" >&2
    exit 1
fi

# Acquire lock — retry up to 30s.
attempts=0
until mkdir "$LOCK_DIR" 2>/dev/null; do
    attempts=$((attempts + 1))
    if [ "$attempts" -gt 60 ]; then
        echo "claim: failed to acquire $LOCK_DIR after 30s; aborting" >&2
        exit 1
    fi
    sleep 0.5
done
trap 'rm -rf "$LOCK_DIR"' EXIT

# Find the first task line that's UNCLAIMED.
TASK_LINE_NO="$(grep -n '^- \[ \] UNCLAIMED:' "$QUEUE" | head -n1 | cut -d: -f1 || true)"
if [ -z "$TASK_LINE_NO" ]; then
    exit 1
fi

TASK_LINE="$(sed -n "${TASK_LINE_NO}p" "$QUEUE")"
TASK_ID="$(echo "$TASK_LINE" | sed -E 's/^- \[ \] UNCLAIMED:\s*([A-Za-z0-9_-]+).*/\1/')"
if [ -z "$TASK_ID" ]; then
    echo "claim: could not parse task id from line: $TASK_LINE" >&2
    exit 1
fi

# Rewrite the line in place — mark it claimed.
NEW_LINE="$(echo "$TASK_LINE" | sed -E "s/^- \[ \] UNCLAIMED:/- [~] CLAIMED:$WORKTREE_NAME:/")"

# Use a temp file for atomic replacement.
TMP_QUEUE="$(mktemp)"
awk -v lineno="$TASK_LINE_NO" -v new="$NEW_LINE" 'NR==lineno{print new; next}{print}' "$QUEUE" > "$TMP_QUEUE"
mv "$TMP_QUEUE" "$QUEUE"

# Resolve brief: assume queue line links to a brief in docs/planning/work/.
BRIEF_PATH="$(echo "$TASK_LINE" | grep -oE 'docs/planning/work/[a-zA-Z0-9_/-]+\.md(#[a-zA-Z0-9_-]+)?' | head -n1 || true)"
BRIEF_FILE="${BRIEF_PATH%%#*}"

echo "TASK: $TASK_ID"
if [ -n "$BRIEF_FILE" ] && [ -f "$BRIEF_FILE" ]; then
    cat "$BRIEF_FILE"
else
    echo "$TASK_LINE"
    echo ""
    echo "(No linked brief found. See docs/planning/work/bedrock.md or streams.md.)"
fi
