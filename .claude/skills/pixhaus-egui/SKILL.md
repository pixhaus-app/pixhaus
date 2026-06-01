---
name: pixhaus-egui
description: >
  Use when writing, reviewing, or debugging any egui / eframe / egui-wgpu UI code in
  the Pixhaus native shell — building panels (layers, timeline, palette, tool options),
  the wgpu canvas, custom widgets, menus, dialogs, theming, input/shortcut handling, or
  texture display. Trigger this for ANY immediate-mode Rust UI work even when the user
  says "the window", "the toolbar", "the canvas viewport", "draw this in the UI", or
  names a widget without saying "egui". egui is immediate-mode and its 0.34 API differs
  sharply from older examples and from retained-mode frameworks, so reach for this skill
  rather than relying on memory.
---

# egui for Pixhaus

egui is an immediate-mode GUI. The whole UI is a function you re-run every frame; there
are no persistent widget objects. This skill is the floor for egui work in the Pixhaus
shell: the mental model that prevents the recurring bugs, the verified 0.34 API for the
app skeleton and panels, and how the pieces map onto a pixel-art editor (panels, the
wgpu canvas, custom widgets, input).

When you need the full API surface for an area, open the matching file in `references/`.
Don't guess signatures from memory — egui's API moves between releases and 0.34 was a
large redesign. The references are derived from docs.rs 0.34.2 and the load-bearing calls
were checked against the live docs.

## Versions — pin in lockstep

The egui family moves together. A mismatched `wgpu` is the most common build break,
because two `wgpu` versions in the tree are different types and won't interoperate.

| Crate | Version |
|---|---|
| `egui` | 0.34.2 |
| `eframe` | 0.34.2 |
| `egui-wgpu` | 0.34.2 |
| `egui-winit` | 0.34.2 |
| `epaint` | 0.34.2 |
| `wgpu` | `=29.0.1` (pin exactly, not `"29"`) |
| `winit` | 0.30.x |

```toml
egui      = "0.34"
eframe    = { version = "0.34", features = ["wgpu"] }
egui-wgpu = "0.34"
wgpu      = "=29.0.1"
```

When you bump any one, bump them all and re-verify against docs.rs — see [[feedback-dep-upgrades]].

## The mental model: immediate mode

Every frame you call the same UI code from scratch. `ui.button("Save")` does not create a
button that lives across frames — it allocates a rectangle, queues shapes for this frame,
hit-tests against last frame's pointer state, and returns a `Response`. Next frame it all
happens again from nothing.

Three consequences drive almost every correct/incorrect decision:

1. **State lives outside the UI code.** A local variable inside the closure is gone next
   frame. Persistent application state (the document, the undo stack, tool selection,
   panel widths you manage yourself) lives in your `eframe::App` struct. Transient UI-only
   state egui manages for you, keyed by `Id`, in its `Memory` store. Never put document
   data in `Memory` — it's for widget internals (scroll offset, collapsed state, focus).

2. **You act on the `Response`, not by polling state.** Read `.clicked()`, `.changed()`,
   `.dragged()` on the returned `Response` and do the side effect right there. Mutating a
   value bound to a slider and then acting on it unconditionally runs the side effect every
   frame, not just when it changed.

   ```rust
   // right: the side effect fires only on actual change
   if ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0)).changed() {
       layer.set_opacity(opacity);
   }
   ```

3. **egui repaints on demand, not on a clock.** It's not burning a core at 60fps when
   idle. After interaction or animation it repaints; otherwise it sleeps. If you're
   animating or polling a background result, call `ui.ctx().request_repaint()` (or
   `request_repaint_after(duration)`) to schedule the next frame yourself. A background
   thread holding a cheap `Context` clone can call `request_repaint()` to wake the UI when
   a result lands — this is how Pixhaus surfaces async AI/IO results into the frame loop.

## The app skeleton (verified 0.34 API)

egui 0.34 changed the entry point. `eframe::App`'s method is now **`ui`**, taking a
`&mut egui::Ui` for the whole window; the old `update(&mut self, ctx, frame)` is
deprecated. Per-frame non-UI work goes in `logic`.

```rust
struct Pixhaus {
    doc: Document,            // owned application state — the single owner
    tool: Tool,
}

impl eframe::App for Pixhaus {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Panels are added to the window Ui. ORDER MATTERS: outer first, central LAST.
        egui::Panel::top("toolbar").show_inside(ui, |ui| self.toolbar(ui));
        egui::Panel::left("layers").resizable(true).default_size(220.0)
            .show_inside(ui, |ui| self.layers_panel(ui));
        egui::Panel::bottom("timeline").resizable(true)
            .show_inside(ui, |ui| self.timeline(ui));
        egui::CentralPanel::default().show_inside(ui, |ui| self.canvas(ui));
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "ui_state", &self.ui_prefs);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,                 // required for the wgpu canvas
        viewport: egui::ViewportBuilder::default().with_title("Pixhaus"),
        ..Default::default()
    };
    eframe::run_native("Pixhaus", options,
        Box::new(|cc| Ok(Box::new(Pixhaus::new(cc)))))
}
```

