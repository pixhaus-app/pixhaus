//! A multi-tab colour picker widget for the palette panel.
//!
//! Five tabs edit the same [`Rgba`] through different colour spaces — HSV, HSL,
//! RGB, a `#RRGGBB` text field, and OKLCH — plus an always-visible alpha slider.
//! Switching tabs preserves the colour; the space math reuses
//! [`pixhaus_core::color::space`]. Mirrors the Tauri `ColorPicker.tsx`.
//!
//! The widget is a pure painting surface: it mutates the `color` in place and
//! returns a combined [`egui::Response`]. The caller decides when to commit an
//! undo entry — see `palette_panel::swatch_editor`, which holds the in-progress
//! colour in a scratch field and pushes one `SpriteEdit` on drag-stop, so a
//! slider drag is one undo step rather than one per tick.

use eframe::egui;
use pixhaus_core::color::space;
use pixhaus_core::project::Rgba;

use crate::editor::PickerTab;
use crate::palette_panel::paint_checker;

/// Draws the multi-tab colour picker for `color`. `tab` selects the active
/// space; `hex_draft` backs the HEX tab's text field. Returns the union of every
/// inner widget's response, so the caller can read `.changed()`,
/// `.drag_stopped()`, and `.lost_focus()` across the whole picker.
pub fn color_picker_ui(ui: &mut egui::Ui, color: &mut Rgba, tab: &mut PickerTab, hex_draft: &mut String) -> egui::Response {
    // Tabs row. Selecting a tab is a change worth returning so the caller can
    // re-seed the hex draft, but it does not edit the colour.
    let mut resp = ui
        .horizontal(|ui| {
            let mut r = ui.selectable_value(tab, PickerTab::Hsv, "HSV");
            r |= ui.selectable_value(tab, PickerTab::Hsl, "HSL");
            r |= ui.selectable_value(tab, PickerTab::Rgb, "RGB");
            r |= ui.selectable_value(tab, PickerTab::Hex, "HEX");
            r |= ui.selectable_value(tab, PickerTab::Oklch, "OKLCH");
            r
        })
        .inner;

    // Preview swatch + hex label.
    preview(ui, *color);

    ui.separator();

    resp |= match *tab {
        PickerTab::Hsv => hsv_tab(ui, color),
        PickerTab::Hsl => hsl_tab(ui, color),
        PickerTab::Rgb => rgb_tab(ui, color),
        PickerTab::Hex => hex_tab(ui, color, hex_draft),
        PickerTab::Oklch => oklch_tab(ui, color),
    };

    // Always-visible alpha slider below the tab body.
    resp |= ui.add(egui::Slider::new(&mut color.a, 0..=255).text("A"));

    resp
}

/// Paints the preview swatch (with a checker under transparent colours) and a
/// `#RRGGBB` label beside it.
fn preview(ui: &mut egui::Ui, color: Rgba) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 24.0), egui::Sense::hover());
        if color.a < 255 {
            paint_checker(ui, rect);
        }
        ui.painter()
            .rect_filled(rect, 2.0, egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a));
        ui.painter().rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0, egui::Color32::from_black_alpha(120)),
            egui::StrokeKind::Middle,
        );
        ui.label(format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b));
    });
}

/// HSV sliders. Writes back through `space::from_hsv`, preserving alpha.
fn hsv_tab(ui: &mut egui::Ui, color: &mut Rgba) -> egui::Response {
    let (mut h, mut s, mut v) = space::to_hsv(*color);
    let mut resp = ui.add(egui::Slider::new(&mut h, 0.0..=360.0).text("H"));
    resp |= ui.add(egui::Slider::new(&mut s, 0.0..=1.0).text("S"));
    resp |= ui.add(egui::Slider::new(&mut v, 0.0..=1.0).text("V"));
    if resp.changed() {
        *color = space::from_hsv(h, s, v, color.a);
    }
    resp
}

/// HSL sliders. Writes back through `space::from_hsl`, preserving alpha.
fn hsl_tab(ui: &mut egui::Ui, color: &mut Rgba) -> egui::Response {
    let (mut h, mut s, mut l) = space::to_hsl(*color);
    let mut resp = ui.add(egui::Slider::new(&mut h, 0.0..=360.0).text("H"));
    resp |= ui.add(egui::Slider::new(&mut s, 0.0..=1.0).text("S"));
    resp |= ui.add(egui::Slider::new(&mut l, 0.0..=1.0).text("L"));
    if resp.changed() {
        *color = space::from_hsl(h, s, l, color.a);
    }
    resp
}

