//! Session, UI, and intent state, and the [`Host`] that owns all three.
//!
//! Ownership map (spec "Owners, no overlap"): durable project state will live in
//! `core` (absent this round); session and UI state are plain structs owned by
//! [`Host`], never egui `Memory`. egui `Memory` holds only widget internals keyed
//! by `Id`.

use std::collections::HashMap;
use std::sync::mpsc;

use crate::contrib_api::ids::{PanelId, ToolId, WorkspaceId};
use crate::registry::{RegistrarWrapper, Registries};
use crate::theme::{Theme, ThemeVariant};

use self::edit_session::EditSession;
use self::intent::IntentSink;
use self::session::{AiStatus, SessionState};
use self::ui_state::{GridMode, UiState};

pub mod edit_session;
pub mod intent;
pub mod session;
pub mod ui_state;

/// A message a background task hands back to the egui loop. Empty surface this round
/// (no senders beyond the bootstrap one); the variants grow as `services` lands.
#[derive(Debug)]
pub enum BackgroundMsg {
    /// A (mock) job changed AI status. Proves the drain path (spec bible rule 5).
    AiStatusChanged(AiStatus),
}

/// The receiver end the egui loop drains in `App::logic`, plus the sender it keeps
/// alive so the channel never disconnects while idle.
pub struct BackgroundChannel {
    /// Drained once per frame in `shell::drain_background`.
    pub rx: mpsc::Receiver<BackgroundMsg>,
    /// Held so `rx` stays connected; handed to background tasks when `services` lands.
    pub tx: mpsc::Sender<BackgroundMsg>,
}

impl Default for BackgroundChannel {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { rx, tx }
    }
}

/// Session + UI state grouped under one owner. `apply_intent` mutates through this.
pub struct ShellState {
    /// Non-durable session model.
    pub session: SessionState,
    /// Layout/view/modal state.
    pub ui: UiState,
}

/// The single owner of every piece of shell-level mutable state.
///
/// `Theme` lives here (not in the eframe `App`) so `apply_intent` can re-apply it on
/// a variant change (spec "Theme owner placement" risk). `scratch` is the one
/// per-panel mutable carve-out `TextEdit` requires.
pub struct Host {
    /// All registered capabilities (panels, tools, workspaces, actions, menus).
    pub registries: Registries,
    /// Session + UI state.
    pub state: ShellState,
    /// The write channel drained after each frame.
    pub intents: IntentSink,
    /// Panel-private draft text, keyed by panel id; mutable per-panel.
    pub scratch: HashMap<PanelId, String>,
    /// The structured Codex editor draft for the selected entry, a sibling of `scratch`
    /// (not inside `UiState`, so the center-stage region can borrow it mutably while
    /// holding a shared borrow of the rest of the state). Reloaded from the selection
    /// in `sync_codex_view`; the Codex Entry Editor binds its `TextEdit`s to it and
    /// commits diffs as `Intent`s.
    pub codex_draft: self::ui_state::CodexEditorDraft,
    /// The active theme; owned here so a variant change can re-apply to visuals.
    pub theme: Theme,
    /// Background results drained in `App::logic`.
    pub bg: BackgroundChannel,
    /// The live document and the services that act on it (command/undo, jobs,
    /// providers, result store). Mutated only in `apply_intent`/`drain_background`/
    /// `canvas_stage`, never borrowed into a `&self` panel.
    pub edit: EditSession,
}

/// The default initial workspace (Draw) and tool (Pencil). The strings are the
/// ids the modules register by; see the per-workspace placement table.
const DEFAULT_WORKSPACE: WorkspaceId = WorkspaceId("draw");
const DEFAULT_TOOL: ToolId = ToolId("pencil");

impl Host {
    /// Build a host with empty registries and the default initial state.
    ///
    /// `theme` is taken by reference and copied in: the token set is a large `Copy`
    /// value, so passing it by reference avoids a by-value move clippy flags, while
    /// `Host` still owns its own copy. Registration happens afterward through
    /// [`Host::registrar`]: each module's `register` is the only path a capability
    /// enters the shell.
    pub fn new(theme: &Theme) -> Self {
        Self {
            registries: Registries::default(),
            state: ShellState {
                session: SessionState {
                    active_workspace: DEFAULT_WORKSPACE,
                    active_tool: DEFAULT_TOOL,
                    dirty: false,
                    jobs: Vec::new(),
                    ai_status: AiStatus::Ready,
                    result_count: 0,
                    selected_result: None,
                    result_kinds: Vec::new(),
                    last_prompt: String::new(),
                    playback: self::session::PlaybackMirror::default(),
                    codex: self::session::CodexView::default(),
                    last_error: None,
                },
                ui: UiState::default(),
            },
            intents: IntentSink::default(),
            scratch: HashMap::new(),
            codex_draft: self::ui_state::CodexEditorDraft::default(),
            theme: *theme,
            bg: BackgroundChannel::default(),
            edit: EditSession::default(),
        }
    }

