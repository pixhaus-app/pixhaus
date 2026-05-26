//! The slim left tool strip: one icon button per tool (Aseprite/Pixelorama
//! style), with dual-button assignment and undo/redo at the bottom.
//!
//! Tool options live in the top context bar, not here, so this column stays
//! narrow and the canvas gets the space.

use eframe::egui;

use crate::app::ShellApp;
use crate::editor::Tool;
use crate::icons;

/// Tools in strip order, with their icon glyph and hover label + shortcut.
const STRIP: &[(Tool, &str, &str)] = &[
    (Tool::Pencil, icons::PENCIL, "Pencil (B)"),
    (Tool::Eraser, icons::ERASER, "Eraser (E)"),
    (Tool::Fill, icons::FILL, "Fill (G)"),
    (Tool::Line, icons::LINE, "Line (L)"),
    (Tool::Rectangle, icons::RECT, "Rectangle (U)"),
    (Tool::Ellipse, icons::ELLIPSE, "Ellipse (O)"),
    (Tool::Picker, icons::PICKER, "Colour picker (I)"),
    (Tool::SelectRect, icons::MARQUEE, "Rectangular marquee (M)"),
    (Tool::SelectEllipse, icons::ELLIPSE_SELECT, "Elliptical marquee"),
    (Tool::Lasso, icons::LASSO, "Lasso (Q)"),
    (Tool::Wand, icons::WAND, "Magic wand (W)"),
    (Tool::Move, icons::MOVE, "Move selection (V)"),
];

impl ShellApp {
    /// Draws the vertical icon tool strip.
    pub(crate) fn tools_panel(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(2.0);
            for (tool, glyph, hint) in STRIP {
                let is_left = self.editor.left_tool == *tool;
                let is_right = self.editor.right_tool == *tool;
                let resp = ui.add(
                    egui::Button::new(egui::RichText::new(*glyph).size(18.0))
                        .selected(is_left)
                        .min_size(egui::vec2(32.0, 30.0)),
                );
                let hover = if is_right {
                    format!("{hint}\nright-click another tool for the secondary button\n(secondary: this tool)")
                } else {
                    format!("{hint}\nright-click to bind to the secondary button")
                };
                let resp = resp.on_hover_text(hover);
                if resp.clicked() {
                    self.editor.left_tool = *tool;
                }
                if resp.secondary_clicked() {
                    self.editor.right_tool = *tool;
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(2.0);

            let undo_label = self
                .editor
                .history
                .undo_label()
                .map_or_else(|| "Undo (Ctrl+Z)".to_owned(), |l| format!("Undo: {l} (Ctrl+Z)"));
            if ui
                .add_enabled(
                    self.editor.history.can_undo(),
                    egui::Button::new(egui::RichText::new(icons::UNDO).size(16.0)).min_size(egui::vec2(32.0, 28.0)),
                )
                .on_hover_text(undo_label)
                .clicked()
            {
                self.do_undo();
            }
            if ui
                .add_enabled(
                    self.editor.history.can_redo(),
                    egui::Button::new(egui::RichText::new(icons::REDO).size(16.0)).min_size(egui::vec2(32.0, 28.0)),
                )
                .on_hover_text("Redo (Ctrl+Y)")
                .clicked()
            {
                self.do_redo();
            }
        });
    }
}
