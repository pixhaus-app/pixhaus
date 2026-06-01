# Layout, panels, and containers

egui 0.34.2. How the editor chrome is arranged. The panel API changed in 0.34 — the
signatures below were checked against the live docs.

## Contents
- Panels (`Panel`, `CentralPanel`)
- `Window` and `Area`
- `Frame`
- `ScrollArea` and virtual scrolling
- `Grid`
- `CollapsingHeader` and `CollapsingState`
- Layouts, sizing, spacing
- Pitfalls

## Panels

`Panel` is the unified side/edge panel (it replaced `SidePanel`/`TopBottomPanel`, which
remain as deprecated aliases). `CentralPanel` fills the remainder. Both render with
`show_inside(ui, …)` taking `&mut Ui`; the old `show(ctx, …)` is deprecated.

```rust
// Constructors take an Id (globally unique).
egui::Panel::left(id)   // resizable by default
egui::Panel::right(id)  // resizable by default
egui::Panel::top(id)    // not resizable by default
egui::Panel::bottom(id) // not resizable by default

// Builders (size is width for left/right, height for top/bottom; includes frame margins):
.resizable(bool)
.default_size(f32)
.min_size(f32) .max_size(f32) .size_range(impl Into<Rangef>)
.exact_size(f32)              // locks size, disables resize
.show_separator_line(bool)
.frame(egui::Frame)

// Display:
.show_inside(ui, |ui| -> R) -> InnerResponse<R>
.show_animated_inside(ui, is_expanded: bool, |ui| -> R) -> Option<InnerResponse<R>>
```

```rust
egui::Panel::left("layers")
    .resizable(true)
    .default_size(220.0)
    .size_range(160.0..=400.0)
    .show_inside(ui, |ui| { /* layers */ });

egui::CentralPanel::default().show_inside(ui, |ui| { /* canvas */ });
// CentralPanel::default() includes a frame; ::no_frame() / ::default_margins() also exist.
```

**Ordering is the #1 panel bug.** Add outer panels first and `CentralPanel` last — it
claims whatever is left. Anything added after `CentralPanel` gets zero space. Don't open a
top-level panel from inside another panel's closure; nest with `Frame` + `ScrollArea`
instead.

## `Window` and `Area`

Both float above panels and still take `&Context` (reach it with `ui.ctx()`).

```rust
let mut open = true;
egui::Window::new("Tool Options")
    .open(&mut open)              // adds a close button; hides when false
    .resizable(true)
    .default_size([280.0, 360.0])
    .show(ui.ctx(), |ui| { /* … */ });
// returns Option<InnerResponse<Option<R>>>: None when closed, inner None when collapsed.
```

`Window::new(title)` uses the title as its id — give a `.id(Id::new("…"))` if the title is
dynamic or duplicated. `Area::new(id)` is a bare floating region (no title bar) for HUDs,
custom palette popups, and overlays; `Order::{Background, Middle, Foreground, Tooltip,
Debug}` controls z-order (`Foreground` for popups/menus, `Background` for a full-window
backdrop).

## `Frame`

Decorates any region with fill, stroke, margins, corner radius, shadow. Not a standalone
layout — wraps content.

```rust
egui::Frame::new()
    .fill(egui::Color32::from_rgb(30, 30, 35))
    .inner_margin(8.0)
    .corner_radius(4.0)              // CornerRadius, not Rounding
    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(60)))
    .show(ui, |ui| { ui.label("boxed content"); });
```

Preset constructors take `&Style`: `Frame::group(style)`, `central_panel(style)`,
`window(style)`, `canvas(style)`, `dark_canvas(style)`.

## `ScrollArea` and virtual scrolling

```rust
egui::ScrollArea::vertical()        // ::horizontal() ::both() ::neither()
    .auto_shrink([false, false])    // fill the panel instead of shrinking to content
    .stick_to_bottom(true)          // follow growing content (logs)
    .show(ui, |ui| { … });          // renders ALL content, even offscreen
```

`auto_shrink` defaults to `[true, true]`; a panel-filling list needs `[false, false]`.

**Virtualize anything that can grow large** — the layers panel and timeline must not build
a widget per row.

