# egui 0.34.2 — Style/Theming, Input, Memory, eframe Bootstrap

Research date: 2026-05-25. All APIs verified against docs.rs 0.34.2 and egui workspace Cargo.toml tag 0.34.2.

---

## Version lockstep (pin all together)

| Crate | Version |
|---|---|
| `egui` | 0.34.2 |
| `eframe` | 0.34.2 |
| `egui-wgpu` | 0.34.2 |
| `egui-winit` | 0.34.2 |
| `egui_glow` | 0.34.2 |
| `epaint` | 0.34.2 |
| `wgpu` | **29.0.1** |
| `winit` | 0.30.13 |
| `glow` | 0.17.0 |

FLAG: `wgpu = "29.0.1"` is what the egui 0.34.2 workspace pins. If you depend on wgpu independently you MUST match this exactly or you will get two wgpu copies and linker/type errors. All egui-ecosystem crates share a single workspace version (0.34.2) and use `{ workspace = true }` internally — you cannot mix minor versions within the egui family.

Cargo.toml minimum viable config for wgpu backend:
```toml
[dependencies]
egui    = "0.34.2"
eframe  = { version = "0.34.2", default-features = false, features = ["wgpu", "persistence"] }
wgpu    = "29.0.1"   # must match egui workspace pin
```

---

## 1. Style and Theming

### Context-level API

```rust
// Exact signatures on egui::Context
pub fn set_style(&self, style: impl Into<Arc<Style>>)
pub fn style(&self) -> Arc<Style>
pub fn style_mut(&self, mutate_style: impl FnOnce(&mut Style))
pub fn set_visuals(&self, visuals: Visuals)
pub fn set_visuals_of(&self, theme: Theme, visuals: Visuals)  // per-theme override
```

Pattern — mutate in place (avoids a full clone):
```rust
ctx.style_mut(|s| {
    s.spacing.item_spacing = egui::vec2(10.0, 6.0);
    s.spacing.button_padding = egui::vec2(12.0, 6.0);
    s.visuals.window_corner_radius = egui::CornerRadius::same(8.0);
});
```

Pattern — clone-then-set (needed when building from a non-default base):
```rust
let mut style = (*ctx.style()).clone();
style.visuals = egui::Visuals::dark();
style.visuals.panel_fill = egui::Color32::from_rgb(28, 28, 30);
ctx.set_style(style);
```

Switch themes:
```rust
ctx.set_visuals(egui::Visuals::dark());   // built-in dark preset
ctx.set_visuals(egui::Visuals::light());  // built-in light preset
```

### egui::Style public fields

```rust
pub struct Style {
    pub override_text_style:              Option<TextStyle>,
    pub override_font_id:                 Option<FontId>,
    pub override_text_valign:             Option<Align>,
    pub text_styles:                      BTreeMap<TextStyle, FontId>,
    pub drag_value_text_style:            TextStyle,
    pub number_formatter:                 NumberFormatter,
    pub wrap_mode:                        Option<TextWrapMode>,  // replaces deprecated `wrap`
    pub spacing:                          Spacing,
    pub interaction:                      Interaction,
    pub visuals:                          Visuals,
    pub animation_time:                   f32,
    pub debug:                            DebugOptions,
    pub explanation_tooltips:             bool,
    pub url_in_tooltip:                   bool,
    pub always_scroll_the_only_direction: bool,
    pub scroll_animation:                 ScrollAnimation,
    pub compact_menu_style:               bool,
}
```

### egui::style::Spacing public fields

```rust
pub struct Spacing {
    pub item_spacing:                       Vec2,   // between widgets
    pub window_margin:                      Margin,
    pub button_padding:                     Vec2,
    pub menu_margin:                        Margin,
    pub indent:                             f32,
    pub interact_size:                      Vec2,   // min touch target
    pub slider_width:                       f32,
    pub slider_rail_height:                 f32,
    pub combo_width:                        f32,
    pub text_edit_width:                    f32,
    pub icon_width:                         f32,
    pub icon_width_inner:                   f32,
    pub icon_spacing:                       f32,
    pub default_area_size:                  Vec2,
    pub tooltip_width:                      f32,
    pub menu_width:                         f32,
    pub menu_spacing:                       f32,
    pub indent_ends_with_horizontal_line:   bool,
    pub combo_height:                       f32,
    pub scroll:                             ScrollStyle,
}
```

