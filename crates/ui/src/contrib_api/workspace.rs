//! The `Workspace` trait and its layout types.
//!
//! A workspace owns layout only - which registered panels and tools fill which
//! region - and never owns data (bible: Draw and Animate are siblings over one
//! sprite-editing core). It is dyn-compatible (`Box<dyn Workspace>`).
//!
//! [`WorkspaceLayout`] and [`StatusItem`] derive `Clone + PartialEq + Debug` so
//! the resolved layout is insta-snapshottable - the registry layer's
//! highest-value regression test.

use crate::contrib_api::ids::{MsgKey, PanelId, ToolId, WorkspaceId};

/// A registered workspace: identity, metadata, and a pure layout function.
///
/// # Object safety
///
/// `&self`, no generics, no `-> Self`: dyn-compatible.
pub trait Workspace {
    /// This workspace's stable id - also its registry key.
    fn id(&self) -> WorkspaceId;

    /// Static metadata: name, icon, purpose, and the Cmd+1..5 shortcut.
    fn meta(&self) -> WorkspaceMeta;

    /// Pure: which registered panels/tools fill which region.
    ///
    /// No egui, no mutation - returns ids only; the shell resolves them against
    /// the registries. This is the snapshot-test target.
    fn layout(&self) -> WorkspaceLayout;
}

/// Static, by-value metadata describing a workspace.
pub struct WorkspaceMeta {
    /// Localization key for the display name (e.g. `MsgKey("workspace.draw.title")`);
    /// resolved at render time.
    pub name: MsgKey,
    /// Phosphor glyph for the workspace tab.
    pub icon: char,
    /// Localization key for the tooltip / command-palette description; resolved at
    /// render time.
    pub purpose: MsgKey,
    /// The activation shortcut, `Modifiers::COMMAND` + `Key::Num1..Num5`.
    pub shortcut: egui::KeyboardShortcut,
}

/// What fills the shell's center region for a workspace.
///
/// Most workspaces draw the navigable wgpu canvas there. The Codex draws a
/// registered panel (its Entry Editor / Visual Board / Graph surface), which is not
/// a pixel canvas, so it asks for a full-center panel instead. `Copy` so a layout
/// stays cheap to clone per frame.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum CenterSurface {
    /// The wgpu canvas stage (Draw, Animate, Tiles, Generate, Export).
    Canvas,
    /// A registered panel rendered full-center, by id (the Codex editor/board/graph).
    Panel(PanelId),
}

/// Where a workspace places registered panels and tools, by id.
///
/// `layout()` returns owned `Vec`s of `Copy` ids - cheap to call once per frame
/// for the active workspace; no panel object moves.
#[derive(Clone, PartialEq, Debug)]
pub struct WorkspaceLayout {
    /// Left-dock card stack, top to bottom. Empty for the canvas workspaces; the
    /// Codex uses it for its Navigator. Rendered as a left side panel inboard of the
    /// tool rail when non-empty.
    pub left_dock: Vec<PanelId>,
    /// Right-dock card stack, top to bottom.
    pub right_dock: Vec<PanelId>,
    /// Bottom-tray tabs, left to right; the first is the default selected tab.
    pub bottom_tray: Vec<PanelId>,
    /// What fills the center: the canvas, or a full-center panel.
    pub center: CenterSurface,
    /// The ordered subset of tools shown in the left rail.
    pub primary_tools: Vec<ToolId>,
    /// The tool selected when this workspace activates.
    pub default_tool: ToolId,
    /// Workspace-specific status-bar entries.
    pub status_items: Vec<StatusItem>,
}

/// A single status-bar entry: an icon glyph and its label.
///
/// `text` is an owned `String` (not `&'static str`) so a future status item can
/// be computed; strings only, so the type stays `Debug`-snapshottable.
#[derive(Clone, PartialEq, Debug)]
pub struct StatusItem {
    /// Phosphor glyph shown before the text.
    pub icon: char,
    /// The status label, e.g. "Pixel Grid On".
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::{CenterSurface, StatusItem, WorkspaceLayout};
    use crate::contrib_api::ids::{PanelId, ToolId};

    #[test]
    fn layout_is_clone_eq_debug() {
        let layout = WorkspaceLayout {
            left_dock: Vec::new(),
            right_dock: vec![PanelId("layers"), PanelId("palette")],
            bottom_tray: vec![PanelId("frames"), PanelId("console")],
            center: CenterSurface::Canvas,
            primary_tools: vec![ToolId("pencil")],
            default_tool: ToolId("pencil"),
            status_items: vec![StatusItem {
                icon: '#',
                text: "Pixel Grid On".to_owned(),
            }],
        };
        // Clone + PartialEq round-trip (the snapshot test relies on both).
        assert_eq!(layout.clone(), layout);
        // Debug is populated (insta-snapshottable).
        assert!(format!("{layout:?}").contains("Pixel Grid On"));
    }

    #[test]
    fn center_surface_distinguishes_canvas_from_panel() {
        assert_eq!(CenterSurface::Canvas, CenterSurface::Canvas);
        assert_ne!(CenterSurface::Canvas, CenterSurface::Panel(PanelId("codex-editor")));
        assert_eq!(CenterSurface::Panel(PanelId("a")), CenterSurface::Panel(PanelId("a")));
    }
}
