//! The 15 shared editing tools (bible rule 2): the manual brushes plus the AI
//! Brush. All 15 live here even though only a subset appears in any one
//! workspace's rail - they are the shared editing core every workspace draws
//! from. Each tool is a `&self` unit struct: it holds no mutable state, and its
//! `options_ui` mock widgets bind to throwaway locals that reset each frame
//! (they move, they drive nothing this round).

use egui::{Key, KeyboardShortcut, Modifiers};
use pixhaus_ui::contrib_api::{ContribCtx, HostRegistrar, MsgKey, Tool, ToolId, ToolMeta};
use pixhaus_ui::icons;

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

/// Full rail order: the manual tools, then the AI Brush last (bible rule 2
/// ordering). The Draw workspace shows all 15; other workspaces show a subset.
pub const ALL: [ToolId; 15] = [
    PENCIL, ERASER, FILL, LINE, RECTANGLE, ELLIPSE, EYEDROPPER, SELECTION, LASSO, MOVE, TRANSFORM, TEXT, HAND, ZOOM, AI_BRUSH,
];

/// The Pencil tool. Worked `Tool` example: `options_ui` renders the inventory
/// option row with live widgets bound to throwaway local state.
pub struct PencilTool;

impl Tool for PencilTool {
    fn id(&self) -> ToolId {
        PENCIL
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.pencil.label"),
            icon: icons::PENCIL,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::B)),
            tooltip: MsgKey("tool.pencil.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            let mut size = 1.0_f32;
            ui.add(egui::DragValue::new(&mut size).prefix("Size ").suffix("px"));
            let mut opacity = 255.0_f32;
            ui.add(egui::DragValue::new(&mut opacity).prefix("Opacity ").range(0.0..=255.0));
            let mut pixel_perfect = true;
            ui.checkbox(&mut pixel_perfect, "Pixel-perfect");
            ui.label("Dither None");
            let mut mirror_x = false;
            ui.checkbox(&mut mirror_x, "Mirror X");
            let mut mirror_y = false;
            ui.checkbox(&mut mirror_y, "Mirror Y");
        });
    }
}

/// The Eraser tool.
pub struct EraserTool;

impl Tool for EraserTool {
    fn id(&self) -> ToolId {
        ERASER
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.eraser.label"),
            icon: icons::ERASER,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::E)),
            tooltip: MsgKey("tool.eraser.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            let mut size = 1.0_f32;
            ui.add(egui::DragValue::new(&mut size).prefix("Size ").suffix("px"));
            let mut opacity = 255.0_f32;
            ui.add(egui::DragValue::new(&mut opacity).prefix("Opacity ").range(0.0..=255.0));
            let mut pixel_perfect = true;
            ui.checkbox(&mut pixel_perfect, "Pixel-perfect");
        });
    }
}

/// The Fill tool.
pub struct FillTool;

impl Tool for FillTool {
    fn id(&self) -> ToolId {
        FILL
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.fill.label"),
            icon: icons::FILL,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::G)),
            tooltip: MsgKey("tool.fill.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            let mut tolerance = 0.0_f32;
            ui.add(egui::DragValue::new(&mut tolerance).prefix("Tolerance "));
            let mut contiguous = true;
            ui.checkbox(&mut contiguous, "Contiguous");
            let mut all_layers = false;
            ui.checkbox(&mut all_layers, "All layers");
        });
    }
}

/// The Line tool.
pub struct LineTool;

impl Tool for LineTool {
    fn id(&self) -> ToolId {
        LINE
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.line.label"),
            icon: icons::LINE,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::L)),
            tooltip: MsgKey("tool.line.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            let mut size = 1.0_f32;
            ui.add(egui::DragValue::new(&mut size).prefix("Size ").suffix("px"));
            let mut fill = false;
            ui.checkbox(&mut fill, "Fill");
            let mut from_center = false;
            ui.checkbox(&mut from_center, "From center");
        });
    }
}

/// The Rectangle tool.
pub struct RectangleTool;

