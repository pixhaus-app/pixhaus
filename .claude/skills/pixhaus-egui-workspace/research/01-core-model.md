# egui core model — research digest

**egui version documented: 0.34.2** (latest as of 2026-05-25)
Source: docs.rs/egui/latest, context7 /emilk/egui, context7 /websites/rs_egui

---

## 1. Immediate-mode execution model

### What "immediate mode" means

There are no persistent widget objects. Every frame you call the same UI code from scratch. `ui.button("Save")` does not create a Button struct that lives across frames — it allocates a rect, paints colored shapes into the current frame's shape list, reads interaction state from the previous frame's hit-test results, and returns a `Response`. When you check `.clicked()` on that response, you are reading the result of last frame's pointer-down combined with this frame's pointer-up within the same rect.

Contrast with retained mode (React, Solid, Qt): in those systems you declare a component tree that persists in memory; the framework diffs it to decide what to repaint. In egui, the "tree" is the call stack of your frame function — it is rebuilt from scratch every frame and thrown away.

Mental traps when coming from React/Solid:
- "I'll store state in a local variable inside the UI function" — that variable is gone next frame. State must live outside the UI closure (in your `App` struct) or in `Memory` (via `IdTypeMap`).
- "I'll conditionally call `.clicked()` only sometimes" — fine, but then the widget is not allocated and gets no input.
- "I'll build the button once and re-use the `Response`" — `Response` is frame-scoped; holding it across frames is meaningless.
- "The widget is re-created every frame so it must be slow" — no: egui only repaints on demand. Between repaints the UI code does not run.

### A frame: RawInput → run_ui → FullOutput

```rust
// Canonical backend loop (simplified)
let mut ctx = egui::Context::default();
loop {
    // 1. Gather OS events into RawInput
    let raw_input: egui::RawInput = gather_input();

    // 2. Run UI code — produces all shapes and output
    let full_output: egui::FullOutput = ctx.run_ui(raw_input, |ui| {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            my_app.update(ui);
        });
    });

    // 3. Handle non-rendering output (clipboard, cursor, IME, …)
    handle_platform_output(full_output.platform_output);

    // 4. Upload changed textures BEFORE drawing
    apply_textures_delta(full_output.textures_delta);

    // 5. Tessellate shapes → triangles
    let clipped_primitives = ctx.tessellate(
        full_output.shapes,
        full_output.pixels_per_point,
    );

    // 6. Paint
    paint(clipped_primitives);
}
```

**`Context::run_ui` signature (preferred, 0.34+):**
```rust
pub fn run_ui(
    &self,
    new_input: RawInput,
    run_ui: impl FnMut(&mut Ui),
) -> FullOutput
```
The closure receives a `&mut Ui` covering the entire content area. Use panels inside it.

**`Context::run` (deprecated):**
```rust
pub fn run(&self, new_input: RawInput, run_ui: impl FnMut(&Self)) -> FullOutput
```
Closure receives `&Context` instead. Replaced by `run_ui`.

### RawInput fields (populated by the backend each frame)

| Field | Type | Purpose |
|---|---|---|
| `viewport_id` | `ViewportId` | Active viewport |
| `viewports` | `ViewportIdMap<ViewportInfo>` | All viewport info |
| `screen_rect` | `Option<Rect>` | Window content area in logical points |
| `pixels_per_point` | `Option<f32>` | DPI scale |
| `max_texture_side` | `Option<usize>` | GPU max texture dimension |
| `time` | `Option<f64>` | Monotonic seconds; drives animations |
| `predicted_dt` | `f32` | Expected frame time; default 1/60 s |
| `modifiers` | `Modifiers` | Currently held modifier keys |
| `events` | `Vec<Event>` | Key/pointer/scroll events in order |
| `hovered_files` | `Vec<HoveredFile>` | Files dragged over the window |
| `dropped_files` | `Vec<DroppedFile>` | Files dropped this frame |
| `focused` | `bool` | Window has OS keyboard focus |
| `system_theme` | `Option<Theme>` | OS dark/light preference |