### egui::Visuals public fields

```rust
pub struct Visuals {
    // Theme toggle
    pub dark_mode: bool,

    // Text
    pub override_text_color: Option<Color32>,  // overrides ALL text
    pub weak_text_alpha:      f32,
    pub weak_text_color:      Option<Color32>,

    // Widget visuals
    pub widgets:       Widgets,    // state-keyed WidgetVisuals
    pub selection:     Selection,  // selected text/items

    // Link
    pub hyperlink_color: Color32,

    // Background hierarchy (darkest to brightest in dark mode)
    pub faint_bg_color:      Color32,  // striped table alt row
    pub extreme_bg_color:    Color32,  // TextEdit, dark inputs
    pub text_edit_bg_color:  Option<Color32>, // defaults to extreme_bg_color
    pub code_bg_color:       Color32,

    // Status colors
    pub warn_fg_color:  Color32,
    pub error_fg_color: Color32,

    // Window/panel
    pub window_corner_radius:    CornerRadius,   // was `window_rounding`
    pub window_shadow:           Shadow,
    pub window_fill:             Color32,
    pub window_stroke:           Stroke,
    pub window_highlight_topmost: bool,
    pub menu_corner_radius:      CornerRadius,
    pub panel_fill:              Color32,
    pub popup_shadow:            Shadow,

    // Interaction
    pub text_cursor:         TextCursorStyle,
    pub clip_rect_margin:    f32,
    pub button_frame:        bool,
    pub collapsing_header_frame: bool,
    pub indent_has_left_vline:   bool,
    pub striped:             bool,
    pub slider_trailing_fill: bool,
    pub handle_shape:        HandleShape,
    pub interact_cursor:     Option<CursorIcon>,
    pub resize_corner_size:  f32,
    pub image_loading_spinners: bool,
    pub numeric_color_space: NumericColorSpace,
    pub disabled_alpha:      f32,
}

// constructors
impl Visuals {
    pub fn dark()  -> Self  // preferred starting point for dark themes
    pub fn light() -> Self

    // helper methods
    pub fn text_color(&self)          -> Color32
    pub fn weak_text_color(&self)     -> Color32
    pub fn strong_text_color(&self)   -> Color32
    pub fn text_edit_bg_color(&self)  -> Color32
    pub fn window_fill(&self)         -> Color32
    pub fn window_stroke(&self)       -> Stroke
    pub fn noninteractive(&self)      -> &WidgetVisuals
    pub fn disable(&self, color: Color32) -> Color32  // reduce opacity
    pub fn gray_out(&self, color: Color32) -> Color32 // desaturate
}
```

### Widgets and WidgetVisuals

```rust
pub struct Widgets {
    pub noninteractive: WidgetVisuals, // labels, separators — not clickable
    pub inactive:       WidgetVisuals, // clickable but not hovered
    pub hovered:        WidgetVisuals,
    pub active:         WidgetVisuals, // currently pressed
    pub open:           WidgetVisuals, // open menus/combos
}

pub struct WidgetVisuals {
    pub bg_fill:       Color32,      // widget background
    pub weak_bg_fill:  Color32,      // secondary background
    pub bg_stroke:     Stroke,       // border
    pub corner_radius: CornerRadius, // RENAMED from `rounding` in earlier versions
    pub fg_stroke:     Stroke,       // text/icon stroke
    pub expansion:     f32,          // padding growth on hover/press
}
// `rounding()` still compiles but is #[deprecated] — use `corner_radius`
```

### Selection

```rust
pub struct Selection {
    pub bg_fill: Color32,  // background behind selected text
    pub stroke:  Stroke,   // color of selected text
}
```

### Per-ui style override

```rust
// Scoped override — only affects widgets in this closure
egui::Frame::default().show(ui, |ui| {
    ui.style_mut().visuals.override_text_color = Some(egui::Color32::from_rgb(0xC8, 0x78, 0x4B));
    ui.label("accent text");
});

// Or directly mutate the Ui's local style copy
ui.style_mut().spacing.item_spacing.y = 2.0;
```

### Porting a CSS-variable palette

