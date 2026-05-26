# egui Widgets — Research Digest

**egui version: 0.34.2** (verified from docs.rs, 2026-05-25)

---

## 1. Widget trait and `ui.add` family

```rust
pub trait Widget {
    fn ui(self, ui: &mut Ui) -> Response;
}
```

All widgets implement this trait. The standard placement methods:

```rust
// Place any Widget implementor
ui.add(widget) -> Response

// Place with an exact bounding box — widget receives that size
ui.add_sized(size: impl Into<Vec2>, widget: impl Widget) -> Response

// Greyed-out, non-interactive if enabled=false. Nested disabled stays disabled.
ui.add_enabled(enabled: bool, widget: impl Widget) -> Response

// Widget is invisible (no space allocated) if visible=false
ui.add_visible(visible: bool, widget: impl Widget) -> Response
```

Convenience shorthand methods on `Ui` (`ui.button("x")`, `ui.label("x")`, etc.) are
thin wrappers over `ui.add(WidgetType::new(...))`.

---

## 2. `Response` — reading interactions

Every `ui.add(...)` call and every convenience method returns `Response`.

### Key predicate methods

```rust
pub fn clicked(&self) -> bool          // primary (left) click, released this frame
pub fn secondary_clicked(&self) -> bool
pub fn double_clicked(&self) -> bool
pub fn hovered(&self) -> bool          // false when another widget is being dragged, false for disabled
pub fn contains_pointer(&self) -> bool // pointer inside rect regardless of drag state
pub fn changed(&self) -> bool          // value was changed by user this frame (sliders, text edits, etc.)
pub fn lost_focus(&self) -> bool       // text field stopped being focused
pub fn gained_focus(&self) -> bool
pub fn has_focus(&self) -> bool
pub fn dragged(&self) -> bool
pub fn drag_started(&self) -> bool
pub fn drag_stopped(&self) -> bool
pub fn drag_delta(&self) -> Vec2       // delta this frame
pub fn total_drag_delta(&self) -> Option<Vec2>
pub fn interact_pointer_pos(&self) -> Option<Pos2>
pub fn hover_pos(&self) -> Option<Pos2>
pub fn is_pointer_button_down_on(&self) -> bool
pub fn enabled(&self) -> bool
```

### Mutation / decoration methods (consume or mutate self)

```rust
pub fn on_hover_text(self, text: impl Into<WidgetText>) -> Self
pub fn on_hover_ui(self, add_contents: impl FnOnce(&mut Ui)) -> Self
pub fn context_menu(&self, add_contents: impl FnOnce(&mut Ui)) -> Option<Response>
pub fn mark_changed(&mut self)         // manually signal changed; needed in custom widgets
pub fn highlight(mut self) -> Self     // draw a highlight rectangle around the widget this frame
pub fn request_focus(&self)
pub fn surrender_focus(&self)
pub fn labelled_by(self, id: Id) -> Self
pub fn interact(&self, sense: Sense) -> Self  // extend sense retroactively
```

### The `.changed()` pattern (immediate-mode idiomatic)

```rust
let mut value: f32 = 0.5;
if ui.add(egui::Slider::new(&mut value, 0.0..=1.0)).changed() {
    apply_value(value);
}
```

---

## 3. Text and display widgets

### `Label`

```rust
// Via ui convenience (most common)
pub fn label(&mut self, text: impl Into<WidgetText>) -> Response

// Direct construction — adds Sense::click if needed
let resp = ui.add(egui::Label::new("text").sense(egui::Sense::click()));
```

Additional `Ui` text shorthands (all return `Response`):

```rust
pub fn colored_label(&mut self, color: impl Into<Color32>, text: impl Into<RichText>) -> Response
pub fn heading(&mut self, text: impl Into<RichText>) -> Response
pub fn monospace(&mut self, text: impl Into<RichText>) -> Response
pub fn code(&mut self, text: impl Into<RichText>) -> Response
pub fn small(&mut self, text: impl Into<RichText>) -> Response
pub fn strong(&mut self, text: impl Into<RichText>) -> Response
pub fn weak(&mut self, text: impl Into<RichText>) -> Response
```

### `RichText` — styled text

