//! Status-bar region: a compact strip. Always-on size/zoom/grid, then the
//! workspace's status items, then the AI status dot colored from `session.ai_status`.

use pixhaus_services::i18n;

use crate::region::region_id;
use crate::registry::resolve_layout;
use crate::state::Host;
use crate::state::session::AiStatus;
use crate::theme::tokens::SurfaceTier;

/// Render the status bar.
pub fn show(host: &mut Host, ui: &mut egui::Ui) {
    let status_items = resolve_layout(host.state.session.active_workspace, &host.registries).status_items;

    let (sprite_w, sprite_h) = host.edit.document.active_sprite_size().unwrap_or(pixhaus_core::DEFAULT_CANVAS_SIZE);

    let Host { state, theme, .. } = &mut *host;

    let frame = egui::Frame::new().fill(theme.surface(SurfaceTier::Elevated)).inner_margin(theme.spacing.xs);

    egui::Panel::bottom(region_id::STATUS_BAR)
        .resizable(false)
        .exact_size(22.0)
        .frame(frame)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                // Always-on items. The canvas size and zoom are live now (size from the
                // active sprite, or the default board when nothing is open); both are
                // numeric and locale-neutral, so they are deliberately not i18n keys. The
                // "Grid" label is a real phrase, so it is keyed - the mode value
                // (Off/Px8/Px16) stays a short technical token shared with the canvas HUD.
                ui.colored_label(theme.roles.text_secondary, format!("{sprite_w} x {sprite_h}"));
                ui.separator();
                ui.colored_label(theme.roles.text_secondary, format!("{:.0}%", state.ui.zoom * 100.0));
                ui.separator();
                let grid = format!("{:?}", state.ui.grid);
                ui.colored_label(theme.roles.text_secondary, i18n::tr_args("app.ui.status.grid", &[("mode", &grid)]));

                // Workspace-specific items.
                for status_item in &status_items {
                    ui.separator();
                    ui.label(egui::RichText::new(format!("{} {}", status_item.icon, status_item.text)).color(theme.roles.text_secondary));
                }

                // AI status dot, right-aligned.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (color, key) = match state.session.ai_status {
                        AiStatus::Ready => (theme.roles.success, "app.ui.ai_status.ready"),
                        AiStatus::Working => (theme.roles.warning, "app.ui.ai_status.working"),
                        AiStatus::Offline => (theme.roles.text_disabled, "app.ui.ai_status.offline"),
                    };
                    ui.colored_label(theme.roles.text_secondary, i18n::tr(key));
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 4.0, color);
                });
            });
        });
}
