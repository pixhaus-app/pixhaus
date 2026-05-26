//! Native look and feel for the Pixhaus shell.
//!
//! Pixhaus is dark-first but follows the OS by default. The brand palette
//! (Pixhaus Indigo on blue-tinted neutrals, mirrored from `brand/tokens.css`)
//! is mapped onto egui [`Visuals`] for both themes, the Geist typeface is
//! bundled so type is identical on every OS, and the metrics are widened for a
//! flatter, more modern feel than egui's defaults.
//!
//! [`install`] registers both themes once at startup; egui then switches
//! between them when the system appearance changes or the user overrides the
//! preference, with no per-frame re-apply.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow,
    Stroke, Style, TextStyle, Theme, ThemePreference, Visuals,
};

/// Semantic brand colors for one theme. Field meanings match the
/// `--color-*` tokens in `brand/tokens.css`.
#[derive(Clone, Copy)]
pub struct Palette {
    /// Window backdrop and inset (text-edit / canvas) background.
    pub bg_app: Color32,
    /// Side and bottom panel fill.
    pub bg_panel: Color32,
    /// Raised surfaces: buttons, group frames, popups.
    pub bg_elevated: Color32,
    /// Hover state fill.
    pub bg_hover: Color32,
    /// Pressed / open state fill.
    pub bg_active: Color32,
    /// Text-selection highlight.
    pub selection: Color32,
    /// Hairline separators and widget outlines.
    pub border: Color32,
    /// Stronger outline for open / focused widgets.
    pub border_strong: Color32,
    /// Primary text (egui derives muted/`.weak()` text from this).
    pub text_primary: Color32,
    /// Text drawn on top of an accent fill.
    pub text_on_accent: Color32,
    /// Brand accent (Pixhaus Indigo) — identical in both themes.
    pub accent: Color32,
    /// Accent hover.
    pub accent_hover: Color32,
    /// Status: success (also used for the "backend ready" indicator).
    pub success: Color32,
    /// Status: warning (maps to `Visuals::warn_fg_color`).
    pub warning: Color32,
    /// Status: error (maps to `Visuals::error_fg_color`).
    pub error: Color32,
}

