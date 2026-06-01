# Pixhaus v3 UI shell foundation - design spec

Status: approved for spec write (2026-06-01). Produced by a design-exploration
workflow (3 competing architects, 4 judges, synthesis, 4 adversarial verifiers,
reconcile) and reconciled against the architecture bible, the visual UX direction,
the reference frames, the pinned egui 0.34 API, and the v2 UI audit.

## Decisions taken

Locked before design:

- Scope: the UI shell foundation only. No `core` domain model (it is an empty
  stub); panels show representative placeholder content.
- Architecture: the full bible Phase-0 scaffold - real Panel/Tool/Workspace/Module
  traits, the registries, and project/session/UI state separation. The `app`
  binary composes the shell purely by consuming registries.
- Theme: fresh tokens from the UX doc - near-black warm slate, violet accent,
  semantic roles, elevation tiers, spacing and type scales; dark-first with light
  and accent variants structured in.

Resolved during design review:

- egui-phosphor: ratified (tool-rail glyphs + AI sparkle). Add it to the
  `Cargo.toml` catalog and add a `pixhaus-egui-phosphor` skill as a follow-up.
- egui_kittest: declined. The one end-to-end smoke test degrades to a headless
  `Host` + `resolve_layout` assertion (still meaningful, no harness needed).
- Bottom tray: multi-tab. `WorkspaceLayout.bottom_tray` is a `Vec<PanelId>` with a
  per-workspace selected-tab map, matching the UX doc and leaving room for a future
  module to contribute a tray tab to another workspace (bible 7.4).

## Overview and scope

This round builds the registry-driven application shell: the real trait surface and
registries the bible mandates (Panel, Tool, Workspace, Module), a
project/session/UI state separation, fresh dark/violet theme tokens, and all seven
window regions rendered with five registered workspaces whose panels show
representative placeholder content. The `app` binary composes the shell purely by
registering modules and consuming registries - no hardcoded layout.

Out of scope, because `core` is an empty stub: nothing references
`Project`/`Document`/`Sprite`/`Layer`/`Frame`/`Cel`/`Palette`/`Selection`/`Command`.
Those are reserved as doc-comment seams (`session.active_document`,
`Intent::Command`, `Tool::on_pointer`). No real commands, no undo stack, no jobs
beyond a single `JobStub` that toggles the AI status dot. No docking or
drag-rearrange. Only `dark()` is visually tuned; light and high-contrast are
structured in but not finished. `CanvasCallback` and `install_canvas_renderer` are
preserved unchanged.

The anchor decision: panels receive a read-only state view plus a single write
channel, so "panels never mutate project/session state directly" (bible rules 12,
21) is a compiler guarantee, not a convention. The one carve-out is a panel's own
scratch text buffer, which `TextEdit` requires as `&mut String` in-frame; that
exception is explicit, disjoint, and per-panel.

## ui-crate module tree and crate wiring

`crates/ui` owns the permanent contract (traits, registries, theme, shell runtime,
shared widgets). Concrete capability bodies live in the `modules/*` crates that will
own the real versions, so nothing has to move when `core` lands.

```
crates/ui/src/
  lib.rs                  re-exports; install_canvas_renderer + CanvasCallback (KEEP, unchanged)

  theme/
    mod.rs                Theme, ThemeVariant, apply_to_visuals(theme, ctx), surface(tier)
    tokens.rs             Surfaces, Roles, AccentTokens, Elevation, Spacing, TypeScale, Radii
    palettes.rs           dark(), light(), accent seed -> AccentTokens derivation
    fonts.rs              install_fonts(ctx): UI sans + mono + phosphor merged as a fallback family
    contrast.rs           wcag_contrast(fg, bg) -> f32 (pure; used by the theme test)
  icons.rs                crate::icons::* phosphor glyph char constants (PENCIL, SPARKLE, ...)

  region.rs               Region enum + per-region egui Id constants

  contrib_api/            THE PERMANENT TRAIT SURFACE
    mod.rs
    ids.rs                PanelId, ToolId, WorkspaceId, ActionId newtypes
    context.rs            ContribCtx<'a>, PanelScope<'a>  (the read-view + write channels)
    panel.rs              Panel trait, PanelMeta
    tool.rs               Tool trait, ToolMeta
    workspace.rs          Workspace trait, WorkspaceMeta, WorkspaceLayout, StatusItem
    module.rs             Module trait, HostRegistrar (the dyn registrar), ActionDesc, MenuGroup
  registry/
    mod.rs                Registry<K,V>; Registries; impl HostRegistrar; ResolvedLayout
    resolve.rs            resolve_layout(workspace_id, &registries) -> ResolvedLayout

  state/
    mod.rs                Host { registries, state, intents, scratch, theme, bg }
    session.rs            SessionState, JobStub, AiStatus
    ui_state.rs           UiState, GridMode, Modal
    intent.rs             Intent, Event, IntentSink, apply_intent(host, intent, ctx)

  shell/
    mod.rs                Shell::run(host, ui); drain_background(host, ctx)
    runtime.rs            per-frame region composition + post-loop intent drain
    regions/
      top_bar.rs          menus + workspace tabs + global status
      tool_options.rs     active tool's options_ui
      left_rail.rs        tool rail
      right_dock.rs       card stack (the load-bearing borrow loop)
      bottom_tray.rs      tab row + the selected tray panel
      status_bar.rs       compact status strip
      canvas_stage.rs     framed artboard + checker + grid + HUD (Painter) + CanvasCallback embed
    command_palette.rs    Ctrl/Cmd+K Area overlay (stub)
    shortcuts.rs          workspace Ctrl/Cmd+1..5, tool keys (focus-gated), palette -> Intent
    menus.rs              top-bar menu structure as data

  widgets/
    mod.rs
    card.rs               panel_card(ui, theme, meta, collapsed, body) -> response
    tool_button.rs        rail icon button: active accent tint + line + shortcut tooltip
    workspace_tab.rs      the violet pill/underline tab
    tray_tab.rs           the tray tab chip (active = accent pill)
    section_header.rs     icon + title header inside cards
    placeholder.rs        mock-row / mock-thumbnail-grid / mock-log helpers
```

`theme`, `contrib_api`, `state`, `registry` reference egui types but never `render`.
`shell` and `widgets` know both egui and `render` (the canvas embed). Nothing pushes
egui into `core`/`render`/`io`/`services`.

### modules/* and app wiring

Each `modules/*` crate constructs and registers its own concrete types. `app` names
module structs and nothing else.

```rust
// modules/sprite-edit/src/lib.rs
use pixhaus_ui::contrib_api::{Module, HostRegistrar};
mod draw;     // DrawWorkspace + the shared Layers/Sprites/Palette/SelectionActions/AiAssistant
              // panels + the shared Frames/Assets/Console tray panels
mod tools;    // the 15 manual + AI-brush Tool impls (the shared editing core, bible rule 2)

pub struct SpriteEditModule;
impl Module for SpriteEditModule {
    fn id(&self) -> &'static str { "sprite-edit" }
    fn register(&self, host: &mut dyn HostRegistrar) {
        tools::register(host);
        draw::register(host);   // Draw workspace + Draw panels + shared tray panels
    }
}
```

