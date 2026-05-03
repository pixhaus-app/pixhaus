#!/usr/bin/env bash
# Mark a task done if CI is green, otherwise return it to the queue.
#
# Usage: bash scripts/finalize-task.sh <worktree-name> <task-id> <ok|fail> [reason]

set -uo pipefail

WORKTREE_NAME="${1:-}"
TASK_ID="${2:-}"
OUTCOME="${3:-fail}"
REASON="${4:-}"

if [ -z "$WORKTREE_NAME" ] || [ -z "$TASK_ID" ]; then
    echo "usage: finalize-task.sh <worktree-name> <task-id> <ok|fail> [reason]" >&2
    exit 2
fi

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

QUEUE="work/queue.md"
LOCK_DIR=".queue.lock"

if [ ! -f "$QUEUE" ]; then
    echo "finalize: $QUEUE missing" >&2
    exit 1
fi

attempts=0
until mkdir "$LOCK_DIR" 2>/dev/null; do
    attempts=$((attempts + 1))
    if [ "$attempts" -gt 60 ]; then
        echo "finalize: failed to acquire $LOCK_DIR after 30s; aborting" >&2
        exit 1
    fi
    sleep 0.5
done
trap 'rm -rf "$LOCK_DIR"' EXIT

LINE_NO="$(grep -n "^- \[~\] CLAIMED:${WORKTREE_NAME}:${TASK_ID}" "$QUEUE" | head -n1 | cut -d: -f1 || true)"
if [ -z "$LINE_NO" ]; then
    echo "finalize: could not find claimed task ${TASK_ID} for ${WORKTREE_NAME}" >&2
    exit 1
fi

LINE="$(sed -n "${LINE_NO}p" "$QUEUE")"

if [ "$OUTCOME" = "ok" ]; then
    NEW_LINE="$(echo "$LINE" | sed -E "s/^- \[~\] CLAIMED:${WORKTREE_NAME}:/- [x] DONE:/")"
    echo "finalize: ${TASK_ID} -> done"
else
    if [ -z "$REASON" ]; then
        REASON="returned to queue"
    fi
    BASE="$(echo "$LINE" | sed -E "s/^- \[~\] CLAIMED:${WORKTREE_NAME}:${TASK_ID}/- [ ] UNCLAIMED: ${TASK_ID}/" | sed -E 's/ \[FAIL: [^]]+\]//')"
    NEW_LINE="${BASE} [FAIL: ${REASON}]"
    echo "finalize: ${TASK_ID} -> returned to queue (reason: ${REASON})"
fi

TMP_QUEUE="$(mktemp)"
awk -v lineno="$LINE_NO" -v new="$NEW_LINE" 'NR==lineno{print new; next}{print}' "$QUEUE" > "$TMP_QUEUE"
mv "$TMP_QUEUE" "$QUEUE"
