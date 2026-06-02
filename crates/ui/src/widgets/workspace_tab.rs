//! The top-bar workspace tab: violet pill plus an accent underline when active.

use crate::theme::Theme;

/// Draw a workspace tab. Active paints an `accent.muted` pill, an `accent.base`
/// underline, and brighter text. Returns the click `Response`; the caller maps a
/// click to `Intent::SelectWorkspace`.
// Radius tokens are small, bounded positive constants; the f32 -> u8 cast cannot
// truncate or lose a sign here.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn workspace_tab(ui: &mut egui::Ui, theme: &Theme, name: &str, active: bool) -> egui::Response {
    let font = egui::FontId::proportional(theme.type_scale.body);
    let galley = ui.painter().layout_no_wrap(name.to_owned(), font.clone(), theme.roles.text_primary);
    let pad = egui::vec2(theme.spacing.md, theme.spacing.sm);
    let size = galley.size() + pad * 2.0;
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if active {
            painter.rect_filled(rect, theme.radius.md as u8, theme.accent.muted);
            painter.line_segment(
                [
                    egui::pos2(rect.left() + theme.spacing.sm, rect.bottom() - 1.0),
                    egui::pos2(rect.right() - theme.spacing.sm, rect.bottom() - 1.0),
                ],
                egui::Stroke::new(2.5, theme.accent.base),
            );
        }
        let text_color = if active { theme.roles.text_primary } else { theme.roles.text_secondary };
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, name, font, text_color);
    }
    response
}
