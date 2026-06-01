# pixhaus-services

The service layer — shared behavior above the domain model and below the UI
(architecture bible sections 4.4, 12, 13).

- **Owns:** command execution and undo/redo, transactions, the background job
  system, asset indexing, and provider dispatch.
- **Depends on:** `core` (and `io` for import/export orchestration). External:
  `tokio`, `tokio-util`.
- **Used by:** the modules and `app`.
- **Status:** stub.

## Boundaries

- MUST NOT depend on `egui`.
- MUST NOT block the UI thread — long or expensive work is a job run off-thread;
  results return over channels the egui loop drains each frame.
- A job produces a result; applying that result is a `Command`. A job never
  mutates the project model directly.
- Never hold a lock across `.await`; CPU-bound work goes through `spawn_blocking`.

Reach for the `pixhaus-tokio` skill for async work. Global rules: root `CLAUDE.md`.
Architecture: `docs/pixhaus_architecture_bible.md`.
