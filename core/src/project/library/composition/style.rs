//! Reusable look modifiers — the artist's main library primitive.

use serde::{Deserialize, Serialize};

use crate::project::library::ai::{ModelId, Quality};

/// Stable id for a Style.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StyleId(pub String);

/// The art-style family a [`Style`] belongs to.
///
/// The selected kind gates the pixel-only steps of the generation pipeline:
/// the pixel prose folded into a pixel-art Style's modifiers and the pixel
/// finisher. Universal disciplines (containment, no-border, identity/scale-lock)
/// stay style-agnostic and run for every kind. `PixelArt` is the default
/// because the mascot is pixel art and most users arrive wanting pixel art.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtStyleKind {
    /// Crisp, palette-limited pixel art — the default look.
    #[default]
    PixelArt,
    /// Lo-fi retro pixel art with a tighter palette and chunkier pixels.
    RetroPixel,
    /// Pixel-inspired art that keeps a hand-drawn feel without a strict grid.
    PixelInspired,
    /// Clean, high-detail rendering with no pixel constraints.
    CleanHd,
    /// Top-down map / tile art tuned for seamless layouts.
    MapStyle,
}

impl ArtStyleKind {
    /// Pixel-class styles run the pixel prose and the pixel finisher.
    #[must_use]
    pub fn is_pixel(self) -> bool {
        matches!(self, Self::PixelArt | Self::RetroPixel | Self::PixelInspired)
    }
}

/// Whether `kind` is the default, used by `skip_serializing_if` to drop it on
/// the wire so a pre-`kind` Style and a default-kind Style serialise
/// identically. `skip_serializing_if` requires a `fn(&T) -> bool`, so the
/// by-reference argument is mandated by serde, not a missed `Copy`.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn art_style_kind_is_default(kind: &ArtStyleKind) -> bool {
    *kind == ArtStyleKind::default()
}

/// Reusable look modifier record.
#[allow(missing_docs)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub id: StyleId,
    pub name: String,
    /// Art-style family driving the pixel-only gates. Defaults to `PixelArt`;
    /// omitted from the wire when default so old projects round-trip unchanged.
    #[serde(default, skip_serializing_if = "art_style_kind_is_default")]
    pub kind: ArtStyleKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub modifiers: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub look_negatives: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_pref: Option<ModelId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn style_round_trips_minimal() {
        let s = Style {
            id: StyleId("test.style".into()),
            name: "SNES".into(),
            kind: ArtStyleKind::PixelArt,
            modifiers: "16-bit palette".into(),
            look_negatives: "blurry".into(),
            model_pref: None,
            quality: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Style = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn empty_optionals_are_skipped() {
        let s = Style {
            id: StyleId("x".into()),
            name: "x".into(),
            kind: ArtStyleKind::PixelArt,
            modifiers: String::new(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        // The default `kind` (PixelArt) is skipped, keeping the legacy wire shape.
        assert_eq!(json, r#"{"id":"x","name":"x"}"#);
    }

    mod art_style_kind {
        use super::*;

        #[test]
        fn default_is_pixel_art() {
            assert_eq!(ArtStyleKind::default(), ArtStyleKind::PixelArt);
        }

        #[rstest]
        #[case::pixel_art(ArtStyleKind::PixelArt, true)]
        #[case::retro_pixel(ArtStyleKind::RetroPixel, true)]
        #[case::pixel_inspired(ArtStyleKind::PixelInspired, true)]
        #[case::clean_hd(ArtStyleKind::CleanHd, false)]
        #[case::map_style(ArtStyleKind::MapStyle, false)]
        fn is_pixel_truth_table(#[case] kind: ArtStyleKind, #[case] expected: bool) {
            assert_eq!(kind.is_pixel(), expected, "is_pixel for {kind:?}");
        }

        #[rstest]
        #[case::pixel_art(ArtStyleKind::PixelArt, "\"pixel_art\"")]
        #[case::retro_pixel(ArtStyleKind::RetroPixel, "\"retro_pixel\"")]
        #[case::pixel_inspired(ArtStyleKind::PixelInspired, "\"pixel_inspired\"")]
        #[case::clean_hd(ArtStyleKind::CleanHd, "\"clean_hd\"")]
        #[case::map_style(ArtStyleKind::MapStyle, "\"map_style\"")]
        fn serializes_snake_case(#[case] kind: ArtStyleKind, #[case] expected: &str) {
            assert_eq!(serde_json::to_string(&kind).unwrap(), expected);
        }
    }

    #[test]
    fn style_missing_kind_defaults_to_pixel_art() {
        // A pre-`kind` saved Style omits the field; it must load as PixelArt so
        // old projects keep their default look.
        let s: Style = serde_json::from_str(r#"{"id":"x","name":"x"}"#).unwrap();
        assert_eq!(s.kind, ArtStyleKind::PixelArt);
    }

    #[test]
    fn style_round_trips_with_non_default_kind() {
        let s = Style {
            id: StyleId("clean".into()),
            name: "Clean HD".into(),
            kind: ArtStyleKind::CleanHd,
            modifiers: "high detail".into(),
            look_negatives: String::new(),
            model_pref: None,
            quality: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        // A non-default kind survives a write/read round-trip and is serialised.
        assert!(json.contains("\"kind\":\"clean_hd\""), "non-default kind must be on the wire: {json}");
        let back: Style = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
