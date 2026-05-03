//! Layer-composition blend modes.
//!
//! The set mirrors Aseprite's so files round-trip without losing
//! intent. Implementation of each mode is the responsibility of the
//! pixel-buffer compositor (lands with stream S01); the data model
//! only declares the names.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Per-layer blend mode used during composition.
///
/// `Normal` is the default. Variants are kept as standalone names —
/// payloads are not attached because all compositing parameters live
/// on the layer (`opacity`, `visible`) rather than on the mode.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_normal() {
        assert_eq!(BlendMode::default(), BlendMode::Normal);
    }

    #[test]
    fn all_modes_round_trip() {
        let modes = [
            BlendMode::Normal,
            BlendMode::Darken,
            BlendMode::Multiply,
            BlendMode::ColorBurn,
            BlendMode::Lighten,
            BlendMode::Screen,
            BlendMode::ColorDodge,
            BlendMode::Addition,
            BlendMode::Overlay,
            BlendMode::SoftLight,
            BlendMode::HardLight,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Subtract,
            BlendMode::Divide,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ];
        for mode in modes {
            let bytes = rmp_serde::to_vec_named(&mode).unwrap();
            let back: BlendMode = rmp_serde::from_slice(&bytes).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn color_burn_serializes_snake_case() {
        let json = serde_json::to_string(&BlendMode::ColorBurn).unwrap();
        assert_eq!(json, "\"color_burn\"");
    }
}
