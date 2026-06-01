# egui Layout, Containers, and Panels

**egui version: 0.34.2** (released March 2026)

> **BREAKING**: egui 0.34.0 overhauled the panel API. `SidePanel` and `TopBottomPanel` are **deprecated** in favor of a unified `Panel` struct. `CentralPanel::show(ctx, ...)` is renamed to `CentralPanel::show(ui, ...)` — `show_inside` is now the deprecated alias. `eframe::App::update` is deprecated in favor of `App::ui(ui, frame)`. All panel display methods now take `&mut Ui` not `&Context`.

---

## 1. Panels

### Panel (unified, replaces SidePanel + TopBottomPanel)

```rust
// Constructors
pub fn left(id: impl Into<Id>) -> Self
pub fn right(id: impl Into<Id>) -> Self
pub fn top(id: impl Into<Id>) -> Self       // not resizable by default
pub fn bottom(id: impl Into<Id>) -> Self    // not resizable by default

// Configuration (all builder pattern, return Self)
pub fn resizable(mut self, resizable: bool) -> Self
pub fn show_separator_line(mut self, show_separator_line: bool) -> Self
pub fn default_size(mut self, default_size: f32) -> Self  // includes frame margins
pub fn min_size(mut self, min_size: f32) -> Self
pub fn max_size(mut self, max_size: f32) -> Self
pub fn size_range(mut self, size_range: impl Into<Rangef>) -> Self
pub fn exact_size(mut self, size: f32) -> Self            // locks, disables resize
pub fn frame(mut self, frame: Frame) -> Self

// Display
pub fn show<R>(
    self,
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R>

// Animated collapse: slides toward fixed edge. Returns None when fully collapsed.
pub fn show_collapsible<R>(
    self,
    ui: &mut Ui,
    is_expanded: &mut bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<R>>

// Animate between two panel configs. bool arg to add_contents = is_expanded.
pub fn show_switched<R>(
    ui: &mut Ui,
    is_expanded: &mut bool,
    collapsed_panel: Self,
    expanded_panel: Self,
    add_contents: impl FnOnce(&mut Ui, bool) -> R,
) -> InnerResponse<R>
```

**size_* methods operate on the panel's own dimension:**
- left/right panels: controls width
- top/bottom panels: controls height
- All sizes include frame margins

**Resizability defaults:**
- `Panel::left` / `Panel::right`: resizable = true
- `Panel::top` / `Panel::bottom`: resizable = false

### Panel ordering rules

The order panels are added to the `Ui` determines nesting:

- First added = outermost (most space claimed)
- Last added = innermost (least space remaining)
- `CentralPanel` **must always be added last** — it fills whatever space remains
- Never open a top-level panel from inside another panel
- Windows and Areas render on top of CentralPanel

```rust
// Correct ordering in App::ui (eframe 0.34+):
fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    Panel::top("toolbar").show(ui, |ui| { /* toolbar */ });
    Panel::left("layers").default_size(200.0).show(ui, |ui| { /* layers */ });
    Panel::right("properties").show(ui, |ui| { /* props */ });
    Panel::bottom("timeline").show(ui, |ui| { /* timeline */ });
    CentralPanel::default().show(ui, |ui| { /* canvas */ });
}
```

**Wrong order** (panels added after CentralPanel get no space):

```rust
// BUG: CentralPanel claims all space first, then left panel gets nothing
CentralPanel::default().show(ui, |ui| { });
Panel::left("layers").show(ui, |ui| { }); // invisible / zero-sized
```

### CentralPanel

```rust
// Constructors
pub fn default() -> Self          // via Default impl; includes standard frame
pub fn no_frame() -> Self         // no background fill or margin
pub fn default_margins() -> Self  // standard margins, no custom fill

// Builder
pub fn frame(mut self, frame: Frame) -> Self

// Display — takes &mut Ui (NOT &Context since 0.34)
pub fn show<R>(
    self,
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R>

// Deprecated alias (was the name in 0.33)
#[deprecated = "Renamed to `show`"]
pub fn show_inside<R>(self, ui: &mut Ui, ...) -> InnerResponse<R>
```

---

## 2. Window

Floating, draggable, optionally resizable overlay. Always renders above panels.

