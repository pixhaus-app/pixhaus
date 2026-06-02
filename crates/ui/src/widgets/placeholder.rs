//! Inert mock-content helpers: a list row, a thumbnail grid, a log block. These
//! stand in for real panel data until `core` lands. All passive - no interaction.

use crate::theme::Theme;

/// One labeled list row at body type scale, secondary text color.
pub fn mock_row(ui: &mut egui::Ui, theme: &Theme, label: &str) {
    ui.label(egui::RichText::new(label).size(theme.type_scale.body).color(theme.roles.text_secondary));
}

/// A wrapping grid of `n` mock sprite/asset/preset thumbnails. Each cell is a
/// transparency checker overlaid with a small colored blob from the `mock.thumbnails`
/// tint set, so the grids read with color and content instead of flat checker (the
/// references show real sprites here; this is the no-document stand-in).
// The radius token is a small, bounded positive constant (f32 -> u8), and the small
// loop indices are exactly representable as f32 - neither cast loses anything.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
pub fn mock_thumbnail_grid(ui: &mut egui::Ui, theme: &Theme, n: usize) {
    let cell = 44.0;
    ui.horizontal_wrapped(|ui| {
        for i in 0..n {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(cell), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                // 4x4 transparency checker so the bounds read.
                let step = cell / 4.0;
                for ty in 0..4 {
                    for tx in 0..4 {
                        let dark = (tx + ty) % 2 == 0;
                        let fill = if dark { theme.surfaces.inset } else { theme.surfaces.elevated };
                        let min = egui::pos2(rect.left() + tx as f32 * step, rect.top() + ty as f32 * step);
                        painter.rect_filled(egui::Rect::from_min_size(min, egui::Vec2::splat(step)), 0u8, fill);
                    }
                }
                // A colored blob over the checker - a body rect plus a lighter head,
                // tinted from the rotating thumbnail set - so the cell reads as a sprite.
                let tint = theme.mock.thumbnails[i % theme.mock.thumbnails.len()];
                let body = egui::Rect::from_min_size(rect.min + egui::vec2(cell * 0.28, cell * 0.4), egui::Vec2::new(cell * 0.44, cell * 0.46));
                painter.rect_filled(body, theme.radius.sm as u8, tint);
                let head = egui::Rect::from_center_size(rect.center() - egui::vec2(0.0, cell * 0.16), egui::Vec2::splat(cell * 0.34));
                painter.rect_filled(head, theme.radius.sm as u8, tint.gamma_multiply(1.4));
                painter.rect_stroke(
                    rect,
                    theme.radius.sm as u8,
                    egui::Stroke::new(1.0, theme.roles.border),
                    egui::StrokeKind::Inside,
                );
            }
        }
    });
}

/// A monospace log block in secondary text, one line per entry (mock console).
pub fn mock_log(ui: &mut egui::Ui, theme: &Theme, lines: &[&str]) {
    for line in lines {
        ui.label(
            egui::RichText::new(*line)
                .monospace()
                .size(theme.type_scale.mono)
                .color(theme.roles.text_secondary),
        );
    }
}