Map CSS vars to Visuals fields:
```rust
// Example mapping from a dark pixel-art editor palette
let mut v = egui::Visuals::dark();
// --bg-primary   → panel_fill / window_fill
v.panel_fill  = Color32::from_rgb(0x1E, 0x1E, 0x2A);
v.window_fill = Color32::from_rgb(0x1E, 0x1E, 0x2A);
// --bg-secondary → widgets.noninteractive.bg_fill
v.widgets.noninteractive.bg_fill = Color32::from_rgb(0x28, 0x28, 0x38);
// --bg-input     → extreme_bg_color
v.extreme_bg_color = Color32::from_rgb(0x12, 0x12, 0x1C);
// --accent       → selection.bg_fill, hyperlink_color
v.selection.bg_fill = Color32::from_rgb(0x5C, 0x7E, 0xF7);
v.hyperlink_color   = Color32::from_rgb(0x5C, 0x7E, 0xF7);
// --text-primary → widgets.noninteractive.fg_stroke.color
v.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(0xE8, 0xE8, 0xF0);
// override ALL text
// v.override_text_color = Some(Color32::WHITE); // use sparingly

ctx.set_visuals(v);
```

---

## 2. Input Handling

### Accessing InputState

```rust
// Read-only — use for querying state
ctx.input(|i: &egui::InputState| {
    // use i here
});

// Mutable — required for consume_key / consume_shortcut
ctx.input_mut(|i: &mut egui::InputState| {
    // use i here; mutations consumed by egui next frame
});
```

Both closures should be **short-lived** — do not call ctx recursively from inside them (deadlock).

### egui::InputState public fields

```rust
pub struct InputState {
    pub raw:              RawInput,       // raw events from backend
    pub pointer:          PointerState,
    pub smooth_scroll_delta: Vec2,        // filtered/smoothed scroll
    pub pixels_per_point: f32,
    pub max_texture_side: usize,
    pub time:             f64,
    pub unstable_dt:      f32,           // raw frame delta, don't use for physics
    pub predicted_dt:     f32,
    pub stable_dt:        f32,           // smoothed, safe for animations
    pub focused:          bool,          // window has keyboard focus
    pub modifiers:        Modifiers,     // modifier state AT FRAME START
    pub keys_down:        HashSet<Key>,  // currently held keys
    pub events:           Vec<Event>,    // in-order raw events
}

// Key methods on InputState
pub fn smooth_scroll_delta(&self)       -> Vec2  // same as field — prefer field
pub fn zoom_delta(&self)                -> f32   // pinch-to-zoom scalar
pub fn zoom_delta_2d(&self)             -> Vec2  // non-uniform zoom
pub fn key_pressed(&self, key: Key)     -> bool  // true only on press frame
pub fn key_down(&self, key: Key)        -> bool  // true while held
pub fn key_released(&self, key: Key)    -> bool
pub fn num_presses(&self, key: Key)     -> usize // repeat-aware press count
pub fn consume_key(&mut self, modifiers: Modifiers, key: Key) -> bool
pub fn translation_delta(&self)         -> Vec2
pub fn is_scrolling(&self)              -> bool
pub fn filtered_events(&self, filter: &EventFilter) -> Vec<Event>
```

### PointerState methods

```rust
// Position
pub fn hover_pos(&self)     -> Option<Pos2>
// Use for: tooltips, highlight-on-hover.
// Returns None when pointer leaves window.

pub fn interact_pos(&self)  -> Option<Pos2>
// Use for: detecting where a click/drag happened.
// Persists through PointerGone events when an interaction is active.
// Prefer this over hover_pos when you've detected response.clicked() or response.dragged().

pub fn latest_pos(&self)    -> Option<Pos2>  // raw last known position
pub fn press_origin(&self)  -> Option<Pos2>  // position when button first pressed

// Movement
pub fn delta(&self)               -> Vec2          // pos change since last frame
pub fn motion(&self)              -> Option<Vec2>  // Some only if moved
pub fn velocity(&self)            -> Vec2
pub fn direction(&self)           -> Vec2
pub fn total_drag_delta(&self)    -> Option<Vec2>  // from press origin
pub fn is_still(&self)            -> bool
pub fn is_moving(&self)           -> bool
pub fn is_decidedly_dragging(&self) -> bool
// true when movement exceeds deadzone — use this to distinguish drag from click

// Button state
pub fn primary_down(&self)        -> bool
pub fn secondary_down(&self)      -> bool
pub fn middle_down(&self)         -> bool
pub fn button_down(&self, button: PointerButton) -> bool
pub fn primary_pressed(&self)     -> bool   // true only on press frame
pub fn secondary_pressed(&self)   -> bool
pub fn primary_released(&self)    -> bool
pub fn secondary_released(&self)  -> bool
pub fn button_pressed(&self, button: PointerButton)  -> bool
pub fn button_released(&self, button: PointerButton) -> bool
pub fn primary_clicked(&self)     -> bool   // pressed AND not a drag
pub fn secondary_clicked(&self)   -> bool
pub fn button_clicked(&self, button: PointerButton)  -> bool

// Presence
pub fn has_pointer(&self)         -> bool
pub fn any_down(&self)            -> bool
pub fn any_pressed(&self)         -> bool
pub fn any_released(&self)        -> bool
pub fn any_click(&self)           -> bool
pub fn could_any_button_be_click(&self) -> bool  // not yet dragged past threshold

// Timing
pub fn time_since_last_movement(&self) -> f32
pub fn time_since_last_click(&self)    -> f32
pub fn press_start_time(&self)         -> Option<f64>
```