### FullOutput fields (produced by run_ui, consumed by backend)

| Field | Type | Backend action |
|---|---|---|
| `platform_output` | `PlatformOutput` | Set cursor, write clipboard, open URLs, IME hints |
| `textures_delta` | `TexturesDelta` | Upload/free font+image textures (apply before draw) |
| `shapes` | `Vec<ClippedShape>` | Tessellate → triangles via `ctx.tessellate()` |
| `pixels_per_point` | `f32` | Pass to `tessellate()` for correct DPI scaling |
| `viewport_output` | `OrderedViewportIdMap<ViewportOutput>` | Spawn/close native windows for child viewports |

`ViewportOutput` contains `repaint_delay: Duration` — if zero, repaint immediately next frame. Prefer registering a `set_request_repaint_callback` instead.

### When egui repaints

egui does NOT repaint on a fixed timer by default. The backend decides when to call `run_ui`. egui signals that a repaint is needed through two mechanisms:

1. **`ViewportOutput::repaint_delay`** — a `Duration` in `FullOutput`. If `Duration::ZERO`, the backend should schedule the next frame immediately (no sleep).
2. **`set_request_repaint_callback`** — installed once; called from any thread whenever `request_repaint`/`request_repaint_after` is invoked, allowing the backend to wake its event loop.

egui sets `repaint_delay = Duration::ZERO` (immediate) when:
- Any widget was interacted with
- An animation is active (e.g., color fade, spinner)
- `ctx.request_repaint()` was called

It sets a non-zero delay when `request_repaint_after(duration)` was called (e.g. for a 1-second pulse animation). It sets `Duration::MAX` when nothing happened and no repaint is needed.

### Repaint control API

```rust
// Trigger repaint immediately after this frame (e.g., during animation)
ctx.request_repaint();

// Repaint after a delay (battery-friendly for slow animations)
// Only the shortest pending duration wins if called multiple times.
ctx.request_repaint_after(std::time::Duration::from_millis(100));

// Install backend wakeup callback (call once during init)
ctx.set_request_repaint_callback(|info| {
    // info.after: Duration — how long to wait before waking
    event_loop_proxy.send_event(WakeUp);
});
```

### Multi-pass within a frame: `request_discard`

```rust
pub fn request_discard(&self, reason: impl Into<Cow<'static, str>>)
```

Requests that the current pass be thrown away and re-run. Used when a layout cannot be known until after painting (e.g., `Grid` guesses column widths on pass 0, measures them, requests discard, re-runs with correct widths on pass 1). Limited by `Options::max_passes` (default: 2). Check current pass with `ctx.current_pass_index() -> usize`.

Frame counters:
- `ctx.cumulative_frame_nr() -> u64` — total completed frames
- `ctx.current_pass_index() -> usize` — pass within current frame (usually 0)
- `ctx.cumulative_pass_nr() -> u64` — total completed passes

---

## 2. `egui::Context`

### Thread safety and clone semantics

```rust
impl Clone for Context { … }  // cheap clone — Arc inside
impl Send for Context { … }
impl Sync for Context { … }
// NOT: RefUnwindSafe, UnwindSafe
```

`Context` is an `Arc` over `RwLock`-protected internal state. Cloning is O(1) reference-count increment. All methods take `&self` — interior mutability via `RwLock`. You can safely clone `Context` and send it to a background thread to call `request_repaint()`.

**Critical**: All state access is done through short-lived closures. You cannot hold the lock across frames or across `.await` points. Calling any locking method (`ctx.input(…)`) while already inside a locking closure on the same `Context` causes a deadlock.

### Key method signatures

