# Widgets and custom widgets

egui 0.34.2. Built-in widgets, the immediate-mode change pattern, custom widgets, menus.

## Contents
- The `.changed()` / `.clicked()` pattern
- Text and display
- Buttons and toggles
- Numeric input (`Slider`, `DragValue`)
- Text input (`TextEdit`)
- Selection (`ComboBox`)
- Tooltips, context menus, menu bars
- Enable/disable scoping
- Custom widgets
- Deprecations

## The change pattern

Every `ui.add(widget)` and convenience method returns a `Response`. Bind the value with
`&mut`, then act on `.changed()` / `.clicked()` — not on the value unconditionally.

```rust
if ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0)).changed() {
    layer.set_opacity(opacity);     // fires only when the user moved it
}
if ui.button("Save").clicked() { save(); }
```

## Text and display

```rust
ui.label("plain");
ui.heading("Title"); ui.monospace("0xFF"); ui.strong("bold"); ui.weak("dim");
ui.colored_label(egui::Color32::RED, "error");

ui.label(egui::RichText::new("Layer 1")
    .color(egui::Color32::from_gray(200)).size(13.0).strong());
// RichText builders: .color .background_color .size .strong .weak .italics .underline
//   .strikethrough .monospace .code .heading .small

ui.hyperlink_to("docs", "https://docs.rs/egui");
ui.separator();
ui.add(egui::Spinner::new().size(16.0));
ui.add(egui::ProgressBar::new(0.7).show_percentage().animate(true));
```

## Buttons and toggles

```rust
if ui.button("Save").clicked() { … }

// Toolbar tool toggle — `selected` highlights the active tool.
if ui.add(egui::Button::new("Pencil").selected(tool == Tool::Pencil)).clicked() {
    tool = Tool::Pencil;
}
// Button builders: .fill .stroke .frame(bool) .small .min_size .corner_radius
//   .selected(bool) .shortcut_text("Ctrl+Z") .image(img) .image_and_text(img, txt) .sense

ui.checkbox(&mut visible, "Visible");
ui.radio_value(&mut tool, Tool::Fill, "Fill");           // preferred radio form
ui.selectable_value(&mut blend, BlendMode::Normal, "Normal");
ui.toggle_value(&mut show_grid, "Grid");                 // looks like a toggle button
```

## Numeric input

```rust
// Slider — bind &mut, give an inclusive range.
ui.add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("Opacity")
    .custom_formatter(|n, _| format!("{:.0}%", n * 100.0))
    .custom_parser(|s| s.trim_end_matches('%').parse::<f64>().ok().map(|v| v / 100.0)));
// .step_by .logarithmic(bool) .clamping(SliderClamping::{Always|Edits|Never}) .suffix .prefix

// DragValue — compact; drag or click-to-type. Ideal for size/position fields.
ui.horizontal(|ui| {
    ui.add(egui::DragValue::new(&mut w).speed(1).range(1..=8192).suffix(" px"));
    ui.label("×");
    ui.add(egui::DragValue::new(&mut h).speed(1).range(1..=8192).suffix(" px"));
});
// .fixed_decimals .min_decimals/.max_decimals .update_while_editing(false) (commit on Enter)
```

## Text input

`TextEdit` takes `&mut` to anything implementing `TextBuffer` (`String` does).

```rust
let r = ui.add(egui::TextEdit::singleline(&mut layer.name)
    .desired_width(120.0).hint_text("Layer name"));
if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
    commit_rename(&layer.name);
}

ui.add(egui::TextEdit::multiline(&mut script).code_editor()       // monospace + lock_focus
    .desired_width(f32::INFINITY));
// .password(true) .interactive(false) (read-only selectable) .clip_text(bool)
```

## Selection (`ComboBox`)

