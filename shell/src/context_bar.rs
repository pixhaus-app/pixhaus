//! The top context bar: the active colours and only the active tool's options.
//!
//! This is the OpenToonz/Aseprite "tool option bar" idea — instead of a wide
//! permanent options pane, a thin strip under the menu shows just what the
//! current tool needs. It keeps the left toolbar to icons and frees the side
//! dock for content.

use eframe::egui;
use pixhaus_core::canvas::BrushShape;

use crate::app::ShellApp;
use crate::editor::{Tool, from_color32, to_color32};
use crate::icons;

impl ShellApp {
    /// Draws the context bar for the primary (left-button) tool.
    pub(crate) fn context_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Active colours: foreground / background + swap.
            let mut fg = to_color32(self.editor.fg);
            if ui.color_edit_button_srgba(&mut fg).on_hover_text("Foreground").changed() {
                self.editor.fg = from_color32(fg);
            }
            let mut bg = to_color32(self.editor.bg);
            if ui.color_edit_button_srgba(&mut bg).on_hover_text("Background").changed() {
                self.editor.bg = from_color32(bg);
            }
            if ui.button(icons::SWAP).on_hover_text("Swap colours (X)").clicked() {
                self.editor.swap_colors();
            }

            ui.separator();
            self.tool_options(ui);
        });
    }

    /// The option controls specific to the primary tool.
    fn tool_options(&mut self, ui: &mut egui::Ui) {
        match self.editor.left_tool {
            Tool::Pencil | Tool::Eraser => {
                self.brush_options(ui);
            }
            Tool::Line | Tool::Rectangle | Tool::Ellipse => {
                self.mirror_options(ui);
                ui.separator();
                ui.label(egui::RichText::new("hold Shift to fill").small().weak());
            }
            Tool::Fill | Tool::Wand => {
                ui.add(egui::Slider::new(&mut self.editor.tolerance, 0..=255).text("tolerance"));
            }
            Tool::Picker => {
                ui.label(egui::RichText::new("click to sample a colour").small().weak());
            }
            Tool::Move => {
                ui.label(egui::RichText::new("drag the selection").small().weak());
            }
            Tool::SelectRect | Tool::SelectEllipse | Tool::Lasso => {
                ui.label(egui::RichText::new("drag to select · Esc to deselect").small().weak());
            }
        }
    }

    /// Brush shape, size, pixel-perfect, and mirroring — the pencil/eraser set.
    fn brush_options(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("ctx_brush_shape")
            .width(78.0)
            .selected_text(brush_label(self.editor.brush_shape))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.editor.brush_shape, BrushShape::Pixel, "Pixel");
                ui.selectable_value(&mut self.editor.brush_shape, BrushShape::Circle, "Circle");
                ui.selectable_value(&mut self.editor.brush_shape, BrushShape::Square, "Square");
            });
        ui.add(egui::Slider::new(&mut self.editor.brush_size, 1..=64).text("size"));
        ui.checkbox(&mut self.editor.pixel_perfect, "Pixel-perfect");
        self.mirror_options(ui);
    }

    /// The mirror-X / mirror-Y toggles, shared by brush and shape tools.
    fn mirror_options(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.editor.mirror_x, "Mirror X");
        ui.checkbox(&mut self.editor.mirror_y, "Mirror Y");
    }
}

fn brush_label(shape: BrushShape) -> &'static str {
    match shape {
        BrushShape::Pixel => "Pixel",
        BrushShape::Circle => "Circle",
        BrushShape::Square => "Square",
    }
}