```rust
// Constructor — title is used as Id salt; must be unique
pub fn new(title: impl IntoAtoms<'a>) -> Self

// Identity
pub fn id(mut self, id: Id) -> Self   // required if title is dynamic

// Visibility
pub fn open(mut self, open: &'a mut bool) -> Self  // adds close button; hides when false
pub fn enabled(mut self, enabled: bool) -> Self
pub fn interactable(mut self, interactable: bool) -> Self

// Positioning
pub fn default_pos(mut self, default_pos: impl Into<Pos2>) -> Self
pub fn current_pos(mut self, current_pos: impl Into<Pos2>) -> Self
pub fn fixed_pos(mut self, pos: impl Into<Pos2>) -> Self
pub fn anchor(mut self, align: Align2, offset: impl Into<Vec2>) -> Self
pub fn movable(mut self, movable: bool) -> Self

// Sizing
pub fn default_size(mut self, default_size: impl Into<Vec2>) -> Self
pub fn default_width(mut self, default_width: f32) -> Self
pub fn default_height(mut self, default_height: f32) -> Self
pub fn default_rect(self, rect: Rect) -> Self
pub fn fixed_size(mut self, size: impl Into<Vec2>) -> Self
pub fn fixed_rect(self, rect: Rect) -> Self
pub fn min_width(mut self, min_width: f32) -> Self
pub fn min_height(mut self, min_height: f32) -> Self
pub fn max_width(mut self, max_width: f32) -> Self
pub fn max_height(mut self, max_height: f32) -> Self
pub fn resizable(mut self, resizable: impl Into<Vec2b>) -> Self  // per-axis

// Appearance
pub fn frame(mut self, frame: Frame) -> Self
pub fn title_bar(mut self, title_bar: bool) -> Self
pub fn collapsible(mut self, collapsible: bool) -> Self
pub fn auto_sized(mut self) -> Self  // sized by contents; disables scroll and wrap

// Scrolling (disabled by default)
pub fn scroll(mut self, scroll: impl Into<Vec2b>) -> Self
pub fn hscroll(mut self, hscroll: bool) -> Self
pub fn vscroll(mut self, vscroll: bool) -> Self

// Display
// Returns None if open==false; Some(InnerResponse{inner:None}) if collapsed
pub fn show<R>(
    self,
    ctx: &Context,   // Window still uses &Context, not &mut Ui
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<InnerResponse<Option<R>>>
```

**`open()` pattern for closeable windows:**

```rust
let mut open = true;
egui::Window::new("My Tool")
    .open(&mut open)
    .show(ctx, |ui| { ui.label("content"); });
// open == false after user clicks X
```

**Note:** `Window::show` still takes `&Context`, not `&mut Ui`. This differs from panels.

---

## 3. Area

A floating, freely-positioned rectangle with no title bar.

```rust
// Constructor — id must be globally unique
pub fn new(id: Id) -> Self
pub fn id(mut self, id: Id) -> Self

// Positioning
pub fn default_pos(mut self, default_pos: impl Into<Pos2>) -> Self
pub fn fixed_pos(mut self, fixed_pos: impl Into<Pos2>) -> Self
pub fn current_pos(mut self, current_pos: impl Into<Pos2>) -> Self
pub fn anchor(mut self, align: Align2, offset: impl Into<Vec2>) -> Self
pub fn pivot(mut self, pivot: Align2) -> Self  // which corner is the anchor point
pub fn default_size(mut self, default_size: impl Into<Vec2>) -> Self

// Interaction
pub fn movable(mut self, movable: bool) -> Self
pub fn interactable(mut self, interactable: bool) -> Self  // false = clicks pass through
pub fn enabled(mut self, enabled: bool) -> Self
pub fn sense(mut self, sense: Sense) -> Self

// Rendering
pub fn order(mut self, order: Order) -> Self
pub fn constrain(mut self, constrain: bool) -> Self        // default true
pub fn constrain_to(mut self, constrain_rect: Rect) -> Self
pub fn fade_in(mut self, fade_in: bool) -> Self
pub fn layout(mut self, layout: Layout) -> Self

// Display — takes &Context like Window
pub fn show<R>(
    self,
    ctx: &Context,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R>
```

**Order enum** (z-layers, in ascending render order):

```rust
pub enum Order {
    Background,   // behind all floating windows
    Middle,       // normal moveable windows (default)
    Foreground,   // popups, menus — always above windows
    Tooltip,      // tooltips — no interaction
    Debug,        // always on top; debug overlays
}
```

Use `Area` for custom palette popups, tool options overlays, HUD elements.

---

## 4. Frame

Decorates any UI region with background, stroke, margin, shadow, rounding. Not a standalone container — wraps content via `show()`.

