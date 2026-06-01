//! Inert mock-content helpers: a list row, a thumbnail grid, a log block. These
//! stand in for real panel data until `core` lands. All passive - no interaction.

use crate::theme::Theme;

/// One labeled list row at body type scale, secondary text color.
pub fn mock_row(ui: &mut egui::Ui, theme: &Theme, label: &str) {
    ui.label(egui::RichText::new(label).size(theme.type_scale.body).color(theme.roles.text_secondary));
}

/// A wrapping grid of `n` checkerboard thumbnail rects (mock sprites / assets /
/// tiles). Each cell is a small two-tone checker so transparent bounds read.
// The radius token is a small, bounded positive constant (f32 -> u8), and the 0..4
// checker indices are exactly representable as f32 - neither cast loses anything.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
pub fn mock_thumbnail_grid(ui: &mut egui::Ui, theme: &Theme, n: usize) {
    let cell = 44.0;
    ui.horizontal_wrapped(|ui| {
        for _ in 0..n {
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(cell), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                // 4x4 checker inside the cell.
                let step = cell / 4.0;
                for ty in 0..4 {
                    for tx in 0..4 {
                        let dark = (tx + ty) % 2 == 0;
                        let fill = if dark { theme.surfaces.inset } else { theme.surfaces.elevated };
                        let min = egui::pos2(rect.left() + tx as f32 * step, rect.top() + ty as f32 * step);
                        painter.rect_filled(egui::Rect::from_min_size(min, egui::Vec2::splat(step)), 0u8, fill);
                    }
                }
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
