//! Status-bar region: a compact strip. Always-on size/zoom/grid, then the
//! workspace's status items, then the AI status dot colored from `session.ai_status`.

use crate::region::region_id;
use crate::registry::resolve_layout;
use crate::state::Host;
use crate::state::session::AiStatus;
use crate::theme::tokens::SurfaceTier;

/// Render the status bar.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let status_items = resolve_layout(host.state.session.active_workspace, &host.registries).status_items;

    let Host { state, theme, .. } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Elevated)).inner_margin(theme.spacing.xs);

    egui::Panel::bottom(region_id::STATUS_BAR)
        .resizable(false)
        .exact_size(22.0)
        .frame(frame)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                // Always-on items.
                ui.colored_label(theme.roles.text_secondary, "64 x 64");
                ui.separator();
                ui.colored_label(theme.roles.text_secondary, format!("{:.0}%", state.ui.zoom * 100.0));
                ui.separator();
                ui.colored_label(theme.roles.text_secondary, format!("Grid {:?}", state.ui.grid));

                // Workspace-specific items.
                for status_item in &status_items {
                    ui.separator();
                    ui.label(egui::RichText::new(format!("{} {}", status_item.icon, status_item.text)).color(theme.roles.text_secondary));
                }

                // AI status dot, right-aligned.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (color, text) = match state.session.ai_status {
                        AiStatus::Ready => (theme.roles.success, "AI Ready"),
                        AiStatus::Working => (theme.roles.warning, "AI Working"),
                        AiStatus::Offline => (theme.roles.text_disabled, "AI Offline"),
                    };
                    ui.colored_label(theme.roles.text_secondary, text);
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, color);
                });
            });
        });
}