```rust
// app/src/main.rs (shape; the existing boot + CanvasCallback seam is preserved)
fn build_host(ctx: &egui::Context) -> pixhaus_ui::state::Host {
    let mut host = pixhaus_ui::state::Host::new(pixhaus_ui::theme::Theme::dark());
    // Registration order is the contract: sprite-edit registers the shared
    // Layers/Sprites/Palette/AI panels and the shared Frames/Assets/Console tray
    // panels FIRST; the other workspaces reference them by id.
    let modules: [Box<dyn pixhaus_ui::contrib_api::Module>; 5] = [
        Box::new(pixhaus_mod_sprite_edit::SpriteEditModule),
        Box::new(pixhaus_mod_animation::AnimationModule),
        Box::new(pixhaus_mod_tiles::TilesModule),
        Box::new(pixhaus_mod_generation::GenerationModule),
        Box::new(pixhaus_mod_export::ExportModule),
    ];
    for m in &modules {
        m.register(&mut host.registrar());   // the ONLY path capabilities enter the shell
    }
    pixhaus_ui::theme::apply_to_visuals(host.theme(), ctx);
    pixhaus_ui::theme::install_fonts(ctx);
    host
}

impl PixhausApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(rs) = cc.wgpu_render_state.as_ref() {
            pixhaus_ui::install_canvas_renderer(rs);          // KEEP - unchanged
        }
        Self { host: build_host(&cc.egui_ctx) }
    }
}

impl eframe::App for PixhausApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Per-frame non-draw work belongs here, not in `ui`: `logic` runs even when
        // the window is occluded but a repaint was requested. Folds channel results
        // into session state and calls ctx.request_repaint() when something landed.
        pixhaus_ui::shell::drain_background(&mut self.host, ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        pixhaus_ui::shell::Shell::run(&mut self.host, ui);
    }
}
```

`pixhaus-mod-core`, `-pixel-art`, and `-providers` are not wired this round - they
own art-mode tools and provider dispatch that need `core`/`services` to be non-fake.
Wiring a module with no real contribution is speculative scaffold; they join when
their backing layer is real. The thin module crates depend only on `ui` this round,
which is within the `core+services+ui` ceiling the CLAUDE.md sets.

## Trait surface and registries

All four registry traits are dyn-compatible and stored as `Box<dyn _>`: registries
are the textbook heterogeneous-collection case, none sits on the 8K per-pixel hot
path (they run O(visible) per frame), and the vtable hop is free here.
Dyn-compatibility is preserved by no generic methods, no `-> Self`, metadata by
value, and `&self` receivers.

### Identity newtypes

```rust
// contrib_api/ids.rs
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)] pub struct PanelId(pub &'static str);
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)] pub struct ToolId(pub &'static str);
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)] pub struct WorkspaceId(pub &'static str);
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)] pub struct ActionId(pub &'static str);
```

### Region

```rust
// region.rs
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Region {
    TopBar,        // shell chrome: menus + workspace tabs + global status
    ToolOptions,   // driven by the active Tool, not the panel registry
    LeftRail,      // filled from ToolRegistry (workspace-filtered)
    Center,        // shell chrome: canvas stage; embeds CanvasCallback
    RightDock,     // filled from PanelRegistry: a card stack
    BottomTray,    // filled from PanelRegistry: a tab row + the selected panel
    StatusBar,     // shell chrome + workspace status items
}

// Each egui Panel needs a unique stable Id; declare them beside the enum.
pub mod region_id {
    pub const TOP_BAR: &str = "pixhaus.topbar";
    pub const TOOL_OPTIONS: &str = "pixhaus.tooloptions";
    pub const LEFT_RAIL: &str = "pixhaus.rail";
    pub const RIGHT_DOCK: &str = "pixhaus.dock";
    pub const BOTTOM_TRAY: &str = "pixhaus.tray";
    pub const STATUS_BAR: &str = "pixhaus.status";
}
```

The top bar, tool-options bar, status bar, and canvas stage are shell chrome - not
heterogeneous, not rearrangeable - so they are not forced through `Box<dyn Panel>`.
Only `LeftRail` (tools), `RightDock`, and `BottomTray` (panels) are registry-fed.

### The context handles - the heart of the borrow story

Two handles. `ContribCtx` is shared by panels and tools: a read-only state view, the
theme, and exactly one write channel (the intent sink). `PanelScope` adds the
panel's `PanelId` and a `&mut String` to that panel's own scratch buffer - the
single, disjoint exception to "intents are the only write channel", required because
`egui::TextEdit` needs a live `&mut String` in-frame.

```rust
// contrib_api/context.rs
/// Read view + the one write channel. Carried by tools and (wrapped) by panels.
/// A contributor physically cannot mutate session or UI state - bible rules 12/21
/// enforced by the type system.
pub struct ContribCtx<'a> {
    pub session: &'a SessionState,   // READ-ONLY
    pub ui_state: &'a UiState,       // READ-ONLY
    pub theme: &'a Theme,
    pub intents: &'a mut IntentSink, // the write channel for everything except scratch text
}

/// What a Panel sees. Adds the panel's id (so the shell - not the panel - scopes
/// egui Ids) and a mutable handle to THIS panel's own scratch text buffer only.
pub struct PanelScope<'a> {
    pub ctx: ContribCtx<'a>,
    pub id: PanelId,
    pub scratch: &'a mut String,     // the single carve-out: TextEdit needs &mut String
}
```

### Panel

```rust
// contrib_api/panel.rs
pub trait Panel {
    fn id(&self) -> PanelId;
    fn meta(&self) -> PanelMeta;
    /// Capability predicate: could this panel ever appear in the given workspace?
    /// Used by the shell only as a debug_assert against a workspace's authored
    /// layout - NOT as a runtime placement filter. The WorkspaceLayout is the sole
    /// placement authority (bible rule 14). Default: usable anywhere it is listed.
    fn relevant_in(&self, _workspace: WorkspaceId) -> bool { true }
    /// Render representative content. Reads through `scope.ctx`; pushes Intents into
    /// scope.ctx.intents; may edit only `scope.scratch`. Nothing else is mutable.
    fn ui(&self, ui: &mut egui::Ui, scope: &mut PanelScope<'_>);
}

pub struct PanelMeta {
    pub title: &'static str,
    pub icon: char,                  // phosphor glyph from crate::icons
    pub default_region: Region,
    pub default_open: bool,
}
```

`&self` is deliberate: a panel holds no mutable state of its own. Its collapse flag
lives in `UiState`, its draft text in `Host.scratch`. Iterating `&registry.panels`
(shared) coexists with `&mut intents` and `&mut scratch` (sibling `Host` fields). No
`get_mut`, no aliasing.

### Tool

```rust
// contrib_api/tool.rs
pub trait Tool {
    fn id(&self) -> ToolId;
    fn meta(&self) -> ToolMeta;
    /// Render this tool's options into the tool-options bar when active.
    /// Takes ContribCtx (no PanelId - a tool is not a panel).
    fn options_ui(&self, ui: &mut egui::Ui, cx: &mut ContribCtx<'_>);
    // When core lands: `fn on_pointer(&self, ev, &mut CommandSink)` arrives here,
    // additive. Tools emit no canvas commands this round.
}

pub struct ToolMeta {
    pub label: &'static str,
    pub icon: char,
    pub shortcut: Option<egui::KeyboardShortcut>,
    pub tooltip: &'static str,       // "Draw individual pixels. Hold Shift for a line."
    pub is_ai: bool,                 // the AI Brush flips this -> violet + sparkle marker
}
```

