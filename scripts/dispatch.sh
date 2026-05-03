#!/usr/bin/env bash
# Dispatch: one-shot Claude run for a single queue task.
#
# Validates the task is UNCLAIMED, creates the worktree if needed, claims
# atomically, runs Claude once with the brief, finalizes ok/fail, tees the
# transcript to logs/dispatch/<timestamp>-<task>.json. Differs from ralph
# in that it dispatches once and exits, and lets you override the model.
#
# Usage: dispatch <task-id> [--model <model>] [--worktree <name>]
#   --model     default: claude-sonnet-4-6
#   --worktree  default: derived from task id (B2 -> stream-b2)

set -uo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

TASK_ID=""
MODEL="claude-sonnet-4-6"
WORKTREE=""

usage() {
    echo "usage: dispatch <task-id> [--model <model>] [--worktree <name>]" >&2
    exit 2
}

while [ $# -gt 0 ]; do
    case "$1" in
        --model)    MODEL="${2:-}";    shift 2 ;;
        --worktree) WORKTREE="${2:-}"; shift 2 ;;
        --help|-h)  usage ;;
        --*)        echo "dispatch: unknown flag: $1" >&2; usage ;;
        *)
            if [ -z "$TASK_ID" ]; then
                TASK_ID="$1"
            else
                echo "dispatch: unexpected positional arg: $1" >&2
                usage
            fi
            shift
            ;;
    esac
done

[ -z "$TASK_ID" ] && usage

# Derive worktree name when not given. Lowercase + stream- prefix.
if [ -z "$WORKTREE" ]; then
    WORKTREE="stream-$(printf '%s' "$TASK_ID" | tr '[:upper:]' '[:lower:]')"
fi

QUEUE="work/queue.md"
if [ ! -f "$QUEUE" ]; then
    echo "dispatch: $QUEUE missing" >&2
    exit 1
fi

# Validate the task is currently UNCLAIMED. The grep matches the same
# format claim-next-task.sh writes against.
if ! grep -qE "^- \[ \] UNCLAIMED:[[:space:]]*${TASK_ID}([[:space:]]|$)" "$QUEUE"; then
    echo "dispatch: task $TASK_ID is not UNCLAIMED in $QUEUE" >&2
    echo "         (already claimed, done, or unknown id)" >&2
    exit 2
fi

# Ensure the worktree exists. new-worktree refuses if the path is taken,
# so check first and reuse if present.
WORKTREE_PATH="../pixhaus-worktrees/$WORKTREE"
if [ ! -d "$WORKTREE_PATH" ]; then
    echo "dispatch: creating worktree $WORKTREE"
    node scripts/run.mjs new-worktree "$WORKTREE" || {
        echo "dispatch: new-worktree failed" >&2
        exit 1
    }
else
    echo "dispatch: reusing existing worktree $WORKTREE"
fi

# Claim atomically. claim-next-task picks the next UNCLAIMED line, which
# is what we want only if the requested task is the next one. Verify
# afterwards.
echo "dispatch: claiming next task as worktree=$WORKTREE"
CLAIM_OUTPUT="$(node scripts/run.mjs claim-next-task "$WORKTREE" 2>&1)"
CLAIM_RC=$?
if [ $CLAIM_RC -ne 0 ] || [ -z "$CLAIM_OUTPUT" ]; then
    echo "dispatch: claim-next-task failed" >&2
    echo "$CLAIM_OUTPUT" >&2
    exit 1
fi

CLAIMED_ID="$(printf '%s\n' "$CLAIM_OUTPUT" | head -n1 | sed -E 's/^TASK:[[:space:]]*//')"
TASK_BRIEF="$(printf '%s\n' "$CLAIM_OUTPUT" | tail -n +2)"

if [ "$CLAIMED_ID" != "$TASK_ID" ]; then
    echo "dispatch: claim returned $CLAIMED_ID, expected $TASK_ID" >&2
    echo "         (someone else claimed first; releasing)" >&2
    node scripts/run.mjs finalize-task "$WORKTREE" "$CLAIMED_ID" fail "wrong task; dispatch wanted $TASK_ID" || true
    exit 1
fi

# Run Claude once. Tee output to logs/dispatch/.
mkdir -p logs/dispatch
TS="$(date -u +%Y%m%dT%H%M%SZ)"
LOG_FILE="logs/dispatch/${TS}-${TASK_ID}.json"

echo "dispatch: running claude (model=$MODEL)"
echo "dispatch: log -> $LOG_FILE"

if ! command -v claude >/dev/null 2>&1; then
    echo "dispatch: claude CLI not on PATH; releasing claim" >&2
    node scripts/run.mjs finalize-task "$WORKTREE" "$TASK_ID" fail "claude CLI missing"
    exit 1
fi

# cd into the worktree so Claude operates against that checkout.
cd "$WORKTREE_PATH"

# tee preserves the user's view of the transcript while persisting it.
if claude --model "$MODEL" \
          --print "$TASK_BRIEF" \
          --output-format json 2>&1 | tee "$REPO_ROOT/$LOG_FILE"; then
    cd "$REPO_ROOT"
    node scripts/run.mjs finalize-task "$WORKTREE" "$TASK_ID" ok
    echo "dispatch: $TASK_ID finalized ok"
else
    cd "$REPO_ROOT"
    node scripts/run.mjs finalize-task "$WORKTREE" "$TASK_ID" fail "claude exited non-zero (see $LOG_FILE)"
    echo "dispatch: $TASK_ID returned to queue (see $LOG_FILE)" >&2
    exit 1
fi

echo "dispatch: log saved to $LOG_FILE"