```rust
// Run one frame of UI code
pub fn run_ui(&self, new_input: RawInput, run_ui: impl FnMut(&mut Ui)) -> FullOutput

// Read input state (locks briefly; do not call inside another ctx lock)
pub fn input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R
pub fn input_mut<R>(&self, writer: impl FnOnce(&mut InputState) -> R) -> R

// Read/write persistent memory (focus, widget state, popup state)
pub fn memory<R>(&self, reader: impl FnOnce(&Memory) -> R) -> R
pub fn memory_mut<R>(&self, writer: impl FnOnce(&mut Memory) -> R) -> R

// Repaint control
pub fn request_repaint(&self)
pub fn request_repaint_after(&self, duration: Duration)
pub fn set_request_repaint_callback(&self, callback: impl Fn(RequestRepaintInfo) + Send + Sync + 'static)

// Multi-pass layout
pub fn request_discard(&self, reason: impl Into<Cow<'static, str>>)

// Font access — valid only after first run_ui call
pub fn fonts<R>(&self, reader: impl FnOnce(&FontsView) -> R) -> R

// Style (note: set_style / style are deprecated; use global_style variants)
pub fn style(&self) -> Arc<Style>          // deprecated; use global_style
pub fn set_style(&self, style: impl Into<Arc<Style>>)  // deprecated
pub fn set_visuals(&self, visuals: Visuals)

// Pointer/keyboard interaction queries
pub fn is_using_pointer(&self) -> bool       // deprecated; use egui_is_using_pointer
pub fn wants_keyboard_input(&self) -> bool   // deprecated; use egui_wants_keyboard_input
pub fn wants_pointer_input(&self) -> bool    // deprecated; use egui_wants_pointer_input

// Debug painting
pub fn debug_painter(&self) -> Painter

// Frame counters
pub fn cumulative_frame_nr(&self) -> u64
pub fn current_pass_index(&self) -> usize
pub fn cumulative_pass_nr(&self) -> u64

// Tessellation (call after run_ui to get triangles)
pub fn tessellate(&self, shapes: Vec<ClippedShape>, pixels_per_point: f32) -> Vec<ClippedPrimitive>
```

### Relationship between Context and Ui

`Context` is the global singleton for a window/viewport. `Ui` is a scoped builder for a region within that window during one frame. `Ui` holds a reference to `Context` (accessible via `ui.ctx()`). When you call `ui.add(widget)`, egui internally calls `ctx.input(…)` to check interaction state and allocates space in the layout managed by the `Ui`.

---

## 3. `egui::Ui`

### Creation

`Ui` is created by panels, windows, and areas — you do not construct it directly in application code:
```rust
// Most common entry points:
egui::CentralPanel::default().show_inside(ui, |ui| { … });
egui::SidePanel::left("sidebar").show_inside(ui, |ui| { … });
egui::Window::new("title").show(ctx, |ui| { … });
egui::Area::new(egui::Id::new("overlay")).show(ctx, |ui| { … });

// Low-level (rare):
pub fn new(ctx: Context, id: Id, ui_builder: UiBuilder) -> Self
```

### Core methods

```rust
// Context access
pub fn ctx(&self) -> &Context

// Style — per-Ui style can differ from global
pub fn style(&self) -> &Arc<Style>
pub fn set_style(&mut self, style: impl Into<Arc<Style>>)
pub fn visuals(&self) -> &Visuals

// Add a Widget trait implementor
pub fn add(&mut self, widget: impl Widget) -> Response
pub fn add_enabled(&mut self, enabled: bool, widget: impl Widget) -> Response
pub fn add_visible(&mut self, visible: bool, widget: impl Widget) -> Response
pub fn add_space(&mut self, amount: f32)

// Convenience widget shortcuts
pub fn label(&mut self, text: impl Into<WidgetText>) -> Response
pub fn button<'a>(&mut self, atoms: impl IntoAtoms<'a>) -> Response
pub fn separator(&mut self) -> Response

// Layout containers (return InnerResponse<R>)
pub fn horizontal<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
pub fn vertical<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
pub fn with_layout<R>(&mut self, layout: Layout, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
pub fn scope<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
pub fn group<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>

// ID scoping — avoids widget ID collisions in loops
pub fn push_id<R>(&mut self, id_salt: impl Hash, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>

// Space queries
pub fn max_rect(&self) -> Rect           // available to this Ui
pub fn min_rect(&self) -> Rect           // space already used
pub fn available_rect_before_wrap(&self) -> Rect  // space left on current row/col
pub fn cursor(&self) -> Rect             // where next widget goes

// Manual space allocation (for custom widgets)
pub fn allocate_response(&mut self, desired_size: Vec2, sense: Sense) -> Response
pub fn allocate_rect(&mut self, rect: Rect, sense: Sense) -> Response
pub fn allocate_at_least(&mut self, desired_size: Vec2, sense: Sense) -> (Rect, Response)
pub fn allocate_exact_size(&mut self, desired_size: Vec2, sense: Sense) -> (Rect, Response)

// Hit testing outside widget allocation
pub fn interact(&self, rect: Rect, id: Id, sense: Sense) -> Response

// Painter (for raw shape drawing)
pub fn painter(&self) -> &Painter

// Spacing configuration
pub fn spacing(&self) -> &Spacing
```

