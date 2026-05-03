#!/usr/bin/env bash
# Ralph loop: claim a task, dispatch Claude with the brief, finalize on green.
# One ralph loop per worktree. Stop with Ctrl-C; the in-flight claim is
# released when finalize runs.
#
# Usage: bash scripts/ralph.sh <worktree-name>
#
# Environment:
#   PIXHAUS_RALPH_MODEL   Claude model (default: claude-sonnet-4-6)
#   PIXHAUS_RALPH_SLEEP   Seconds to sleep when queue is empty (default: 300)
#   PIXHAUS_RALPH_MAX     Max iterations before exit (default: unlimited)

set -uo pipefail

WORKTREE_NAME="${1:-}"
if [ -z "$WORKTREE_NAME" ]; then
    echo "usage: bash scripts/ralph.sh <worktree-name>" >&2
    exit 2
fi

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

MODEL="${PIXHAUS_RALPH_MODEL:-claude-sonnet-4-6}"
SLEEP_SEC="${PIXHAUS_RALPH_SLEEP:-300}"
MAX_ITER="${PIXHAUS_RALPH_MAX:-0}"

LOG_DIR="logs/ralph"
mkdir -p "$LOG_DIR"

iter=0

while :; do
    iter=$((iter + 1))
    if [ "$MAX_ITER" -gt 0 ] && [ "$iter" -gt "$MAX_ITER" ]; then
        echo "ralph[$WORKTREE_NAME]: reached max iterations ($MAX_ITER), exiting"
        break
    fi

    echo ""
    echo "ralph[$WORKTREE_NAME]: iteration $iter — claiming next task"

    CLAIM_OUTPUT="$(bash scripts/claim-next-task.sh "$WORKTREE_NAME" 2>&1)"
    CLAIM_RC=$?

    if [ $CLAIM_RC -ne 0 ] || [ -z "$CLAIM_OUTPUT" ]; then
        echo "ralph[$WORKTREE_NAME]: no tasks available; sleeping ${SLEEP_SEC}s"
        sleep "$SLEEP_SEC"
        continue
    fi

    TASK_ID="$(echo "$CLAIM_OUTPUT" | head -n1 | sed -E 's/^TASK:\s*//')"
    TASK_BRIEF="$(echo "$CLAIM_OUTPUT" | tail -n +2)"

    if [ -z "$TASK_ID" ]; then
        echo "ralph[$WORKTREE_NAME]: claim returned no task id; sleeping ${SLEEP_SEC}s"
        sleep "$SLEEP_SEC"
        continue
    fi

    TS="$(date -u +%Y%m%dT%H%M%SZ)"
    LOG_FILE="$LOG_DIR/${TS}-${WORKTREE_NAME}-${TASK_ID}.json"

    echo "ralph[$WORKTREE_NAME]: claimed $TASK_ID; dispatching Claude (model=$MODEL)"
    echo "ralph[$WORKTREE_NAME]: log -> $LOG_FILE"

    if ! command -v claude >/dev/null 2>&1; then
        echo "ralph[$WORKTREE_NAME]: claude CLI not on PATH; releasing claim and exiting"
        bash scripts/finalize-task.sh "$WORKTREE_NAME" "$TASK_ID" "fail" "claude CLI missing"
        exit 1
    fi

    if claude --model "$MODEL" \
              --print "$TASK_BRIEF" \
              --output-format json > "$LOG_FILE" 2>&1; then
        bash scripts/finalize-task.sh "$WORKTREE_NAME" "$TASK_ID" "ok"
    else
        bash scripts/finalize-task.sh "$WORKTREE_NAME" "$TASK_ID" "fail" \
            "claude exited non-zero (see $LOG_FILE)"
    fi
done