Notes that are easy to get wrong:

- **Panel display is `show_inside(ui, …)`**, taking `&mut Ui`. The `show(ctx, …)` form is
  deprecated. `SidePanel`/`TopBottomPanel` are deprecated aliases of `Panel`; use
  `Panel::left/right/top/bottom(id)`.
- **`CentralPanel` is added last.** It fills whatever space the other panels left. Any
  panel added after it gets zero space.
- **`Window` and `Area` still take `&Context`**, not `&mut Ui` — reach it with
  `ui.ctx()`. They float above panels.
- Set up fonts/style and grab the wgpu render state in `Pixhaus::new(cc)` via
  `cc.egui_ctx` and `cc.wgpu_render_state`.

## Rules that prevent the recurring bugs

- **Give stateful widgets in loops unique ids.** egui derives ids from call-site location;
  a loop reuses one location, so all rows collide and share scroll/collapse/focus/drag
  state. Wrap each iteration in `ui.push_id(index, |ui| { … })` or pass `.id_salt(index)`.
  Stable ids are mandatory for `CollapsingHeader`, `ScrollArea`, `Window`, `Grid`,
  `TextEdit` cursors, and drag-and-drop.
- **Query `available_size()` immediately before you use it.** It shrinks as you add
  widgets; a value fetched early is stale.
- **Virtualize long lists.** A 500-layer panel or a long timeline must not build a widget
  per row. Use `ScrollArea::show_rows` (uniform height) or `show_viewport` (variable
  height); see `references/layout-and-panels.md`.
- **Never hold a `Context` lock across `.await` or across another `ctx` access.** All of
  `ctx.input(|i| …)`, `ctx.memory_mut(|m| …)`, `ctx.fonts(|f| …)` take short closures for
  exactly this reason. Calling one inside another on the same `Context` deadlocks.
- **A `Response` is frame-scoped.** Don't store it across frames.

## Pixhaus applications

This is where the editor's surfaces map onto egui. Each links to deeper reference material.

- **The canvas is a `wgpu` paint callback, not an egui image.** Allocate the viewport,
  push an `egui_wgpu::Callback`, and render the pixel buffer in a raw `wgpu` pass whose
  GPU resources live in `egui_wgpu::CallbackResources`. This is the make-or-break feature
  and the reason for the native rewrite — pixels never leave the GPU. Full pattern in
  `references/custom-wgpu-canvas.md`.
- **If you display pixels as a texture instead, always use `TextureOptions::NEAREST`** and
  update dirty regions with `TextureHandle::set_partial`, never a full re-upload per frame.
  `load_texture` allocates GPU memory — call it once and cache the handle. The 8K perf
  constraint ([[8k-perf-constraint]]) lives here. See `references/painting-and-textures.md`.
- **Panels carry the editor chrome:** left/right `Panel`s for layers and properties, a
  bottom `Panel` for the timeline, a top `Panel` for the toolbar and menu bar, a
  `CentralPanel` for the canvas. Layers and timeline need virtual scrolling. See
  `references/layout-and-panels.md`.
- **Tools, swatches, and toggles are often custom widgets** — allocate a rect, sense
  `click()`/`click_and_drag()`, paint with the `Painter`, call `response.mark_changed()`
  when internal state flips. The color-swatch and tool-toggle patterns are in
  `references/widgets.md`.
- **Tool input routes off the canvas `Response`** (`drag_started`/`drag_delta`/
  `drag_stopped`, `interact_pointer_pos`) plus `ui.ctx().input(|i| …)` for modifiers,
  wheel zoom, and keyboard shortcuts. Routing and shortcuts are in
  `references/input-state-and-theming.md`.
- **Theming** to match the editor's look is done through `Visuals`/`Style` set once in
  `Pixhaus::new`. Same reference file.

## References

Open the file for the area you're working in; each is a dense API reference for egui 0.34.2.

| File | Covers |
|---|---|
| `references/core-model.md` | Frame cycle, `Context`, `Ui`, `Response`, `Sense`, `Id`, `Memory`, repaint control |
| `references/layout-and-panels.md` | Panels, `Window`, `Area`, `Frame`, `ScrollArea` + virtual scroll, `Grid`, `CollapsingHeader`, layouts, sizing |
| `references/widgets.md` | Every built-in widget, the `.changed()` pattern, custom widgets, menus, tooltips, context menus |
| `references/painting-and-textures.md` | `Painter`, `Shape`, `Color32` (premultiplied alpha), textures, `NEAREST`, `set_partial`, fonts/galleys |
| `references/custom-wgpu-canvas.md` | `egui_wgpu::CallbackTrait`, `Callback`, `CallbackResources`, the canvas render-pass pattern |
| `references/input-state-and-theming.md` | `InputState`, pointer/keyboard, shortcuts, `Memory` persistence, `Visuals`/`Style` theming |

A standing caution: the references record the 0.34.2 API faithfully, but a few deep
signatures were flagged during research as unverifiable from the rendered docs (noted
inline as "verify"). When one is load-bearing for what you're building, confirm it against
https://docs.rs/egui/0.34.2/ or the source before depending on it.
