//! Color primitives and the project-level color mode.
//!
//! [`Rgba`] is the universal pixel type; even indexed-mode sprites
//! resolve through their palette to RGBA at the rendering boundary.
//! [`ColorMode`] is the *authoring* mode — what the editor enforces
//! when the artist places a pixel.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A single 8-bit-per-channel RGBA color.
///
/// Channels are non-premultiplied; conversion to premultiplied space
/// happens in the rendering layer, not in the data model.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Rgba {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel. `0` is fully transparent, `255` fully opaque.
    pub a: u8,
}

impl Rgba {
    /// Constructs an RGBA color.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Constructs an opaque color from RGB, with alpha set to `255`.
    #[must_use]
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// The fully transparent color.
    #[must_use]
    pub const fn transparent() -> Self {
        Self::new(0, 0, 0, 0)
    }

    /// Returns the channels in `[r, g, b, a]` order, suitable for
    /// memcpy into a pixel buffer.
    #[must_use]
    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

/// The color authoring mode of a sprite.
///
/// Pixhaus stores all rendered pixels as RGBA internally, but the
/// editor restricts what an artist can place based on mode.
/// Indexed-mode sprites carry palette indices and resolve to RGBA at
/// the render boundary; this is reflected in the per-cel data layout
/// (a separate stream owns that pixel buffer code).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ColorMode {
    /// 8-bit-per-channel RGBA. The default for new sprites.
    #[default]
    Rgba,
    /// 8-bit grayscale with an alpha channel. Stored as RGBA with
    /// `r == g == b`; the editor enforces that constraint on input.
    Grayscale,
    /// Palette-indexed. Each pixel is an index into the active
    /// palette; the palette resolves to RGBA at render time.
    Indexed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_round_trip() {
        let c = Rgba::new(10, 20, 30, 40);
        let bytes = rmp_serde::to_vec_named(&c).unwrap();
        let back: Rgba = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn opaque_sets_alpha_to_255() {
        assert_eq!(Rgba::opaque(1, 2, 3).a, 255);
    }

    #[test]
    fn transparent_is_all_zero() {
        assert_eq!(Rgba::transparent(), Rgba::new(0, 0, 0, 0));
    }

    #[test]
    fn color_mode_serializes_snake_case() {
        let json = serde_json::to_string(&ColorMode::Grayscale).unwrap();
        assert_eq!(json, "\"grayscale\"");
    }

    #[test]
    fn color_mode_round_trip() {
        for mode in [ColorMode::Rgba, ColorMode::Grayscale, ColorMode::Indexed] {
            let bytes = rmp_serde::to_vec_named(&mode).unwrap();
            let back: ColorMode = rmp_serde::from_slice(&bytes).unwrap();
            assert_eq!(mode, back);
        }
    }
}