### Workspace (layout-only, owns no data)

```rust
// contrib_api/workspace.rs
pub trait Workspace {
    fn id(&self) -> WorkspaceId;
    fn meta(&self) -> WorkspaceMeta;          // name, icon, purpose, shortcut (Ctrl/Cmd+1..5)
    /// Pure: which registered panels/tools fill which region. No egui, no mutation.
    /// Returns ids only; the shell resolves them. The snapshot-test target.
    fn layout(&self) -> WorkspaceLayout;
}

pub struct WorkspaceMeta {
    pub name: &'static str,                   // "Draw"
    pub icon: char,
    pub purpose: &'static str,                // tooltip / command-palette description
    pub shortcut: egui::KeyboardShortcut,     // Modifiers::COMMAND + Key::Num1..Num5
}

#[derive(Clone, PartialEq, Debug)]            // Debug => insta-snapshottable
pub struct WorkspaceLayout {
    pub right_dock: Vec<PanelId>,             // top-to-bottom card stack
    pub bottom_tray: Vec<PanelId>,            // tray tabs, left-to-right; first is the default tab
    pub primary_tools: Vec<ToolId>,           // ordered subset shown in the rail
    pub default_tool: ToolId,
    pub status_items: Vec<StatusItem>,        // workspace-specific status entries
}

#[derive(Clone, PartialEq, Debug)]
pub struct StatusItem { pub icon: char, pub text: String }  // strings only - snapshots use Debug
```

`layout()` returns owned `Vec`s of `Copy` ids - cheap to call once per frame for the
active workspace. No panel object moves.

### Module and the registrar

The registrar is a `dyn` trait so a module never sees the concrete `Registries`.

```rust
// contrib_api/module.rs
pub trait HostRegistrar {
    fn add_panel(&mut self, panel: Box<dyn Panel>);
    fn add_tool(&mut self, tool: Box<dyn Tool>);
    fn add_workspace(&mut self, ws: Box<dyn Workspace>);
    fn add_action(&mut self, action: ActionDesc);
    fn add_menu_group(&mut self, group: MenuGroup);   // modules contribute Sprite/Layer/Frame menus
    // add_importer/exporter/provider/validator land with their registries later.
}

pub struct ActionDesc {
    pub id: ActionId, pub label: &'static str, pub icon: char, pub palette_visible: bool,
}

pub trait Module {
    fn id(&self) -> &'static str;
    fn register(&self, host: &mut dyn HostRegistrar);
}
```

GenerationModule and ExportModule register only a workspace + panels + tray + menus
this round; provider/exporter/validator registration arrives with `services`/`io`
bodies. Their QA-warning and provider panels render mock content.

### Registries

```rust
// registry/mod.rs
pub struct Registry<K: Copy + Eq + Hash, V> {
    items: Vec<V>,                 // insertion order = display order (rail, tabs)
    index: HashMap<K, usize>,
}
impl<K: Copy + Eq + Hash, V> Registry<K, V> {
    fn insert(&mut self, key: K, value: V) {
        // A compiled-in module registering a duplicate id is a programming error,
        // not a recoverable event. Loud in debug; last value wins in release.
        debug_assert!(!self.index.contains_key(&key), "duplicate registry id");
        match self.index.get(&key).copied() {
            Some(i) => self.items[i] = value,
            None => { self.index.insert(key, self.items.len()); self.items.push(value); }
        }
    }
    pub fn get(&self, key: K) -> Option<&V> { self.index.get(&key).map(|&i| &self.items[i]) }
    pub fn iter(&self) -> impl Iterator<Item = &V> { self.items.iter() }
}

pub type PanelRegistry     = Registry<PanelId,     Box<dyn Panel>>;
pub type ToolRegistry      = Registry<ToolId,      Box<dyn Tool>>;
pub type WorkspaceRegistry = Registry<WorkspaceId, Box<dyn Workspace>>;

pub struct Registries {
    pub panels: PanelRegistry,
    pub tools: ToolRegistry,
    pub workspaces: WorkspaceRegistry,
    pub actions: Registry<ActionId, ActionDesc>,
    pub menus: Vec<MenuGroup>,
}
// A thin wrapper over &mut Registries implements HostRegistrar; insert keys come
// from each value's id().
```

`get` returns `&V`, not `&mut V`: because `Panel::ui`/`Tool::options_ui` are `&self`,
the loops never need `get_mut`, which makes the disjoint-field borrow trivial.

A compile-time dyn-compatibility guard, on the actual storage form:

```rust
const _: () = {
    fn _assert_boxable(_: Box<dyn Panel>, _: Box<dyn Tool>, _: Box<dyn Workspace>, _: Box<dyn Module>) {}
};
```

### Layout resolution

```rust
// registry/resolve.rs
#[derive(Clone, PartialEq, Debug)]
pub struct ResolvedLayout {
    pub right_dock: Vec<PanelId>,    // filtered to registered ids
    pub bottom_tray: Vec<PanelId>,   // filtered to registered ids (tray tabs)
    pub primary_tools: Vec<ToolId>,
    pub default_tool: ToolId,
    pub status_items: Vec<StatusItem>,
}

pub fn resolve_layout(ws: WorkspaceId, r: &Registries) -> ResolvedLayout {
    let Some(workspace) = r.workspaces.get(ws) else { return ResolvedLayout::empty(); };
    let layout = workspace.layout();
    let keep_panel = |id: &PanelId| match r.panels.get(*id) {
        Some(panel) => {
            debug_assert!(panel.relevant_in(ws), "workspace listed an irrelevant panel");
            true
        }
        None => { tracing::warn!(?id, "workspace references an unregistered panel; skipping"); false }
    };
    ResolvedLayout {
        right_dock:  layout.right_dock.iter().copied().filter(keep_panel).collect(),
        bottom_tray: layout.bottom_tray.iter().copied().filter(keep_panel).collect(),
        primary_tools: layout.primary_tools,
        default_tool: layout.default_tool,
        status_items: layout.status_items,
    }
}
```

A missing shared panel (e.g. Animate referencing `layers` that sprite-edit failed to
register) is a loud `warn`, not a silent gap. `mem::take` of the panel registry - the
v2 take-and-reinsert lifeline - is explicitly not used (a panic between take and
reinsert drops the panel); the `&self` + disjoint-field pattern compiles directly.

## State and event model

### Owners, no overlap

| Layer | Lives in | Owned by | This round |
|---|---|---|---|
| Durable project state | `core` types | document | absent - core is a stub |
| Session state | `SessionState` | `Host` | present, minimal |
| UI state | `UiState` | `Host` | present |
| Panel scratch text | `Host.scratch` | `Host` | present (own field, mutable, disjoint) |
| Widget internals (scroll, focus, drag, collapse animation) | egui `Memory`, keyed by `Id` | egui | automatic |

