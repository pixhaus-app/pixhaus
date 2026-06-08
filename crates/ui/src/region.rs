//! Window regions and their stable egui [`Id`](egui::Id) source strings.
//!
//! The shell draws seven regions every frame (architecture bible section 8). The
//! [`Region`] enum names them; [`region_id`] holds the stable id source strings
//! each `egui` side/top/bottom panel needs so its layout memory survives across
//! frames. Only the registry-fed regions (`LeftRail`, `RightDock`, `BottomTray`)
//! are populated from the registries; the rest are shell chrome.

/// The seven window regions the shell composes each frame.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Region {
    /// Shell chrome: menus + workspace tabs + global status.
    TopBar,
    /// Driven by the active [`Tool`](crate::contrib_api::Tool), not the panel registry.
    ToolOptions,
    /// Filled from the tool registry, workspace-filtered.
    LeftRail,
    /// Filled from the panel registry: a left-edge card stack inboard of the rail
    /// (the Codex Navigator). Empty for the canvas workspaces.
    LeftDock,
    /// Shell chrome: the canvas stage, or a full-center registered panel.
    Center,
    /// Filled from the panel registry: a top-to-bottom card stack.
    RightDock,
    /// Filled from the panel registry: a tab row plus the selected panel.
    BottomTray,
    /// Shell chrome plus the active workspace's status items.
    StatusBar,
}

/// Stable id source strings for the regions egui draws as panels.
///
/// Each `egui` panel needs a unique stable id so its size and scroll memory
/// persist across frames. Three chrome regions (top bar, tool options, status bar)
/// and three registry-fed regions (left tool rail, right dock, bottom tray) each
/// get one. The center canvas stage and the left dock draw without a stable
/// `region_id` here.
pub mod region_id {
    /// Id source for the top bar panel.
    pub const TOP_BAR: &str = "pixhaus.topbar";
    /// Id source for the tool-options panel.
    pub const TOOL_OPTIONS: &str = "pixhaus.tooloptions";
    /// Id source for the left tool rail panel.
    pub const LEFT_RAIL: &str = "pixhaus.rail";
    /// Id source for the right dock panel.
    pub const RIGHT_DOCK: &str = "pixhaus.dock";
    /// Id source for the bottom tray panel.
    pub const BOTTOM_TRAY: &str = "pixhaus.tray";
    /// Id source for the status bar panel.
    pub const STATUS_BAR: &str = "pixhaus.status";
}

#[cfg(test)]
mod tests {
    use super::region_id;

    /// Every region id source string must be distinct, or two egui panels share
    /// layout memory and one silently inherits the other's size.
    #[test]
    fn region_ids_are_unique() {
        let ids = [
            region_id::TOP_BAR,
            region_id::TOOL_OPTIONS,
            region_id::LEFT_RAIL,
            region_id::RIGHT_DOCK,
            region_id::BOTTOM_TRAY,
            region_id::STATUS_BAR,
        ];
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), ids.len(), "region id source strings must be unique");
    }
}