hover_pos vs interact_pos — the rule:
- Use `hover_pos` for passive decorations (cursor highlight, tooltip anchor).
- Use `interact_pos` when you've already confirmed an interaction (`response.clicked()`, `response.dragged()`) and need the position — it stays valid even if the pointer briefly left the window mid-drag.

### Scrolling and zooming

```rust
ctx.input(|i| {
    let scroll = i.smooth_scroll_delta; // Vec2, y-axis for wheel, both for trackpad
    let zoom   = i.zoom_delta();        // f32 scalar — 1.0 = no change
    let zoom2d = i.zoom_delta_2d();     // Vec2 for non-uniform pinch
});
```

raw_scroll_delta lives on `i.raw.scroll_delta` (the unsmoothed value from `RawInput`). Prefer `smooth_scroll_delta` for viewport panning.

### Modifiers

```rust
pub struct Modifiers {
    pub alt:     bool,
    pub ctrl:    bool,
    pub shift:   bool,
    pub mac_cmd: bool,  // always false on non-Mac
    pub command: bool,  // ctrl on Win/Linux, cmd on Mac — USE THIS for shortcuts
}

// Constants
Modifiers::NONE
Modifiers::ALT
Modifiers::CTRL
Modifiers::SHIFT
Modifiers::MAC_CMD
Modifiers::COMMAND   // cross-platform shortcut modifier

// Comparison methods (prefer over field access)
modifiers.matches_logically(other: Modifiers) -> bool
modifiers.matches_exact(other: Modifiers)     -> bool
modifiers.cmd_ctrl_matches()                  -> bool
modifiers.contains(other: Modifiers)          -> bool
```

### Keyboard shortcuts

```rust
pub struct KeyboardShortcut {
    pub modifiers:   Modifiers,
    pub logical_key: Key,
}

impl KeyboardShortcut {
    pub fn new(modifiers: Modifiers, logical_key: Key) -> Self
    pub fn format(&self, names: &[ModifierNames], is_mac: bool) -> String
}
```

Usage patterns:
```rust
// 1. Consume a shortcut (marks it as handled — no widget sees it)
let save = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);
ctx.input_mut(|i| {
    if i.consume_shortcut(&save) {
        do_save();
    }
});

// 2. Display shortcut hint on a button
let shortcut_text = ctx.format_shortcut(&save);
if ui.add(egui::Button::new("Save").shortcut_text(shortcut_text)).clicked() {
    do_save();
}

// 3. Consume a one-off key (without defining a KeyboardShortcut)
ctx.input_mut(|i| {
    if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
        close_modal();
    }
});

// 4. Non-consuming key query (other widgets can still see it)
let pressed = ctx.input(|i| i.key_pressed(egui::Key::Z));
```

### egui::Key (108 variants)

Representative subset:
- Arrow: `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`
- Control: `Enter`, `Escape`, `Tab`, `Space`, `Delete`, `Backspace`
- Nav: `Home`, `End`, `PageUp`, `PageDown`
- Function: `F1`–`F35`
- Alpha: `A`–`Z`
- Digit: `Num0`–`Num9`
- Clipboard: `Copy`, `Cut`, `Paste`

```rust
Key::from_name("Space") -> Option<Key>  // parse from string
key.name()              -> &'static str
key.symbol_or_name()    -> &'static str // emoji/symbol if available
```

### Event enum — raw events