### Layout cursor advancement

Each call to `ui.add`, `ui.label`, etc. allocates a rect in the current layout and advances the layout cursor. The layout is set by the surrounding container (`horizontal`, `vertical`, `with_layout`). You cannot go "backwards" — egui is strictly single-pass top-to-bottom / left-to-right within a layout.

---

## 4. `egui::Response`

### Struct definition (public fields)

```rust
pub struct Response {
    pub ctx: Context,           // for tooltip and further interaction checks
    pub layer_id: LayerId,      // which layer this widget is on
    pub id: Id,                 // widget's Id
    pub rect: Rect,             // screen area occupied
    pub interact_rect: Rect,    // clipped area that senses input (may be smaller)
    pub sense: Sense,           // what interactions were registered for
    // + private flags bitfield
}
```

**Note:** `Response` contains a `Context` clone. Do NOT call `ctx.input(…)` or any locking `Context` method from inside a `Response` method — you may already hold the lock.

### Interaction methods

```rust
// Click detection (pointer-up within rect, or keyboard Space/Enter, or accessibility activate)
pub fn clicked(&self) -> bool
pub fn double_clicked(&self) -> bool
pub fn triple_clicked(&self) -> bool
pub fn clicked_elsewhere(&self) -> bool   // pointer-up OUTSIDE this rect
pub fn clicked_by(&self, button: PointerButton) -> bool

// Drag detection
pub fn dragged(&self) -> bool
pub fn dragged_by(&self, button: PointerButton) -> bool
pub fn drag_delta(&self) -> Vec2          // points moved since last frame
pub fn drag_started(&self) -> bool        // true for exactly one frame at drag start
pub fn drag_stopped(&self) -> bool        // true for exactly one frame at drag end
pub fn is_pointer_button_down_on(&self) -> bool  // primary button held anywhere on widget

// Hover and pointer position
pub fn hovered(&self) -> bool             // pointer over rect AND no other widget dragging
pub fn contains_pointer(&self) -> bool    // pointer inside rect (ignores other drags)

// Data change (sliders, text edits)
pub fn changed(&self) -> bool

// Focus (keyboard)
pub fn has_focus(&self) -> bool
pub fn gained_focus(&self) -> bool        // true one frame on gaining focus
pub fn lost_focus(&self) -> bool          // true one frame on losing focus
pub fn request_focus(&self)
pub fn surrender_focus(&self)

// Add more interaction retroactively (e.g., make a label clickable)
pub fn interact(&self, sense: Sense) -> Self

// Tooltips / hover decoration
pub fn on_hover_text(self, text: impl Into<WidgetText>) -> Self
pub fn on_hover_cursor(self, cursor: CursorIcon) -> Self
pub fn on_disabled_hover_text(self, text: impl Into<WidgetText>) -> Self

// Context menu (right-click)
pub fn context_menu(self, add_contents: impl FnOnce(&mut Ui)) -> Self

// Visual emphasis
pub fn highlight(self) -> Self

// Accessibility
pub fn labelled_by(self, id: Id) -> Self

// Merge two responses (logical OR of all flags; id comes from self)
// WARNING: cannot call .interact() on the union result
pub fn union(self, other: Self) -> Self
```

