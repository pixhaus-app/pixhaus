# Core model: frame cycle, Context, Ui, Response, Sense, Id, Memory

egui 0.34.2. The pieces every other reference builds on.

## Contents
- The frame cycle
- Repaint control
- `Context`
- `Ui`
- `Response`
- `Sense`
- `InnerResponse`
- `Id` and widget identity
- `Memory` and per-widget state

## The frame cycle

A frame is: gather OS events into `RawInput` → run your UI closure → get `FullOutput`
(shapes, texture deltas, platform output) → tessellate → paint. With `eframe` you never
write this loop; eframe calls your `App::ui` each frame and does the rest. The closure
receives a `&mut Ui` covering the whole window.

The conceptual loop (eframe does this for you):

```rust
let full_output = ctx.run(raw_input, |ctx| { /* build UI */ });   // eframe-internal
// apply full_output.textures_delta, then ctx.tessellate(full_output.shapes, ppp), then paint
```

`RawInput` carries `screen_rect`, `pixels_per_point` (DPI), `time`, `predicted_dt`,
`modifiers`, an ordered `events: Vec<Event>`, and `dropped_files`/`hovered_files`.
`FullOutput` carries `platform_output` (cursor, clipboard, IME), `textures_delta` (font
and image texture uploads — apply before drawing), `shapes`, and `viewport_output`
(per-viewport repaint timing).

## Repaint control

egui repaints on demand. It schedules an immediate next frame when a widget was
interacted with, an animation is active, or `request_repaint` was called; otherwise it
sleeps until the next OS event.

```rust
ctx.request_repaint();                                   // repaint asap after this frame
ctx.request_repaint_after(Duration::from_millis(100));   // schedule a later frame (shortest wins)
```

A `Context` is a cheap `Arc` clone and is `Send + Sync`. Clone it into a background task;
when a result is ready, call `request_repaint()` to wake the UI. This is how Pixhaus pulls
async results (AI verbs, file IO, video decode) into the frame loop: the task sends on a
channel and pings `request_repaint`; the next `ui` drains the channel.

## `Context`

The per-window singleton; interior-mutable behind an `RwLock`. All access is through short
closures — never hold one across `.await` or nest two on the same `Context` (deadlock).

```rust
ctx.input(|i| i.pointer.hover_pos());        // read InputState
ctx.input_mut(|i| i.consume_key(mods, key)); // read+consume
ctx.memory_mut(|m| m.data.insert_temp(id, v));
ctx.fonts(|f| f.layout_no_wrap(text, font, color));  // valid only after first frame
ctx.request_repaint();
ctx.set_visuals(egui::Visuals::dark());
let ctx = ui.ctx();                          // reach the Context from a Ui
```

## `Ui`

The per-region builder, created by panels/windows/areas (you rarely construct it). It owns
a layout cursor that advances as you add widgets — strictly forward, top-to-bottom /
left-to-right within the current layout.

```rust
ui.label("text");                       // -> Response (convenience over ui.add(Label::new))
ui.add(widget);                          // place any Widget -> Response
ui.add_sized([120.0, 20.0], widget);     // place at an exact size
ui.add_enabled(enabled, widget);         // greyed + inert when false
ui.horizontal(|ui| { … });               // InnerResponse<R>; left-to-right, centered
ui.vertical(|ui| { … });
ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { … });
ui.scope(|ui| { … });                    // child Ui; style/spacing changes don't leak out
ui.push_id(index, |ui| { … });           // salt all ids inside (loops!)
ui.painter();                            // &Painter for raw drawing on this Ui's layer
ui.available_size();                     // space left RIGHT NOW (shrinks as you add)
ui.allocate_exact_size(size, sense);     // (Rect, Response) — custom widgets
ui.allocate_painter(size, sense);        // (Response, Painter) — custom drawing + input
ui.ctx();                                // the Context
```

## `Response`

Returned by every widget and layout closure. Carries `rect`, `id`, and the interaction
results from this frame. Act on it directly.

```rust
r.clicked(); r.secondary_clicked(); r.double_clicked();
r.changed();                         // user changed a value this frame (sliders, text)
r.hovered(); r.contains_pointer();
r.dragged(); r.drag_started(); r.drag_stopped(); r.drag_delta();
r.interact_pointer_pos();            // Option<Pos2> where the click/drag is
r.hover_pos();                       // Option<Pos2> for tooltips
r.has_focus(); r.gained_focus(); r.lost_focus();
r.on_hover_text("tip");              // chainable
r.context_menu(|ui| { … });          // right-click menu
r.interact(egui::Sense::click());    // add interaction to e.g. a Label retroactively
r.mark_changed();                    // in a custom widget, signal a value change
```

`PointerButton`: `Primary` (left), `Secondary` (right), `Middle`, `Extra1`, `Extra2`.
`r.clicked_by(PointerButton::Middle)` etc.

## `Sense`

What input a widget reacts to. `click()` and `drag()` also make a widget keyboard-focusable.
`click_and_drag()` adds a few frames of latency to disambiguate, so don't use it when one
suffices.

```rust
egui::Sense::hover()            // hover only, no focus
egui::Sense::click()
egui::Sense::drag()
egui::Sense::click_and_drag()   // canvas viewport uses this (click to place, drag to paint)
```

## `InnerResponse`

Layout closures return `InnerResponse<R>` — `.inner` is your closure's return value,
`.response` covers the whole region.

```rust
let picked = ui.horizontal(|ui| {
    ui.label("Tool:");
    ui.button("Pencil").clicked()     // closure returns bool
});
if picked.inner { /* … */ }
picked.response.on_hover_text("Choose a tool");
```

## `Id` and widget identity

egui derives most ids from source-code location plus a label hash. That breaks in two
cases: widgets in a **loop** (same location every iteration) and widgets that **move**
between frames. Colliding ids share `Memory` state — two collapsing headers open together,
focus jumps, drag-and-drop fails.

```rust
egui::Id::new("layers_panel_state");        // explicit stable id
let child = parent_id.with("header");        // derived child id

for (i, layer) in layers.iter().enumerate() {
    ui.push_id(i, |ui| { /* unique id space per row */ });
    // or: CollapsingHeader::new(&layer.name).id_salt(i)
}
```

Stable ids are required for window position/size, `CollapsingHeader` open state,
`ScrollArea` offset, `TextEdit` cursor, and drag-and-drop.

## `Memory` and per-widget state

`Memory` holds focus, open-popup tracking, and a typed `data` store keyed by `(Id, TypeId)`.
Use it for transient UI state only — the document model and undo stack live in your `App`
struct, not here.

```rust
// transient (cleared between sessions unless persisted)
ctx.memory_mut(|m| m.data.insert_temp(id, my_small_state));
let s: Option<MyState> = ctx.memory(|m| m.data.get_temp(id));

// persisted across runs (needs the persistence feature; T: SerializableAny)
ctx.memory_mut(|m| m.data.insert_persisted(id, value));

// focus
ctx.memory_mut(|m| m.request_focus(id));
```

`get_temp` clones the value, so keep stored types small (or wrap in `Arc`).
`CollapsingState::load_with_default_open(ctx, id, true)` is the ergonomic wrapper for
collapsible state (used by custom layer-group headers; see layout reference).

## Flagged / verify

- `Context::run` vs `run_ui`: the closure-arg form was being reworked in 0.34; eframe calls
  it for you, so you rarely touch it. Verify the exact name if you drive egui without eframe.
- A handful of `Context` query methods (`wants_keyboard_input`, `is_using_pointer`) were
  being renamed with an `egui_`-prefixed form in 0.34 — verify against docs if you use them.
