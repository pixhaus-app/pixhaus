# pixhaus-render

The UI-agnostic `wgpu` viewport renderer (architecture bible sections 4.5, 16).
The perf-critical code and the reason for the native rewrite: pixel data is
composited on the GPU and never crosses a CPU copy per painted pixel.

- **Owns:** the canvas render pipelines, the texture cache, dirty-rect uploads,
  overlays drawn on the GPU.
- **Depends on:** `core`. External: `wgpu` (pinned `=29.0.1`), `bytemuck`/`glam`
  as needed.
- **Used by:** `ui`, through the egui paint callback.
- **Status:** runnable spine — `ViewportRenderer` draws a flat viewport fill.

## Boundaries

- MUST NOT know about `egui` or `egui-wgpu`. It exposes only raw `wgpu` types so
  it survives a UI-toolkit change. The egui glue lives in `ui`.
- MUST NOT own project data. GPU textures are caches and views; `core` is the
  source of truth. `render` is the render/GPU execution lane and owns the
  derived-cache bucket, isolated from project truth (bible sections 22.6 and 31.2).
- MUST NOT begin, end, or submit the egui render pass from `paint()` — record
  draws into the pass egui-wgpu owns.
- Bound work by the dirty region, not the canvas size (the 8192x8192 ceiling).
- Tracing spans stay OFF the per-pixel / per-scanline hot path — coarse only (a
  whole texture upload or composite, not the pixels inside it). A span has real
  cost; at 8K a per-pixel span is the opposite of what the dirty-rect bound buys.
  See the `pixhaus-tracing` skill.
- No user-facing strings on the GPU path: `render` is UI-agnostic and never
  localizes. Debug and perf traces stay English developer text. See the
  `pixhaus-i18n` skill.

Reach for the `pixhaus-wgpu` skill before touching GPU code. Global rules: root
`CLAUDE.md`. Architecture: `docs/pixhaus_architecture_bible.md`.