Rule: session and UI state are our own plain structs in `Host`, never egui `Memory`;
`Memory` holds only widget internals it already manages by `Id`. Panel collapse lives
in `UiState` (not `CollapsingHeader`'s own memory) because the command palette and
future layout presets must read and set it. We never duplicate scroll offsets or
focus into our structs.

```rust
// state/session.rs
pub struct SessionState {
    pub active_workspace: WorkspaceId,
    pub active_tool: ToolId,
    pub dirty: bool,
    pub jobs: Vec<JobStub>,         // mock entries so the status dot / console have content
    pub ai_status: AiStatus,        // Ready | Working | Offline -> the status-bar dot
    // active_document / selection / undo_stack arrive with core. Slots reserved in doc comments.
}

// state/ui_state.rs
pub struct UiState {
    pub right_dock_width: f32,
    pub bottom_tray_height: f32,
    pub collapsed: HashMap<PanelId, bool>,
    pub tray_tab: HashMap<WorkspaceId, PanelId>,   // selected tray tab per workspace
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub grid: GridMode,
    pub onion_skin: bool,
    pub snap: bool,
    pub modal: Option<Modal>,        // CommandPalette | Confirm
    pub palette_query: String,
}

// state/mod.rs
pub struct Host {
    pub registries: Registries,
    pub state: ShellState,           // { session: SessionState, ui: UiState }
    pub intents: IntentSink,
    pub scratch: HashMap<PanelId, String>,   // panel-private draft text; mutable per-panel
    pub theme: Theme,                // here (not in PixhausApp) so apply_intent can re-apply
    pub bg: BackgroundChannel,       // mpsc receiver drained in logic(); empty this round
}
```

Durable prefs are a separate plain-types struct, not the live `UiState`:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Prefs {
    pub default_workspace: String,   // WorkspaceId's &'static str
    pub variant: ThemeVariant,
    pub accent: [u8; 3],
    pub dock_width: f32,
    pub tray_height: f32,
    pub grid: GridMode,
}
```

`eframe`'s `persistence` feature is enabled (`Cargo.toml`), so `App::save`/`new` can
round-trip `Prefs` via `eframe::set_value`/`get_value`. Wiring is deferred this round;
the struct is reserved and `serde`-ready (plain types only, no `egui::Vec2`/`Color32`).

### Intents and events

```rust
// state/intent.rs
pub enum Intent {
    SelectWorkspace(WorkspaceId),
    SelectTool(ToolId),
    SelectTrayTab(PanelId),         // applies to the active workspace's tray
    TogglePanelCollapsed(PanelId),
    SetGrid(GridMode),
    ToggleOnionSkin,
    ToggleSnap,
    SetZoom(f32),
    OpenCommandPalette,
    CloseModal,
    SetThemeVariant(ThemeVariant),  // View > Theme actually repaints (see apply path)
    RunAction(ActionId),            // mock: pushes a JobStub + emits an Event; NEVER mutates the model
    // Reserved, lands with core - the named command-path seam (bible rules 3, 4, 13):
    // Command(Box<dyn core::Command>),
}

pub enum Event {                    // bible 21.3: "something happened", distinct from a command
    WorkspaceChanged(WorkspaceId),
    ToolChanged(ToolId),
    ActionDispatched(ActionId),
}

#[derive(Default)]
pub struct IntentSink(Vec<Intent>);
impl IntentSink { pub fn push(&mut self, i: Intent) { self.0.push(i); } }
```

Invariant: `RunAction` never mutates project state. When `core` lands, any action
that applies a result or edits the model emits `Intent::Command`, and the `RunAction`
arm is restricted to non-mutating UI affordances (toasts, opening panels). `Event` is
produced only inside `apply_intent` (post-loop) and consumed only on the next frame;
this round it is a `tracing::debug!` sink. There is no intra-frame event bus - panels
never read events during render, so the no-panel-to-panel-coupling rule (bible 21.1)
holds and the borrow guarantee has no hole.

Theme application needs the `Context`, so `apply_intent` takes it and re-applies on a
variant change:

```rust
pub fn apply_intent(host: &mut Host, intent: Intent, ctx: &egui::Context) {
    match intent {
        Intent::SelectWorkspace(w) => { host.state.session.active_workspace = w;
            tracing::debug!(?w, "WorkspaceChanged"); }
        Intent::SelectTool(t) => { host.state.session.active_tool = t; }
        Intent::SelectTrayTab(p) => {
            let w = host.state.session.active_workspace;
            host.state.ui.tray_tab.insert(w, p);
        }
        Intent::TogglePanelCollapsed(p) => {
            let e = host.state.ui.collapsed.entry(p).or_insert(false); *e = !*e;
        }
        Intent::SetThemeVariant(v) => {
            host.theme = Theme::for_variant(v, host.theme.accent_seed());
            apply_to_visuals(&host.theme, ctx);   // re-apply so pixels actually change
        }
        Intent::RunAction(a) => {
            host.state.session.jobs.push(JobStub::queued(a));     // mock side effect only
            tracing::debug!(?a, "ActionDispatched");
        }
        // ... SetGrid / ToggleOnionSkin / ToggleSnap / SetZoom / OpenCommandPalette / CloseModal
    }
}
```

### The borrow-safe per-frame loop

The naive loop fails - you cannot hold `&mut registries`, `&mut state`, and `&mut ui`
at once. Three things make the real loop compile:

1. Panels get a read-only state view (`&SessionState`, `&UiState`) plus write channels
   (`&mut IntentSink`, and for scratch a `&mut String`).
2. `Panel::ui`/`Tool::options_ui` are `&self`, so iterating `&registry.panels` is a
   shared borrow that coexists with the sibling `&mut` fields.
3. Mutation is deferred past the loop: intents are drained and applied after all region
   borrows drop. The one-frame latency is invisible in immediate mode.

The load-bearing egui rule: split `host` into field bindings via reborrow-then-
destructure (`&mut *host`) before entering a `show_inside` closure; never let the
closure capture `host`.

```rust
// shell/runtime.rs
impl Shell {
    pub fn run(host: &mut Host, ui: &mut egui::Ui) {
        host.intents.0.clear();
        shortcuts::collect(ui.ctx(), &host.registries, &mut host.intents); // Cmd+1..5, tool keys, Cmd+K

        // egui panel order: outer panels first, CentralPanel LAST.
        regions::top_bar::show(host, ui);
        regions::tool_options::show(host, ui);
        regions::left_rail::show(host, ui);
        regions::status_bar::show(host, ui);      // outermost bottom - pins below the tray
        regions::bottom_tray::show(host, ui);
        regions::right_dock::show(host, ui);
        regions::canvas_stage::show(host, ui);     // CentralPanel - fills the rest
        command_palette::overlay(host, ui);        // Area on top if modal == CommandPalette

        // All region borrows dropped. Apply intents in push order.
        let intents = std::mem::take(&mut host.intents.0);
        for intent in intents { apply_intent(host, intent, ui.ctx()); }
    }
}
```

The right-dock region - the loop the verifier scrutinized - uses
reborrow-then-destructure, wraps each panel body in `ui.push_id(panel.id(), ...)` (so
colliding IDs across panels rendered at the same call site are impossible), and
reborrows `&mut intents`/`&mut scratch` per panel:

```rust
// shell/regions/right_dock.rs
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    // 1. Resolve ids by value FIRST - the &registries/&state borrows end at the semicolon.
    let ids = resolve_layout(host.state.session.active_workspace, &host.registries).right_dock;

    // 2. Reborrow-then-destructure into disjoint field bindings. Must be `&mut *host`,
    //    not `host` - a by-value field pattern on `&mut Host` is a move-out-of-borrow (E0507).
    let Host { registries, state, intents, scratch, theme, .. } = &mut *host;

    egui::Panel::right(region::region_id::RIGHT_DOCK)
        .resizable(true)
        .default_size(state.ui.right_dock_width)   // 0.34 Panel API: default_size, not default_width
        .show_inside(ui, |ui| {
            for id in ids {
                let Some(panel) = registries.panels.get(id) else { continue };
                let meta = panel.meta();
                let collapsed = state.ui.collapsed.get(&id).copied().unwrap_or(!meta.default_open);
                // The SHELL scopes ids - not the panel. Distinct call site per PanelId.
                ui.push_id(id, |ui| {
                    widgets::card(ui, theme, &meta, collapsed, |ui| {
                        let buf = scratch.entry(id).or_default();   // &mut String for this panel only
                        let mut scope = PanelScope {
                            ctx: ContribCtx {
                                session: &state.session,
                                ui_state: &state.ui,
                                theme,
                                intents: &mut *intents,            // reborrowed, not moved
                            },
                            id,
                            scratch: buf,
                        };
                        panel.ui(ui, &mut scope);
                    });
                });
            }
        });
}
```

`registries.panels.get(id)` is a shared borrow; `&state.session`/`&state.ui` are
shared borrows of a sibling field; `&mut *intents` and `scratch.entry(id)` are mutable
borrows of two further sibling fields, reborrowed each iteration. Provably disjoint, no
`RefCell`. The bottom-tray region resolves its tab `Vec`, renders a tab row (selectable
chips, active = accent pill, click -> `Intent::SelectTrayTab`), then renders the
selected tray panel through the same disjoint-field + `push_id` path. The tool-options
region uses the same split but builds a bare `ContribCtx` (no `PanelScope`, since a
tool is not a panel and has no scratch/id).

## Theme token system

A runtime `Theme` struct with semantic roles, dark-first, with light and accent
variants from the same role set. Accent is a separate axis (a seed color) so a future
preference can recolor independently of light/dark - the bible treats "Theme" and
"Accent color" as two preferences. Panels and regions ask for `theme.surfaces.panel`
or `theme.surface(SurfaceTier::Elevated)`, never a hex literal.

```rust
// theme/tokens.rs
pub struct Theme {
    pub variant: ThemeVariant,
    pub surfaces: Surfaces,
    pub roles: Roles,
    pub accent: AccentTokens,        // derived from a seed; the separable preference axis
    pub elevation: Elevation,
    pub spacing: Spacing,
    pub type_scale: TypeScale,
    pub radius: Radii,
}