```rust
// Frame fields (inside-out order):
pub struct Frame {
    pub inner_margin: Margin,    // padding inside the painted rect
    pub fill: Color32,           // background fill
    pub stroke: Stroke,          // border line
    pub corner_radius: CornerRadius,
    pub outer_margin: Margin,    // margin outside the painted rect
    pub shadow: Shadow,
}

// Constructors
pub const NONE: Frame;                        // all zeros
pub const fn new() -> Self;                   // same as NONE
pub fn group(style: &Style) -> Self;          // grouped widgets, rounded + stroke
pub fn side_top_panel(style: &Style) -> Self; // matches Panel frame
pub fn central_panel(style: &Style) -> Self;  // 8px inner margin
pub fn window(style: &Style) -> Self;         // window with shadow
pub fn menu(style: &Style) -> Self;
pub fn popup(style: &Style) -> Self;
pub fn canvas(style: &Style) -> Self;         // extreme background color
pub fn dark_canvas(style: &Style) -> Self;

// Builders
pub fn inner_margin(self, inner_margin: impl Into<Margin>) -> Self
pub fn fill(self, fill: Color32) -> Self
pub fn stroke(self, stroke: impl Into<Stroke>) -> Self
pub fn corner_radius(self, corner_radius: impl Into<CornerRadius>) -> Self
pub fn outer_margin(self, outer_margin: impl Into<Margin>) -> Self
pub fn shadow(self, shadow: Shadow) -> Self
pub fn multiply_with_opacity(self, opacity: f32) -> Self

// Display
pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>

// Low-level: start frame, get Prepared, paint manually
pub fn begin(self, ui: &mut Ui) -> Prepared
// Prepared has: frame, where_to_put_background: ShapeIdx, content_ui: Ui
// Call prepared.end(ui) to paint; prepared.paint(ui) for immediate paint
```

**Usage pattern for custom filled panels:**

```rust
Frame::new()
    .fill(Color32::from_rgb(30, 30, 35))
    .inner_margin(8.0)
    .corner_radius(4.0)
    .show(ui, |ui| {
        ui.label("content inside box");
    });
```

---

## 5. Resize

A user-resizable area; wraps a child UI that can be dragged to resize.

```rust
impl Default for Resize { fn default() -> Self }

pub fn id_salt(mut self, id_salt: impl AsIdSalt) -> Self
pub fn default_size(mut self, default_size: impl Into<Vec2>) -> Self
pub fn min_size(mut self, min_size: impl Into<Vec2>) -> Self
pub fn max_size(mut self, max_size: impl Into<Vec2>) -> Self
pub fn fixed_size(mut self, size: impl Into<Vec2>) -> Self  // prevents resize
pub fn auto_sized(self) -> Self   // sized by content; no text wrapping
pub fn resizable(mut self, resizable: impl Into<Vec2b>) -> Self  // per-axis

// Returns R (not InnerResponse)
#[must_use = "You should call .show()"]
pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R
```

**Caveats:**
- State persists across frames; first frame may size incorrectly
- `auto_sized()` prevents text wrapping; content drives size
- Use `Resize` for custom resizable panels within a layout; use `Panel::resizable` for docked panels

---

## 6. ScrollArea

### Constructors

```rust
pub fn vertical() -> Self
pub fn horizontal() -> Self
pub fn both() -> Self
pub fn neither() -> Self
pub fn new(direction_enabled: impl Into<Vec2b>) -> Self
```

### Builder methods

```rust
pub fn max_width(mut self, max_width: f32) -> Self
pub fn max_height(mut self, max_height: f32) -> Self
pub fn min_scrolled_width(mut self, min_scrolled_width: f32) -> Self
pub fn min_scrolled_height(mut self, min_scrolled_height: f32) -> Self
pub fn auto_shrink(mut self, auto_shrink: impl Into<Vec2b>) -> Self
    // true (default) = shrink to content; false = fill available space
pub fn scroll_bar_visibility(mut self, v: ScrollBarVisibility) -> Self
    // AlwaysHidden | VisibleWhenNeeded (default) | AlwaysVisible
pub fn id_salt(mut self, id_salt: impl AsIdSalt) -> Self
pub fn scroll_offset(mut self, offset: Vec2) -> Self
pub fn vertical_scroll_offset(mut self, offset: f32) -> Self
pub fn horizontal_scroll_offset(mut self, offset: f32) -> Self
pub fn stick_to_bottom(mut self, stick: bool) -> Self
    // scroll position follows growing content (log viewers, terminals)
pub fn stick_to_right(mut self, stick: bool) -> Self
pub fn hscroll(mut self, hscroll: bool) -> Self
pub fn vscroll(mut self, vscroll: bool) -> Self
pub fn scroll(mut self, direction_enabled: impl Into<Vec2b>) -> Self
pub fn scroll_source(mut self, scroll_source: ScrollSource) -> Self
pub fn wheel_scroll_multiplier(mut self, multiplier: Vec2) -> Self
pub fn animated(mut self, animated: bool) -> Self
pub fn content_margin(mut self, margin: impl Into<Margin>) -> Self
pub fn scroll_bar_rect(mut self, scroll_bar_rect: Rect) -> Self
pub fn on_hover_cursor(mut self, cursor: CursorIcon) -> Self
pub fn on_drag_cursor(mut self, cursor: CursorIcon) -> Self
```

