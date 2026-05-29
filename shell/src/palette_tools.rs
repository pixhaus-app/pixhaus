//! The palette panel's generator tools: a colour ramp and harmony suggestions.
//!
//! Both live in collapsing sections hung below the swatch grid. The ramp
//! interpolates between two existing swatches in Oklab; harmony derives a small
//! set of related colours from the foreground. Each "add" routes one
//! [`push_sprite_edit`] so the inserted colours undo as a single step. The maths
//! lives in `core::color::{ops, harmony}`; this module is only the UI and the
//! two thin selection helpers it tests.

use eframe::egui;
use pixhaus_core::color::harmony;
use pixhaus_core::color::ops::color_ramp;
use pixhaus_core::project::{PaletteEntry, Rgba};

use crate::app::ShellApp;
use crate::commands::push_sprite_edit;
use crate::document::palette_by_id_mut;
use crate::editor::{HarmonyKind, to_color32};
use crate::palette_panel::paint_checker;

/// The lowest step count the ramp tool offers.
const RAMP_MIN_STEPS: usize = 3;
/// The highest step count the ramp tool offers.
const RAMP_MAX_STEPS: usize = 32;

/// The intermediate colours a ramp of `steps` between `start` and `end`
/// contributes when added to the palette: the ramp without its two endpoints.
///
/// Returns an empty `Vec` for `steps <= 2` (a one- or two-colour ramp has no
/// intermediates), and `steps - 2` colours otherwise. Pure so the panel and its
/// test share one rule.
#[must_use]
pub fn ramp_intermediates(start: Rgba, end: Rgba, steps: usize) -> Vec<Rgba> {
    if steps <= 2 {
        return Vec::new();
    }
    let ramp = color_ramp(start, end, steps);
    // color_ramp(steps >= 3) always returns `steps` colours, so the endpoint
    // trim is in range; the explicit bound keeps this total if that ever shifts.
    if ramp.len() <= 2 {
        return Vec::new();
    }
    ramp[1..ramp.len() - 1].to_vec()
}

/// The suggested colours for a harmony `kind` derived from `source`. Maps each
/// kind onto its `core::color::harmony` function. Pure so the panel and its test
/// share one mapping.
#[must_use]
pub fn harmony_colors(kind: HarmonyKind, source: Rgba) -> Vec<Rgba> {
    match kind {
        HarmonyKind::Complement => vec![harmony::complement(source)],
        HarmonyKind::SplitComplement => harmony::split_complement(source).to_vec(),
        HarmonyKind::Triad => harmony::triad(source).to_vec(),
        HarmonyKind::Tetrad => harmony::tetrad(source).to_vec(),
        HarmonyKind::Analogous => harmony::analogous_standard(source).to_vec(),
    }
}

impl ShellApp {
    /// Draws the ramp and harmony generator sections below the swatch grid. A
    /// no-op when there is no active palette.
    pub(crate) fn palette_tools(&mut self, ui: &mut egui::Ui) {
        let Some(palette) = self.doc.active_palette_by_id(self.editor.active_palette_id) else {
            return;
        };
        let colors: Vec<Rgba> = palette.colors.iter().map(|e| e.color).collect();

        self.ramp_section(ui, &colors);
        self.harmony_section(ui);
    }

    /// The "Ramp" collapsing section: pick two swatch indices and a step count,
    /// preview the Oklab ramp, and append its intermediates in one undo step.
    fn ramp_section(&mut self, ui: &mut egui::Ui, colors: &[Rgba]) {
        egui::CollapsingHeader::new("Ramp").id_salt("palette_ramp").default_open(false).show(ui, |ui| {
            if colors.len() < 2 {
                ui.label("Add at least two swatches to build a ramp.");
                return;
            }
            let max_index = colors.len() - 1;
            // Clamp the stored indices to the live palette before they drive the
            // drag values, so a shorter palette never leaves a stale endpoint.
            self.editor.ramp_from = self.editor.ramp_from.min(max_index);
            self.editor.ramp_to = self.editor.ramp_to.min(max_index);
            self.editor.ramp_steps = self.editor.ramp_steps.clamp(RAMP_MIN_STEPS, RAMP_MAX_STEPS);

            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut self.editor.ramp_from).range(0..=max_index).prefix("from "));
                ui.add(egui::DragValue::new(&mut self.editor.ramp_to).range(0..=max_index).prefix("to "));
                ui.add(
                    egui::DragValue::new(&mut self.editor.ramp_steps)
                        .range(RAMP_MIN_STEPS..=RAMP_MAX_STEPS)
                        .prefix("steps "),
                );
            });

            let start = colors[self.editor.ramp_from];
            let end = colors[self.editor.ramp_to];
            let ramp = color_ramp(start, end, self.editor.ramp_steps);
            preview_strip(ui, &ramp);

            let intermediates = ramp_intermediates(start, end, self.editor.ramp_steps);
            let enabled = !intermediates.is_empty();
            if ui
                .add_enabled(enabled, egui::Button::new(format!("{} Add to palette", crate::icons::ADD)))
                .on_disabled_hover_text("A ramp needs at least three steps to add intermediates")
                .clicked()
            {
                let sel = self.editor.active_palette_id;
                push_sprite_edit(&mut self.editor, &mut self.doc, "Add ramp", |sprite| {
                    if let Some(p) = palette_by_id_mut(sprite, sel) {
                        p.colors.extend(intermediates.iter().copied().map(PaletteEntry::new));
                    }
                });
            }
        });
    }

    /// The "Harmony" collapsing section: pick a scheme, see the colours it
    /// derives from the foreground, and append a clicked suggestion in one undo
    /// step.
    fn harmony_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Harmony")
            .id_salt("palette_harmony")
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for kind in [
                        HarmonyKind::Complement,
                        HarmonyKind::SplitComplement,
                        HarmonyKind::Triad,
                        HarmonyKind::Tetrad,
                        HarmonyKind::Analogous,
                    ] {
                        ui.selectable_value(&mut self.editor.harmony_kind, kind, kind.label());
                    }
                });

                let source = self.editor.fg;
                let suggestions = harmony_colors(self.editor.harmony_kind, source);
                // The colour the artist clicks this frame, added after the swatch
                // loop ends so the panel's borrow is released first.
                let mut add: Option<Rgba> = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(3.0, 3.0);
                    // The source swatch leads the strip as the harmony anchor.
                    swatch(ui, source, false);
                    ui.separator();
                    for &c in &suggestions {
                        if swatch(ui, c, true).clicked() {
                            add = Some(c);
                        }
                    }
                });

                if let Some(c) = add {
                    let sel = self.editor.active_palette_id;
                    push_sprite_edit(&mut self.editor, &mut self.doc, "Add harmony color", |sprite| {
                        if let Some(p) = palette_by_id_mut(sprite, sel) {
                            p.colors.push(PaletteEntry::new(c));
                        }
                    });
                }
            });
    }
}