`RichText` implements `Into<WidgetText>`. Builder-style, all methods return `Self`.

```rust
egui::RichText::new("hello")
    .color(Color32::RED)
    .background_color(Color32::YELLOW)
    .size(18.0)
    .strong()           // bolder / brighter
    .weak()             // fainter
    .underline()
    .strikethrough()
    .italics()
    .monospace()        // fixed-width font
    .code()             // monospace + gray bg
    .small()            // smaller font
    .heading()          // heading style
    .extra_letter_spacing(2.0)
    .line_height(Some(24.0))

// Usage:
ui.label(egui::RichText::new("Red bold text").color(Color32::RED).strong());
```

### `Hyperlink`

```rust
// URL as label
pub fn new(url: impl ToString) -> Self
// Custom label
pub fn from_label_and_url(label: impl Into<WidgetText>, url: impl ToString) -> Self
// Open in new browser tab (web targets)
pub fn open_in_new_tab(mut self, new_tab: bool) -> Self

// Convenience on Ui:
pub fn hyperlink(&mut self, url: impl ToString) -> Response
pub fn hyperlink_to(&mut self, label: impl Into<WidgetText>, url: impl ToString) -> Response
```

Example:
```rust
ui.hyperlink_to("egui on GitHub", "https://www.github.com/emilk/egui/");
// or:
ui.add(egui::Hyperlink::from_label_and_url("egui", "https://github.com/emilk/egui")
    .open_in_new_tab(true));
```

### `Separator`

```rust
ui.separator(); // horizontal rule spanning available width (vertical in horizontal layouts)
// Direct: ui.add(egui::Separator::default());
```

### `Spinner`

```rust
pub struct Spinner { size: Option<f32>, color: Option<Color32> }

impl Spinner {
    pub fn new() -> Self
    pub fn size(mut self, size: f32) -> Self         // square; defaults to style interact_size
    pub fn color(mut self, color: impl Into<Color32>) -> Self
    pub fn paint_at(&self, ui: &Ui, rect: Rect)     // paint without allocating space
}
// impl Widget for Spinner

ui.add(egui::Spinner::new().size(24.0).color(Color32::WHITE));
```

### `ProgressBar`

```rust
impl ProgressBar {
    pub fn new(progress: f32) -> Self               // progress in [0.0, 1.0]
    pub fn desired_width(self, desired_width: f32) -> Self   // fills horizontal space if not set
    pub fn desired_height(self, desired_height: f32) -> Self // defaults to interact_size
    pub fn fill(self, color: Color32) -> Self
    pub fn text(self, text: impl Into<WidgetText>) -> Self   // overlay text
    pub fn show_percentage(self) -> Self                      // "42%"
    pub fn animate(self, animate: bool) -> Self               // loading animation when < 1.0
    pub fn corner_radius(self, corner_radius: impl Into<CornerRadius>) -> Self
    // rounding() is deprecated alias for corner_radius()
}
// impl Widget for ProgressBar

ui.add(egui::ProgressBar::new(0.7).show_percentage().animate(true));
```

---

## 4. Buttons and toggles

### `Button`

All builder methods return `Self`. Constructor:

```rust
pub fn new(atoms: impl IntoAtoms<'a>) -> Self
pub fn selectable(selected: bool, atoms: impl IntoAtoms<'a>) -> Self   // highlighted if selected
pub fn image(image: impl Into<Image<'a>>) -> Self                      // icon-only button
pub fn image_and_text(image: impl Into<Image<'a>>, text: impl Into<WidgetText>) -> Self
pub fn opt_image_and_text(image: Option<Image<'a>>, text: Option<WidgetText>) -> Self
```

Builder options:

```rust
.fill(impl Into<Color32>)              // override background; enables frame
.stroke(impl Into<Stroke>)
.frame(bool)                           // draw outer frame
.frame_when_inactive(bool)             // frame only on hover/active
.small()                               // compact, inline-friendly
.wrap()                                // text wrapping
.truncate()                            // truncate overflowing text
.min_size(Vec2)
.corner_radius(impl Into<CornerRadius>)
.selected(bool)                        // highlight as selected (toolbar toggle pattern)
.shortcut_text(impl IntoAtoms<'a>)    // e.g. "Ctrl+Z" shown right-aligned, weak color
.left_text(impl IntoAtoms<'a>)        // extra text left of main label
.right_text(impl IntoAtoms<'a>)       // extra text right of main label
.sense(Sense)                          // override sense (e.g. add drag)
.gap(f32)                              // spacing between image and text
.image_tint_follows_text_color(bool)
```