### Display methods

```rust
// Basic: renders all content even if offscreen
pub fn show<R>(
    self,
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> ScrollAreaOutput<R>

// Virtual rows: renders only visible rows — use for 100+ items
pub fn show_rows<R>(
    self,
    ui: &mut Ui,
    row_height_sans_spacing: f32,  // height of ONE row, NOT including item_spacing
    total_rows: usize,
    add_contents: impl FnOnce(&mut Ui, std::ops::Range<usize>) -> R,
) -> ScrollAreaOutput<R>

// Viewport: gives raw Rect of visible area — for fully custom rendering
pub fn show_viewport<R>(
    self,
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut Ui, Rect) -> R,  // Rect = visible portion
) -> ScrollAreaOutput<R>
```

### ScrollAreaOutput

```rust
pub struct ScrollAreaOutput<R> {
    pub inner: R,            // return value from add_contents closure
    pub id: Id,              // stable ID for this scroll area
    pub state: State,        // current scroll state (offset, etc.)
    pub content_size: Vec2,  // size of the inner content
    pub inner_rect: Rect,    // the visible rect of the scroll area
}
```

### Virtual scrolling for large lists (layers panel, timeline rows)

`show_rows` assumes **uniform row height**. It computes visible range from scroll offset, allocates dead space for invisible rows before and after, then calls your closure with only `Range<usize>`.

```rust
// Layers panel with 500+ layers — uniform height
let row_height = ui.text_style_height(&egui::TextStyle::Body);
egui::ScrollArea::vertical()
    .auto_shrink([false, false])  // fill panel, don't shrink to content
    .show_rows(ui, row_height, layers.len(), |ui, row_range| {
        for idx in row_range {
            let layer = &layers[idx];
            // Must use idx as unique ID part to avoid collisions
            ui.push_id(idx, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut layer.visible, "");
                    ui.label(&layer.name);
                });
            });
        }
    });
```

**Critical:** `row_height_sans_spacing` does not include `ui.spacing().item_spacing.y`. Get it via:

```rust
let row_height = ui.text_style_height(&egui::TextStyle::Body);
// or for interactive rows:
let row_height = ui.spacing().interact_size.y;
```

### Variable-height virtual scrolling with show_viewport

When rows have different heights (e.g., timeline with keyframe groups), use `show_viewport`:

```rust
egui::ScrollArea::vertical().show_viewport(ui, |ui, viewport| {
    // viewport.min.y = scroll offset; viewport.max.y = scroll offset + visible height
    let mut y = 0.0;
    for (idx, row) in timeline_rows.iter().enumerate() {
        let row_height = row.height();
        let row_rect = Rect::from_min_size(
            Pos2::new(0.0, y),
            Vec2::new(ui.available_width(), row_height),
        );
        // Only actually render visible rows
        if row_rect.max.y >= viewport.min.y && row_rect.min.y <= viewport.max.y {
            let response = ui.allocate_rect(row_rect, egui::Sense::click());
            if response.is_visible() {
                ui.painter().rect_filled(row_rect, 0.0, row.color());
            }
        }
        y += row_height + ui.spacing().item_spacing.y;
    }
    // Must allocate total size so scroll range is correct
    ui.allocate_space(Vec2::new(ui.available_width(), y));
});
```

**Pitfall:** With `show_viewport`, you are responsible for allocating the full content height (via `ui.allocate_space`). Failing to do this collapses the scroll range.

---

## 7. Grid

Tabular layout with auto-sizing columns. Column widths determined by widest cell.