#[derive(Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ThemeVariant { Dark, Light, AccentHighContrast }

#[derive(Copy, Clone)]
pub struct Surfaces {              // per-region tiers (UX 6.2), near-black warm slate
    pub app_frame: Color32,        // darkest - the app frame
    pub panel: Color32,            // dark charcoal/slate - panels, left rail, tray
    pub elevated: Color32,         // slightly lighter - cards, top bars
    pub stage: Color32,            // deepest neutral - behind the artboard
    pub inset: Color32,            // text fields, wells, HUD
}
#[derive(Copy, Clone)]
pub struct Roles {
    pub border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    pub success: Color32,          // muted green
    pub warning: Color32,          // muted amber
    pub error: Color32,            // muted red
}
#[derive(Copy, Clone)]
pub struct AccentTokens {          // all derived from one seed (default ~#7c6cef violet)
    pub seed: Color32,
    pub base: Color32,             // the accent
    pub hover: Color32,
    pub muted: Color32,            // low-alpha fill behind active tool/tab/row
    pub ai: Color32,               // sparkle marker tint (named for intent)
    pub ai_glow: Color32,          // softer halo behind AI affordances
}
#[derive(Copy, Clone)]
pub struct Elevation {             // shadow tiers (UX 6.2)
    pub raised: egui::epaint::Shadow,    // card Frames
    pub overlay: egui::epaint::Shadow,   // command palette / windows
    // The artboard "shadow" is painted manually (see canvas stage) - Shadow is not a paint primitive.
}
#[derive(Copy, Clone)]
pub struct Spacing { pub xs: f32, pub sm: f32, pub md: f32, pub lg: f32, pub xl: f32 } // 2,4,8,12,16
#[derive(Copy, Clone)]
pub struct TypeScale { pub label: f32, pub body: f32, pub section_header: f32, pub title: f32, pub mono: f32 } // 11,13,13,15,12
#[derive(Copy, Clone)]
pub struct Radii { pub sm: f32, pub md: f32 } // 2,3 - a production cockpit, not rounded mobile

