//! Theme token system: semantic roles, surfaces, accent, spacing, type, radii.
//! Dark-first; light and accent-high-contrast variants share the same role set.
//!
//! Filled by the theme layer: `Theme`, `ThemeVariant`, `apply_to_visuals`,
//! `install_fonts`, and the token structs in `tokens`/`palettes`/`contrast`.

mod contrast;
mod fonts;
mod palettes;
pub mod tokens;

pub use contrast::wcag_contrast;
pub use fonts::install_fonts;
pub use palettes::DEFAULT_ACCENT_SEED;
pub use tokens::{AccentTokens, Elevation, Radii, Roles, SurfaceTier, Surfaces, Theme, ThemeVariant, TypeScale};

/// Map theme tokens onto egui's `Visuals`/`Style`. Called once at boot and re-applied
/// by `apply_intent` on a variant change so a theme switch actually repaints. Uses
/// `global_style_mut` to avoid cloning the whole style.
pub fn apply_to_visuals(theme: &Theme, ctx: &egui::Context) {
    ctx.global_style_mut(|style| {
        let v = &mut style.visuals;
        v.dark_mode = theme.variant != ThemeVariant::Light;
        v.panel_fill = theme.surfaces.panel;
        v.window_fill = theme.surfaces.elevated;
        v.extreme_bg_color = theme.surfaces.inset;
        v.faint_bg_color = theme.surfaces.elevated;
        v.override_text_color = Some(theme.roles.text_primary);
        v.hyperlink_color = theme.accent.base;
        v.selection.bg_fill = theme.accent.muted;
        v.selection.stroke = egui::Stroke::new(1.0, theme.accent.base);
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, theme.roles.border);
        v.widgets.hovered.bg_fill = theme.accent.muted;
        v.widgets.active.bg_fill = theme.accent.base;
        v.window_shadow = theme.elevation.overlay;
        v.popup_shadow = theme.elevation.overlay;
        style.spacing.item_spacing = egui::vec2(theme.spacing.sm, theme.spacing.xs);
        style.spacing.button_padding = egui::vec2(theme.spacing.sm, theme.spacing.xs);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(theme.type_scale.body));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(theme.type_scale.title));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(theme.type_scale.label));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, egui::FontId::monospace(theme.type_scale.mono));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Context;

    #[test]
    fn dark_panel_fill_maps_from_surfaces_panel() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert_eq!(ctx.global_style().visuals.panel_fill, theme.surfaces.panel);
    }

    #[test]
    fn dark_selection_stroke_maps_from_accent_base() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert_eq!(ctx.global_style().visuals.selection.stroke.color, theme.accent.base);
    }

    #[test]
    fn dark_extreme_bg_maps_from_inset() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert_eq!(ctx.global_style().visuals.extreme_bg_color, theme.surfaces.inset);
    }

    #[test]
    fn dark_override_text_color_maps_from_text_primary() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert_eq!(ctx.global_style().visuals.override_text_color, Some(theme.roles.text_primary));
    }

    #[test]
    fn light_variant_sets_dark_mode_false() {
        let theme = Theme::light();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert!(!ctx.global_style().visuals.dark_mode);
    }

    #[test]
    fn dark_variant_sets_dark_mode_true() {
        let theme = Theme::dark();
        let ctx = Context::default();
        apply_to_visuals(&theme, &ctx);
        assert!(ctx.global_style().visuals.dark_mode);
    }
}
