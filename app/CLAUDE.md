# pixhaus-app

The host application binary — the eframe + egui shell (architecture bible section
4.1). Depends on everything; nothing depends on it.

- **Owns:** app and window lifecycle, the single tokio runtime, viewport boot,
  module registration, the egui loop, top-level error handling, and the tracing
  subscriber + rolling file appender (`src/diagnostics.rs`).
- **Depends on:** the whole workspace (`ui` and `platform` directly, the rest
  transitively). External: `eframe`, `egui`, `egui-wgpu`, `tokio`, `tracing`,
  `tracing-subscriber`, `tracing-appender`, `tracing-log`, `anyhow`.
- **Status:** runnable spine — boots a window with the wgpu-drawn canvas.

## Boundaries

- This is the ONLY crate that uses `anyhow` (libraries use `thiserror`), and the
  ONLY owner of the tokio runtime — no `#[tokio::main]` and no hidden runtimes
  anywhere else.
- MUST NOT hold detailed sprite-editing logic. The binary wires things together —
  it boots the host, registers modules, runs the loop. Editor features live in
  `core` and the modules.
- The egui loop runs on one thread and owns the document directly; background task
  results return over channels the loop drains each frame. Never block the loop.
- Boot installs the design system on the egui `Context` before the loop — theme
  (`apply_to_visuals`), fonts (`install_fonts`), and image loaders
  (`install_image_loaders`) — and sets the OS window icon from `pixhaus_ui::brand`.
  The look itself belongs to `ui`; the binary only wires it up.
- This is the ONLY crate that installs a tracing subscriber (`src/diagnostics.rs`):
  console + rolling file under `pixhaus_platform::log_dir()`, one shared `EnvFilter`,
  the `log` bridge for wgpu/winit. Build it BEFORE the tokio runtime so startup
  itself is logged. The `WorkerGuard` it returns MUST be held for all of `main`
  (`let _guard = ...`, never `let _ = ...`) — dropping it early flushes the
  non-blocking writer and loses the log tail. Libraries emit; only `app` configures.
  See the `pixhaus-tracing` skill.

Reach for `pixhaus-eframe` for boot/window and `pixhaus-egui` for the loop. Global
rules: root `CLAUDE.md`. Architecture: `docs/pixhaus_architecture_bible.md`.