```rust
egui::ComboBox::from_label("Blend")
    .selected_text(format!("{blend:?}"))
    .show_ui(ui, |ui| {
        ui.selectable_value(&mut blend, BlendMode::Normal, "Normal");
        ui.selectable_value(&mut blend, BlendMode::Multiply, "Multiply");
    });

// Index form over a slice:
egui::ComboBox::from_id_salt("palette")
    .selected_text(&palettes[sel].name)
    .show_index(ui, &mut sel, palettes.len(), |i| &palettes[i].name);
```

`from_id_salt` when there's no visible label; `from_label` derives the id from the label.

## Tooltips, context menus, menu bars

```rust
ui.button("Merge Down")
    .on_hover_text("Merge into the layer below (Ctrl+E)")
    .on_hover_ui(|ui| { ui.label("rich tooltip with widgets"); });

let r = ui.label("Layer 0");
r.context_menu(|ui| {                          // right-click
    if ui.button("Rename").clicked() { start_rename(); ui.close(); }
    if ui.button("Delete").clicked() { delete(); ui.close(); }
    ui.menu_button("Move to", |ui| {
        if ui.button("Top").clicked() { move_top(); ui.close(); }
    });
});

// Menu bar (top panel)
egui::Panel::top("menubar").show_inside(ui, |ui| {
    ui.horizontal(|ui| {
        ui.menu_button("File", |ui| {
            if ui.button("New…").clicked() { new_doc(); ui.close(); }
            if ui.button("Quit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    });
});
```

Dismiss a menu from inside its closure with `ui.close()` (renamed from `ui.close_menu()`
in 0.29+; verify if you see the old form).

## Enable/disable scoping

```rust
ui.add_enabled(can_save, egui::Button::new("Save"));   // single widget
ui.add_enabled_ui(editing, |ui| { … });                // whole region, reversible
ui.scope(|ui| {                                          // isolate style/spacing changes
    ui.spacing_mut().item_spacing.y = 2.0;
    ui.style_mut().visuals.override_text_color = Some(egui::Color32::GOLD);
});
```

## Custom widgets

Pixhaus tools, swatches, and toggles are often custom: allocate a rect, sense input, paint,
and `mark_changed()` when internal state flips. Two forms.

```rust
// Reusable: implement Widget for `ui.add(ColorSwatch::new(c, sel))`.
pub struct ColorSwatch { color: egui::Color32, selected: bool, size: f32 }

impl egui::Widget for ColorSwatch {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::splat(self.size), egui::Sense::click());
        if ui.is_rect_visible(rect) {                  // skip painting offscreen
            let p = ui.painter();
            p.rect_filled(rect.shrink(1.0), 2.0, self.color);   // 2.0 -> CornerRadius
            let stroke = if self.selected {
                egui::Stroke::new(2.0, egui::Color32::WHITE)
            } else {
                ui.style().interact(&response).bg_stroke
            };
            p.rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Inside);
        }
        response
    }
}
// if ui.add(ColorSwatch::new(palette[i], i == sel)).clicked() { sel = i; }
```

```rust
// Free function (simpler, module-local):
fn tool_toggle(ui: &mut egui::Ui, label: &str, active: bool) -> egui::Response {
    ui.add(egui::Button::new(label).selected(active).min_size(egui::vec2(32.0, 32.0)))
}
```

Key allocators: `allocate_exact_size(size, sense) -> (Rect, Response)`,
`allocate_painter(size, sense) -> (Response, Painter)`, `allocate_space(size) -> (Id, Rect)`
(no interaction). Guard paints with `ui.is_rect_visible(rect)`. In a composite widget that
wraps a value change, call `response.mark_changed()` so callers can use `.changed()`.

## Deprecations (still compile in 0.34.2, prefer the replacement)

- `ImageButton` → `egui::Button::image(img)` / `Button::image_and_text(img, txt)`.
- `SelectableLabel` / `ui.selectable_label` → `ui.selectable_value` or `Button::selectable`.
- `ui.close_menu()` → `ui.close()`.
- `.rounding()` → `.corner_radius()` (and the type `Rounding` → `CornerRadius`).