Usage:
```rust
// Basic click
if ui.button("Save").clicked() { save(); }

// Toolbar toggle (tool selection)
let active = current_tool == Tool::Pencil;
if ui.add(egui::Button::new("Pencil").selected(active)).clicked() {
    current_tool = Tool::Pencil;
}

// Disabled
ui.add_enabled(false, egui::Button::new("Unavailable"));

// Small inline
ui.add(egui::Button::new("x").small().frame(false));
```

**Note:** `ImageButton` is **deprecated** as of egui 0.28+. Use `Button::image(...)` or
`Button::image_and_text(...)` instead.

### `Checkbox`

```rust
impl<'a> Checkbox<'a> {
    pub fn new(checked: &'a mut bool, atoms: impl IntoAtoms<'a>) -> Self
    pub fn without_text(checked: &'a mut bool) -> Self
    pub fn indeterminate(self, indeterminate: bool) -> Self  // visual only; still toggles on click
}
// impl Widget for Checkbox

ui.checkbox(&mut my_bool, "Enable feature");
// or:
if ui.add(egui::Checkbox::new(&mut val, "Label")).changed() { ... }
```

### `RadioButton`

```rust
impl<'a> RadioButton<'a> {
    pub fn new(checked: bool, atoms: impl IntoAtoms<'a>) -> Self
}
// impl Widget for RadioButton

// Preferred convenience form:
ui.radio_value(&mut my_enum, Enum::First, "First");

// Equivalent manual form:
if ui.add(egui::RadioButton::new(my_enum == Enum::First, "First")).clicked() {
    my_enum = Enum::First;
}
```

### `ui.selectable_value` / `SelectableLabel`

`SelectableLabel` is **deprecated** (still compiles in 0.34.2 but prefer alternatives).
Use `ui.selectable_value` or `Button::selectable`.

```rust
// Ui convenience — most common
pub fn selectable_value<Value: PartialEq>(
    &mut self,
    current: &mut Value,
    value: Value,
    text: impl Into<WidgetText>,
) -> Response

// Usage (e.g., inside ComboBox or a list):
ui.selectable_value(&mut selected_blend, BlendMode::Normal, "Normal");
ui.selectable_value(&mut selected_blend, BlendMode::Multiply, "Multiply");
```

### `ui.toggle_value`

```rust
pub fn toggle_value<'a>(&mut self, selected: &mut bool, atoms: impl IntoAtoms<'a>) -> Response
// Looks like Button::selectable, acts like a checkbox. Clicks toggle the bool.

let mut show_grid = false;
ui.toggle_value(&mut show_grid, "Grid");
```

---

## 5. Numeric input

### `Slider`

```rust
pub fn new(value: &'a mut Num, range: RangeInclusive<Num>) -> Self
// Num: emath::Numeric (f32, f64, i32, usize, …)
```

Builder:
```rust
.step_by(step: f64)                    // minimum change per drag tick
.logarithmic(bool)                     // logarithmic scale for huge ranges
.clamping(SliderClamping)              // Always (default) | Edits | Never
.suffix(impl ToString)                 // unit suffix shown after value
.prefix(impl ToString)                 // prefix before value
.text(impl Into<WidgetText>)           // label beside slider
.show_value(bool)                      // default true
.trailing_fill(bool)                   // color behind handle from left edge
.handle_shape(HandleShape)             // shape of the drag handle
.orientation(SliderOrientation)        // Horizontal (default) | Vertical
.custom_formatter(|n: f64, _decimals: RangeInclusive<usize>| -> String { ... })
.custom_parser(|s: &str| -> Option<f64> { ... })
```

`SliderClamping` enum: `Always`, `Edits`, `Never`.