```rust
pub fn new(id_salt: impl AsIdSalt) -> Self

pub fn num_columns(mut self, num_columns: usize) -> Self
    // last column expands to fill remaining width

pub fn min_col_width(mut self, min_col_width: f32) -> Self
pub fn min_row_height(mut self, min_row_height: f32) -> Self
pub fn max_col_width(mut self, max_col_width: f32) -> Self  // triggers wrap if exceeded
pub fn spacing(mut self, spacing: impl Into<Vec2>) -> Self  // [horizontal, vertical]

pub fn striped(self, striped: bool) -> Self
    // alternating row colors; uses Visuals::striped_row_color by default

pub fn with_row_color<F>(mut self, color_picker: F) -> Self
where
    F: Send + Sync + Fn(usize, &Style) -> Option<Color32> + 'static
    // row_index 0-based; return None to use default

pub fn start_row(mut self, start_row: usize) -> Self

pub fn show<R>(
    self,
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R>
```

**Row termination:** call `ui.end_row()` at the end of each row inside the Grid closure.

```rust
egui::Grid::new("color_settings")
    .num_columns(2)
    .spacing([12.0, 4.0])
    .striped(true)
    .show(ui, |ui| {
        ui.label("Foreground");
        ui.color_edit_button_srgba(&mut fg_color);
        ui.end_row();

        ui.label("Background");
        ui.color_edit_button_srgba(&mut bg_color);
        ui.end_row();
    });
```

**ID collision pitfall:** `Grid::new` takes `id_salt`. If two Grids share the same ID string, their column widths corrupt each other. Use distinct, stable ID strings.

---

## 8. CollapsingHeader

Expandable/collapsible section with an arrow indicator.

```rust
pub fn new(text: impl Into<WidgetText>) -> Self
pub fn default_open(mut self, open: bool) -> Self
pub fn open(mut self, open: Option<bool>) -> Self  // None = user-controlled
pub fn id_salt(mut self, id_salt: impl AsIdSalt) -> Self
pub fn enabled(mut self, enabled: bool) -> Self
pub fn show_background(mut self, show_background: bool) -> Self
pub fn icon(mut self, icon_fn: impl FnOnce(&mut Ui, f32, &Response) + 'static) -> Self

pub fn show<R>(
    self,
    ui: &mut Ui,
    add_body: impl FnOnce(&mut Ui) -> R,
) -> CollapsingResponse<R>

pub fn show_unindented<R>(
    self,
    ui: &mut Ui,
    add_body: impl FnOnce(&mut Ui) -> R,
) -> CollapsingResponse<R>
```

**Custom header via CollapsingState:**

```rust
// When you need non-text headers (e.g., a layer group row with icon + checkbox)
let id = ui.make_persistent_id("layer_group_0");
let mut state = egui::CollapsingState::load_with_default_open(ui.ctx(), id, true);

state
    .show_header(ui, |ui| {
        ui.checkbox(&mut group.visible, "");
        ui.label(&group.name);
        // any widgets here
    })
    .body(|ui| {
        // child layers rendered here, indented
        for layer in &group.layers {
            ui.label(&layer.name);
        }
    });
```

`CollapsingState` methods:

```rust
pub fn load(ctx: &Context, id: Id) -> Option<Self>
pub fn load_with_default_open(ctx: &Context, id: Id, default_open: bool) -> Self
pub fn store(&self, ctx: &Context)
pub fn remove(&self, ctx: &Context)
pub fn is_open(&self) -> bool
pub fn set_open(&mut self, open: bool)
pub fn toggle(&mut self, ui: &Ui)
pub fn openness(&self, ctx: &Context) -> f32  // 0.0=closed, 1.0=open (animated)
pub fn show_header<H>(mut self, ui: &mut Ui, add_header: impl FnOnce(&mut Ui) -> H) -> HeaderResponse<'_, H>
pub fn show_body_indented<R>(&mut self, header_response: &Response, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>>
pub fn show_body_unindented<R>(&mut self, ui: &mut Ui, add_body: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<R>>
pub fn show_toggle_button(&mut self, ui: &mut Ui, icon_fn: impl FnOnce(&mut Ui, f32, &Response) + 'static) -> Response
```

`HeaderResponse` chains:

```rust
pub fn body<BodyRet>(mut self, add_body: impl FnOnce(&mut Ui) -> BodyRet) 
    -> (Response, InnerResponse<HeaderRet>, Option<InnerResponse<BodyRet>>)
pub fn body_unindented<BodyRet>(...) -> (...)
pub fn is_open(&self) -> bool
pub fn set_open(&mut self, open: bool)
pub fn toggle(&mut self)
```

