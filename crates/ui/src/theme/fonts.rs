//! Font installation. Registers egui's bundled sans/mono this round and merges the
//! phosphor glyph ranges as a fallback family so `crate::icons::*` resolve. A
//! higher-quality UI font is a later polish step - fonts are an asset decision, not
//! architecture (spec). No emoji literals anywhere: egui's default fonts render emoji
//! as tofu, and phosphor private-use codepoints render blank without this font.

use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

/// The font key under which phosphor's regular glyphs are registered. Matches the key
/// `egui_phosphor::add_to_fonts` uses, so the two are interchangeable for the lookup.
const PHOSPHOR_KEY: &str = "phosphor";

/// Build the merged `FontDefinitions`: egui's bundled sans/mono plus phosphor as a
/// fallback in both the proportional and monospace families. Pure, so it is the
/// test target.
fn merged_fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    // Phosphor regular variant as a static font, registered once.
    fonts.font_data.insert(
        PHOSPHOR_KEY.to_owned(),
        Arc::new(FontData::from_static(egui_phosphor::Variant::Regular.font_bytes())),
    );

    // Append phosphor as a fallback so icon glyphs resolve inside ordinary text in
    // both families. Push to the back: the bundled font wins for normal characters,
    // phosphor only fills its private-use icon range.
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push(PHOSPHOR_KEY.to_owned());
    }

    fonts
}

/// Install the merged fonts on the context. Call once, at boot.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(merged_fonts());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phosphor_is_registered_as_font_data() {
        let fonts = merged_fonts();
        assert!(fonts.font_data.contains_key(PHOSPHOR_KEY), "phosphor font data not registered");
    }

    #[test]
    fn phosphor_is_a_fallback_in_proportional() {
        let fonts = merged_fonts();
        let Some(fam) = fonts.families.get(&FontFamily::Proportional) else {
            panic!("proportional family missing");
        };
        assert!(fam.iter().any(|k| k == PHOSPHOR_KEY), "phosphor not in proportional fallback list");
    }

    #[test]
    fn phosphor_is_a_fallback_in_monospace() {
        let fonts = merged_fonts();
        let Some(fam) = fonts.families.get(&FontFamily::Monospace) else {
            panic!("monospace family missing");
        };
        assert!(fam.iter().any(|k| k == PHOSPHOR_KEY), "phosphor not in monospace fallback list");
    }
}
