//! UI state: layout, view, and modal flags the shell owns directly.
//!
//! This is our own plain struct, never egui `Memory`. Panel collapse lives here
//! (not in `CollapsingHeader`'s own memory) because the command palette and future
//! layout presets must read and set it (spec "Owners, no overlap"). Scroll offsets
//! and focus are NOT duplicated here - egui owns those.

use std::collections::HashMap;

use pixhaus_core::ClipId;
use pixhaus_core::codex::{AnchorStrength, CodexEntryId, CodexFolderId, CoverageTemplateId};
use pixhaus_services::codex::CompiledPrompt;

use crate::contrib_api::ids::{PanelId, WorkspaceId};

/// Mutable, non-durable UI state owned by [`crate::state::Host`].
pub struct UiState {
    /// Left-dock width in points (resizable by the user). Used by workspaces with a
    /// left dock (the Codex Navigator); the canvas workspaces have no left dock.
    pub left_dock_width: f32,
    /// Right-dock width in points (resizable by the user).
    pub right_dock_width: f32,
    /// Bottom-tray height in points (resizable by the user).
    pub bottom_tray_height: f32,
    /// Per-panel collapse flag. Absent key means "use the panel's `default_open`".
    pub collapsed: HashMap<PanelId, bool>,
    /// Selected tray tab per workspace. Absent key means "the first tab".
    pub tray_tab: HashMap<WorkspaceId, PanelId>,
    /// Canvas zoom: the true scale in screen points per sprite pixel. `1.0` is a
    /// literal 1:1 (an honest 100%); the canvas auto-fits on the first frame and when
    /// the active sprite's dimensions change, so a small sprite is never a speck.
    pub zoom: f32,
    /// Canvas pan offset in points (the sprite's displacement from the stage center).
    pub pan: egui::Vec2,
    /// The active sprite dimensions the canvas last auto-fit for. `None` until the
    /// first fit, and reset to `None` by a "fit to window" request. The canvas re-fits
    /// whenever this differs from the active sprite's size, so opening or switching to a
    /// differently-sized sprite re-fits while a manual zoom (same dimensions) is kept.
    pub last_fit_size: Option<(u32, u32)>,
    /// Pixel-perfect zoom mode: when set, zoom snaps to whole points-per-pixel steps
    /// (and unit fractions below 1x) so cells stay even; when clear, zoom is continuous
    /// for non-pixel art styles. Later drivable by the document's art mode.
    // Two zoom modes behind a user-switchable toggle, not integer-snap everywhere:
    // Pixhaus is multi-style, so non-pixel art must not be forced into integer/unit-
    // fraction snapping. Pixel-perfect snaps so cells stay even; clear it for
    // continuous zoom. The camera math itself stays egui-level in canvas/view.rs.
    pub pixel_perfect_zoom: bool,
    /// Active grid spacing mode.
    pub grid: GridMode,
    /// Onion-skin toggle (Animate).
    pub onion_skin: bool,
    /// Pixel-snap toggle.
    pub snap: bool,
    /// The open modal overlay, if any.
    pub modal: Option<Modal>,
    /// The startup splash phase. Begins `Active` and advances to `Done` once.
    pub splash: SplashPhase,
    /// Live text in the command-palette search field.
    pub palette_query: String,
    /// Transient animation-playback state (Animate workspace).
    pub playback: PlaybackState,
    /// Transient Codex-workspace UI state: the center mode, the selected entry, the
    /// Navigator search query, and the generation context stack. The shell owns it; the
    /// session-side [`CodexView`](crate::state::session::CodexView) mirror is rebuilt from
    /// it plus the document each frame.
    pub codex: CodexUi,
}

/// The center surface a Codex workspace shows. The center panel switches on this; the
/// bottom strip switches `Coverage`/`Test` views on it too. Plain data so it can later
/// persist.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CodexMode {
    /// The entry editor with the navigator-selected entry's fields (the default).
    #[default]
    Edit,
    /// The Navigator-led browse view: entry cards, no editor focus.
    Browse,
    /// The visual board: thumbnails, references, generated examples.
    Board,
    /// The relationship graph between entries.
    Graph,
    /// The coverage checklist for the selected entry.
    Coverage,
    /// The in-workspace test-generation view.
    Test,
}

/// The center detail tab for the selected entry (the production-cockpit tab bar).
///
/// Distinct from [`CodexMode`], which still drives the bottom Board/Graph/Coverage
/// strip's view toggle. This is the per-entry detail view in the center: a rich
/// entry page split into tabs, not the workspace-wide mode.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum CodexDetailTab {
    /// The multi-card overview grid (the default).
    #[default]
    Overview,
    /// Key visual, palette, silhouette, and generation readiness.
    Visual,
    /// The anchor editor: positive/negative rules and per-anchor strength.
    Anchors,
    /// The prompt composer: fragments and the compiled preview.
    Prompt,
    /// The per-slot coverage cards.
    Coverage,
    /// Outgoing/incoming relationships, as a list or a graph.
    Relations,
    /// The version-history timeline.
    History,
}