impl Tool for RectangleTool {
    fn id(&self) -> ToolId {
        RECTANGLE
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.rectangle.label"),
            icon: icons::RECT,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::U)),
            tooltip: MsgKey("tool.rectangle.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            let mut size = 1.0_f32;
            ui.add(egui::DragValue::new(&mut size).prefix("Size ").suffix("px"));
            let mut fill = false;
            ui.checkbox(&mut fill, "Fill");
            let mut from_center = false;
            ui.checkbox(&mut from_center, "From center");
        });
    }
}

/// The Ellipse tool.
pub struct EllipseTool;

impl Tool for EllipseTool {
    fn id(&self) -> ToolId {
        ELLIPSE
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.ellipse.label"),
            icon: icons::ELLIPSE,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::O)),
            tooltip: MsgKey("tool.ellipse.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            let mut size = 1.0_f32;
            ui.add(egui::DragValue::new(&mut size).prefix("Size ").suffix("px"));
            let mut fill = false;
            ui.checkbox(&mut fill, "Fill");
            let mut from_center = false;
            ui.checkbox(&mut from_center, "From center");
        });
    }
}

/// The Eyedropper tool.
pub struct EyedropperTool;

impl Tool for EyedropperTool {
    fn id(&self) -> ToolId {
        EYEDROPPER
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.eyedropper.label"),
            icon: icons::EYEDROPPER,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::I)),
            tooltip: MsgKey("tool.eyedropper.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Sample Composite");
            let mut add_to_palette = false;
            ui.checkbox(&mut add_to_palette, "Add to palette");
        });
    }
}

/// The Selection (marquee) tool.
pub struct SelectionTool;

impl Tool for SelectionTool {
    fn id(&self) -> ToolId {
        SELECTION
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.selection.label"),
            icon: icons::SELECT,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::M)),
            tooltip: MsgKey("tool.selection.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Mode Replace");
            let mut feather = 0.0_f32;
            ui.add(egui::DragValue::new(&mut feather).prefix("Feather "));
            let mut snap_pixel = true;
            ui.checkbox(&mut snap_pixel, "Snap Pixel");
            ui.label("Origin Center");
        });
    }
}

/// The Lasso (freeform selection) tool.
pub struct LassoTool;

impl Tool for LassoTool {
    fn id(&self) -> ToolId {
        LASSO
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.lasso.label"),
            icon: icons::LASSO,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::Q)),
            tooltip: MsgKey("tool.lasso.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Mode Replace");
            let mut feather = 0.0_f32;
            ui.add(egui::DragValue::new(&mut feather).prefix("Feather "));
        });
    }
}

/// The Move tool.
pub struct MoveTool;

impl Tool for MoveTool {
    fn id(&self) -> ToolId {
        MOVE
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.move.label"),
            icon: icons::MOVE,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::V)),
            tooltip: MsgKey("tool.move.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Origin Center");
            let mut snap = false;
            ui.checkbox(&mut snap, "Snap");
        });
    }
}

/// The Transform (scale/rotate/skew) tool.
pub struct TransformTool;

impl Tool for TransformTool {
    fn id(&self) -> ToolId {
        TRANSFORM
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.transform.label"),
            icon: icons::TRANSFORM,
            shortcut: Some(KeyboardShortcut::new(Modifiers::SHIFT, Key::T)),
            tooltip: MsgKey("tool.transform.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Origin Center");
            let mut snap = false;
            ui.checkbox(&mut snap, "Snap");
        });
    }
}

/// The pixel-text tool.
pub struct TextTool;

impl Tool for TextTool {
    fn id(&self) -> ToolId {
        TEXT
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.text.label"),
            icon: icons::TEXT,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::X)),
            tooltip: MsgKey("tool.text.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Font Pixel");
            let mut size = 8.0_f32;
            ui.add(egui::DragValue::new(&mut size).prefix("Size "));
        });
    }
}

/// The Hand (pan) tool.
pub struct HandTool;

impl Tool for HandTool {
    fn id(&self) -> ToolId {
        HAND
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.hand.label"),
            icon: icons::HAND,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::H)),
            tooltip: MsgKey("tool.hand.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Zoom 1600%");
            let _ = ui.button("Fit");
            let _ = ui.button("100%");
        });
    }
}

/// The Zoom tool.
pub struct ZoomTool;

