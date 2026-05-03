#!/usr/bin/env bash
# Fan-out: print the parallel ralph commands for unclaimed bedrock tasks.
#
# By default scans work/queue.md for UNCLAIMED B3..B7 and prints a numbered
# command block for each (worktree create + cd + ralph). Honors the
# [OPUS-REQUIRED] tag on the queue line by setting PIXHAUS_RALPH_MODEL.
#
# With --background, instead of printing, runs each ralph loop as a
# backgrounded process with logs going to logs/ralph/<task>.log. Print
# PIDs at the end. Bash uses nohup ... & disown; PowerShell version uses
# Start-Job — these genuinely diverge.
#
# Bedrock = B2..B7. B2 is run manually first (single agent, careful
# review), so fan-out covers only B3..B7.

set -uo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

BACKGROUND=0
while [ $# -gt 0 ]; do
    case "$1" in
        --background) BACKGROUND=1; shift ;;
        --help|-h)    echo "usage: fan-out-bedrock [--background]"; exit 0 ;;
        *)            echo "fan-out: unknown flag: $1" >&2; exit 2 ;;
    esac
done

QUEUE="work/queue.md"
if [ ! -f "$QUEUE" ]; then
    echo "fan-out: $QUEUE missing" >&2
    exit 1
fi

# Find UNCLAIMED bedrock tasks B3..B7. Capture id and the line so we can
# look for the [OPUS-REQUIRED] marker.
TASKS_FILE="$(mktemp)"
trap 'rm -f "$TASKS_FILE"' EXIT
grep -E '^- \[ \] UNCLAIMED:[[:space:]]*B[3-7]([[:space:]]|$)' "$QUEUE" > "$TASKS_FILE" || true

if [ ! -s "$TASKS_FILE" ]; then
    echo "fan-out: no UNCLAIMED bedrock tasks (B3..B7) in $QUEUE"
    exit 0
fi

declare -a STARTED_PIDS=()

n=0
while IFS= read -r line; do
    n=$((n + 1))
    task_id="$(printf '%s' "$line" | sed -E 's/^- \[ \] UNCLAIMED:[[:space:]]*(B[0-9]+).*/\1/')"
    worktree="stream-$(printf '%s' "$task_id" | tr '[:upper:]' '[:lower:]')"
    model_env=""
    if printf '%s' "$line" | grep -q 'OPUS-REQUIRED'; then
        model_env="PIXHAUS_RALPH_MODEL=claude-opus-4-7 "
    fi

    if [ "$BACKGROUND" -eq 1 ]; then
        mkdir -p logs/ralph
        log_file="logs/ralph/${task_id}.log"
        echo "fan-out: starting ralph for $task_id (worktree=$worktree, log=$log_file)"
        if [ ! -d "../pixhaus-worktrees/$worktree" ]; then
            node scripts/run.mjs new-worktree "$worktree" >> "$log_file" 2>&1 || true
        fi
        # Run in background. Use env -i to apply the model override only
        # when needed; otherwise inherit the caller's env.
        if [ -n "$model_env" ]; then
            PIXHAUS_RALPH_MODEL=claude-opus-4-7 nohup node scripts/run.mjs ralph "$worktree" >> "$log_file" 2>&1 &
        else
            nohup node scripts/run.mjs ralph "$worktree" >> "$log_file" 2>&1 &
        fi
        pid=$!
        disown "$pid" 2>/dev/null || true
        STARTED_PIDS+=("$task_id:$pid")
    else
        echo ""
        echo "# Terminal $n  ($task_id)"
        echo "node scripts/run.mjs new-worktree $worktree"
        echo "cd ../pixhaus-worktrees/$worktree"
        echo "${model_env}node scripts/run.mjs ralph $worktree"
    fi
done < "$TASKS_FILE"

if [ "$BACKGROUND" -eq 1 ]; then
    echo ""
    echo "fan-out: started $n ralph loop(s):"
    for entry in "${STARTED_PIDS[@]}"; do
        echo "  $entry"
    done
    echo ""
    echo "Stop a loop:  kill <pid>"
    echo "Stop all:     kill $(printf '%s\n' "${STARTED_PIDS[@]}" | cut -d: -f2 | tr '\n' ' ')"
else
    echo ""
    echo "# Run each block above in a separate terminal."
fi
