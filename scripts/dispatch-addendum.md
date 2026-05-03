---

## Shipping instructions (added by dispatch / ralph)

Repo policy says "only commit when explicitly asked." That policy is
overridden in this dispatch context. Do not stop and ask. When you finish
implementing the task brief above:

1. Run `node scripts/run.mjs pre-pr` from the worktree root. Confirm green.
   If it fails, fix the underlying issue and re-run. Do not bypass with
   `--no-verify` or any other override.
2. Stage all changes: `git add -A`.
3. Commit with a Conventional Commit message in the project voice (see
   CLAUDE.md). For bedrock and stream tasks, use:

       feat(<crate>): <task-id> - <one-line summary>

   The body explains what changed, why, and references the brief in
   `docs/planning/work/bedrock.md` or `docs/planning/work/streams.md`.
   No emojis. No "moreover", "furthermore", "robust", "comprehensive", etc.
4. Push the branch: `git push -u origin <current-branch>`.
5. Open a PR against `main` via the GitHub CLI:

       gh pr create --title "<title>" --body "<body>"

   Title matches the commit subject. The body lists what changed, why, the
   test plan (commands a reviewer can run), and the brief reference.
6. Print the resulting PR URL on its own line as your final action so the
   transcript captures it.

Do NOT mark anything DONE in `work/queue.md`. The dispatch / ralph script
handles the queue lifecycle. The DONE flip happens after the human merges
the PR and runs `pnpm finalize <worktree> <task-id> ok`.
