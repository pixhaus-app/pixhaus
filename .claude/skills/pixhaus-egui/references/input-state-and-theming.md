# Input, shortcuts, persistence, and theming

egui 0.34.2. Reading pointer/keyboard, routing tool input off the canvas, keyboard
shortcuts, persisting state, and theming the editor.

## Contents
- Reading input (`InputState`, pointer, keyboard, scroll, zoom)
- Routing canvas tool input
- Keyboard shortcuts
- Persistence (eframe + `Memory`)
- Theming (`Visuals`, `Style`)

## Reading input

All input is read through a short closure: `ctx.input(|i| …)` (read) or
`ctx.input_mut(|i| …)` (read + consume). Reach the context with `ui.ctx()`.

```rust
ui.ctx().input(|i| {
    let mods   = i.modifiers;                 // .ctrl .shift .alt .command .mac_cmd
    let hover  = i.pointer.hover_pos();        // Option<Pos2> — for tooltips/cursor
    let act    = i.pointer.interact_pos();     // Option<Pos2> — confirmed click/drag pos
    let primary_down = i.pointer.primary_down();
    let dragging = i.pointer.is_decidedly_dragging();
    let wheel  = i.smooth_scroll_delta;        // prefer over raw_scroll_delta
    let zoom   = i.zoom_delta();               // pinch / ctrl-wheel zoom factor
    let space  = i.key_pressed(egui::Key::Space);
});
```

- `interact_pos` is the position of an in-progress click/drag; `hover_pos` is just where the
  pointer is. Use `interact_pos` to drive tools, `hover_pos` to draw the brush cursor.
- `is_decidedly_dragging()` separates a drag from a click after egui's small threshold.
- `Modifiers::COMMAND` is the cross-platform "primary" modifier (Ctrl on Windows/Linux,
  Cmd on macOS) — use it for shortcuts so Mac users get ⌘.
- `egui::Key` covers letters, digits, arrows, function keys, `Space`, `Enter`, `Escape`,
  `Delete`, `Backspace`, `Tab`, etc.

## Routing canvas tool input

Drive tools off the canvas `Response` (from `allocate_painter`) plus modifiers from
`InputState`. The priority order Pixhaus follows: an in-progress transform/drag wins, then
space/middle-mouse pan, then the active tool.

```rust
let (response, painter) =
    ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
let rect = response.rect;
let ctx = ui.ctx();

// pan: space-drag or middle mouse
let panning = ctx.input(|i| i.key_down(egui::Key::Space))
    || response.dragged_by(egui::PointerButton::Middle);

if panning {
    self.camera.scroll -= response.drag_delta();
} else if response.drag_started() {
    if let Some(p) = response.interact_pointer_pos() {
        self.begin_stroke(self.screen_to_canvas(p, rect));
    }
} else if response.dragged() {
    if let Some(p) = response.interact_pointer_pos() {
        self.extend_stroke(self.screen_to_canvas(p, rect));  // accumulate; rasterize inline
    }
} else if response.drag_stopped() {
    self.end_stroke();
}

// wheel zoom about the cursor
let (scroll, zoom) = ctx.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
if zoom != 1.0 { if let Some(p) = response.hover_pos() { self.camera.zoom_at(p, zoom); } }
```

Because drawing is synchronous in the egui frame, stroke points accumulate and rasterize
inline, bounded by the dirty region — there is no IPC round trip and no per-move re-upload.

## Keyboard shortcuts

```rust
const SAVE: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);

if ui.ctx().input_mut(|i| i.consume_shortcut(&SAVE)) {
    self.save();
}
```

`consume_shortcut` both tests and consumes the event, so a focused `TextEdit` and the
global handler don't both fire. Show the binding on a button with
`Button::new("Save").shortcut_text("Ctrl+S")`. `i.consume_key(modifiers, key)` is the
lower-level form.

## Persistence

Application/document state belongs in your `App` struct, saved through eframe; `Memory` is
only for transient widget state (see `core-model.md`).

```rust
// eframe app-level persistence (needs the eframe "persistence" feature):
impl eframe::App for Pixhaus {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "ui_prefs", &self.ui_prefs);   // T: serde::Serialize
    }
}
// restore in Pixhaus::new:
let prefs = cc.storage
    .and_then(|s| eframe::get_value::<UiPrefs>(s, "ui_prefs"))
    .unwrap_or_default();

// transient widget state egui persists for you (with the egui persistence feature):
ctx.memory_mut(|m| m.data.insert_persisted(id, value));   // T: SerializableAny
```

## Theming

Set the look once in `Pixhaus::new` via `cc.egui_ctx`. `Visuals` carries colors,
`Style.spacing` the metrics. Mutate in place with `style_mut` to avoid cloning the whole
style.

```rust
fn install_theme(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());          // or light()
    ctx.style_mut(|style| {
        let v = &mut style.visuals;
        v.panel_fill        = egui::Color32::from_rgb(24, 24, 28);
        v.window_fill       = egui::Color32::from_rgb(30, 30, 35);
        v.extreme_bg_color  = egui::Color32::from_rgb(16, 16, 18);   // text edit / canvas bg
        v.selection.bg_fill = egui::Color32::from_rgb(64, 110, 180);
        // per-state widget visuals: noninteractive / inactive / hovered / active / open
        v.widgets.inactive.bg_fill   = egui::Color32::from_rgb(48, 48, 54);
        v.widgets.hovered.bg_fill    = egui::Color32::from_rgb(60, 60, 68);
        v.widgets.active.bg_fill     = egui::Color32::from_rgb(72, 72, 82);
        // metrics:
        style.spacing.item_spacing  = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(6.0, 3.0);
    });
}
```

Per-region overrides use `ui.style_mut()` inside a `ui.scope(|ui| …)` so they don't leak.
To port the old CSS-variable palette, map each variable to the corresponding `Visuals`
field once here; `corner_radius` lives on `Visuals::widgets.*.corner_radius` and individual
`Frame`s.

## Flagged / verify

- A few `Context` input-query helpers (`wants_keyboard_input`, `is_using_pointer`,
  `wants_pointer_input`) were being renamed with an `egui_`-prefixed form in 0.34 — verify
  the current name if you gate global shortcuts on "is a text field focused".
- Exact field paths under `Visuals::widgets` (e.g. `fg_stroke` vs `text_color`) are stable
  in spirit but confirm against docs.rs when porting a precise palette.
