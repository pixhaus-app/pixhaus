//! UI state: layout, view, and modal flags the shell owns directly.
//!
//! This is our own plain struct, never egui `Memory`. Panel collapse lives here
//! (not in `CollapsingHeader`'s own memory) because the command palette and future
//! layout presets must read and set it (spec "Owners, no overlap"). Scroll offsets
//! and focus are NOT duplicated here - egui owns those.

use std::collections::HashMap;

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
    /// Canvas zoom factor (mock; 1.0 == 100%).
    pub zoom: f32,
    /// Canvas pan offset in points.
    pub pan: egui::Vec2,
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
            grid: GridMode::default(),
            onion_skin: false,
            snap: true,
            modal: None,
            splash: SplashPhase::default(),
            palette_query: String::new(),
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
        assert_eq!(ui.zoom, 1.0, "default zoom is 100%");
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
}
