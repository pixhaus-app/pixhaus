# pixhaus-ui

The egui contribution surface — workspace runtime, registries, and the Module
trait (architecture bible sections 4.2, 7, 8). The only crate that knows both
egui and `render`.

- **Owns:** the Panel/Tool/Workspace/Provider/Importer/Exporter/Validator traits,
  the registries, the `Module` trait, theme tokens, and the egui-to-`render`
  canvas paint callback.
- **Depends on:** `core`, `services`, `render`, `io`. External: `egui`,
  `egui-wgpu`, `wgpu`.
- **Used by:** the modules and `app`.
- **Status:** runnable spine — `CanvasCallback` and `install_canvas_renderer`.

## Boundaries

- This is the ONLY crate that may know both egui and `render`. Don't push egui
  types down into `core`/`render`/`io`/`services`.
- MUST NOT own durable project data or long-running jobs — those are
  `core`/`services`.
- Panels capture intent and display state; they request mutations through commands
  and never mutate the model directly.
- egui is the presentation layer, not the architecture — keep workspace business
  logic out of widget code.

Reach for `pixhaus-egui` and `pixhaus-egui-wgpu` skills here. Global rules: root
`CLAUDE.md`. Architecture: `docs/pixhaus_architecture_bible.md`.