    /// The registrar a module registers capabilities through. Borrows the registries
    /// mutably for the duration of registration.
    pub fn registrar(&mut self) -> RegistrarWrapper<'_> {
        self.registries.registrar()
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
}

/// Durable preferences, round-tripped via eframe persistence (wiring deferred this
/// round, spec open decision 5). Plain types only - no `egui::Vec2`/`Color32` - so
/// `serde` derives cleanly and the format stays toolkit-independent.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct Prefs {
    /// The `WorkspaceId`'s `&'static str` to open on launch.
    pub default_workspace: String,
    /// The theme variant.
    pub variant: ThemeVariant,
    /// The accent seed as RGB bytes. The live seed is a `Color32` (RGBA); this
    /// stores RGB only. When persistence is wired (deferred), the conversion must
    /// deliberately drop the seed's alpha on save and restore it on load - the
    /// accent seed is opaque, so reconstruct the `Color32` with full alpha
    /// (`Color32::from_rgb`), not a default-zero one.
    pub accent: [u8; 3],
    /// Right-dock width in points.
    pub dock_width: f32,
    /// Bottom-tray height in points.
    pub tray_height: f32,
    /// The grid mode.
    pub grid: GridMode,
    /// The persisted UI language (e.g. "en", "es"). `None` follows the OS language
    /// the binary detects at boot. Applied once Prefs persistence and an in-app
    /// language picker land (deferred, spec open decision 5); `#[serde(default)]`
    /// keeps older persisted prefs loadable.
    #[serde(default)]
    pub language: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_host_starts_in_draw_with_pencil() {
        let host = Host::new(&Theme::dark());
        assert_eq!(host.state.session.active_workspace, WorkspaceId("draw"), "the default workspace is Draw");
        assert_eq!(host.state.session.active_tool, ToolId("pencil"), "Draw's default tool is Pencil");
    }

    #[test]
    fn new_host_has_no_jobs_and_ready_ai() {
        let host = Host::new(&Theme::dark());
        assert!(host.state.session.jobs.is_empty(), "no jobs queued at boot");
        assert_eq!(host.state.session.ai_status, AiStatus::Ready, "AI starts Ready");
    }

    #[test]
    fn new_host_theme_variant_matches_argument() {
        let host = Host::new(&Theme::dark());
        assert_eq!(host.theme().variant, ThemeVariant::Dark, "the host holds the theme it was built with");
    }

    /// `Prefs` round-trips through JSON unchanged. Asserted field-by-field because
    /// `Prefs` does not derive `PartialEq`, and the accent is stored as RGB only.
    #[test]
    fn prefs_round_trips_through_json() {
        let prefs = Prefs {
            default_workspace: "animate".to_owned(),
            variant: ThemeVariant::Light,
            accent: [0x7c, 0x3a, 0xed],
            dock_width: 312.0,
            tray_height: 180.0,
            grid: GridMode::Px16,
            language: Some("es".to_owned()),
        };
        let Ok(json) = serde_json::to_string(&prefs) else {
            panic!("Prefs serializes to JSON");
        };
        let Ok(back) = serde_json::from_str::<Prefs>(&json) else {
            panic!("the serialized Prefs deserializes");
        };
        assert_eq!(back.default_workspace, prefs.default_workspace);
        assert_eq!(back.variant, prefs.variant);
        assert_eq!(back.accent, prefs.accent);
        assert_eq!(back.dock_width, prefs.dock_width);
        assert_eq!(back.tray_height, prefs.tray_height);
        assert_eq!(back.grid, prefs.grid);
        assert_eq!(back.language, prefs.language);
    }

    /// Older persisted prefs that predate the `language` field still load: the
    /// `#[serde(default)]` fills it with `None`.
    #[test]
    fn prefs_without_language_defaults_to_none() {
        let json = r#"{
            "default_workspace": "draw",
            "variant": "Dark",
            "accent": [124, 58, 237],
            "dock_width": 300.0,
            "tray_height": 160.0,
            "grid": "Off"
        }"#;
        let Ok(prefs) = serde_json::from_str::<Prefs>(json) else {
            panic!("a payload missing `language` deserializes");
        };
        assert_eq!(prefs.language, None, "an omitted language defaults to None (follow the OS)");
    }
}
