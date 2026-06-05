# pixhaus-services

The service layer — shared behavior above the domain model and below the UI
(architecture bible sections 4.4, 12, 13).

- **Owns:** command execution and undo/redo, transactions, the background job
  system, asset indexing, and provider dispatch.
- **Depends on:** `core` (and `io` for import/export orchestration). External:
  `tokio`, `tokio-util`.
- **Used by:** the modules and `app`.
- **Status:** foundation landed. Command execution and undo, the job system,
  provider dispatch, the one localization service, and the Codex service subtree
  have real bodies; the layer fills out as the roadmap (bible section 26) reaches
  it.

## Boundaries

- MUST NOT depend on `egui`.
- MUST NOT block the UI thread — long or expensive work is a job run off-thread;
  results return over channels the egui loop drains each frame.
- A job produces a result; applying that result is a `Command`. A job never
  mutates the project model directly.
- Never hold a lock across `.await`; CPU-bound work goes through `spawn_blocking`.
  This crate owns the CPU-worker and async-I/O lanes, the background-worker
  contract, and the parallelization priorities (architecture bible sections 31.2,
  13.6, 23.5); `rayon` is a candidate for data-parallel batches, not adopted.
- `#[instrument]` job bodies and command apply; `info!` on start/finish, `error!` /
  `warn!` on failure. Instrument spawned async tasks so their work is traced off the
  UI thread (pair the `pixhaus-tokio` and `pixhaus-tracing` skills).
- This crate owns the ONE localization service (`src/i18n.rs`): the single
  `rust-i18n` wiring (`i18n!`) and the embedded `locales/*.yaml` bundles, parallel to
  the command executor it owns. It stays egui-free and resolves keys to `String`;
  `app` sets the active language at boot, libraries and modules emit keys. See the
  `pixhaus-i18n` skill.

- Record the why: when a choice here is made for a non-obvious reason — a
  trade-off, a rejected alternative, a constraint, or a workaround — state that
  reason in a `//` comment at each spot it shaped, not just in the commit. See the
  root `CLAUDE.md` "Recording decisions" rule.

Reach for the `pixhaus-tokio` skill for async work. Global rules: root `CLAUDE.md`.
Architecture: `docs/pixhaus_architecture_bible.md`.