/// Paints a horizontal strip of small rects, one per colour in `ramp`. The
/// ramp's perceptual spacing reads as a smooth gradient when the count is high.
fn preview_strip(ui: &mut egui::Ui, ramp: &[Rgba]) {
    if ramp.is_empty() {
        return;
    }
    let height = 18.0;
    // count is a palette-scale step count (<= 32), well within f32's exact
    // integer range, so the cast cannot lose precision.
    #[allow(clippy::cast_precision_loss)]
    let width = (ramp.len() as f32 * 14.0).min(ui.available_width());
    let (rect, _resp) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    #[allow(clippy::cast_precision_loss)]
    let cell = rect.width() / ramp.len() as f32;
    for (i, &c) in ramp.iter().enumerate() {
        // i is bounded by the same palette-scale count as above.
        #[allow(clippy::cast_precision_loss)]
        let x = rect.left() + i as f32 * cell;
        let cell_rect = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(cell, height));
        if c.a < 255 {
            paint_checker(ui, cell_rect);
        }
        ui.painter().rect_filled(cell_rect, 0.0, to_color32(c));
    }
}

/// Paints a fixed-size clickable swatch tile and returns its response. When
/// `interactive`, the tile senses clicks; otherwise it only displays.
fn swatch(ui: &mut egui::Ui, color: Rgba, interactive: bool) -> egui::Response {
    let size = 18.0;
    let sense = if interactive { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), sense);
    if color.a < 255 {
        paint_checker(ui, rect);
    }
    ui.painter().rect_filled(rect, 2.0, to_color32(color));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120)),
        egui::StrokeKind::Middle,
    );
    resp
}

#[cfg(test)]
mod tests {
    use pixhaus_core::project::Rgba;
    use rstest::rstest;

    use super::{harmony_colors, ramp_intermediates};
    use crate::editor::HarmonyKind;

    fn red() -> Rgba {
        Rgba::opaque(255, 0, 0)
    }
    fn blue() -> Rgba {
        Rgba::opaque(0, 0, 255)
    }

    // ── ramp_intermediates ────────────────────────────────────────────────

    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(2)]
    fn ramp_intermediates_empty_for_two_or_fewer_steps(#[case] steps: usize) {
        // A one- or two-colour ramp is just its endpoints; nothing to add.
        assert!(ramp_intermediates(red(), blue(), steps).is_empty());
    }

    #[rstest]
    #[case(3, 1)]
    #[case(5, 3)]
    #[case(8, 6)]
    #[case(32, 30)]
    fn ramp_intermediates_count_is_steps_minus_two(#[case] steps: usize, #[case] expect: usize) {
        // Adding a ramp contributes its intermediates only: ramp.len() - 2.
        let inter = ramp_intermediates(red(), blue(), steps);
        assert_eq!(inter.len(), expect);
        assert_eq!(inter.len(), super::color_ramp(red(), blue(), steps).len() - 2);
    }

    #[test]
    fn ramp_intermediates_exclude_both_endpoints() {
        // The first and last ramp colours (the chosen swatches) are not added.
        let inter = ramp_intermediates(red(), blue(), 4);
        assert!(!inter.contains(&red()), "start endpoint must not be added");
        assert!(!inter.contains(&blue()), "end endpoint must not be added");
    }

    // ── harmony_colors ────────────────────────────────────────────────────

    #[rstest]
    #[case(HarmonyKind::Complement, 1)]
    #[case(HarmonyKind::SplitComplement, 2)]
    #[case(HarmonyKind::Triad, 2)]
    #[case(HarmonyKind::Tetrad, 3)]
    #[case(HarmonyKind::Analogous, 5)]
    fn harmony_colors_count_matches_the_scheme(#[case] kind: HarmonyKind, #[case] expect: usize) {
        assert_eq!(harmony_colors(kind, red()).len(), expect);
    }

    #[test]
    fn harmony_complement_of_red_is_cyan_ish() {
        // The single suggestion is the 180° rotation; sanity-check it shifted.
        let c = harmony_colors(HarmonyKind::Complement, red())[0];
        assert!(c.r < 10 && c.g > 240 && c.b > 240, "complement of red should be cyan-ish, got {c:?}");
    }
}
