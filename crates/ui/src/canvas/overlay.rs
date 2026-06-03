//! Canvas view overlays drawn over the blitted frame: the selection marquee and the
//! active tool's preview.
//!
//! Both are scaffolds. `core` has no selection model and the tool surface produces no
//! preview yet, so the data is always absent today — these render nothing rather than
//! fake data, and exist so the overlay pass has its ordered slot (over the frame, under
//! the HUD) for when the model and tools land. Onion skin, the one overlay with real
//! data now, is the sibling [`crate::canvas::onion`] module.

use egui::{Painter, Rect};

use crate::theme::Theme;

/// A selection outline in sprite-pixel space, to draw as marching ants. Absent until
/// `core` grows a selection model and a command that sets one (bible 9/12).
pub struct SelectionMask;

/// Paint `selection` as a marching-ants outline over the artboard. A no-op while no
/// selection exists, so it never draws fake data.
pub fn paint_selection(_painter: &Painter, _artboard: Rect, _scale: f32, selection: Option<&SelectionMask>, _theme: &Theme) {
    let Some(_selection) = selection else {
        return;
    };
    // Lands with the selection model: walk the mask boundary and stroke a dashed,
    // time-animated outline in the accent, mapped through the view scale.
}

/// The active tool's on-canvas preview — a brush outline, a shape rubber-band, a
/// transform handle box. Absent until the tool surface drives it.
pub struct ToolPreview;

/// Paint the active tool's `preview` over the artboard. A no-op until a tool produces
/// one; the call reserves the overlay order (over the frame, under the HUD).
pub fn paint_tool_preview(_painter: &Painter, _artboard: Rect, _scale: f32, preview: Option<&ToolPreview>, _theme: &Theme) {
    let Some(_preview) = preview else {
        return;
    };
    // Lands with the tools: each active tool renders its own preview shape here.
}