```rust
let mut opacity: f32 = 1.0;
if ui.add(
    egui::Slider::new(&mut opacity, 0.0..=1.0)
        .text("Opacity")
        .suffix("%")
        .custom_formatter(|n, _| format!("{:.0}%", n * 100.0))
        .custom_parser(|s| s.trim_end_matches('%').parse::<f64>().ok().map(|v| v / 100.0))
).changed() {
    layer.set_opacity(opacity);
}

// Integer step slider (frame count):
let mut frame: i32 = 0;
ui.add(egui::Slider::new(&mut frame, 0..=timeline_len).step_by(1.0).text("Frame"));
```

### `DragValue`

Compact numeric: drag horizontally or click-to-type.

```rust
pub fn new(value: &'a mut Num) -> Self
```

Builder:
```rust
.speed(impl Into<f64>)                 // change per logical pixel dragged; must be > 0
.range(RangeInclusive<Num>)            // valid bounds
.clamp_existing_to_range(bool)         // default true; clamps even without interaction
.min_decimals(usize)
.max_decimals(usize)
.fixed_decimals(usize)                 // exact decimal places (sets min and max)
.prefix(impl IntoAtoms<'a>)            // e.g. "x: "
.suffix(impl IntoAtoms<'a>)            // e.g. " px"
.update_while_editing(bool)            // default true; false = commit only on Enter/blur
.custom_formatter(impl Fn(f64, RangeInclusive<usize>) -> String)
.custom_parser(impl Fn(&str) -> Option<f64>)
```

```rust
// Pixel-art editor: position inputs side by side
ui.horizontal(|ui| {
    ui.label("Size:");
    ui.add(egui::DragValue::new(&mut width).speed(1).range(1..=8192).suffix(" px"));
    ui.label("×");
    ui.add(egui::DragValue::new(&mut height).speed(1).range(1..=8192).suffix(" px"));
});

// Angle with degree symbol
ui.add(
    egui::DragValue::new(&mut angle_deg)
        .speed(0.5)
        .range(-360.0..=360.0)
        .custom_formatter(|n, _| format!("{:.1}°", n))
        .custom_parser(|s| s.trim_end_matches('°').parse().ok()),
);
```

---

## 6. Text input — `TextEdit`

`TextEdit` takes a mutable reference to any type that implements `TextBuffer`
(blanket-implemented for `String`).

```rust
pub fn singleline(text: &'t mut dyn TextBuffer) -> Self
pub fn multiline(text: &'t mut dyn TextBuffer) -> Self
```

Builder:
```rust
.desired_width(f32)          // 0.0 = shrink to content; f32::INFINITY = fill available
.hint_text(impl IntoAtoms<'static>)    // shown when empty
.password(bool)              // mask characters, disable copy
.code_editor(self) -> Self   // convenience: monospace + lock_focus(true)
.lock_focus(bool)            // true: Tab inserts '\t' instead of moving focus
.interactive(bool)           // false: read-only selectable text
.font(impl Into<FontSelection>)
.clip_text(bool)             // singleline: clip overflow vs. expand widget
.return_key(impl Into<Option<KeyboardShortcut>>)  // override Enter behavior
```

```rust
// Layer name editor
let resp = ui.add(
    egui::TextEdit::singleline(&mut layer.name)
        .desired_width(120.0)
        .hint_text("Layer name"),
);
if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
    commit_rename(&layer.name);
}
if resp.changed() { /* live update */ }

// Lua script editor
ui.add(
    egui::TextEdit::multiline(&mut script_source)
        .code_editor()
        .desired_width(f32::INFINITY),
);

// Password
ui.add(egui::TextEdit::singleline(&mut password).password(true));

// Read-only display
ui.add(egui::TextEdit::multiline(&mut display_text.to_string()).interactive(false));
```

---

## 7. Selection — `ComboBox`

```rust
pub fn from_label(label: impl Into<WidgetText>) -> Self    // ID derived from label text
pub fn from_id_salt(id_salt: impl AsIdSalt) -> Self        // no visible label; explicit ID
```

Builder:
```rust
.selected_text(impl Into<WidgetText>)  // text shown on the closed button
.width(f32)                            // minimum overall width
.wrap()                                // enable text wrapping in dropdown
// height(), icon(), wrap_mode(), close_behavior() also available
```