impl Tool for ZoomTool {
    fn id(&self) -> ToolId {
        ZOOM
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.zoom.label"),
            icon: icons::ZOOM,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::Z)),
            tooltip: MsgKey("tool.zoom.tooltip"),
            is_ai: false,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, _cx: &mut ContribCtx<'_>) {
        ui.horizontal(|ui| {
            ui.label("Zoom 1600%");
            let _ = ui.button("Fit");
            let _ = ui.button("100%");
        });
    }
}

/// The AI Brush tool. Flips `is_ai`, so the rail paints it with the AI accent and
/// a sparkle. Its options row leads with the sparkle marker.
pub struct AiBrushTool;

impl Tool for AiBrushTool {
    fn id(&self) -> ToolId {
        AI_BRUSH
    }

    fn meta(&self) -> ToolMeta {
        ToolMeta {
            label: MsgKey("tool.ai_brush.label"),
            icon: icons::SPARKLE,
            shortcut: Some(KeyboardShortcut::new(Modifiers::NONE, Key::J)),
            tooltip: MsgKey("tool.ai_brush.tooltip"),
            is_ai: true,
        }
    }

    fn options_ui(&self, ui: &mut egui::Ui, cx: &mut ContribCtx<'_>) {
        let ai_tint = cx.theme.accent.ai;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(icons::SPARKLE.to_string()).color(ai_tint));
            ui.label("Mode Fill");
            let mut use_palette = true;
            ui.checkbox(&mut use_palette, "Use Palette");
            let mut preserve_outline = true;
            ui.checkbox(&mut preserve_outline, "Preserve Outline");
            let mut variations = 4.0_f32;
            ui.add(egui::DragValue::new(&mut variations).prefix("Variations "));
            let mut strength = 0.65_f32;
            ui.add(egui::DragValue::new(&mut strength).prefix("Strength ").range(0.0..=1.0).speed(0.01));
        });
    }
}

/// Register every shared editing tool, in rail order (bible rule 2).
pub fn register(host: &mut dyn HostRegistrar) {
    host.add_tool(Box::new(PencilTool));
    host.add_tool(Box::new(EraserTool));
    host.add_tool(Box::new(FillTool));
    host.add_tool(Box::new(LineTool));
    host.add_tool(Box::new(RectangleTool));
    host.add_tool(Box::new(EllipseTool));
    host.add_tool(Box::new(EyedropperTool));
    host.add_tool(Box::new(SelectionTool));
    host.add_tool(Box::new(LassoTool));
    host.add_tool(Box::new(MoveTool));
    host.add_tool(Box::new(TransformTool));
    host.add_tool(Box::new(TextTool));
    host.add_tool(Box::new(HandTool));
    host.add_tool(Box::new(ZoomTool));
    host.add_tool(Box::new(AiBrushTool));
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn pencil_meta() {
        let m = PencilTool.meta();
        assert_eq!(PencilTool.id(), PENCIL);
        assert_eq!(m.label, MsgKey("tool.pencil.label"));
        assert!(!m.is_ai);
        assert_eq!(m.shortcut, Some(KeyboardShortcut::new(Modifiers::NONE, Key::B)));
    }

    #[test]
    fn ai_brush_is_flagged_ai() {
        assert!(AiBrushTool.meta().is_ai);
        assert_eq!(AiBrushTool.meta().shortcut, Some(KeyboardShortcut::new(Modifiers::NONE, Key::J)));
    }

    #[test]
    fn transform_uses_shift_t() {
        assert_eq!(TransformTool.meta().shortcut, Some(KeyboardShortcut::new(Modifiers::SHIFT, Key::T)));
    }

    #[test]
    fn all_tool_ids_are_unique_and_fifteen() {
        let set: HashSet<_> = ALL.iter().collect();
        assert_eq!(set.len(), ALL.len(), "no duplicate tool ids");
        assert_eq!(ALL.len(), 15, "the shared editing core is the 15-tool set");
    }

    #[test]
    fn all_is_in_rail_order_with_ai_brush_last() {
        assert_eq!(ALL.first(), Some(&PENCIL), "Pencil leads the rail");
        assert_eq!(ALL.last(), Some(&AI_BRUSH), "AI Brush is last (bible rule 2 ordering)");
    }
}
