//! The elevated card frame for right-dock and tray panels.

use crate::icons;
use crate::theme::Theme;

/// Draw an elevated card: a framed header (`meta.icon` + `meta.title` + a collapse
/// chevron) and, when `!collapsed`, the `body`.
///
/// Collapse state is read-only here. The returned [`egui::Response`] is the header
/// click; the caller maps a click to `Intent::TogglePanelCollapsed` - this widget
/// owns no state and mutates nothing. The shell scopes egui ids via `push_id`
/// around this call, so `card` adds no id salt of its own.
// Radius/spacing tokens are small, bounded positive constants; the f32 -> u8/i8
// casts cannot truncate or lose a sign here.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn card(ui: &mut egui::Ui, theme: &Theme, meta: &crate::contrib_api::PanelMeta, collapsed: bool, body: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let frame = egui::Frame {
        fill: theme.surfaces.elevated,
        inner_margin: egui::Margin::same(theme.spacing.sm as i8),
        corner_radius: egui::CornerRadius::same(theme.radius.md as u8),
        shadow: theme.elevation.raised,
        stroke: egui::Stroke::new(1.0, theme.roles.border),
        ..Default::default()
    };

    let mut header_response = None;
    frame.show(ui, |ui| {
        // Header: icon + title on the left, collapse chevron on the right. The whole
        // strip is one interactive rect so a click anywhere toggles.
        let resp = ui
            .horizontal(|ui| {
                let size = theme.type_scale.section_header;
                ui.label(egui::RichText::new(meta.icon.to_string()).size(size).color(theme.accent.base));
                ui.label(egui::RichText::new(meta.title).size(size).color(theme.roles.text_primary).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let chevron = if collapsed { icons::CARET_RIGHT } else { icons::CARET_DOWN };
                    ui.label(
                        egui::RichText::new(chevron.to_string())
                            .size(theme.type_scale.label)
                            .color(theme.roles.text_secondary),
                    );
                });
            })
            .response
            .interact(egui::Sense::click());
        header_response = Some(resp);

        if !collapsed {
            ui.add_space(theme.spacing.xs);
            body(ui);
        }
    });

    // `frame.show` always runs the closure once, so the header response is set.
    header_response.unwrap_or_else(|| ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover()))
}