### PointerButton enum

```rust
pub enum PointerButton {
    Primary   = 0,  // left mouse button
    Secondary = 1,  // right mouse button (context menus)
    Middle    = 2,  // middle / scroll wheel click
    Extra1    = 3,  // browser back
    Extra2    = 4,  // browser forward
}
```

---

## 5. `egui::Sense`

Bitflags struct (no public fields). Controls what input a widget reacts to.

```rust
// Constructors
pub fn click() -> Sense             // clicks + hover; widget becomes keyboard-focusable
pub fn drag() -> Sense              // drags + hover; widget becomes keyboard-focusable
pub fn click_and_drag() -> Sense    // both clicks and drags; adds latency to distinguish
pub fn hover() -> Sense             // hover detection only; no click, no drag, no focus
pub fn focusable_noninteractive() -> Sense  // keyboard focus only (for screen readers on labels)

// Associated constants (same as above but as consts)
Sense::CLICK
Sense::DRAG
Sense::HOVER
Sense::FOCUSABLE

// Combine senses
pub fn union(self, other: Sense) -> Sense
```

**`click_and_drag` latency note**: when both click and drag are sensed, egui must wait a few frames before it knows which one is happening. Avoid if only one is needed.

**Custom widget pattern:**
```rust
let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());
// rect: Rect — paint here
// response: check .clicked(), .dragged(), .drag_delta()
```

---

## 6. `egui::InnerResponse<R>`

```rust
pub struct InnerResponse<R> {
    pub inner: R,           // return value of the user closure
    pub response: Response, // the Response of the container area
}

impl<R> InnerResponse<R> {
    pub fn new(inner: R, response: Response) -> Self
}
```

All layout closures (`horizontal`, `vertical`, `scope`, `group`, `push_id`, `with_layout`) return `InnerResponse<R>` where `R` is what the closure returns.

```rust
// Return a value out of a layout closure
let result = ui.horizontal(|ui| {
    ui.label("Pick:");
    ui.button("A").clicked()   // returns bool
});
// result.inner:    bool  — whether "A" was clicked
// result.response: Response — covers the entire horizontal strip

// Attach tooltip to the whole strip:
result.response.on_hover_text("Choose A or B");

// Closure returns () if you ignore the return value:
ui.horizontal(|ui| {
    ui.label("same");
    ui.label("row");
});
// returns InnerResponse<()>
```

---

## 7. `egui::Id` and widget identity

### Construction

```rust
pub fn new(source: impl Hash) -> Self   // hash any hashable value into an Id
pub fn with(self, child: impl Hash) -> Self  // derive a child Id
pub fn value(&self) -> u64              // raw hash value

pub const NULL: Id    // fallback; used for singleton Memory entries
```

### How widget IDs are derived

egui derives most widget IDs from the **source-code location** (file + line + column) combined with a hash of the widget's label or explicit salt. This is done automatically by macros in the widget implementations — you rarely call `Id::new` directly.

Location-based IDs work for stateless widgets. They break for:
- Widgets created in a **loop** — all iterations get the same source location
- Widgets that **move** in the layout between frames — the same widget ends up at a different location hash

### ID collision consequences

Colliding IDs share Memory state: two `CollapsingHeader`s with the same `Id` will open/close together. Focus can jump unexpectedly. Drag-and-drop fails if the dragged item's `Id` changes between frames.

### Avoiding collisions

```rust
// 1. push_id: salt all IDs inside a closure
for (i, item) in items.iter().enumerate() {
    ui.push_id(i, |ui| {
        ui.label(&item.name);
        // all widgets inside get IDs derived from i
    });
}

// 2. Id::new with a unique stable key
let id = egui::Id::new("my_window_state");

// 3. Id::with to derive a namespaced child
let child_id = parent_id.with("header");
```