---

## 9. ComboBox

A dropdown that opens a popup with arbitrary UI content.

```rust
pub fn new(id_salt: impl AsIdSalt, label: impl Into<WidgetText>) -> Self
pub fn from_label(label: impl Into<WidgetText>) -> Self  // id derived from label
pub fn from_id_salt(id_salt: impl AsIdSalt) -> Self       // no label

pub fn width(mut self, width: f32) -> Self
pub fn height(mut self, height: f32) -> Self  // max popup height before scrolling
pub fn selected_text(mut self, selected_text: impl Into<WidgetText>) -> Self
pub fn wrap_mode(mut self, wrap_mode: TextWrapMode) -> Self
pub fn wrap(mut self) -> Self
pub fn truncate(mut self) -> Self
pub fn close_behavior(mut self, close_behavior: PopupCloseBehavior) -> Self
pub fn popup_style(mut self, popup_style: StyleModifier) -> Self
pub fn icon(mut self, icon_fn: impl FnOnce(...) + 'static) -> Self

// Arbitrary UI in popup
pub fn show_ui<R>(
    self,
    ui: &mut Ui,
    menu_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>>

// Convenience for indexed slice
pub fn show_index<Text: Into<WidgetText>>(
    self,
    ui: &mut Ui,
    selected: &mut usize,
    len: usize,
    get: impl Fn(usize) -> Text,
) -> Response
```

```rust
let blend_modes = ["Normal", "Multiply", "Screen", "Overlay"];
egui::ComboBox::from_label("Blend")
    .selected_text(blend_modes[self.blend])
    .show_index(ui, &mut self.blend, blend_modes.len(), |i| blend_modes[i]);
```

---

## 10. The Layout System

### Direction and Align

```rust
pub enum Direction {
    LeftToRight,
    RightToLeft,
    TopDown,
    BottomUp,
}

pub enum Align {
    Min,    // left or top
    Center,
    Max,    // right or bottom
}

// Constants
Align::LEFT   = Align::Min
Align::RIGHT  = Align::Max
Align::TOP    = Align::Min
Align::BOTTOM = Align::Max
```

```rust
pub struct Align2([Align; 2]);  // [horizontal, vertical]

// Constants
Align2::LEFT_TOP      = Align2([Align::Min, Align::Min])
Align2::LEFT_CENTER   = Align2([Align::Min, Align::Center])
Align2::LEFT_BOTTOM   = Align2([Align::Min, Align::Max])
Align2::CENTER_TOP    = Align2([Align::Center, Align::Min])
Align2::CENTER_CENTER = Align2([Align::Center, Align::Center])
Align2::CENTER_BOTTOM = Align2([Align::Center, Align::Max])
Align2::RIGHT_TOP     = Align2([Align::Max, Align::Min])
Align2::RIGHT_CENTER  = Align2([Align::Max, Align::Center])
Align2::RIGHT_BOTTOM  = Align2([Align::Max, Align::Max])

// Key Align2 methods
pub fn x(&self) -> Align
pub fn y(&self) -> Align
pub fn anchor_rect(&self, rect: Rect) -> Rect
pub fn anchor_size(&self, origin: Pos2, size: Vec2) -> Rect
pub fn align_size_within_rect(&self, size: Vec2, container: Rect) -> Rect
pub fn pos_in_rect(&self, frame: &Rect) -> Pos2
```

### Layout constructors

```rust
// Main constructors — cross_align is alignment on the perpendicular axis
pub fn left_to_right(cross_align: Align) -> Self   // horizontal, children aligned vertically
pub fn right_to_left(cross_align: Align) -> Self
pub fn top_down(cross_align: Align) -> Self          // vertical, children aligned horizontally
pub fn bottom_up(cross_align: Align) -> Self
pub fn centered_and_justified(main_dir: Direction) -> Self  // one widget filling all space
pub fn top_down_justified(cross_align: Align) -> Self       // top_down that fills width
pub fn from_main_dir_and_cross_align(main_dir: Direction, cross_align: Align) -> Self

// Builder modifiers
pub fn with_main_wrap(self, wrap: bool) -> Self         // overflow to next line
pub fn with_main_align(self, align: Align) -> Self
pub fn with_cross_align(self, align: Align) -> Self
pub fn with_main_justify(self, justify: bool) -> Self   // fill main axis (stretch)
pub fn with_cross_justify(self, justify: bool) -> Self  // fill cross axis

// Inspectors
pub fn main_dir(&self) -> Direction
pub fn main_wrap(&self) -> bool
pub fn cross_align(&self) -> Align
pub fn cross_justify(&self) -> bool
pub fn is_horizontal(&self) -> bool
pub fn is_vertical(&self) -> bool
pub fn horizontal_align(&self) -> Align
pub fn vertical_align(&self) -> Align
```

