//! Theme construction. `for_variant` is the single source; `dark`/`light`/
//! `accent_high_contrast` are named wrappers. Accent tokens derive from one seed so
//! a future accent preference recolors independently of light/dark. Only `dark()` is
//! visually tuned this round; the other variants are structured in.

use egui::Color32;

use super::tokens::{AccentTokens, Elevation, Radii, Roles, Spacing, SurfaceTier, Surfaces, Theme, ThemeVariant, TypeScale};

/// Default accent seed: a warm violet (~#7c6cef).
pub const DEFAULT_ACCENT_SEED: Color32 = Color32::from_rgb(0x7c, 0x6c, 0xef);

impl Theme {
    /// The tuned dark theme - the only finished variant this round.
    pub fn dark() -> Self {
        Self::for_variant(ThemeVariant::Dark, DEFAULT_ACCENT_SEED)
    }

    /// Build a theme for a variant with a given accent seed.
    pub fn for_variant(v: ThemeVariant, accent_seed: Color32) -> Self {
        let accent = accent_from_seed(accent_seed);
        let spacing = Spacing {
            xs: 2.0,
            sm: 4.0,
            md: 8.0,
            lg: 12.0,
            xl: 16.0,
        };
        let type_scale = TypeScale {
            label: 11.0,
            body: 13.0,
            section_header: 13.0,
            title: 15.0,
            mono: 12.0,
        };
        let radius = Radii { sm: 2.0, md: 3.0 };
        let elevation = Elevation {
            raised: egui::epaint::Shadow {
                offset: [0, 2],
                blur: 8,
                spread: 0,
                color: Color32::from_black_alpha(96),
            },
            overlay: egui::epaint::Shadow {
                offset: [0, 6],
                blur: 24,
                spread: 0,
                color: Color32::from_black_alpha(128),
            },
        };

        let (surfaces, roles) = match v {
            ThemeVariant::Dark => (
                Surfaces {
                    app_frame: Color32::from_rgb(0x12, 0x11, 0x16),
                    panel: Color32::from_rgb(0x1b, 0x1a, 0x20),
                    elevated: Color32::from_rgb(0x24, 0x22, 0x2b),
                    stage: Color32::from_rgb(0x0d, 0x0c, 0x10),
                    inset: Color32::from_rgb(0x15, 0x14, 0x19),
                },
                Roles {
                    border: Color32::from_rgb(0x33, 0x31, 0x3c),
                    text_primary: Color32::from_rgb(0xe6, 0xe3, 0xef),
                    text_secondary: Color32::from_rgb(0xa8, 0xa4, 0xb4),
                    text_disabled: Color32::from_rgb(0x6c, 0x69, 0x77),
                    success: Color32::from_rgb(0x6f, 0xb5, 0x84),
                    warning: Color32::from_rgb(0xd1, 0xa8, 0x5f),
                    error: Color32::from_rgb(0xd1, 0x6f, 0x6f),
                },
            ),
            ThemeVariant::Light => (
                // Structured in, not tuned this round. Values clear the no-black-leak
                // floor; visual tuning is a follow-up.
                Surfaces {
                    app_frame: Color32::from_rgb(0xd8, 0xd6, 0xde),
                    panel: Color32::from_rgb(0xec, 0xea, 0xf0),
                    elevated: Color32::from_rgb(0xf6, 0xf5, 0xf9),
                    stage: Color32::from_rgb(0xc8, 0xc6, 0xd0),
                    inset: Color32::from_rgb(0xff, 0xff, 0xff),
                },
                Roles {
                    border: Color32::from_rgb(0xc2, 0xc0, 0xcc),
                    text_primary: Color32::from_rgb(0x1b, 0x1a, 0x20),
                    text_secondary: Color32::from_rgb(0x53, 0x50, 0x5e),
                    text_disabled: Color32::from_rgb(0x9a, 0x97, 0xa6),
                    success: Color32::from_rgb(0x2f, 0x7d, 0x4c),
                    warning: Color32::from_rgb(0x8a, 0x66, 0x1f),
                    error: Color32::from_rgb(0x9a, 0x33, 0x33),
                },
            ),
            ThemeVariant::AccentHighContrast => (
                // Structured in, not tuned this round.
                Surfaces {
                    app_frame: Color32::from_rgb(0x00, 0x00, 0x00),
                    panel: Color32::from_rgb(0x0a, 0x0a, 0x0d),
                    elevated: Color32::from_rgb(0x16, 0x15, 0x1c),
                    stage: Color32::from_rgb(0x00, 0x00, 0x00),
                    inset: Color32::from_rgb(0x05, 0x05, 0x07),
                },
                Roles {
                    border: accent.base,
                    text_primary: Color32::from_rgb(0xff, 0xff, 0xff),
                    text_secondary: Color32::from_rgb(0xd6, 0xd4, 0xe4),
                    text_disabled: Color32::from_rgb(0x8a, 0x87, 0x99),
                    success: Color32::from_rgb(0x7c, 0xe0, 0x9a),
                    warning: Color32::from_rgb(0xf0, 0xc8, 0x6f),
                    error: Color32::from_rgb(0xf0, 0x8a, 0x8a),
                },
            ),
        };

        Self {
            variant: v,
            surfaces,
            roles,
            accent,
            elevation,
            spacing,
            type_scale,
            radius,
        }
    }

    /// The light variant (structured in, not visually tuned this round).
    pub fn light() -> Self {
        Self::for_variant(ThemeVariant::Light, DEFAULT_ACCENT_SEED)
    }

    /// The accent-high-contrast variant (structured in, not tuned this round).
    pub fn accent_high_contrast() -> Self {
        Self::for_variant(ThemeVariant::AccentHighContrast, DEFAULT_ACCENT_SEED)
    }

