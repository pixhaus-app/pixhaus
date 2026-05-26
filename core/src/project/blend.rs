//! Layer-composition blend modes.
//!
//! The set mirrors Aseprite's so files round-trip without losing
//! intent. Implementation of each mode is the responsibility of the
//! pixel-buffer compositor; the data model only declares the names.

use serde::{Deserialize, Serialize};

/// Per-layer blend mode used during composition.
///
/// `Normal` is the default. Variants are kept as standalone names —
/// payloads are not attached because all compositing parameters live
/// on the layer (`opacity`, `visible`) rather than on the mode.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlendMode {
    /// Source replaces destination, weighted by alpha.
    #[default]
    Normal,
    /// `min(src, dst)` per channel.
    Darken,
    /// `(src * dst) / 255` per channel.
    Multiply,
    /// Photoshop-style color burn.
    ColorBurn,
    /// `max(src, dst)` per channel.
    Lighten,
    /// Inverse-multiply: `255 - ((255 - src) * (255 - dst) / 255)`.
    Screen,
    /// Photoshop-style color dodge.
    ColorDodge,
    /// `min(src + dst, 255)` per channel.
    Addition,
    /// Multiply for dark dst, screen for light dst.
    Overlay,
    /// Soft, photoshop-style soft light.
    SoftLight,
    /// Multiply for dark src, screen for light src.
    HardLight,
    /// `|src - dst|` per channel.
    Difference,
    /// `src + dst - 2 * src * dst / 255` per channel.
    Exclusion,
    /// `max(0, dst - src)` per channel.
    Subtract,
    /// `(dst * 255) / src` per channel, clamped.
    Divide,
    /// Replaces dst hue with src hue.
    Hue,
    /// Replaces dst saturation with src saturation.
    Saturation,
    /// Replaces dst hue and saturation with src.
    Color,
    /// Replaces dst luminosity with src.
    Luminosity,
    /// `max(b + s - 255, 0)` per channel.
    LinearBurn,
    /// Picks the color whose Rec.709 luma is lower (whole-pixel choice,
    /// not per-channel).
    DarkerColor,
    /// `min(b + s, 255)` per channel. Equivalent to `Addition`; kept as
    /// a distinct variant for UI parity with Photoshop.
    LinearDodge,
    /// Picks the color whose Rec.709 luma is higher (whole-pixel
    /// choice, not per-channel).
    LighterColor,
    /// `s < 128 ? color_burn(b, 2s) : color_dodge(b, 2s - 255)`.
    VividLight,
    /// `s < 128 ? linear_burn(b, 2s) : linear_dodge(b, 2s - 255)`.
    LinearLight,
    /// `s < 128 ? darken(b, 2s) : lighten(b, 2s - 255)`.
    PinLight,
    /// `vivid_light(b, s) < 128 ? 0 : 255` per channel.
    HardMix,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_normal() {
        assert_eq!(BlendMode::default(), BlendMode::Normal);
    }

    #[test]
    fn color_burn_serializes_snake_case() {
        let json = serde_json::to_string(&BlendMode::ColorBurn).unwrap();
        assert_eq!(json, "\"color_burn\"");
    }
}
