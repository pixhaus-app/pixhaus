//! The canonical shared editing tool ids and their rail order.
//!
//! The 15 shared editing tools (bible rule 2) are referenced by id from several
//! workspaces - Draw shows the whole rail, the others a subset - so the ids and their
//! order are defined once here, in the contribution surface every module already
//! depends on, rather than re-typed as string literals per workspace. The `Tool`
//! implementations themselves live in `mod-sprite-edit`; this module owns only the
//! shared identity and order every workspace draws from.

use super::ids::ToolId;

/// Pencil tool id.
pub const PENCIL: ToolId = ToolId("pencil");
/// Eraser tool id.
pub const ERASER: ToolId = ToolId("eraser");
/// Flood-fill tool id.
pub const FILL: ToolId = ToolId("fill");
/// Line tool id.
pub const LINE: ToolId = ToolId("line");
/// Rectangle tool id.
pub const RECTANGLE: ToolId = ToolId("rectangle");
/// Ellipse tool id.
pub const ELLIPSE: ToolId = ToolId("ellipse");
/// Eyedropper tool id.
pub const EYEDROPPER: ToolId = ToolId("eyedropper");
/// Marquee selection tool id.
pub const SELECTION: ToolId = ToolId("selection");
/// Lasso selection tool id.
pub const LASSO: ToolId = ToolId("lasso");
/// Move tool id.
pub const MOVE: ToolId = ToolId("move");
/// Transform tool id.
pub const TRANSFORM: ToolId = ToolId("transform");
/// Pixel-text tool id.
pub const TEXT: ToolId = ToolId("text");
/// Pan (hand) tool id.
pub const HAND: ToolId = ToolId("hand");
/// Zoom tool id.
pub const ZOOM: ToolId = ToolId("zoom");
/// AI Brush tool id.
pub const AI_BRUSH: ToolId = ToolId("ai_brush");

/// The full rail in order: the manual tools, then the AI Brush last (bible rule 2).
/// The Draw workspace shows all 15; other workspaces show a subset, but every
/// workspace draws these ids from this one definition rather than re-typing them.
pub const TOOL_RAIL: [ToolId; 15] = [
    PENCIL, ERASER, FILL, LINE, RECTANGLE, ELLIPSE, EYEDROPPER, SELECTION, LASSO, MOVE, TRANSFORM, TEXT, HAND, ZOOM, AI_BRUSH,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn rail_is_fifteen_tools_pencil_first_ai_brush_last() {
        assert_eq!(TOOL_RAIL.len(), 15);
        assert_eq!(TOOL_RAIL[0], PENCIL);
        assert_eq!(TOOL_RAIL[14], AI_BRUSH);
    }

    #[test]
    fn rail_has_no_duplicate_ids() {
        let mut seen = HashSet::new();
        for id in TOOL_RAIL {
            assert!(seen.insert(id), "duplicate tool id in the rail: {id:?}");
        }
    }
}