### Ui layout methods

```rust
// --- Horizontal ---
pub fn horizontal<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
    // Layout::left_to_right(Align::Center)

pub fn horizontal_top<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
    // Layout::left_to_right(Align::Min) — top-aligned children

pub fn horizontal_centered<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>

pub fn horizontal_wrapped<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
    // Wraps to next line when out of space

// --- Vertical ---
pub fn vertical<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
    // Layout::top_down(Align::Left)

pub fn vertical_centered<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
pub fn vertical_centered_justified<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>

// --- Generic layout ---
pub fn with_layout<R>(
    &mut self,
    layout: Layout,
    add_contents: impl FnOnce(&mut Self) -> R,
) -> InnerResponse<R>

pub fn centered_and_justified<R>(&mut self, add_contents: impl FnOnce(&mut Self) -> R) -> InnerResponse<R>

// --- Columns ---
pub fn columns<R>(
    &mut self,
    num_columns: usize,
    add_contents: impl FnOnce(&mut [Self]) -> R,
) -> R

pub fn columns_const<const NUM_COL: usize, R>(
    &mut self,
    add_contents: impl FnOnce(&mut [Self; NUM_COL]) -> R,
) -> R

// --- Indentation ---
pub fn indent<R>(
    &mut self,
    id_salt: impl AsIdSalt,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R>

// --- Grouping ---
pub fn group<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
    // Draws a box around content; like Frame::group but as a method

pub fn scope<R>(&mut self, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R>
    // Creates a child Ui with same layout; scopes style changes
```

### Space and size allocation

```rust
// Query available space
pub fn available_size(&self) -> Vec2
pub fn available_width(&self) -> f32
pub fn available_height(&self) -> f32
pub fn available_size_before_wrap(&self) -> Vec2
pub fn available_rect_before_wrap(&self) -> Rect

// Constrain child Ui size
pub fn set_min_width(&mut self, width: f32)
pub fn set_min_height(&mut self, height: f32)
pub fn set_min_size(&mut self, size: Vec2)
pub fn set_max_width(&mut self, width: f32)
pub fn set_max_height(&mut self, height: f32)
pub fn set_max_size(&mut self, size: Vec2)
pub fn set_width(&mut self, width: f32)    // exact width (min=max)
pub fn set_height(&mut self, height: f32)

// Reserve space
pub fn add_space(&mut self, amount: f32)
    // adds gap in current main direction

pub fn allocate_space(&mut self, desired_size: Vec2) -> (Id, Rect)
    // reserves space, returns (interaction_id, allocated_rect)

pub fn allocate_response(&mut self, desired_size: Vec2, sense: Sense) -> Response
pub fn allocate_exact_size(&mut self, desired_size: Vec2, sense: Sense) -> (Rect, Response)
    // allocates exactly desired_size; may overflow if unavailable

pub fn allocate_at_least(&mut self, desired_size: Vec2, sense: Sense) -> (Rect, Response)
    // allocates at least desired_size; may be larger

pub fn allocate_rect(&mut self, rect: Rect, sense: Sense) -> Response

// Child Ui with specific size / layout
pub fn allocate_ui<R>(
    &mut self,
    desired_size: Vec2,
    add_contents: impl FnOnce(&mut Self) -> R,
) -> InnerResponse<R>

pub fn allocate_ui_with_layout<R>(
    &mut self,
    desired_size: Vec2,
    layout: Layout,
    add_contents: impl FnOnce(&mut Self) -> R,
) -> InnerResponse<R>

pub fn allocate_painter(&mut self, desired_size: Vec2, sense: Sense) -> (Response, Painter)
    // allocates space and returns a Painter for custom drawing

// Widget sizing
pub fn add_sized(&mut self, max_size: impl Into<Vec2>, widget: impl Widget) -> Response
    // constrains widget to max_size; overrides widget's natural size
```

### Spacing fields (ui.spacing())