```rust
// Uniform row height (layers, palette swatches). Closure gets only the visible range.
let row_h = ui.text_style_height(&egui::TextStyle::Body);  // EXCLUDES item_spacing.y
egui::ScrollArea::vertical().auto_shrink([false, false])
    .show_rows(ui, row_h, layers.len(), |ui, range| {
        for i in range {
            ui.push_id(i, |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut layers[i].visible, "");
                    ui.label(&layers[i].name);
                });
            });
        }
    });
```

`row_height_sans_spacing` must NOT include `item_spacing.y` — `show_rows` adds spacing
between rows itself, and including it makes items drift on scroll.

```rust
// Variable row heights (timeline groups). You compute visible rows from the viewport rect
// and MUST allocate the full content height or the scrollbar range collapses.
egui::ScrollArea::vertical().show_viewport(ui, |ui, viewport| {
    let mut y = 0.0;
    for row in &rows {
        let h = row.height();
        let rect = egui::Rect::from_min_size(ui.min_rect().min + egui::vec2(0.0, y),
                                             egui::vec2(ui.available_width(), h));
        if rect.intersects(viewport.translate(ui.min_rect().min.to_vec2())) {
            // draw the row
        }
        y += h + ui.spacing().item_spacing.y;
    }
    ui.allocate_space(egui::vec2(ui.available_width(), y));   // REQUIRED
});
```

## `Grid`

Auto-sized two-column-style forms (tool options, properties). Call `ui.end_row()` after
each row. The id_salt must be unique and stable, or column widths corrupt across grids.

```rust
egui::Grid::new("layer_props").num_columns(2).spacing([12.0, 4.0]).striped(true)
    .show(ui, |ui| {
        ui.label("Opacity");
        ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0));
        ui.end_row();
        ui.label("Blend");
        ui.add(/* ComboBox */);
        ui.end_row();
    });
```

## `CollapsingHeader` and `CollapsingState`

```rust
egui::CollapsingHeader::new("Effects").default_open(true).id_salt(group_idx)
    .show(ui, |ui| { /* body */ });
```

For non-text headers (a layer-group row with a visibility checkbox + name + drag handle),
drive it manually with `CollapsingState`:

```rust
let id = ui.make_persistent_id(("layer_group", group_idx));
egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
    .show_header(ui, |ui| {
        ui.checkbox(&mut group.visible, "");
        ui.label(&group.name);
    })
    .body(|ui| {
        for layer in &group.layers { ui.label(&layer.name); }
    });
```

## Layouts, sizing, spacing

```rust
ui.horizontal(|ui| { … });            // left_to_right(Align::Center)
ui.horizontal_wrapped(|ui| { … });    // wrap to next line on overflow (toolbars)
ui.vertical(|ui| { … });              // top_down(Align::Min)
ui.vertical_centered(|ui| { … });
ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
    // place items from the right — handy for status-bar widgets
});
ui.columns(3, |cols| { cols[0].label("a"); cols[1].label("b"); cols[2].label("b"); });

// Sizing / space
ui.available_size();                  // query just before use; it shrinks
ui.add_space(8.0);
ui.allocate_exact_size(size, sense);  // (Rect, Response)
ui.allocate_ui_with_layout(size, layout, |ui| { … });   // fixed-size sub-UI, own layout
ui.set_min_width(120.0); ui.set_max_height(400.0);
ui.add_sized([80.0, 24.0], widget);

// Spacing knobs
ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
ui.spacing().interact_size;           // min interactive widget size (default ~[18,18])
```

`Layout::left_to_right(cross_align)` / `right_to_left` / `top_down` / `bottom_up`;
`Align::{Min, Center, Max}`; `Align2::{LEFT_TOP, CENTER_CENTER, RIGHT_BOTTOM, …}` for
anchoring rects and text.

## Pitfalls

- `CentralPanel` not last → other panels get no space.
- Reused ids in loops → shared scroll/collapse/focus state. `push_id`/`id_salt`.
- `show_rows` height including `item_spacing` → drift on scroll.
- `show_viewport` without `allocate_space(total)` → broken scroll range.
- `ScrollArea` default `auto_shrink [true,true]` → a list that won't fill its panel; set
  `[false,false]`.
- Duplicate `Window` titles or `Grid` id_salts → shared/corrupted state.
