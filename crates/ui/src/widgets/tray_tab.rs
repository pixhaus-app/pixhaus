//! The bottom-tray tab chip: a solid accent pill when active.

use crate::theme::Theme;

/// Draw a tray tab chip. Active paints a solid `accent.muted` pill; inactive is bare
/// text. Returns the click `Response`; the caller maps a click to
/// `Intent::SelectTrayTab`.
// Radius tokens are small, bounded positive constants; the f32 -> u8 cast cannot
// truncate or lose a sign here.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn tray_tab(ui: &mut egui::Ui, theme: &Theme, title: &str, active: bool) -> egui::Response {
    let font = egui::FontId::proportional(theme.type_scale.label);
    let galley = ui.painter().layout_no_wrap(title.to_owned(), font.clone(), theme.roles.text_primary);
    let pad = egui::vec2(theme.spacing.sm, theme.spacing.xs);
    let size = galley.size() + pad * 2.0;
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if active || response.hovered() {
            painter.rect_filled(rect, theme.radius.md as u8, theme.accent.muted);
        }
        let text_color = if active { theme.roles.text_primary } else { theme.roles.text_secondary };
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, title, font, text_color);
    }
    response
}
