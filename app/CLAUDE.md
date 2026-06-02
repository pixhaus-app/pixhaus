# pixhaus-app

The host application binary — the eframe + egui shell (architecture bible section
4.1). Depends on everything; nothing depends on it.

- **Owns:** app and window lifecycle, the single tokio runtime, viewport boot,
  module registration, the egui loop, and top-level error handling.
- **Depends on:** the whole workspace (`ui`, and transitively the rest). External:
  `eframe`, `egui`, `egui-wgpu`, `tokio`, `tracing-subscriber`, `anyhow`.
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

Reach for `pixhaus-eframe` for boot/window and `pixhaus-egui` for the loop. Global
rules: root `CLAUDE.md`. Architecture: `docs/pixhaus_architecture_bible.md`.