impl Palette {
    /// Dark theme (default, dark-first brand).
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            bg_app: Color32::from_rgb(0x0f, 0x0f, 0x13),
            bg_panel: Color32::from_rgb(0x17, 0x17, 0x1e),
            bg_elevated: Color32::from_rgb(0x1e, 0x1e, 0x27),
            bg_hover: Color32::from_rgb(0x25, 0x25, 0x30),
            bg_active: Color32::from_rgb(0x2c, 0x2c, 0x3a),
            selection: Color32::from_rgb(0x2a, 0x3a, 0x5c),
            border: Color32::from_rgb(0x2a, 0x2a, 0x35),
            border_strong: Color32::from_rgb(0x3d, 0x3d, 0x50),
            text_primary: Color32::from_rgb(0xe6, 0xe6, 0xf0),
            text_on_accent: Color32::WHITE,
            accent: ACCENT,
            accent_hover: Color32::from_rgb(0x90, 0x80, 0xf8),
            success: Color32::from_rgb(0x4c, 0xaf, 0x6e),
            warning: Color32::from_rgb(0xe8, 0xa5, 0x34),
            error: Color32::from_rgb(0xe0, 0x52, 0x52),
        }
    }

    /// Light theme. The accent stays indigo (the old web tokens drifted to a
    /// blue accent in light mode; we keep the brand identity consistent).
    #[must_use]
    pub const fn light() -> Self {
        Self {
            bg_app: Color32::from_rgb(0xf0, 0xf0, 0xf4),
            bg_panel: Color32::from_rgb(0xf8, 0xf8, 0xfc),
            bg_elevated: Color32::from_rgb(0xff, 0xff, 0xff),
            bg_hover: Color32::from_rgb(0xe8, 0xe8, 0xf0),
            bg_active: Color32::from_rgb(0xdc, 0xdc, 0xe8),
            selection: Color32::from_rgb(0xd0, 0xe4, 0xff),
            border: Color32::from_rgb(0xd4, 0xd4, 0xe0),
            border_strong: Color32::from_rgb(0xb0, 0xb0, 0xc8),
            text_primary: Color32::from_rgb(0x1a, 0x1a, 0x2e),
            text_on_accent: Color32::WHITE,
            accent: ACCENT,
            accent_hover: Color32::from_rgb(0x6c, 0x5a, 0xe0),
            success: Color32::from_rgb(0x2e, 0x8a, 0x50),
            warning: Color32::from_rgb(0xc0, 0x78, 0x20),
            error: Color32::from_rgb(0xc4, 0x30, 0x30),
        }
    }

    /// The palette egui should use for `theme`.
    #[must_use]
    pub const fn for_theme(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self::dark(),
            Theme::Light => Self::light(),
        }
    }

    /// Maps this palette onto a full [`Visuals`], starting from egui's default
    /// for the theme so anything we don't set keeps a sane value.
    #[must_use]
    pub fn visuals(&self, theme: Theme) -> Visuals {
        let dark = theme == Theme::Dark;
        let mut v = theme.default_visuals();

        v.panel_fill = self.bg_panel;
        v.window_fill = self.bg_elevated;
        v.window_stroke = Stroke::new(1.0, self.border);
        v.window_corner_radius = CornerRadius::same(RADIUS_WINDOW);
        v.extreme_bg_color = self.bg_app;
        v.faint_bg_color = self.bg_hover;
        v.code_bg_color = self.bg_app;
        v.hyperlink_color = self.accent;
        v.warn_fg_color = self.warning;
        v.error_fg_color = self.error;

        v.selection.bg_fill = self.selection;
        v.selection.stroke = Stroke::new(1.0, self.accent);

        // Soft, flat shadow rather than egui's heavier default.
        let shadow = Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(if dark { 120 } else { 40 }),
        };
        v.window_shadow = shadow;
        v.popup_shadow = Shadow {
            offset: [0, 2],
            blur: 10,
            ..shadow
        };

        let w = &mut v.widgets;

        // Non-interactive: panel text, separators, labels.
        w.noninteractive.bg_fill = self.bg_panel;
        w.noninteractive.weak_bg_fill = self.bg_panel;
        w.noninteractive.bg_stroke = Stroke::new(1.0, self.border);
        w.noninteractive.fg_stroke = Stroke::new(1.0, self.text_primary);
        w.noninteractive.corner_radius = CornerRadius::same(RADIUS_WIDGET);

        // Inactive: a button or field at rest.
        w.inactive.bg_fill = self.bg_elevated;
        w.inactive.weak_bg_fill = self.bg_elevated;
        w.inactive.bg_stroke = Stroke::new(1.0, self.border);
        w.inactive.fg_stroke = Stroke::new(1.0, self.text_primary);
        w.inactive.corner_radius = CornerRadius::same(RADIUS_WIDGET);

        // Hovered: accent-tinted outline.
        w.hovered.bg_fill = self.bg_hover;
        w.hovered.weak_bg_fill = self.bg_hover;
        w.hovered.bg_stroke = Stroke::new(1.0, self.accent);
        w.hovered.fg_stroke = Stroke::new(1.0, self.text_primary);
        w.hovered.corner_radius = CornerRadius::same(RADIUS_WIDGET);

        // Active: pressed — fill with the accent, text on top of it.
        w.active.bg_fill = self.accent;
        w.active.weak_bg_fill = self.bg_active;
        w.active.bg_stroke = Stroke::new(1.0, self.accent_hover);
        w.active.fg_stroke = Stroke::new(1.0, self.text_on_accent);
        w.active.corner_radius = CornerRadius::same(RADIUS_WIDGET);

        // Open: an expanded combo box or menu.
        w.open.bg_fill = self.bg_active;
        w.open.weak_bg_fill = self.bg_active;
        w.open.bg_stroke = Stroke::new(1.0, self.border_strong);
        w.open.fg_stroke = Stroke::new(1.0, self.text_primary);
        w.open.corner_radius = CornerRadius::same(RADIUS_WIDGET);

        v
    }
}

/// Pixhaus Indigo — the brand primary (`#7c6cef`), shared by both themes.
const ACCENT: Color32 = Color32::from_rgb(0x7c, 0x6c, 0xef);

/// Corner radius for widgets (brand `--radius-md`).
const RADIUS_WIDGET: u8 = 5;
/// Corner radius for windows and menus (brand `--radius-lg`).
const RADIUS_WINDOW: u8 = 8;

/// Family name for the medium-weight Geist used by headings.
const GEIST_MEDIUM: &str = "Geist-Medium";

