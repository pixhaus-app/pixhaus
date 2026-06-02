//! About dialog overlay: an `egui::Area` drawn after the central panel, gated on
//! `UiState.modal == About`. Shows the brand wordmark, the crate version, the MIT
//! license line, and a one-line tagline. Escape or the Close button dismisses.
//!
//! Mirrors [`crate::shell::command_palette::overlay`] - the same foreground `Area`,
//! elevated frame, and `CloseModal` dismiss path.

use pixhaus_services::i18n;

use crate::state::Host;
use crate::state::intent::Intent;
use crate::state::ui_state::Modal;
use crate::theme::tokens::SurfaceTier;

/// The crate version, baked at compile time. The workspace pins one version across
/// every crate, so `pixhaus-ui`'s version is the app's version.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The widest the wordmark may render, in points. Tuned so it sits comfortably inside
/// the dialog without overflowing the frame; NEAREST keeps the pixel letters crisp.
const WORDMARK_MAX_WIDTH: f32 = 320.0;

/// Draw the About overlay when its modal is open.
pub fn overlay(host: &mut Host, ui: &mut egui::Ui) {
    if !matches!(host.state.ui.modal, Some(Modal::About)) {
        return;
    }

    // Escape closes.
    if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
        host.intents.push(Intent::CloseModal);
        return;
    }

    let Host { intents, theme, .. } = &mut *host;

    // content_rect is the area available to the UI (screen_rect was split in 0.34).
    let content = ui.ctx().content_rect();
    let area_pos = egui::pos2(content.center().x - 200.0, content.center().y - 140.0);

    egui::Area::new(egui::Id::new("pixhaus.about"))
        .order(egui::Order::Foreground)
        .fixed_pos(area_pos)
        .show(ui.ctx(), |ui| {
            let frame = egui::Frame::new()
                .fill(theme.surface(SurfaceTier::Elevated))
                .inner_margin(theme.spacing.xl)
                .corner_radius(theme.radius.md)
                .shadow(theme.elevation.overlay);
            frame.show(ui, |ui| {
                ui.set_min_width(400.0);
                ui.vertical_centered(|ui| {
                    ui.add(
                        egui::Image::new(crate::brand::WORDMARK)
                            .texture_options(egui::TextureOptions::NEAREST)
                            .max_width(WORDMARK_MAX_WIDTH)
                            .maintain_aspect_ratio(true),
                    );
                    ui.add_space(theme.spacing.md);
                    ui.colored_label(theme.roles.text_secondary, i18n::tr_args("app.ui.about.version", &[("version", VERSION)]));
                    ui.colored_label(theme.roles.text_secondary, i18n::tr("app.ui.about.license"));
                    ui.add_space(theme.spacing.sm);
                    ui.colored_label(theme.roles.text_secondary, i18n::tr("app.ui.about.tagline"));
                    ui.add_space(theme.spacing.lg);
                    if ui.button(i18n::tr("app.ui.action.close")).clicked() {
                        intents.push(Intent::CloseModal);
                    }
                });
            });
        });
}
