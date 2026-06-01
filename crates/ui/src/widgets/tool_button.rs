//! The left-rail tool button: icon, active accent tint + left line, AI marker.

use crate::icons;
use crate::theme::Theme;

/// Draw a rail tool button. `active` paints an `accent.muted` background and a 2px
/// `accent.base` left line; `meta.is_ai` tints the glyph `accent.ai` and overlays a
/// sparkle. The tooltip reads `"{label} ({shortcut})\n{tooltip}"` (the shortcut
/// clause is dropped when the tool has none).
// Radius tokens are small, bounded positive constants; the f32 -> u8 casts cannot
// truncate or lose a sign here.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn tool_button(ui: &mut egui::Ui, theme: &Theme, meta: &crate::contrib_api::ToolMeta, active: bool) -> egui::Response {
    let side = 40.0;
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(side), egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Active background fill, then the 2px accent left line.
        if active {
            painter.rect_filled(rect, theme.radius.sm as u8, theme.accent.muted);
            let line_x = rect.left() + 1.0;
            painter.line_segment(
                [egui::pos2(line_x, rect.top() + 2.0), egui::pos2(line_x, rect.bottom() - 2.0)],
                egui::Stroke::new(2.0, theme.accent.base),
            );
        } else if response.hovered() {
            painter.rect_filled(rect, theme.radius.sm as u8, theme.accent.muted.gamma_multiply(0.5));
        }

        // The glyph. AI tools paint in the AI accent; everything else uses primary
        // text, brightened to the accent when active.
        let glyph_color = if meta.is_ai {
            theme.accent.ai
        } else if active {
            theme.accent.base
        } else {
            theme.roles.text_secondary
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            meta.icon.to_string(),
            egui::FontId::proportional(theme.type_scale.title),
            glyph_color,
        );

        // AI marker: a small sparkle in the top-right corner.
        if meta.is_ai {
            painter.text(
                egui::pos2(rect.right() - 4.0, rect.top() + 4.0),
                egui::Align2::RIGHT_TOP,
                icons::SPARKLE.to_string(),
                egui::FontId::proportional(theme.type_scale.label),
                theme.accent.ai,
            );
        }
    }

    let tooltip = match meta.shortcut {
        Some(shortcut) => format!("{} ({})\n{}", meta.label, ui.ctx().format_shortcut(&shortcut), meta.tooltip),
        None => format!("{}\n{}", meta.label, meta.tooltip),
    };
    response.on_hover_text(tooltip)
}