```rust
pub enum Event {
    Key { key: Key, physical_key: Option<Key>, pressed: bool, repeat: bool, modifiers: Modifiers },
    PointerMoved(Pos2),
    PointerButton { pos: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers },
    PointerGone,
    Scroll(Vec2),
    Zoom(f32),
    Text(String),    // printable characters — use for text input, NOT Key events
    // ... others
}
```

For text input always use `Event::Text`, not `Event::Key`. For shortcuts use `consume_shortcut`/`consume_key`.

---

## 3. Memory and Persistence

### Architecture rule — THIS IS CRITICAL

**Do NOT store app/document data in egui Memory.** egui's `Memory` is for UI-ephemeral state: open menus, scroll positions, animation timers, widget-specific transient values. Your application's document model, settings, and any data the user would care about losing lives in **your App struct** and is persisted via `eframe::Storage` / `eframe::set_value`.

### Accessing Memory

```rust
// Read-only
ctx.memory(|m: &Memory| {
    // query m
});

// Mutable
ctx.memory_mut(|m: &mut Memory| {
    // mutate m
});
```

### Memory public field

```rust
pub struct Memory {
    pub data: IdTypeMap,  // the actual storage — both temp and persisted slots
    // focus / areas / popups — impl details, access via methods
}
```

### Memory focus methods

```rust
impl Memory {
    pub fn request_focus(&mut self, id: Id)
    pub fn has_focus(&self, id: Id) -> bool
    pub fn focused(&self) -> Option<Id>
}

// Usage
ctx.memory_mut(|m| m.request_focus(my_widget_id));
let focused = ctx.memory(|m| m.has_focus(my_widget_id));
```

### IdTypeMap — temp storage methods

Temp values exist only for the current process run. Not persisted to disk.
```rust
// T must be: 'static + Any + Clone + Send + Sync
pub fn insert_temp<T: 'static + Any + Clone + Send + Sync>(&mut self, id: Id, value: T)
pub fn get_temp<T: 'static + Clone>(&self, id: Id) -> Option<T>  // clones on read
pub fn get_temp_mut_or<T: 'static + Any + Clone + Send + Sync>(&mut self, id: Id, or_insert: T) -> &mut T
pub fn get_temp_mut_or_default<T: 'static + Any + Clone + Send + Sync + Default>(&mut self, id: Id) -> &mut T
pub fn get_temp_mut_or_insert_with<T: 'static + Any + Clone + Send + Sync>(
    &mut self, id: Id, insert_with: impl FnOnce() -> T
) -> &mut T
pub fn remove_temp<T: 'static + Default>(&mut self, id: Id) -> Option<T>
```

### IdTypeMap — persisted storage methods

Persisted values survive app restarts when the `persistence` feature is enabled. Requires `T: SerializableAny` (Serialize + Deserialize + Any + Clone + Send + Sync).
```rust
pub fn insert_persisted<T: SerializableAny>(&mut self, id: Id, value: T)
pub fn get_persisted<T: SerializableAny>(&mut self, id: Id) -> Option<T>
// note: &mut self — first call deserializes and caches
pub fn get_persisted_mut_or<T: SerializableAny>(&mut self, id: Id, or_insert: T) -> &mut T
pub fn get_persisted_mut_or_default<T: SerializableAny + Default>(&mut self, id: Id) -> &mut T
pub fn get_persisted_mut_or_insert_with<T: SerializableAny>(
    &mut self, id: Id, insert_with: impl FnOnce() -> T
) -> &mut T
```

### Usage patterns for transient UI state

```rust
// Store per-widget state keyed by Id
let id = ui.id().with("my_animation_t");
let t: f32 = ctx.memory_mut(|m| {
    *m.data.get_temp_mut_or_default::<f32>(id)
});
ctx.memory_mut(|m| m.data.insert_temp(id, t + dt));

// Persist a UI preference (e.g., panel width) across restarts
let panel_id = egui::Id::new("left_panel_width");
let width: f32 = ctx.memory_mut(|m| {
    m.data.get_persisted_mut_or(panel_id, 200.0_f32).clone()
});
ctx.memory_mut(|m| m.data.insert_persisted(panel_id, new_width));
```

### egui::Id

```rust
egui::Id::new("string_seed")          // from static string
egui::Id::new(42u64)                  // from integer
id.with("sub_component")              // derive child id
ui.id()                               // id of current Ui region
ui.id().with(discriminant)            // stable child id
```

