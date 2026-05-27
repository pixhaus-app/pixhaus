//! Color primitives and the project-level color mode.
//!
//! [`Rgba`] is the universal pixel type; even indexed-mode sprites
//! resolve through their palette to RGBA at the rendering boundary.
//! [`ColorMode`] is the *authoring* mode — what the editor enforces
//! when the artist places a pixel.

use serde::{Deserialize, Serialize};

/// A single 8-bit-per-channel RGBA color.
///
/// Channels are non-premultiplied; conversion to premultiplied space
/// happens in the rendering layer, not in the data model.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
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

    /// Returns `true` when every channel is within `tolerance` of `other`'s,
    /// measured as a per-channel absolute difference. `tolerance == 0` is an
    /// exact match; `255` matches anything. This is the shared color test
    /// behind flood-fill and magic-wand selection.
    #[must_use]
    pub const fn within_tolerance(self, other: Self, tolerance: u8) -> bool {
        self.r.abs_diff(other.r) <= tolerance
            && self.g.abs_diff(other.g) <= tolerance
            && self.b.abs_diff(other.b) <= tolerance
            && self.a.abs_diff(other.a) <= tolerance
    }
}

/// The color authoring mode of a sprite.
///
/// Pixhaus stores all rendered pixels as RGBA internally, but the
/// editor restricts what an artist can place based on mode.
/// Indexed-mode sprites carry palette indices and resolve to RGBA at
/// the render boundary; this is reflected in the per-cel data layout
/// (a separate stream owns that pixel buffer code).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
    fn rgba_round_trips_via_json() {
        let c = Rgba::new(10, 20, 30, 40);
        let json = serde_json::to_string(&c).unwrap();
        let back: Rgba = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn within_tolerance_matches_per_channel() {
        let base = Rgba::new(100, 100, 100, 255);
        assert!(base.within_tolerance(base, 0), "exact match at tolerance 0");
        assert!(base.within_tolerance(Rgba::new(105, 95, 100, 255), 5), "all channels within 5");
        assert!(!base.within_tolerance(Rgba::new(106, 100, 100, 255), 5), "one channel over 5 fails");
        // Symmetric, and alpha participates.
        assert!(base.within_tolerance(Rgba::new(100, 100, 100, 250), 5));
        assert!(!base.within_tolerance(Rgba::new(100, 100, 100, 249), 5));
        assert!(base.within_tolerance(Rgba::new(0, 0, 0, 0), 255), "tolerance 255 matches anything");
    }
}