Display methods:
```rust
pub fn show_ui<R>(
    self,
    ui: &mut Ui,
    menu_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>>
// Returns InnerResponse { inner: None } when closed

pub fn show_index<Text: Into<WidgetText>>(
    self,
    ui: &mut Ui,
    selected: &mut usize,
    len: usize,
    get: impl Fn(usize) -> Text,
) -> Response
```

```rust
#[derive(Debug, PartialEq, Clone, Copy)]
enum BlendMode { Normal, Multiply, Screen, Overlay }

let mut blend = BlendMode::Normal;

egui::ComboBox::from_label("Blend")
    .selected_text(format!("{:?}", blend))
    .show_ui(ui, |ui| {
        ui.selectable_value(&mut blend, BlendMode::Normal,   "Normal");
        ui.selectable_value(&mut blend, BlendMode::Multiply, "Multiply");
        ui.selectable_value(&mut blend, BlendMode::Screen,   "Screen");
        ui.selectable_value(&mut blend, BlendMode::Overlay,  "Overlay");
    });

// Index-based (when items are a Vec):
egui::ComboBox::from_id_salt("palette_selector")
    .selected_text(&palettes[selected_idx].name)
    .show_index(ui, &mut selected_idx, palettes.len(), |i| &palettes[i].name);
```

---

## 8. Tooltips, context menus, and `menu_button`

### Tooltips

```rust
// Simple text tooltip (chained onto any Response)
pub fn on_hover_text(self, text: impl Into<WidgetText>) -> Self

// Custom UI tooltip — can contain any widgets
pub fn on_hover_ui(self, add_contents: impl FnOnce(&mut Ui)) -> Self

// Tooltip shown only when widget is disabled (works opposite to on_hover_text)
pub fn on_disabled_hover_text(self, text: impl Into<WidgetText>) -> Self
```

```rust
ui.button("Merge Down")
    .on_hover_text("Merge this layer into the one below (Ctrl+E)")
    .on_hover_ui(|ui| {
        ui.image(egui::include_image!("../assets/merge-preview.png"));
    });
```

Multiple tooltip calls stack (they all show). Tooltip popups are interactive; the pointer
can move over them without dismissing.

### Context menus (right-click)

```rust
pub fn context_menu(&self, add_contents: impl FnOnce(&mut Ui)) -> Option<Response>
// Returns Some(response) if the menu was shown this frame.
```

```rust
let resp = ui.label("Layer 0");
resp.context_menu(|ui| {
    if ui.button("Rename").clicked() { start_rename(); ui.close(); }
    if ui.button("Delete").clicked() { delete_layer(); ui.close(); }
    ui.separator();
    ui.menu_button("Move to", |ui| {
        if ui.button("Top").clicked() { move_layer_top(); ui.close(); }
    });
});
```

Use `ui.close()` (0.34.x) to dismiss the menu from within the closure.
Note: older docs show `ui.close_menu()` — this was renamed to `ui.close()` in 0.29+.
**Verify** which form is current for 0.34.2; the source uses `ui.close()`.

### `ui.menu_button`

```rust
pub fn menu_button<'a, R>(
    &mut self,
    atoms: impl IntoAtoms<'a>,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>>

// With image:
pub fn menu_image_button<'a, R>(
    &mut self,
    image: impl Into<Image<'a>>,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>>

// With image + text:
pub fn menu_image_text_button<'a, R>(...)
```

When called from within a menu context, `menu_button` creates a submenu rather than a
top-level popup. Used for building menu bars:

```rust
egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
    ui.horizontal(|ui| {
        ui.menu_button("File", |ui| {
            if ui.button("New…").clicked() { new_file(); ui.close(); }
            if ui.button("Open…").clicked() { open_file(); ui.close(); }
            ui.separator();
            if ui.button("Quit").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
        });
        ui.menu_button("Edit", |ui| {
            if ui.button("Undo  Ctrl+Z").clicked() { undo(); ui.close(); }
        });
    });
});
```

---

## 9. Enable/disable and `ui.scope`

### `ui.add_enabled` (single widget)

