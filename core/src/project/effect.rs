//! Non-destructive per-layer effects.
//!
//! A layer carries an ordered `Vec<LayerEffect>` (see [`super::layer::Layer`]).
//! Effects are applied to a copy of the layer's composited pixels at render
//! time and never written back to the cel, so they stay reversible and
//! re-orderable. The application logic lives in `crate::canvas::effects`; this
//! module owns only the serializable description.
//!
//! Design adopted from Pixelorama's per-layer effect stack
//! (`src/Classes/Layers/BaseLayer.gd` `effects`). See `THIRD_PARTY_NOTICES.md`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::color::Rgba;
use super::geometry::IVec2;

/// One non-destructive effect in a layer's effect stack.
///
/// Variants are intentionally CPU-cheap and deterministic so they are
/// snapshot-testable and usable as the result target for AI verbs that emit
/// overlay-style output (a generated outline, a generated tint).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum LayerEffect {
    /// Draws an outline of `color` around opaque pixels, `thickness` pixels
    /// wide. Fills only previously-transparent pixels, so the source art
    /// stays on top.
    Outline {
        /// Outline color.
        color: Rgba,
        /// Outline width in pixels. Clamped to at least `1` at apply time.
        thickness: u8,
        /// Whether to include diagonal (8-connected) neighbours. `false`
        /// gives the crisp 4-connected outline pixel artists usually want.
        diagonal: bool,
    },
    /// Casts a `color` shadow offset by `offset`, composited beneath the
    /// source pixels. `opacity` scales the shadow's alpha.
    DropShadow {
        /// Shadow color (its own alpha is ignored; `opacity` drives it).
        color: Rgba,
        /// Pixel offset of the shadow from the source.
        offset: IVec2,
        /// Shadow strength, `0` (invisible) through `255` (full).
        opacity: u8,
    },
    /// Adds `delta` to each RGB channel, clamped to `0..=255`. Negative
    /// darkens, positive brightens. Alpha is untouched.
    Brightness {
        /// Per-channel delta in `-255..=255`.
        delta: i16,
    },
    /// Inverts the RGB channels. Alpha is untouched.
    Invert,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_round_trips() {
        let e = LayerEffect::Outline {
            color: Rgba::opaque(0, 0, 0),
            thickness: 2,
            diagonal: false,
        };
        let bytes = rmp_serde::to_vec_named(&e).unwrap();
        let back: LayerEffect = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn drop_shadow_round_trips() {
        let e = LayerEffect::DropShadow {
            color: Rgba::opaque(0, 0, 0),
            offset: IVec2::new(2, 3),
            opacity: 128,
        };
        let bytes = rmp_serde::to_vec_named(&e).unwrap();
        let back: LayerEffect = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn invert_uses_kind_tag_in_json() {
        let json = serde_json::to_string(&LayerEffect::Invert).unwrap();
        assert_eq!(json, "{\"kind\":\"invert\"}");
    }

    #[test]
    fn brightness_round_trips() {
        let e = LayerEffect::Brightness { delta: -40 };
        let bytes = rmp_serde::to_vec_named(&e).unwrap();
        let back: LayerEffect = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(e, back);
    }
}