### App-level persistence via eframe::Storage

For real app data (settings, recent files, window layout) use eframe's storage which serializes to RON:
```rust
// Signatures
pub fn set_value<T: Serialize>(storage: &mut dyn Storage, key: &str, value: &T)
pub fn get_value<T: DeserializeOwned>(storage: &dyn Storage, key: &str) -> Option<T>

// In App::save
fn save(&mut self, storage: &mut dyn eframe::Storage) {
    eframe::set_value(storage, eframe::APP_KEY, self);
    // APP_KEY = "app" — the conventional key for the top-level app state
}

// In MyApp::new (CreationContext)
fn new(cc: &eframe::CreationContext<'_>) -> Self {
    if let Some(storage) = cc.storage {
        return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
    }
    Self::default()
}
```

`eframe::APP_KEY` is `"app"`. Use distinct string keys for sub-objects.

---

## 4. eframe Bootstrap

### run_native signature

```rust
pub fn run_native(
    app_name: &str,            // used as window title fallback + Wayland app_id + persistence key
    native_options: NativeOptions,
    app_creator: AppCreator<'_>, // Box<dyn FnOnce(&CreationContext<'_>) -> Result<Box<dyn App>>>
) -> Result  // eframe::Result = Result<(), Error>
```

### Minimal bootstrap

```rust
fn main() -> eframe::Result {
    eframe::run_native(
        "Pixhaus",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("Pixhaus")
                .with_inner_size([1280.0, 800.0])
                .with_min_inner_size([800.0, 600.0]),
            renderer: eframe::Renderer::Wgpu,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(PixhausApp::new(cc)))),
    )
}
```

### NativeOptions public fields

```rust
pub struct NativeOptions {           // cfg(not(target_arch = "wasm32"))
    pub viewport:              ViewportBuilder,
    pub vsync:                 bool,
    pub multisampling:         u16,
    pub depth_buffer:          u8,
    pub stencil_buffer:        u8,
    pub hardware_acceleration: HardwareAcceleration, // Required | Preferred | Off
    pub renderer:              Renderer,             // Renderer::Wgpu | Renderer::Glow
    pub run_and_return:        bool,   // false = block until exit
    pub event_loop_builder:    Option<EventLoopBuilderHook>,
    pub window_builder:        Option<WindowBuilderHook>,
    pub shader_version:        Option<ShaderVersion>,
    pub centered:              bool,
    pub wgpu_options:          WgpuConfiguration,   // device features, limits, etc.
    pub persist_window:        bool,   // restore window size/pos from last session
    pub persistence_path:      Option<PathBuf>, // override default storage location
    pub dithering:             bool,
}

pub enum Renderer { Glow, Wgpu }
```

### ViewportBuilder key methods (builder pattern, all return Self)

```rust
pub fn with_title(self, title: impl Into<String>)              -> Self
pub fn with_inner_size(self, size: impl Into<Vec2>)            -> Self
pub fn with_min_inner_size(self, size: impl Into<Vec2>)        -> Self
pub fn with_icon(self, icon: impl Into<Arc<IconData>>)         -> Self
pub fn with_resizable(self, resizable: bool)                   -> Self
pub fn with_maximize_button(self, value: bool)                 -> Self  // no-op on X11
pub fn with_decorations(self, decorations: bool)               -> Self  // default true
pub fn with_active(self, active: bool)                         -> Self
pub fn with_position(self, pos: impl Into<Pos2>)               -> Self
pub fn with_fullscreen(self, fullscreen: bool)                 -> Self
```

### eframe::App trait

```rust
pub trait App {
    // REQUIRED — called every repaint frame
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame);

    // OPTIONAL lifecycle / hooks (all have no-op defaults)
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {}
    // logic() runs ONCE before each ui() call (and also when hidden but repaint requested)
    // do NOT paint/add widgets inside logic()

    fn save(&mut self, storage: &mut dyn eframe::Storage) {}
    // called on auto-save interval and on clean shutdown; requires "persistence" feature

    fn on_exit(&mut self, gl: Option<&glow::Context>) {}
    // called after save() on shutdown

    fn auto_save_interval(&self) -> std::time::Duration { Duration::from_secs(30) }
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] { /* dark grey */ }
    fn persist_egui_memory(&self) -> bool { true }
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {}
    // intercept and modify input before egui sees it — useful for global shortcuts
}

// NOTE: `update()` exists but is #[deprecated] as of eframe 0.34.
// Do not implement update(); implement ui() instead.
```

