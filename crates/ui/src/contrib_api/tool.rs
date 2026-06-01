//! The `Tool` trait and its metadata.
//!
//! A tool contributes options into the tool-options bar when active. Like
//! [`Panel`], it is dyn-compatible (`Box<dyn Tool>` in the registry): `&self`
//! receivers, no generics, no `-> Self`, metadata by value.
//!
//! [`Panel`]: crate::contrib_api::panel::Panel

use crate::contrib_api::context::ContribCtx;
use crate::contrib_api::ids::ToolId;

/// A registered tool: stable identity, metadata, and an options renderer.
///
/// # Object safety
///
/// `&self` receivers, no generic methods, no `-> Self`: dyn-compatible, stored
/// as `Box<dyn Tool>`.
pub trait Tool {
    /// This tool's stable id - also its registry key.
    fn id(&self) -> ToolId;

    /// Static metadata: label, icon, shortcut, tooltip, AI marker.
    fn meta(&self) -> ToolMeta;

    /// Render this tool's options into the tool-options bar when active.
    ///
    /// Takes a bare [`ContribCtx`] - a tool is not a panel, so it has no
    /// [`PanelId`] and no scratch buffer. State changes go through
    /// `cx.intents`.
    ///
    /// [`PanelId`]: crate::contrib_api::ids::PanelId
    fn options_ui(&self, ui: &mut egui::Ui, cx: &mut ContribCtx<'_>);

    // When `core` lands, `fn on_pointer(&self, ev, &mut CommandSink)` arrives
    // here, additive (bible rules 3/4). Tools emit no canvas commands this round.
}

/// Static, by-value metadata describing a tool.
pub struct ToolMeta {
    /// Display label shown in tooltips and the command palette.
    pub label: &'static str,
    /// Phosphor glyph from [`crate::icons`] painted on the rail button.
    pub icon: char,
    /// Optional keyboard shortcut (e.g. `B` for pencil). `None` means no key.
    pub shortcut: Option<egui::KeyboardShortcut>,
    /// One-line help, e.g. "Draw individual pixels. Hold Shift for a line.".
    pub tooltip: &'static str,
    /// The AI Brush flips this - it renders with the accent AI tint + sparkle.
    pub is_ai: bool,
}
