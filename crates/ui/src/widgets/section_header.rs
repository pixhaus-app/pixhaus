//! Icon + title header, used as a card title and as an in-body section divider.

use crate::theme::Theme;

/// Draw an `icon title` header row at the section-header type scale.
///
/// `icon` is a `crate::icons` phosphor glyph. The glyph paints in `accent.base`,
/// the title in `text_primary`. Passive: no interaction, no `Response`.
pub fn section_header(ui: &mut egui::Ui, theme: &Theme, icon: char, title: &str) {
    let size = theme.type_scale.section_header;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon.to_string()).size(size).color(theme.accent.base));
        ui.label(egui::RichText::new(title).size(size).color(theme.roles.text_primary).strong());
    });
}