pub enum SurfaceTier { AppFrame, Panel, Elevated, Stage, Inset }
```

### Variants and accent

```rust
// theme/palettes.rs
impl Theme {
    pub fn dark() -> Self { Self::for_variant(ThemeVariant::Dark, DEFAULT_ACCENT_SEED) }
    pub fn for_variant(v: ThemeVariant, accent_seed: Color32) -> Self { /* surfaces+roles per variant; accent derived from seed */ }
    pub fn accent_seed(&self) -> Color32 { self.accent.seed }
    pub fn surface(&self, t: SurfaceTier) -> Color32 { /* AppFrame|Panel|Elevated|Stage|Inset */ }
}
```

`light()` is `for_variant(Light, ...)`; `accent_high_contrast()` is
`for_variant(AccentHighContrast, ...)`. Only `dark()` is visually tuned this round.

### Token to egui Visuals (mapped once; re-applied on variant change)

```rust
// theme/mod.rs
pub fn apply_to_visuals(theme: &Theme, ctx: &egui::Context) {
    ctx.style_mut(|style| {                 // style_mut avoids cloning the whole style
        let v = &mut style.visuals;
        v.dark_mode          = theme.variant != ThemeVariant::Light;
        v.panel_fill         = theme.surfaces.panel;
        v.window_fill        = theme.surfaces.elevated;
        v.extreme_bg_color   = theme.surfaces.inset;       // text fields / wells
        v.faint_bg_color     = theme.surfaces.elevated;
        v.override_text_color = Some(theme.roles.text_primary);
        v.hyperlink_color    = theme.accent.base;
        v.selection.bg_fill  = theme.accent.muted;
        v.selection.stroke   = egui::Stroke::new(1.0, theme.accent.base);
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme.roles.border);
        v.widgets.hovered.bg_fill = theme.accent.muted;
        v.widgets.active.bg_fill  = theme.accent.base;
        v.window_shadow = theme.elevation.overlay;
        v.popup_shadow  = theme.elevation.overlay;
        style.spacing.item_spacing   = egui::vec2(theme.spacing.sm, theme.spacing.xs);
        style.spacing.button_padding = egui::vec2(theme.spacing.sm, theme.spacing.xs);
        style.text_styles.insert(egui::TextStyle::Body,      egui::FontId::proportional(theme.type_scale.body));
        style.text_styles.insert(egui::TextStyle::Heading,   egui::FontId::proportional(theme.type_scale.title));
        style.text_styles.insert(egui::TextStyle::Small,     egui::FontId::proportional(theme.type_scale.label));
        style.text_styles.insert(egui::TextStyle::Monospace, egui::FontId::monospace(theme.type_scale.mono));
    });
}
```

Confirm the exact `egui::epaint::Shadow` field shape against the pinned egui 0.34 API
(via the `pixhaus-egui` skill) during implementation; it is isolated to two tokens.

### Per-region tiers (UX 6.2)

`Visuals` carries one `panel_fill`, so each region paints an explicit
`egui::Frame { fill, .. }` with its tier rather than relying on the global.

| Region | Tier | Notes |
|---|---|---|
| App frame (root) | `app_frame` | darkest |
| Top bar / tool options / status bar | `elevated` | slightly raised |
| Left rail | `panel` | compact; active tool button paints `accent.muted` + 2px `accent.base` left line |
| Canvas stage backdrop | `stage` | deepest neutral |
| Artboard | transparent over checker | manually-painted drop shadow |
| Right-dock cards | `elevated` + `elevation.raised` | header + border |
| Bottom tray | `panel` | lighter than the frame so it reads as connected to the canvas |
| Active item | `accent.base` | tab/tool/row |
| AI affordance | `accent.ai` glyph + `accent.ai_glow` | the phosphor sparkle |

### Fonts and icons

`theme::fonts::install_fonts(ctx)` registers a UI sans for proportional text and a
mono for values, and merges the phosphor glyph ranges into `FontDefinitions` as a
fallback family so `crate::icons::*` resolve. The AI sparkle is `icons::SPARKLE` (a
phosphor glyph), used wherever `AccentTokens::ai` applies. No emoji literals anywhere
(egui's default fonts render emoji as tofu; phosphor private-use codepoints render
blank without the font). egui-phosphor is the ratified icon dependency. A high-quality
UI font (Geist/GeistMono pairing, if MIT-shippable) is a polish step; the foundation
may ship with egui's bundled font plus one bundled mono and layer the chosen font in
later - fonts are an asset decision, not architecture.

## Region composition and the shell runtime

The shell draws the seven regions every frame, outer first, central last. Content
comes from the active workspace's `WorkspaceLayout` resolved against the registries;
`app` declares nothing.

- Top bar (`Panel::top(region_id::TOP_BAR)`): three rows in one elevated frame. The
  menu strip is `ui.horizontal(|ui| for group in &registries.menus { ui.menu_button(group.label, |ui| { for item in &group.items { if ui.button(item.label).clicked() { intents.push(Intent::RunAction(item.action)); ui.close(); } } }) })`
  - the verified 0.34 idiom, dismissed with `ui.close()`, no `egui::menu::bar`. Then
  the workspace tab strip (iterate `registries.workspaces.iter()`, active = `accent.muted`
  pill + `accent.base` underline + brighter text, click -> `Intent::SelectWorkspace`).
  Then a thin global-status strip. Workspace tabs get the "real presence" UX 8.2 wants.
- Tool-options bar (`Panel::top(region_id::TOOL_OPTIONS)`): the active tool's
  `options_ui` via the bare-`ContribCtx` split. Content swaps with the active tool.
- Left rail (`Panel::left(region_id::LEFT_RAIL).resizable(false).exact_size(48.0)`):
  iterate `layout.primary_tools`, paint each via `widgets::tool_button`. Active =
  `accent.muted` bg + 2px `accent.base` left line + tooltip `"{label} ({shortcut})\n{tooltip}"`.
  AI Brush renders with `accent.ai` + sparkle. Click -> `Intent::SelectTool`.
- Status bar (`Panel::bottom(region_id::STATUS_BAR).exact_size(22.0)`, declared before
  the tray so it pins below it): `layout.status_items` + always-on items (size, zoom,
  grid) + the AI status dot colored from `session.ai_status` (UX 27). Earlier-declared
  bottom panels claim the lower edge, so status sits below the tray.
- Bottom tray (`Panel::bottom(region_id::BOTTOM_TRAY).resizable(true).default_size(state.ui.bottom_tray_height)`):
  a tab row built from the resolved `bottom_tray` Vec (selected = `tray_tab[active_ws]`
  or the first tab), then the selected tray panel rendered via the disjoint-field +
  `push_id` path. Tabs and content both swap per workspace.
- Right dock (`Panel::right(region_id::RIGHT_DOCK).resizable(true)`): the card-stack loop above.
- Canvas stage (`CentralPanel`, last) - `canvas_stage.rs`, where the existing seam is
  preserved exactly:
  1. Fill the central panel with `surfaces.stage`.
  2. Compute the artboard rect from `UiState.zoom`/`pan` (mock 64x64 default).
  3. Checkerboard behind the artboard for transparent bounds (`painter.rect_filled` tiles).
  4. Artboard drop shadow painted manually (an offset translucent dark rect) -
     `egui::epaint::Shadow` is not a paint primitive and cannot be `painter.add`-ed; it
     is reserved for card `Frame`s and the palette `Area`.
  5. Embed the renderer unchanged:
     `let (resp, painter) = ui.allocate_painter(artboard.size(), egui::Sense::click_and_drag()); painter.add(egui_wgpu::Callback::new_paint_callback(resp.rect, pixhaus_ui::CanvasCallback));`
     - exactly the current `app/src/main.rs` seam, now inside the framed artboard.
  6. Minor + major grid lines (8/16px per `UiState.grid`) as `painter` strokes over the
     callback rect.
  7. Floating HUD painted directly with the central panel's `Painter` at
     `stage_rect.left_bottom()` - `painter.rect_filled` for the `inset` chip +
     `painter.text` for `64 x 64   1600%   Grid 8px   Palette: Bit` (UX 10.3),
     `text_secondary`. Static content, so no `Area`/z-order/anchoring math.
- Command palette: an `egui::Area` (which takes `&Context` via `ui.ctx()`) drawn after
  the central panel so it floats above everything, with `elevation.overlay`.

`shell::drain_background(host, ctx)` (called from `App::logic`) owns the one
mpsc-drain front door: fold any channel results into `session.jobs`/`ai_status` and
`ctx.request_repaint()`. This round it is a structured no-op - an empty `try_recv` loop
with no sender - except one `JobStub` that flips `ai_status` Working->Ready to prove the
path (bible rule 5).

## Workspaces and placeholder panel inventory

All content is static/mock; every interactive control emits an `Intent`, edits its own
scratch buffer, or is inert. Panels render via `widgets::card`, `section_header`, and
`widgets::placeholder` helpers.

### Left-rail tools (one registry; workspaces pick the subset/order)

Pencil (B), Eraser (E), Fill (G), Line (L), Rectangle (U), Ellipse (O), Eyedropper (I),
Selection (M), Lasso (Q), Move (V), Transform (Shift+T), Text (X), Hand (H), Zoom (Z),
AI Brush (J, `is_ai`, sparkle). Draw/Animate/Tiles show the full set; Generate shows
`{Hand, Zoom, Selection, AI Brush}`; Export shows `{Hand, Zoom}`. All 15 are `Tool`
impls in `modules/sprite-edit/src/tools.rs` (the shared editing core, bible rule 2).

### Tool-options bar per tool (mock, UX 8.3)

- Pencil: `Size 1px - Opacity 255 - Pixel-perfect [x] - Dither None - Mirror X [ ] - Mirror Y [ ]`
- Eraser: `Size 1px - Opacity 255 - Pixel-perfect [x]`
- Fill: `Tolerance 0 - Contiguous [x] - All layers [ ]`
- Line/Rect/Ellipse: `Size 1px - Fill [ ] - From center [ ]`
- Selection: `Mode Replace - Feather 0 - Snap Pixel [x] - Origin Center`
- Eyedropper: `Sample Composite - Add to palette [ ]`
- Move/Transform: `Origin Center - Snap [ ]`
- Text: `Font Pixel - Size 8`
- Hand/Zoom: `Zoom 1600% - [Fit] [100%]`
- AI Brush: `[sparkle] Mode Fill - Use Palette [x] - Preserve Outline [x] - Variations 4 - Strength 0.65`

All controls are live egui widgets bound to throwaway local state inside the tool; they
move, they drive nothing.

### Per-workspace placement

| Workspace (shortcut) | Owned by | Right dock (top->bottom) | Bottom tray tabs (default first) | Workspace status items |
|---|---|---|---|---|
| Draw (Cmd+1, default) | sprite-edit | Layers, Sprites, Palette, Selection Actions, AI Assistant | Frames, Assets, Console | `Pixel Grid On` |
| Animate (Cmd+2) | animation | Layers, Sprites, Frames, Clip Properties, AI Animation Assistant | Timeline, Frames, Console | `15 frames`, `Onion Skin Off`, `12 FPS` |
| Tiles (Cmd+3) | tiles | Tileset, Rule Type, Material, Seam QA, AI Tile Assistant | Tile Variants, Assets, Console | `Tile 16px`, `Seams OK` |
| Generate (Cmd+4) | generation | Prompt, Recipe, Structure, Style, Palette Behavior, Advanced Settings | Results, History, Console | `dot AI Ready`, `Seed 123456` |
| Export (Cmd+5) | export | Export Format, Engine Preset, Animation Metadata, QA Warnings | Export Log, Console | `PNG + sheet`, `0 warnings` |

Shared panels are registered once by their owner and reused by id (bible rule 2):
sprite-edit registers Layers, Sprites, Palette, AI Assistant and the shared tray panels
Frames, Assets, and Console; animation registers Timeline; tiles registers Tile
Variants; generation registers Results and History; export registers Export Log.
Registration order is the contract: sprite-edit registers first, so Animate's layout can
reference `PanelId("layers")` / `PanelId("frames")` / `PanelId("console")` by id. A
missing shared panel warns loudly in `resolve_layout` rather than degrading silently.

### Panel mock content

- Layers: `+ New Layer`; rows Layer 3 / Layer 2 / Layer 1 / Background with eye + lock
  toggles, opacity slider, blend `Normal`; selected row = `accent.muted`.
- Sprites: grid of 6 mock sprite thumbnails (checkerboard rects), `+ New Sprite`.
- Palette: name `Bit`; 8x4 swatch grid; FG/BG indicator; `Ramp` / `Harmony` / `Reduce to palette`.
- Selection Actions: `Cut - Copy - Paste - Invert - Crop`, plus AI-marked `[sparkle] Fill - Clean up - Make seamless`.
- AI Assistant (and Animation/Tile variants): the UX 11.3 quick-action list - `Fill
  selection`, `Clean up`, `Reduce colors`, `Suggest ramp`, `Create variations`, `Remove
  background` - each pushes `Intent::RunAction` (mock toast + JobStub).
- Frames: horizontal strip of thumbnails `0..7`, add/duplicate/delete, current frame highlighted.
- Clip Properties: `Clip jump - Frames 8-15 - FPS 12 - Loop [ ] - Export name bit_jump`.
- Tileset / Rule Type / Material / Seam QA: 4x4 tile grid; radio `Single - Seamless -
  3x3 Autotile - 47-blob`; material chips; seam checklist with `success`/`error` badges
  (`OK Top - OK Left - WARN Bottom seam`).
- Prompt: multiline `TextEdit` bound to `scope.scratch` (the `&mut String` carve-out)
  with highlighted `{variable}` chips; `[sparkle] Generate` primary button (accent).
- Recipe / Structure / Style: card lists with mock preview thumbnails; recipes show
  built-in (locked) vs user badges.
- Palette Behavior: the UX 12.3 checkbox set (`Use current palette only [x]`, `Add colors automatically [ ]`, ...).
- Advanced Settings: collapsed by default - `Seed - Steps - Strength - Negative prompt - Model`.
- Results (tray): 8 mock result cards (number, seed, sparkle, star, selected = `accent`
  ring) + actions `Use selected - Insert as new sprite - Create variations - Generate more`.
- History (tray): a list of prior mock generations (prompt summary, seed, timestamp).
- Timeline (tray): the UX 13.3 four bands via `Painter` - Playback (`play prev next
  100ms 1.00x 12 FPS Loop [ ]`), Animation clips (`idle | walk | run | jump | attack`
  spans), Frame ruler (`0..14` with a violet playhead at frame 11), Layer tracks (`Body
  / Effects / Shadow`). Selected cell = violet outline. The Animate reference frame is the target.
- Tile Variants (tray): a row of mock tile patches + a seamless-tiling preview.
- Assets (tray): a thumbnail grid of mock project assets with category chips.
- Export Format / Engine Preset / Metadata / QA Warnings: radio `PNG - Spritesheet -
  GIF - APNG - JSON`; `Unity` highlighted, others listed as future; `Per-animation
  export [x] - Trim - Padding 2 - Pivot Center`; UX 28.3 checklist `OK All frames same
  size - OK Transparent bg - OK Palette < 32 - WARN "jump" does not loop - WARN 2
  missing animations` with `Fix - Ignore` actions.
- Export Log / Console (tray): scrolling mock log (`info backend ready`, `info project
  loaded`), monospace, `text_secondary`.

## Tooling stubs (command palette, shortcuts, menus, status bar)

### Menus (`menus.rs`, data-driven; modules contribute their groups)

`Pixhaus - File - Edit - Sprite - Layer - Frame - Select - View - Window - Help`. Each
is `MenuGroup { label, items: Vec<MenuItem { label, shortcut, action: ActionId }> }`
rendered via `ui.horizontal` + `ui.menu_button`. Most items push `Intent::RunAction`
(mock toast `"File > New (not yet wired)"`); a few work now: `View > Theme > Dark/Light/
Accent` -> `Intent::SetThemeVariant`, `View > Toggle Grid` -> `Intent::SetGrid`, `Window
> Command Palette (Ctrl/Cmd+K)` -> `Intent::OpenCommandPalette`. sprite-edit contributes
Sprite/Layer; animation contributes Frame; the shell owns the always-present groups.

### Command palette (Ctrl/Cmd+K) stub

`Intent::OpenCommandPalette` sets `UiState.modal = CommandPalette`. The overlay is an
`egui::Area` with `elevation.overlay`: a `TextEdit` bound to `palette_query` and a list
seeded from (a) every workspace (`Switch to {name}` - live, `SelectWorkspace`), (b)
every tool (`Select {tool}` - live), (c) registered `ActionDesc` + UX 20.2 examples
(`Generate sprite from prompt`, `Create variations`, `Reduce to palette`, `Make
seamless`, `Open Composition Library` - mock toast). Escape closes. Context-aware
ranking (UX 20.3) is a doc-comment TODO. Proves the registry-fed palette + modal +
intent path without core.

### Shortcuts (`shortcuts.rs`)

`shortcuts::collect` reads input once per frame with `consume_shortcut`/`consume_key` so
a focused `TextEdit` and the global handler do not both fire:

- Workspace switch: each workspace's `meta().shortcut` = `KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1..Num5)`
  -> `SelectWorkspace`. `COMMAND` resolves Ctrl-vs-Cmd cross-platform.
- Command palette: `KeyboardShortcut::new(Modifiers::COMMAND, Key::K)` -> `OpenCommandPalette`.
- Tool single-key shortcuts (B, E, G, ...): gated behind "is a text field focused" - if a
  text field has focus, tool keys are skipped so typing "b" in the prompt does not switch
  to Pencil. (Confirm the focus-query helper name against the pinned egui 0.34 API via the
  `pixhaus-egui` skill; `consume_key` also pre-empts a focused widget, as a fallback.)

All shortcuts route as intents - never direct mutation.

### Status bar

Compact strip (UX 27): always-on `size - zoom - grid` + the workspace's `status_items` +
an AI status dot colored from `session.ai_status` (`success`=Ready, `warning`=Working,
`text_disabled`=Offline) + a console toggle.

## Test plan

Test the mechanism, not the mock pixels. Headless (no GPU, no event loop), under `cargo
nextest`. `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]` at the
crate root so tests follow conventions.

1. Registry registration (`rstest`). Build `Registries`, run each module's `register`,
   assert expected ids present and unique, `iter()` preserves insertion order. A duplicate
   id `debug_assert`-panics in debug (test asserts the panic via `#[should_panic]`). One
   case per module - catches a dropped or collided panel.