Frame execution order each repaint:
1. `logic()` — non-UI updates, may be called when window is hidden
2. `ui()` — painting; window must be visible

### CreationContext fields

```rust
pub struct CreationContext<'s> {
    pub egui_ctx:          egui::Context,      // configure fonts/style here before first frame
    pub integration_info:  IntegrationInfo,
    pub storage:           Option<&'s dyn eframe::Storage>, // None if persistence feature off
    pub gl:                Option<Arc<glow::Context>>,      // Some only with Renderer::Glow
    pub wgpu_render_state: Option<egui_wgpu::RenderState>, // Some only with Renderer::Wgpu
    pub get_proc_address:  Option<Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>>,
}
```

Canonical `new` pattern:
```rust
impl PixhausApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // 1. Set fonts before first frame
        let mut fonts = egui::FontDefinitions::default();
        // fonts.font_data.insert("Inter", ...);
        cc.egui_ctx.set_fonts(fonts);

        // 2. Apply custom theme
        cc.egui_ctx.set_visuals(build_pixhaus_visuals());

        // 3. Restore persisted state
        let mut app: Self = cc.storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();

        // 4. Grab wgpu render state for custom canvas integration
        if let Some(wgpu_state) = &cc.wgpu_render_state {
            app.canvas_renderer = Some(CanvasRenderer::new(wgpu_state));
        }

        app
    }
}
```

### wgpu backend usage

Set `renderer: eframe::Renderer::Wgpu` in NativeOptions. Access `cc.wgpu_render_state` (a `egui_wgpu::RenderState`) in `new()` to initialize custom wgpu pipelines. During rendering, use `egui_wgpu::Callback` to inject wgpu draw calls into the egui paint list.

```rust
// During ui(), schedule a wgpu draw call
ui.painter().add(egui_wgpu::Callback::new_paint_callback(
    viewport_rect,
    MyCanvasCallback { ... },
));

// MyCanvasCallback must implement egui_wgpu::CallbackTrait
impl egui_wgpu::CallbackTrait for MyCanvasCallback {
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue,
               screen_descriptor: &ScreenDescriptor, encoder: &mut CommandEncoder,
               resources: &mut CallbackResources) -> Vec<CommandBuffer> { ... }
    fn paint(&self, info: PaintCallbackInfo, render_pass: &mut RenderPass,
             resources: &CallbackResources) { ... }
}
```

wgpu 29.0.1 is pinned — if your canvas pipeline code references wgpu types they must come from the same crate instance (enforced by Cargo's dependency deduplication only when versions match exactly).

---

## Pitfalls

1. **`update()` is deprecated** — implement `ui()` not `update()`. Implementing both is undefined behavior in terms of which gets called.
2. **`rounding` renamed to `corner_radius`** — WidgetVisuals, Visuals, Frame all use `corner_radius: CornerRadius` not `Rounding`. Old code will not compile.
3. **`ctx.input()` closure deadlock** — never call `ctx.*()` recursively from inside an `input()` closure. Gather what you need in one closure, process outside.
4. **IdTypeMap clones on every `get_temp` read** — keep transient-state types `Copy` or `Arc`-wrapped; avoid large types.
5. **`get_persisted` takes `&mut self`** — first call deserializes; subsequent reads hit cache. Always call inside `ctx.memory_mut`, not `ctx.memory`.
6. **wgpu version must match exactly** — adding `wgpu = "29"` in your own Cargo.toml alongside `eframe = "0.34.2"` is safe only if both resolve to 29.0.1. A semver-compatible update on either side (e.g. eframe 0.35 bumping to wgpu 30) will break custom pipeline code silently until you rebuild.
7. **`override_text_color` is nuclear** — it overrides ALL text including icons, error messages, placeholder text. Use `widgets.*.fg_stroke.color` or per-widget style overrides for narrower scoping.
8. **`smooth_scroll_delta` vs `raw.scroll_delta`** — on trackpads, `raw_scroll_delta` is noisy; `smooth_scroll_delta` applies momentum filtering. Prefer smooth for viewport pan; use raw only when you need exact physical events (e.g., detecting scroll start/stop precisely).