### Stable IDs are required for

- Window positions and sizes
- `CollapsingHeader` open/closed state
- `ScrollArea` scroll offset
- `TextEdit` cursor position
- Drag-and-drop source/target tracking
- Popup and menu open state

---

## 8. Widget state across frames: Memory and IdTypeMap

### Architecture

```rust
// Access from Context
pub fn memory<R>(&self, reader: impl FnOnce(&Memory) -> R) -> R
pub fn memory_mut<R>(&self, writer: impl FnOnce(&mut Memory) -> R) -> R
```

`Memory` contains:
- Focus tracking: which `Id` has keyboard focus
- Popup state: which popup `Id` is open
- `data: IdTypeMap` — arbitrary typed widget state keyed by `(Id, TypeId)`

### Memory focus API

```rust
pub fn has_focus(&self, id: Id) -> bool
pub fn focused(&self) -> Option<Id>
pub fn request_focus(&mut self, id: Id)
pub fn surrender_focus(&mut self, id: Id)
pub fn close_popup(&mut self, popup_id: Id)
pub fn open_popup(&mut self, popup_id: Id)
pub fn is_popup_open(&self, popup_id: Id) -> bool
pub fn any_popup_open(&self) -> bool
```

### IdTypeMap: per-widget persistent state

Key is `(Id, TypeId)` — you can store multiple distinct types under one `Id`.

```rust
// Temporary (cleared on ctx.memory().clear() or between sessions without persistence)
pub fn insert_temp<T: 'static + Any + Clone + Send + Sync>(&mut self, id: Id, value: T)
pub fn get_temp<T: 'static + Clone>(&self, id: Id) -> Option<T>
pub fn get_temp_mut_or<T: 'static + Any + Clone + Send + Sync>(
    &mut self, id: Id, or_insert: T
) -> &mut T

// Persisted (serialized with the "persistence" feature flag)
pub fn insert_persisted<T: SerializableAny>(&mut self, id: Id, value: T)
pub fn get_persisted<T: SerializableAny>(&mut self, id: Id) -> Option<T>

// Removal
pub fn remove<T: 'static>(&mut self, id: Id)
```

**Performance note:** Each `get_temp` clones the value. Keep stored values small or wrap in `Arc<Mutex<…>>`.

**Singleton state** (not per-widget): use `Id::NULL` as key.

### Custom widget state pattern

```rust
// In your widget's allocate_and_paint function:
let id = ui.make_persistent_id("my_widget_unique_salt");

// Read previous frame's state
let mut state: MyState = ui.ctx().memory_mut(|mem| {
    mem.data.get_temp(id).unwrap_or_default()
});

// ... compute new state based on Response ...

// Write state for next frame
ui.ctx().memory_mut(|mem| {
    mem.data.insert_temp(id, state);
});
```

**IMPORTANT**: `Memory` (and `IdTypeMap`) is not meant for critical application data. It is transient UI state. Put your document model, undo stack, etc. in your own `App` struct.

---

## Flags / things to verify

- `run` is documented as deprecated in favor of `run_ui`. Double-check in 0.34.2 source that `run` still compiles (it should — deprecated != removed).
- `style()` / `set_style()` on `Context` are marked deprecated in favor of `global_style`/`set_global_style` methods — verify exact replacement method names in 0.34.2.
- `interact_with_hovered` on `Ui` is documented as deprecated; `interact` is the replacement.
- `available_rect` / `used_rect` on `Context` are deprecated — verify the replacement (likely queried from `FullOutput` or via `Ui` methods).
- `wants_keyboard_input`, `wants_pointer_input`, `is_using_pointer` on `Context` are deprecated — replacement names add `egui_` prefix (e.g., `egui_wants_keyboard_input`). Confirm these exist in 0.34.2.
- `IdTypeMap` struct location: `egui::util::id_type_map::IdTypeMap` — may be re-exported at crate root.
- `get_temp_mut_or` signature confirmed; check if `get_temp_mut` without `or_insert` exists.