2. Layout resolution -> `ResolvedLayout` (`insta` snapshots). Register the full module
   set, then snapshot each workspace's resolved layout (right-dock ids, tray-tab ids,
   primary tools, default tool, status items). Highest-value test - the regression v2 could
   not have. A moved panel or new workspace is a snapshot diff; an unregistered reference
   shows as a gap (and the `warn` fires). Must run with all five modules registered.
3. Intent application to state (`rstest`). Drive `apply_intent` directly (a headless
   `Context` where the theme path needs it): `SelectWorkspace`/`SelectTool` flip the
   session; `SelectTrayTab` updates the per-workspace tab; `TogglePanelCollapsed` flips the
   `UiState` map; `SetThemeVariant` swaps the variant; `OpenCommandPalette` sets the modal.
   The bible 21 event-model contract, and where the future `Command` variant gets its first test.
4. Theme token -> `Visuals` (`rstest`). `apply_to_visuals(Theme::dark(), ctx)` then assert
   `panel_fill == surfaces.panel`, `selection.stroke.color == accent.base`, etc. Assert all
   three variants populate every role (no default-black leak) and surface tiers are strictly
   ordered in lightness (app_frame < panel < elevated for dark). Plus WCAG contrast
   (`contrast.rs`): assert `wcag_contrast(text_primary, panel) >= 4.5`, `(text_secondary,
   panel) >= 4.5`, `(text_primary, elevated) >= 4.5`, `(text_primary, accent.muted) >= 3.0`
   - a pure function over the tokens, the cheapest place to enforce the accessibility ask.