/// A Navigator smart filter, set by clicking a COLLECTIONS row. Narrows the entry
/// list to entries that match a derived condition. Plain data the shell owns.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum NavFilter {
    /// No filter: show every entry (the default).
    #[default]
    All,
    /// Only entries with an incomplete coverage report.
    MissingCoverage,
    /// Only entries that resolve at least one broken `@`-reference.
    BrokenReferences,
}

/// One pinned reference in the generation context stack: the entry and the strength
/// the compiler should weight it at. Plain data the shell owns.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ContextRef {
    /// The pinned Codex entry.
    pub entry: CodexEntryId,
    /// The strength the compiler weights this reference at.
    pub strength: AnchorStrength,
}

/// Transient Codex-workspace UI state owned by [`UiState`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodexUi {
    /// The active center mode (editor / board / graph / coverage / test).
    pub mode: CodexMode,
    /// The active center detail tab for the selected entry (the cockpit tab bar).
    pub detail_tab: CodexDetailTab,
    /// The active Navigator smart filter (none / missing-coverage / broken-refs).
    pub nav_filter: NavFilter,
    /// The Navigator-selected entry, if any. The editor and inspector read it.
    pub selected: Option<CodexEntryId>,
    /// Live text in the Navigator search field; drives the suggestion list.
    pub search: String,
    /// The generation context stack: entries pinned as references, with strengths.
    pub context: Vec<ContextRef>,
    /// The latest compiled-prompt preview from the Codex test view, owned by the shell
    /// (a `CompileCodexPrompt` intent sets it; the session mirror clones it). `None`
    /// until the user compiles one.
    pub compiled: Option<CompiledPrompt>,
    /// The folder the Navigator is filing a new folder under, when the "new folder"
    /// affordance is open; `None` means the codex root.
    pub new_folder_parent: Option<CodexFolderId>,
    /// The coverage-editor scratch: the in-progress new-template name, new-slot label,
    /// the template selected for editing, and the per-entry add-custom-slot text. Plain
    /// session UI state, never the model.
    pub coverage_draft: CoverageEditorDraft,
}

/// Scratch for the coverage editor: the buffers behind the "new template", "add slot",
/// and "add custom slot" affordances, plus which project template the slot editor is
/// focused on. Plain owned data the shell owns; the coverage panel reads it through the
/// session mirror and edits its `TextEdit` buffers through the `PanelScope.scratch`
/// carve-out. Holds no model state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoverageEditorDraft {
    /// The template currently open in the slot editor, or `None` when none is selected.
    pub selected_template: Option<CoverageTemplateId>,
    /// Add-field buffer for a new project template's name.
    pub new_template_name: String,
    /// Add-field buffer for a new slot's label in the selected template.
    pub new_slot_label: String,
    /// Add-field buffer for a per-entry custom slot's label.
    pub new_custom_slot_label: String,
}

/// A structured, per-selection editor draft the shell owns and the Codex editor panel
/// edits in place.
///
/// Each field mirrors one editable facet of the selected entry. The draft is reloaded
/// from the entry whenever the selection changes (`loaded_id` no longer matches the
/// selection), so editing entry A then selecting B shows B's values, and edits commit
/// to the right entry. The editor commits a field by diffing the draft against the
/// detail snapshot on lost-focus and pushing the matching intent. List fields
/// (aliases, tags, prompt fragments) are edited through the inspector's `editable_list`,
/// which reads the current values straight off the detail snapshot and commits each
/// add/remove as its own intent. Plain owned data; no egui types.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodexEditorDraft {
    /// The entry the draft was last loaded from. `None` before the first load; when it
    /// differs from the selection the shell reloads the draft.
    pub loaded_id: Option<CodexEntryId>,
    /// Display name.
    pub name: String,
    /// Primary `@`-handle, without the leading `@`.
    pub handle: String,
    /// Free-text description.
    pub description: String,
    /// Lore / backstory.
    pub lore: String,
    /// Visual-identity text.
    pub visual_description: String,
    /// Add-field buffer for the alias editable-list.
    pub alias_add: String,
    /// Add-field buffer for the tag editable-list.
    pub tag_add: String,
    /// Add-field buffer for the prompt-fragment editable-list.
    pub fragment_add: String,
    /// Add-field buffer for the negative-fragment editable-list.
    pub negative_add: String,
    /// Whether the rename field (name + handle) is open in the editor.
    pub renaming: bool,
}

