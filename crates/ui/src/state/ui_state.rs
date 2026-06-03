//! UI state: layout, view, and modal flags the shell owns directly.
//!
//! This is our own plain struct, never egui `Memory`. Panel collapse lives here
//! (not in `CollapsingHeader`'s own memory) because the command palette and future
//! layout presets must read and set it (spec "Owners, no overlap"). Scroll offsets
//! and focus are NOT duplicated here - egui owns those.

use std::collections::HashMap;

use pixhaus_core::ClipId;

use crate::contrib_api::ids::{PanelId, WorkspaceId};

/// Mutable, non-durable UI state owned by [`crate::state::Host`].
pub struct UiState {
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
    fn default_playback_is_stopped_at_zero() {
        let ui = UiState::default();
        assert!(!ui.playback.playing, "playback starts stopped");
        assert_eq!(ui.playback.playhead_seconds, 0.0, "the playhead starts at zero");
        assert!(ui.playback.clip.is_none(), "no clip selected on a fresh session");
    }
}