```rust
pub fn add_enabled(&mut self, enabled: bool, widget: impl Widget) -> Response
// Widget appears greyed-out and non-interactive when enabled=false.
// Nested: if already inside a disabled Ui, widget stays disabled even if enabled=true.
```

### `ui.add_enabled_ui` (whole region)

```rust
pub fn add_enabled_ui<R>(
    &mut self,
    enabled: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R>
```

```rust
let mut editing = false;
ui.checkbox(&mut editing, "Edit mode");
ui.add_enabled_ui(editing, |ui| {
    ui.add(egui::TextEdit::singleline(&mut layer.name));
    if ui.button("Apply").clicked() { commit(); }
});
```

### `ui.disable()` (one-shot, irreversible for that Ui)

```rust
// Greys out the Ui and all children. Cannot be re-enabled on the same Ui.
// Prefer add_enabled_ui for reversible control.
ui.group(|ui| {
    if !some_condition { ui.disable(); }
    ui.button("Affected");
});
```

### `ui.scope`

```rust
pub fn scope<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
// Creates an isolated child Ui. Changes to style/layout inside don't leak out.
// Used to temporarily override spacing, style, or layout direction.
```

```rust
ui.scope(|ui| {
    ui.style_mut().visuals.override_text_color = Some(Color32::GOLD);
    ui.spacing_mut().item_spacing.y = 2.0;
    for item in &items {
        ui.label(item);
    }
});
```

---

## 10. Custom widgets — the allocate/sense/paint pattern

### The `Widget` trait approach (reusable struct)

Implement `Widget` on a struct to get `ui.add(MyWidget::new(...))` syntax.

```rust
/// A clickable color swatch — realistic pixel-art palette use case.
pub struct ColorSwatch {
    color: egui::Color32,
    selected: bool,
    size: f32,
}

impl ColorSwatch {
    pub fn new(color: egui::Color32, selected: bool) -> Self {
        Self { color, selected, size: 16.0 }
    }
    pub fn size(mut self, size: f32) -> Self { self.size = size; self }
}

impl egui::Widget for ColorSwatch {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let size = egui::Vec2::splat(self.size);
        // 1. Allocate space and register interaction sense.
        let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());

        // 2. Optional: emit accessibility info.
        response.widget_info(|| egui::WidgetInfo::new(egui::WidgetType::Button));

        // 3. Paint — only if rect is visible (respects clipping).
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            let painter = ui.painter();

            // Fill with color
            painter.rect_filled(rect.shrink(1.0), 2.0, self.color);

            // Selection ring
            if self.selected {
                painter.rect_stroke(
                    rect,
                    2.0,
                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                    egui::StrokeKind::Inside,
                );
            } else {
                // Hover/active border from style
                painter.rect_stroke(
                    rect,
                    2.0,
                    visuals.bg_stroke,
                    egui::StrokeKind::Inside,
                );
            }
        }

        response
    }
}

// Usage:
if ui.add(ColorSwatch::new(palette[i], i == selected_idx)).clicked() {
    selected_idx = i;
}
```

### Free-function widget pattern (simpler, non-reusable outside module)

```rust
fn tool_toggle(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    ui.add(egui::Button::new(label).selected(active).min_size(egui::vec2(32.0, 32.0)))
}
```

### Key `Ui` allocation methods

```rust
// Allocate space and sense interaction (returns rect + response)
pub fn allocate_response(&mut self, desired_size: Vec2, sense: Sense) -> Response

// Same, but widget is forced to exactly this size (no layout stretch)
pub fn allocate_exact_size(&mut self, desired_size: Vec2, sense: Sense) -> (Rect, Response)

// Allocate space only (no sense) — use when painting manually without interaction
pub fn allocate_space(&mut self, desired_size: Vec2) -> (Id, Rect)

// Access the painter for the current Ui clip rect
pub fn painter(&self) -> &Painter
// Access painter for an arbitrary rect (e.g., draw outside allocated area)
pub fn painter_at(&self, rect: Rect) -> Painter
```

### `Painter` drawing methods (most-used)