    /// The seed the accent tokens were derived from (the separable preference axis).
    pub fn accent_seed(&self) -> Color32 {
        self.accent.seed
    }

    /// Resolve a surface tier to its color at runtime.
    pub fn surface(&self, t: SurfaceTier) -> Color32 {
        match t {
            SurfaceTier::AppFrame => self.surfaces.app_frame,
            SurfaceTier::Panel => self.surfaces.panel,
            SurfaceTier::Elevated => self.surfaces.elevated,
            SurfaceTier::Stage => self.surfaces.stage,
            SurfaceTier::Inset => self.surfaces.inset,
        }
    }
}

/// Lighten each channel toward white by `t` (0.0 = unchanged, 1.0 = white).
// The `.round().clamp(0.0, 255.0)` bounds the value to a valid u8 before the cast,
// so truncation and sign loss cannot occur.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn lighten(c: Color32, t: f32) -> Color32 {
    let mix = |ch: u8| -> u8 {
        let v = f32::from(ch);
        (v + (255.0 - v) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

/// Darken each channel toward black by `t` (0.0 = unchanged, 1.0 = black).
// The `.round().clamp(0.0, 255.0)` bounds the value to a valid u8 before the cast,
// so truncation and sign loss cannot occur.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn darken(c: Color32, t: f32) -> Color32 {
    let mix = |ch: u8| -> u8 { (f32::from(ch) * (1.0 - t)).round().clamp(0.0, 255.0) as u8 };
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

/// Derive the full accent token set from one seed.
fn accent_from_seed(seed: Color32) -> AccentTokens {
    AccentTokens {
        seed,
        base: seed,
        hover: lighten(seed, 0.15),
        // Low-alpha fill; the muted value the contrast test reads is the opaque
        // mix of the seed darkened toward the dark panel, a conservative proxy.
        muted: darken(seed, 0.55),
        ai: lighten(seed, 0.10),
        ai_glow: Color32::from_rgba_unmultiplied(seed.r(), seed.g(), seed.b(), 40),
    }
}

#[cfg(test)]
mod tests {
    use super::super::contrast::wcag_contrast;
    use super::*;

    /// Perceptual lightness proxy (sRGB luma), only used to order surface tiers.
    fn luma(c: Color32) -> f32 {
        0.2126 * f32::from(c.r()) + 0.7152 * f32::from(c.g()) + 0.0722 * f32::from(c.b())
    }

    /// No role color may be left at the default all-zero black (a population leak).
    fn assert_no_black_leak(theme: &Theme) {
        for (name, c) in [
            ("border", theme.roles.border),
            ("text_primary", theme.roles.text_primary),
            ("text_secondary", theme.roles.text_secondary),
            ("text_disabled", theme.roles.text_disabled),
            ("success", theme.roles.success),
            ("warning", theme.roles.warning),
            ("error", theme.roles.error),
            ("accent.base", theme.accent.base),
            ("accent.hover", theme.accent.hover),
            ("accent.ai", theme.accent.ai),
        ] {
            assert_ne!(c, Color32::BLACK, "{name} left at default black");
        }
    }

    #[test]
    fn dark_uses_the_default_accent_seed() {
        assert_eq!(Theme::dark().accent_seed(), DEFAULT_ACCENT_SEED);
    }

    #[test]
    fn every_variant_populates_every_role() {
        assert_no_black_leak(&Theme::dark());
        assert_no_black_leak(&Theme::light());
        assert_no_black_leak(&Theme::accent_high_contrast());
    }

    #[test]
    fn dark_surface_tiers_are_ordered_by_lightness() {
        let t = Theme::dark();
        // app_frame is darkest; panel sits above it; elevated above that.
        assert!(luma(t.surfaces.app_frame) < luma(t.surfaces.panel), "app_frame must be darker than panel");
        assert!(luma(t.surfaces.panel) < luma(t.surfaces.elevated), "panel must be darker than elevated");
    }

    #[test]
    fn surface_helper_matches_fields() {
        let t = Theme::dark();
        assert_eq!(t.surface(SurfaceTier::AppFrame), t.surfaces.app_frame);
        assert_eq!(t.surface(SurfaceTier::Panel), t.surfaces.panel);
        assert_eq!(t.surface(SurfaceTier::Elevated), t.surfaces.elevated);
        assert_eq!(t.surface(SurfaceTier::Stage), t.surfaces.stage);
        assert_eq!(t.surface(SurfaceTier::Inset), t.surfaces.inset);
    }

    #[test]
    fn dark_text_meets_wcag_floors() {
        let t = Theme::dark();
        assert!(
            wcag_contrast(t.roles.text_primary, t.surfaces.panel) >= 4.5,
            "text_primary on panel below 4.5: {}",
            wcag_contrast(t.roles.text_primary, t.surfaces.panel)
        );
        assert!(
            wcag_contrast(t.roles.text_secondary, t.surfaces.panel) >= 4.5,
            "text_secondary on panel below 4.5: {}",
            wcag_contrast(t.roles.text_secondary, t.surfaces.panel)
        );
        assert!(
            wcag_contrast(t.roles.text_primary, t.surfaces.elevated) >= 4.5,
            "text_primary on elevated below 4.5: {}",
            wcag_contrast(t.roles.text_primary, t.surfaces.elevated)
        );
        assert!(
            wcag_contrast(t.roles.text_primary, t.accent.muted) >= 3.0,
            "text_primary on accent.muted below 3.0: {}",
            wcag_contrast(t.roles.text_primary, t.accent.muted)
        );
    }
}
