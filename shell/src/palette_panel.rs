//! The palette panel: a swatch grid bound to the active sprite's first palette.
//!
//! Click a swatch to set the foreground colour, right-click to set the
//! background, and use the editor popup to recolour an entry. Auto-add-on-draw,
//! sorting, and a grid lock are the Pixelorama-derived curation aids; see
//! `THIRD_PARTY_NOTICES.md`.

use eframe::egui;
use pixhaus_core::project::{PaletteEntry, Rgba};

use crate::app::ShellApp;
use crate::commands::push_sprite_edit;
use crate::editor::{PaletteSort, from_color32, to_color32};
use crate::icons;

impl ShellApp {
    /// Draws the palette panel.
    pub(crate) fn palette_panel(&mut self, ui: &mut egui::Ui) {
        let Some(palette) = self.doc.active_palette() else {
            ui.label("No palette.");
            return;
        };
        let colors: Vec<Rgba> = palette.colors.iter().map(|e| e.color).collect();
        let count = colors.len();

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.editor.auto_add_palette, "Auto-add")
                .on_hover_text("Add painted colours to the palette");
            ui.checkbox(&mut self.editor.lock_palette_grid, "Lock");
        });
        ui.add(egui::Slider::new(&mut self.editor.swatch_size, 12.0..=32.0).text("swatch"));

        ui.horizontal(|ui| {
            ui.label("Sort:");
            if ui.small_button("Hue").clicked() {
                self.sort_palette(PaletteSort::Hue);
            }
            if ui.small_button("Lum").clicked() {
                self.sort_palette(PaletteSort::Luminance);
            }
        });

        ui.separator();

        let sw = self.editor.swatch_size;
        let avail = ui.available_width();
        let per_row = ((avail / (sw + 4.0)).floor() as usize).max(1);
        let mut set_fg: Option<Rgba> = None;
        let mut set_bg: Option<Rgba> = None;
        let mut edit_idx: Option<usize> = None;

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
            for (i, &color) in colors.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(egui::vec2(sw, sw), egui::Sense::click());
                // Checker under transparent swatches.
                if color.a < 255 {
                    paint_checker(ui, rect);
                }
                ui.painter().rect_filled(rect, 2.0, to_color32(color));
                let selected = color == self.editor.fg;
                let stroke = if selected {
                    egui::Stroke::new(2.0, egui::Color32::WHITE)
                } else {
                    egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120))
                };
                ui.painter().rect_stroke(rect, 2.0, stroke, egui::StrokeKind::Middle);
                if resp.clicked() {
                    set_fg = Some(color);
                }
                if resp.secondary_clicked() {
                    set_bg = Some(color);
                }
                if resp.double_clicked() {
                    edit_idx = Some(i);
                }
                let _ = per_row; // wrapped layout handles row breaks
            }
        });

        if let Some(c) = set_fg {
            self.editor.fg = c;
        }
        if let Some(c) = set_bg {
            self.editor.bg = c;
        }
        if let Some(i) = edit_idx {
            self.editor.editing_swatch = Some(i);
        }

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(format!("{} add fg", icons::ADD)).clicked() {
                let c = self.editor.fg;
                push_sprite_edit(&mut self.editor, &mut self.doc, "Add swatch", |sprite| {
                    if let Some(p) = sprite.palettes.first_mut() {
                        p.colors.push(PaletteEntry::new(c));
                    }
                });
            }
            if count > 0 && ui.button(format!("{} remove", icons::REMOVE)).clicked() {
                push_sprite_edit(&mut self.editor, &mut self.doc, "Remove swatch", |sprite| {
                    if let Some(p) = sprite.palettes.first_mut() {
                        p.colors.pop();
                    }
                });
            }
        });

        self.swatch_editor(ui);
    }

    /// The popup colour editor for the double-clicked swatch.
    fn swatch_editor(&mut self, ui: &mut egui::Ui) {
        let Some(idx) = self.editor.editing_swatch else {
            return;
        };
        let current = self.doc.active_palette().and_then(|p| p.colors.get(idx)).map(|e| e.color);
        let Some(current) = current else {
            self.editor.editing_swatch = None;
            return;
        };
        let mut col = to_color32(current);
        let mut close = false;
        egui::Window::new(format!("Edit swatch {idx}"))
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                if ui.color_edit_button_srgba(&mut col).changed() {
                    let new = from_color32(col);
                    push_sprite_edit(&mut self.editor, &mut self.doc, "Edit swatch", |sprite| {
                        if let Some(p) = sprite.palettes.first_mut() {
                            if let Some(e) = p.colors.get_mut(idx) {
                                e.color = new;
                            }
                        }
                    });
                }
                if ui.button("Done").clicked() {
                    close = true;
                }
            });
        if close {
            self.editor.editing_swatch = None;
        }
    }

    /// Reorders the active palette's swatches by the requested key.
    fn sort_palette(&mut self, sort: PaletteSort) {
        push_sprite_edit(&mut self.editor, &mut self.doc, "Sort palette", |sprite| {
            if let Some(p) = sprite.palettes.first_mut() {
                match sort {
                    PaletteSort::Hue => p
                        .colors
                        .sort_by(|a, b| hue(a.color).partial_cmp(&hue(b.color)).unwrap_or(std::cmp::Ordering::Equal)),
                    PaletteSort::Luminance => {
                        p.colors.sort_by_key(|e| luminance(e.color));
                    }
                }
            }
        });
    }
}

/// Paints a small 2x2 checkerboard inside `rect` to back transparent swatches.
fn paint_checker(ui: &egui::Ui, rect: egui::Rect) {
    let p = ui.painter();
    p.rect_filled(rect, 2.0, egui::Color32::from_gray(90));
    let h = rect.height() / 2.0;
    let w = rect.width() / 2.0;
    let dark = egui::Color32::from_gray(60);
    p.rect_filled(egui::Rect::from_min_size(rect.min, egui::vec2(w, h)), 0.0, dark);
    p.rect_filled(egui::Rect::from_min_size(rect.min + egui::vec2(w, h), egui::vec2(w, h)), 0.0, dark);
}

/// Approximate hue angle in `0..360` for sorting.
fn hue(c: Rgba) -> f32 {
    let r = f32::from(c.r) / 255.0;
    let g = f32::from(c.g) / 255.0;
    let b = f32::from(c.b) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    if d < f32::EPSILON {
        return 0.0;
    }
    let h = if (max - r).abs() < f32::EPSILON {
        ((g - b) / d) % 6.0
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0 + 360.0) % 360.0
}

/// Rec.601 luminance scaled to `0..=255` for sorting.
fn luminance(c: Rgba) -> u32 {
    (u32::from(c.r) * 299 + u32::from(c.g) * 587 + u32::from(c.b) * 114) / 1000
}