5. Dyn-compatibility (compile-time). The `const _: () = { fn _assert_boxable(Box<dyn Panel>, ...) }`
   guard - if any trait regresses, the crate stops compiling. Free, permanent.
6. Shortcut mapping (`rstest`). Feed a synthetic `Key` + `Modifiers`, assert the emitted
   `Intent`, including that a tool key with a text field focused emits nothing. Pure key->intent fn.
7. One smoke test (headless, no egui_kittest). Boot the `Host`, assert `resolve_layout`
   succeeds and produces a non-empty right dock and a non-empty tray for all five
   workspaces, and the resolved top-bar tab set contains the five workspace names.
   (egui_kittest was declined; if it is later ratified, upgrade this to run one real frame
   per workspace asserting no panic.)

Not worth testing yet: panel body content, tool `options_ui` widgets, canvas chrome
geometry (manual-verify), the palette filter, the `CanvasCallback` GPU path (unchanged,
already size-asserted). `proptest` has nothing to bite on until pixel ops arrive.

## Open decisions for the human

1. Bottom tray - RESOLVED: multi-tab (`Vec<PanelId>` per workspace + a per-workspace
   selected-tab map). Reflected throughout this spec.
2. egui-phosphor - RESOLVED: ratified. Add to the `Cargo.toml` catalog; add a
   `pixhaus-egui-phosphor` skill as a follow-up.
3. egui_kittest - RESOLVED: declined. Test 7 stays headless.
4. `ContribCtx`/`PanelScope` shape vs the eventual document view. `session.active_document`
   is a reserved slot; the read-view type is a guess until `core` exists. The chosen seam
   keeps this round's code honest (panels are UI concerns today). Revisit only if the
   priority is exercising the cross-crate `core -> module -> panel` read path before `core`
   has types - the cost is low but the underlying data would still be fake. Default: leave as
   specified.
5. Preferences persistence is deferred (struct reserved, `App::save`/restore not wired). The
   `persistence` feature is on, so this is a small follow-up. Confirm the desired prefs set
   (default workspace, theme variant, accent seed, dock/tray sizes, grid) before wiring.
   Default: as the `Prefs` struct above.

## Risks

- Dyn-compatibility regression. Adding a generic method or a `-> Self` to any registry trait
  silently breaks `Box<dyn _>`. Mitigated by the compile-time `const _` guard (test 5), which
  fails the build immediately.
- Registration-order coupling. Animate reusing sprite-edit's shared panels by id means
  sprite-edit must register first. Mitigated by the loud `warn` in `resolve_layout` and the
  full-set requirement on snapshot test 2; the ordering is documented in `build_host`. If this
  proves fragile as modules grow, promote shared panels to a base module registered ahead of
  all workspaces.
- The scratch `&mut String` carve-out. The one place "intents-only" purity is broken. Contained:
  per-panel, disjoint, only `TextEdit` uses it. Risk is a future panel routing real mutation
  through scratch instead of an intent - guarded by code review and the doc-comment on
  `PanelScope::scratch` naming the exception.
- Pinned egui 0.34 API specifics flagged "verify": the `egui::epaint::Shadow` field shape and
  the text-field focus-query helper name. Confirm against the `pixhaus-egui` skill before
  depending on them; both are isolated (two tokens, one shortcut gate) and cheap to adjust.
- Theme owner placement. `Theme` must live in `Host` for `apply_intent` to re-apply on a
  variant change; if a later refactor moves it back into `PixhausApp`, "View > Theme" silently
  stops repainting. Guarded by test 3 asserting the variant swap and by keeping `Theme` a `Host` field.
- `ui` crate scope creep. Keeping concrete bodies in `modules/*` (not `ui`) is the discipline
  that prevents `ui` becoming the v2 god-object. `widgets/` holds only shared helpers; a
  panel/tool/workspace impl placed there should fail review.
