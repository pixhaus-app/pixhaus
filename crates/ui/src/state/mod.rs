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

use self::intent::IntentSink;
use self::session::{AiStatus, SessionState};
use self::ui_state::{GridMode, UiState};

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
    /// The active theme; owned here so a variant change can re-apply to visuals.
    pub theme: Theme,
    /// Background results drained in `App::logic`. Empty this round.
    pub bg: BackgroundChannel,
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
                },
                ui: UiState::default(),
            },
            intents: IntentSink::default(),
            scratch: HashMap::new(),
            theme: *theme,
            bg: BackgroundChannel::default(),
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
}