```rust
pub struct Spacing {
    pub item_spacing: Vec2,      // gap between widgets (default [8.0, 3.0])
    pub window_margin: Margin,   // inside window frames
    pub button_padding: Vec2,    // text-to-edge in buttons
    pub menu_margin: Margin,
    pub interact_size: Vec2,     // minimum interactive widget size [18.0, 18.0]
    pub indent: f32,             // CollapsingHeader / indent() depth [18.0]
    pub slider_width: f32,
    pub text_edit_width: f32,
    pub combo_width: f32,
    pub tooltip_width: f32,
    pub menu_width: f32,
    pub icon_width: f32,         // outer icon size
    pub icon_width_inner: f32,   // inner icon size
    pub icon_spacing: f32,
    pub default_area_size: Vec2, // first-frame size of Areas
    pub combo_height: f32,       // ComboBox popup max height before scroll
    pub scroll: ScrollStyle,     // scrollbar dimensions and behavior
}
```

Mutate via: `ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);`

---

## 11. Separator

```rust
// Ui method — inserts a horizontal or vertical line depending on current layout direction
ui.separator();

// Or via widget directly
ui.add(egui::Separator::default());
ui.add(egui::Separator::default().horizontal());
ui.add(egui::Separator::default().vertical());
ui.add(egui::Separator::default().spacing(16.0));  // extra margin around line
```

---

## 12. Common Pitfalls

### Panel ordering
CentralPanel must come last. Adding any panel after CentralPanel gets it zero space (CentralPanel already consumed everything). The egui debug layer will warn about this in debug builds.

### ID collisions in loops
Every widget needing persistent state (CollapsingHeader, ScrollArea, Window, Grid) takes an ID. Reusing the same ID string in a loop gives all instances the same state.

**Wrong:**
```rust
for layer in &layers {
    egui::CollapsingHeader::new(&layer.name).show(ui, |ui| { });
    // Names may collide; IDs derived from label text
}
```

**Correct:**
```rust
for (idx, layer) in layers.iter().enumerate() {
    egui::CollapsingHeader::new(&layer.name)
        .id_salt(idx)  // makes ID unique per row
        .show(ui, |ui| { });
}
```

Or use `ui.push_id(idx, |ui| { ... })` to scope the entire row.

### Immediate mode sizing
`ui.available_size()` returns the space available at the moment of the call. After adding widgets, it shrinks. If you query `available_size()` to size a later widget, the value may be stale. Query it immediately before use.

### show_rows row_height must exclude item_spacing
`show_rows` adds `item_spacing.y` between rows itself. If you include spacing in `row_height_sans_spacing`, the scroll math drifts and items shift on scroll.

### show_viewport must allocate full content size
If using `show_viewport` for variable-height virtual scrolling, you must call `ui.allocate_space(Vec2::new(w, total_height))` at the end, or the scroll range will be the visible size only.

### No panels inside panels at the top level
`Panel::show` takes `&mut Ui`. If that Ui is itself inside another Panel's closure, you cannot open another top-level Panel from it. Use `Frame::show` + `ScrollArea` for nested scrollable sections within a panel.

### auto_shrink default
`ScrollArea` defaults `auto_shrink = [true, true]`: the scroll area shrinks to its content if the content is smaller. For a layers panel that should always fill the panel height, use `.auto_shrink([false, false])`.

### Window title used as Id
`Window::new("Tool Options")` uses the string as the ID. If two windows share a title, they share state. Use `.id(Id::new("unique_tool_options"))` or give each a distinct title.

---

## 13. Verified API Status

All signatures verified against egui master (0.34.2) source at `crates/egui/src/containers/`:
- `panel.rs` — Panel, CentralPanel
- `scroll_area.rs` — ScrollArea, ScrollAreaOutput
- `window.rs` — Window
- `area.rs` — Area
- `frame.rs` — Frame, Prepared
- `resize.rs` — Resize
- `collapsing_header.rs` — CollapsingHeader, CollapsingState
- `combo_box.rs` — ComboBox
- `grid.rs` — Grid
- `layers.rs` / `layer_id.rs` — Order enum

**Uncertainty flags:**
- `Ui::allocate_ui_at_rect` seen referenced in some docs but not confirmed in 0.34 source; use `allocate_rect` + `allocate_ui_with_layout` instead.
- `Panel::show_collapsible` is new in 0.34; behavior with `eframe::App::update` (deprecated) vs `App::ui` may differ — test with `App::ui`.
- `eframe::App::update` deprecation status: confirmed deprecated per changelog, but still present for compatibility. Prefer `App::ui`.
- `ComboBox::icon` closure signature not fully captured (complex fn type). Consult source if customizing.