impl CodexEditorDraft {
    /// Reload every field from `detail`, stamping `loaded_id` so the shell does not
    /// reload again until the selection changes. Called from the shell when the
    /// selection differs from `loaded_id`.
    pub fn load_from(&mut self, detail: &crate::state::session::CodexEntryDetail) {
        self.loaded_id = Some(detail.summary.id);
        self.name.clone_from(&detail.summary.name);
        self.handle.clone_from(&detail.summary.handle);
        self.description.clone_from(&detail.description);
        self.lore.clone_from(&detail.lore);
        self.visual_description.clone_from(&detail.visual_description);
        // Transient draft-UI buffers reset on a selection change.
        self.alias_add.clear();
        self.tag_add.clear();
        self.fragment_add.clear();
        self.negative_add.clear();
        self.renaming = false;
    }
}

/// Transient animation-playback state. View-only: the canvas renders the
/// playhead-selected frame and the document's `active_frame` is never touched, so
/// nothing here bumps the document revision or emits a command (no undo pollution).
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct PlaybackState {
    /// Whether the clip is advancing. `false` = paused/stopped (the playhead freezes).
    pub playing: bool,
    /// Seconds elapsed within the active range since playback's logical origin. The
    /// frame index is DERIVED from this (`seconds * fps`), so the canvas and the
    /// timeline read one scalar and cannot drift. Reset to 0 on Stop.
    pub playhead_seconds: f32,
    /// The clip whose range is playing. `None` = the sprite's first clip, or the
    /// implicit "all frames" range when the sprite has no clips.
    pub clip: Option<ClipId>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            left_dock_width: 240.0,
            right_dock_width: 280.0,
            bottom_tray_height: 200.0,
            collapsed: HashMap::new(),
            tray_tab: HashMap::new(),
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            last_fit_size: None,
            pixel_perfect_zoom: true,
            grid: GridMode::default(),
            onion_skin: false,
            snap: true,
            modal: None,
            splash: SplashPhase::default(),
            palette_query: String::new(),
            playback: PlaybackState::default(),
            codex: CodexUi::default(),
        }
    }
}

/// The startup splash phase. Not serialized: the splash shows once per launch and the
/// timestamp is a frame-clock value, meaningless across sessions.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum SplashPhase {
    /// The splash is showing. `since` is the `ctx.input(|i| i.time)` seconds the splash
    /// first painted, stamped once; `None` until the first active frame stamps it.
    Active {
        /// Frame-clock seconds at which the splash first painted, or `None` pre-stamp.
        since: Option<f64>,
    },
    /// The splash has been dismissed (timed out or skipped).
    Done,
}

impl Default for SplashPhase {
    fn default() -> Self {
        Self::Active { since: None }
    }
}

/// Canvas grid spacing. Plain data (no egui types) so [`crate::state::Prefs`] can
/// serialize it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GridMode {
    /// No grid drawn.
    Off,
    /// 8px minor grid. The default.
    #[default]
    Px8,
    /// 16px major grid.
    Px16,
}

/// A modal overlay covering the shell.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Modal {
    /// The Ctrl/Cmd+K command palette.
    CommandPalette,
    /// A yes/no confirmation prompt.
    Confirm,
    /// The About Pixhaus dialog (wordmark, version, license).
    About,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ui_state_has_no_modal_and_unit_zoom() {
        let ui = UiState::default();
        assert!(ui.modal.is_none(), "nothing is modal on a fresh session");
        assert_eq!(ui.zoom, 1.0, "default zoom is a literal 1:1");
        assert!(ui.last_fit_size.is_none(), "no fit recorded until the first frame");
        assert!(ui.pixel_perfect_zoom, "pixel-perfect zoom is the default mode");
        assert!(ui.collapsed.is_empty(), "no panel overrides by default");
        assert!(ui.tray_tab.is_empty(), "no tray-tab overrides by default");
    }

    #[test]
    fn default_splash_is_active_and_unstamped() {
        let ui = UiState::default();
        assert_eq!(
            ui.splash,
            SplashPhase::Active { since: None },
            "the splash starts active and unstamped on a fresh session",
        );
    }

    #[test]
    fn default_grid_mode_is_eight_px() {
        assert_eq!(GridMode::default(), GridMode::Px8, "default grid is the 8px minor grid");
    }

    #[test]
    fn default_codex_detail_tab_is_overview_and_filter_is_all() {
        let ui = UiState::default();
        assert_eq!(ui.codex.detail_tab, CodexDetailTab::Overview, "the cockpit opens on the overview tab");
        assert_eq!(ui.codex.nav_filter, NavFilter::All, "no smart filter is active on a fresh session");
    }

    #[test]
    fn default_playback_is_stopped_at_zero() {
        let ui = UiState::default();
        assert!(!ui.playback.playing, "playback starts stopped");
        assert_eq!(ui.playback.playhead_seconds, 0.0, "the playhead starts at zero");
        assert!(ui.playback.clip.is_none(), "no clip selected on a fresh session");
    }
}
