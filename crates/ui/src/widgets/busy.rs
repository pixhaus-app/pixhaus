//! A spinner + label row that signals work in progress (e.g. AI generation).

use crate::theme::Theme;

/// Draw a spinner beside `label`, the in-progress indicator for long work like AI
/// generation. The spinner paints in `accent.ai` (the AI affordance tint) and
/// animates itself — egui's `Spinner` requests a repaint each frame, so it keeps
/// turning through a long job without extra plumbing. Passive: no interaction, no
/// `Response`.
pub fn busy_indicator(ui: &mut egui::Ui, theme: &Theme, label: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(theme.type_scale.body).color(theme.accent.ai));
        ui.add_space(theme.spacing.xs);
        ui.label(egui::RichText::new(label).size(theme.type_scale.label).color(theme.roles.text_secondary));
    });
}