/// RGB integer sliders, 0..=255 each. Edits the channels in place.
fn rgb_tab(ui: &mut egui::Ui, color: &mut Rgba) -> egui::Response {
    let mut resp = ui.add(egui::Slider::new(&mut color.r, 0..=255).text("R"));
    resp |= ui.add(egui::Slider::new(&mut color.g, 0..=255).text("G"));
    resp |= ui.add(egui::Slider::new(&mut color.b, 0..=255).text("B"));
    resp
}

/// A `#RRGGBB` text field. Re-seeds the draft from the live colour when it does
/// not match, then commits the parse on Enter or focus loss, preserving alpha.
fn hex_tab(ui: &mut egui::Ui, color: &mut Rgba, hex_draft: &mut String) -> egui::Response {
    // Re-seed the draft from the current colour unless the field has focus (so a
    // mid-edit draft is not clobbered) and unless it already parses to `color`.
    let seeded = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    let in_sync = parse_hex(hex_draft).is_some_and(|c| c.r == color.r && c.g == color.g && c.b == color.b);

    let resp = ui.text_edit_singleline(hex_draft);
    if !resp.has_focus() && !in_sync {
        *hex_draft = seeded;
    }

    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
    if resp.lost_focus() || (resp.has_focus() && enter) {
        if let Some(parsed) = parse_hex(hex_draft) {
            *color = Rgba::new(parsed.r, parsed.g, parsed.b, color.a);
        }
        // On a bad parse, re-seed from the live colour so the field never holds
        // a value the swatch does not show.
        *hex_draft = format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    }
    resp
}

/// OKLCH sliders. Writes back through `space::from_oklch` (clamped to gamut),
/// preserving alpha.
fn oklch_tab(ui: &mut egui::Ui, color: &mut Rgba) -> egui::Response {
    let (mut l, mut c, mut h) = space::to_oklch(*color);
    let mut resp = ui.add(egui::Slider::new(&mut l, 0.0..=1.0).text("L"));
    resp |= ui.add(egui::Slider::new(&mut c, 0.0..=0.4).text("C"));
    resp |= ui.add(egui::Slider::new(&mut h, 0.0..=360.0).text("H"));
    if resp.changed() {
        *color = space::from_oklch(l, c, h, color.a);
    }
    resp
}

/// Parses a `#RRGGBB` or `#RGB` hex string into an opaque [`Rgba`].
///
/// Accepts an optional leading `#`, surrounding whitespace, and the 3-digit
/// shorthand (`#abc` expands to `#aabbcc`). Returns `None` for any other length
/// or a non-hex digit. Alpha is set to `255`; the caller preserves the editing
/// colour's alpha. Mirrors the Tauri `color-utils.ts` `hexToRgb`.
#[must_use]
pub fn parse_hex(s: &str) -> Option<Rgba> {
    let t = s.trim().strip_prefix('#').unwrap_or(s.trim());
    let expanded = match t.len() {
        3 => t.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => t.to_string(),
        _ => return None,
    };
    if !expanded.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
    let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
    let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
    Some(Rgba::opaque(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_six_digit_with_hash() {
        assert_eq!(parse_hex("#1A2B3C"), Some(Rgba::opaque(0x1A, 0x2B, 0x3C)));
    }

    #[test]
    fn parses_six_digit_without_hash() {
        assert_eq!(parse_hex("ff0080"), Some(Rgba::opaque(255, 0, 128)));
    }

    #[test]
    fn expands_three_digit_shorthand() {
        // #abc -> #aabbcc.
        assert_eq!(parse_hex("#abc"), Some(Rgba::opaque(0xAA, 0xBB, 0xCC)));
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(parse_hex("#FfFfFf"), parse_hex("#ffffff"));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        assert_eq!(parse_hex("  #00ff00  "), Some(Rgba::opaque(0, 255, 0)));
    }

    #[test]
    fn rejects_wrong_length() {
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#1234567"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn rejects_non_hex_digits() {
        assert_eq!(parse_hex("#gggggg"), None);
        assert_eq!(parse_hex("#12345z"), None);
    }

    #[test]
    fn parse_returns_opaque_alpha() {
        // The helper always returns alpha 255; the caller preserves the
        // editing colour's own alpha.
        assert_eq!(parse_hex("#000000").map(|c| c.a), Some(255));
    }
}