```rust
painter.rect_filled(rect: Rect, corner_radius: impl Into<CornerRadius>, fill: impl Into<Color32>) -> ShapeIdx
painter.rect_stroke(rect, corner_radius, stroke: impl Into<Stroke>, kind: StrokeKind) -> ShapeIdx
painter.rect(rect, corner_radius, fill, stroke, stroke_kind) -> ShapeIdx
painter.circle_filled(center: Pos2, radius: f32, fill: impl Into<Color32>) -> ShapeIdx
painter.circle_stroke(center, radius, stroke) -> ShapeIdx
painter.line_segment(points: [Pos2; 2], stroke: impl Into<Stroke>) -> ShapeIdx
painter.text(pos: Pos2, anchor: Align2, text: impl ToString, font: FontId, color: Color32) -> Rect
painter.galley(pos: Pos2, galley: Arc<Galley>, color: Color32)
painter.add(shape: impl Into<Shape>) -> ShapeIdx  // arbitrary shape
```

`StrokeKind`: `Inside`, `Outside`, `Middle` — controls where the stroke is drawn relative
to the rect edge.

### `Sense` flags

```rust
Sense::hover()          // hover detection only
Sense::click()          // click + hover
Sense::drag()           // drag + hover
Sense::click_and_drag() // both
Sense::NONE             // allocate space, no interaction
```

### `is_rect_visible` guard

Always wrap paint calls in `ui.is_rect_visible(rect)` to skip off-screen widgets — egui's
layout still allocates space even when clipped, but painting is expensive.

---

## 11. Known pitfalls and idioms

### ID collisions in loops

Every widget needs a unique ID. In loops, use `.push_id` or `Id::new`:

```rust
for (i, layer) in layers.iter_mut().enumerate() {
    // Bad: all rows share the same ID
    ui.label(&layer.name);

    // Good: scope gives each row its own ID space
    ui.push_id(i, |ui| {
        ui.label(&layer.name);
        if ui.button("x").clicked() { to_delete = Some(i); }
    });
}
```

### Mutating via `Response` vs. direct mutation

In immediate mode, always read `.changed()` or `.clicked()` on the returned `Response`
and apply side effects there:

```rust
// Correct: check Response
let r = ui.add(egui::Slider::new(&mut val, 0..=255));
if r.changed() { update_color(val); }

// Incorrect: val was mutated but side effect was skipped
ui.add(egui::Slider::new(&mut val, 0..=255));
update_color(val); // runs every frame, even when val didn't change
```

### `mark_changed()` in custom widgets

When building custom composite widgets that wrap primitive state changes, call
`response.mark_changed()` so callers can use `.changed()`:

```rust
if inner_button.clicked() {
    *value = new_value;
    response.mark_changed();
}
```

### `SelectableLabel` deprecation

`egui::SelectableLabel` and `ui.selectable_label` are still present in 0.34.2 but marked
deprecated. Use `ui.selectable_value` or `Button::selectable` instead.

### `ImageButton` deprecation

`egui::ImageButton` is deprecated. Use `egui::Button::image(...)` instead:
```rust
// Old (deprecated):
ui.add(egui::ImageButton::new(texture_id, [32.0, 32.0]));
// New:
ui.add(egui::Button::image(egui::Image::new(texture_id).fit_to_exact_size(vec2(32.0, 32.0))));
```

### `ui.close()` vs `ui.close_menu()`

In egui 0.29+, `ui.close_menu()` was renamed to `ui.close()`. Use `ui.close()` in
0.34.2. [UNCERTAIN — verify in source if close_menu still compiles as alias]

### TextEdit and `TextBuffer` trait

`TextEdit` accepts `&mut dyn TextBuffer`. `String` implements `TextBuffer`. To use a
fixed-capacity buffer, implement `TextBuffer` on a newtype (not provided by egui).

### `add_sized` for fixed-width controls

```rust
// Force a TextEdit to fill a specific rect
ui.add_sized([120.0, 20.0], egui::TextEdit::singleline(&mut name));
```

---

## Sources

- `docs.rs/egui/0.34.2` widget module index (verified version: **0.34.2**)
- `github.com/emilk/egui` source: `widgets/button.rs`, `widgets/color_picker.rs`,
  `containers/combo_box.rs`, `response.rs`
- Context7 `/websites/rs_egui` (43 K snippets, High reputation)
- Context7 `/emilk/egui` (123 snippets, High reputation)