/// Registers the bundled Geist faces, keeping egui's default fallbacks (Latin
/// extras, CJK, emoji) so existing symbol glyphs (▶ ⏸ ✓) still render.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "Geist".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Geist-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        GEIST_MEDIUM.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/Geist-Medium.ttf"
        ))),
    );
    fonts.font_data.insert(
        "GeistMono".to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/GeistMono-Regular.ttf"
        ))),
    );

    if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
        proportional.insert(0, "Geist".to_owned());
    }
    if let Some(monospace) = fonts.families.get_mut(&FontFamily::Monospace) {
        monospace.insert(0, "GeistMono".to_owned());
    }

    // A named family for headings: medium weight first, then the proportional
    // chain (now Geist-led) for fallback glyphs.
    let mut heading = vec![GEIST_MEDIUM.to_owned()];
    if let Some(proportional) = fonts.families.get(&FontFamily::Proportional) {
        heading.extend(proportional.iter().cloned());
    }
    fonts
        .families
        .insert(FontFamily::Name(GEIST_MEDIUM.into()), heading);

    ctx.set_fonts(fonts);
}

/// Sizes the text styles to the brand type scale and widens the spacing for a
/// flatter, roomier layout than egui's compact defaults.
fn configure_metrics(style: &mut Style) {
    let proportional = FontFamily::Proportional;
    let monospace = FontFamily::Monospace;
    let medium = FontFamily::Name(GEIST_MEDIUM.into());

    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, proportional.clone())),
        (TextStyle::Body, FontId::new(13.0, proportional.clone())),
        (TextStyle::Button, FontId::new(13.0, proportional)),
        (TextStyle::Heading, FontId::new(18.0, medium)),
        (TextStyle::Monospace, FontId::new(12.5, monospace)),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = egui::vec2(8.0, 6.0);
    s.button_padding = egui::vec2(8.0, 4.0);
    s.menu_margin = Margin::same(6);
    s.window_margin = Margin::same(10);
    s.indent = 16.0;
    s.interact_size.y = 24.0;
}

/// Installs fonts, both themed palettes, and the shared metrics, then applies
/// the saved theme preference. Call once from `ShellApp::new`.
pub fn install(ctx: &egui::Context, preference: ThemePreference) {
    install_fonts(ctx);
    ctx.set_visuals_of(Theme::Dark, Palette::dark().visuals(Theme::Dark));
    ctx.set_visuals_of(Theme::Light, Palette::light().visuals(Theme::Light));
    ctx.all_styles_mut(configure_metrics);
    ctx.set_theme(preference);
}

/// Cycles the preference for the toolbar toggle: System -> Dark -> Light -> …
#[must_use]
pub fn next_preference(preference: ThemePreference) -> ThemePreference {
    match preference {
        ThemePreference::System => ThemePreference::Dark,
        ThemePreference::Dark => ThemePreference::Light,
        ThemePreference::Light => ThemePreference::System,
    }
}

/// Short label for the preference, shown on the toggle and in the status bar.
#[must_use]
pub fn preference_label(preference: ThemePreference) -> &'static str {
    match preference {
        ThemePreference::System => "Theme: System",
        ThemePreference::Dark => "Theme: Dark",
        ThemePreference::Light => "Theme: Light",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_is_indigo_in_both_themes() {
        assert_eq!(Palette::dark().accent, ACCENT);
        assert_eq!(Palette::light().accent, ACCENT);
        assert_eq!(ACCENT, Color32::from_rgb(0x7c, 0x6c, 0xef));
    }

    #[test]
    fn for_theme_selects_the_right_palette() {
        assert_eq!(
            Palette::for_theme(Theme::Dark).bg_app,
            Palette::dark().bg_app
        );
        assert_eq!(
            Palette::for_theme(Theme::Light).bg_app,
            Palette::light().bg_app
        );
    }

    #[test]
    fn visuals_use_palette_surfaces() {
        let p = Palette::dark();
        let v = p.visuals(Theme::Dark);
        assert_eq!(v.panel_fill, p.bg_panel);
        assert_eq!(v.window_fill, p.bg_elevated);
        assert_eq!(v.extreme_bg_color, p.bg_app);
        assert_eq!(v.selection.bg_fill, p.selection);
        assert_eq!(v.hyperlink_color, p.accent);
    }

    #[test]
    fn preference_cycle_returns_to_system() {
        let mut pref = ThemePreference::System;
        for _ in 0..3 {
            pref = next_preference(pref);
        }
        assert_eq!(pref, ThemePreference::System);
    }
}
