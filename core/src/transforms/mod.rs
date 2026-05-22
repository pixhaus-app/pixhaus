//! Transform operations for pixel buffers.
//!
//! Every transform takes a [`crate::canvas::buffer::PixelBuffer`] and
//! returns a new transformed buffer. No pixel data is mutated in place;
//! callers record before/after state through the undo system (S05).
//!
//! # Scope
//!
//! | Operation | Function | Pixel-perfect? |
//! |-----------|----------|----------------|
//! | Translate | [`fn@translate`] | yes |
//! | Scale (nearest-neighbour) | [`scale_nearest`] | yes |
//! | Scale (integer multiple) | [`scale_integer`] | yes |
//! | Scale (integer divisor) | [`scale_integer_down`] | yes |
//! | Flip horizontal | [`flip_horizontal`] | yes |
//! | Flip vertical | [`flip_vertical`] | yes |
//! | Rotate 90° CW | [`rotate_90_cw`] | yes |
//! | Rotate 90° CCW | [`rotate_90_ccw`] | yes |
//! | Rotate 180° | [`rotate_180`] | yes |
//! | Rotate arbitrary (`RotSprite`) | [`rotate_rotsprite`] | pixel-art quality |
//! | Rotate arbitrary (bilinear) | [`rotate_bilinear`] | no |
//! | Skew X | [`skew_x`] | integer factors only |
//! | Skew Y | [`skew_y`] | integer factors only |
//! | Perspective | [`fn@perspective`] | no (documented) |
//!
//! # Selection awareness
//!
//! [`fn@translate`], [`flip_horizontal`], and [`flip_vertical`] accept an
//! optional [`crate::selection::SelectionMask`]. When a mask is supplied
//! only the covered pixels are moved; unmasked pixels remain in place.
//! The remaining operations apply to the full buffer. Selection-scoped
//! variants of those operations are the responsibility of the editor
//! command layer (S16).
//!
//! # Undo integration
//!
//! Transforms produce a new [`crate::canvas::buffer::PixelBuffer`]. The
//! calling layer (the Tauri command or a future undo `Command` impl in
//! `app/`) is responsible for saving the before-state and registering
//! the before/after pair with the undo history. Pixel buffer bytes live
//! in `DocumentStore`, not in `Project`, so the integration lives in
//! `app/` rather than in this crate.
//!
//! Implements: docs/planning/work/streams.md#s04

pub mod antialias;
pub mod error;
pub mod flip;
pub mod perspective;
pub mod rotate;
pub mod scale;
pub mod skew;
pub mod translate;

pub use antialias::{MlaaConfig, morphological_antialias};
pub use error::{Error, Result};
pub use flip::{flip_horizontal, flip_vertical};
pub use perspective::perspective;
pub use rotate::{
    RotationAlgorithm, rotate, rotate_90_ccw, rotate_90_cw, rotate_180, rotate_bilinear,
    rotate_nearest, rotate_rotsprite,
};
pub use scale::{scale_integer, scale_integer_down, scale_nearest};
pub use skew::{skew_x, skew_y};
pub use translate::translate;

use crate::canvas::buffer::PixelBuffer;
use crate::selection::SelectionMask;

// ──────────────────────────────────────────────────────────────────────────────
// TransformSpec — describes one transform operation
// ──────────────────────────────────────────────────────────────────────────────

/// Sampling mode used by [`TransformSpec::Scale`].
#[derive(Clone, Debug, PartialEq)]
pub enum ScaleMode {
    /// Nearest-neighbour sampling. Preserves hard pixel edges.
    /// Default for pixel art.
    NearestNeighbor,
    /// Integer-multiple upscale only. Each source pixel becomes an
    /// `N×N` block; strictly lossless.
    IntegerMultiple(u32),
    /// Integer-divisor downscale. Samples the top-left pixel of each
    /// `N×N` block.
    IntegerDivisor(u32),
}

/// Interpolation mode used by [`TransformSpec::RotateArbitrary`].
#[derive(Clone, Debug, PartialEq)]
pub enum RotateMode {
    /// `RotSprite` — Scale2x × 2 upscale, bilinear rotate, nearest
    /// downscale. Best quality for pixel art.
    RotSprite,
    /// Direct bilinear interpolation on the source. Faster but
    /// introduces sub-pixel blending.
    Bilinear,
}

/// A description of a single transform that can be applied to a
/// [`PixelBuffer`] via [`apply_transform`].
///
/// This enum is the canonical way to record "what transform was applied"
/// for undo history, IPC serialisation, and logging.
#[derive(Clone, Debug, PartialEq)]
pub enum TransformSpec {
    /// Integer-pixel translation by `(dx, dy)`.
    Translate {
        /// Horizontal offset in pixels. Positive moves right.
        dx: i32,
        /// Vertical offset in pixels. Positive moves down.
        dy: i32,
    },
    /// Resize to `(new_width, new_height)` using the given sampling mode.
    Scale {
        /// Target width in pixels.
        new_width: u32,
        /// Target height in pixels.
        new_height: u32,
        /// Resampling algorithm.
        mode: ScaleMode,
    },
    /// 90° clockwise rotation.
    Rotate90Cw,
    /// 90° counter-clockwise rotation.
    Rotate90Ccw,
    /// 180° rotation.
    Rotate180,
    /// Rotation by an arbitrary angle in degrees, counter-clockwise.
    RotateArbitrary {
        /// Angle in degrees, counter-clockwise.
        degrees: f32,
        /// Interpolation algorithm.
        mode: RotateMode,
    },
    /// Horizontal mirror.
    FlipHorizontal,
    /// Vertical mirror.
    FlipVertical,
    /// Horizontal shear by `factor` pixels per row.
    SkewX {
        /// Shear magnitude: horizontal offset per pixel of height.
        factor: f32,
    },
    /// Vertical shear by `factor` pixels per column.
    SkewY {
        /// Shear magnitude: vertical offset per pixel of width.
        factor: f32,
    },
    /// Projective warp to the given destination corners `[TL, TR, BR, BL]`.
    Perspective {
        /// Destination corners in canvas space: `[TL, TR, BR, BL]`.
        corners: [(f32, f32); 4],
    },
}

// ──────────────────────────────────────────────────────────────────────────────
// Dispatch
// ──────────────────────────────────────────────────────────────────────────────

/// Applies `spec` to `buf`, returning a new transformed buffer.
///
/// `mask` is forwarded to transforms that support selection-scoped
/// operation ([`fn@translate`], [`flip_horizontal`], [`flip_vertical`]).
/// All other transforms ignore it and operate on the full buffer.
///
/// # Errors
///
/// Forwards any error from the underlying transform function.
pub fn apply_transform(
    spec: &TransformSpec,
    buf: &PixelBuffer,
    mask: Option<&SelectionMask>,
) -> Result<PixelBuffer> {
    match spec {
        TransformSpec::Translate { dx, dy } => translate(buf, *dx, *dy, mask),

        TransformSpec::Scale {
            new_width,
            new_height,
            mode,
        } => match mode {
            ScaleMode::NearestNeighbor => scale_nearest(buf, *new_width, *new_height),
            ScaleMode::IntegerMultiple(factor) => scale_integer(buf, *factor),
            ScaleMode::IntegerDivisor(divisor) => scale_integer_down(buf, *divisor),
        },

        TransformSpec::Rotate90Cw => rotate_90_cw(buf),
        TransformSpec::Rotate90Ccw => rotate_90_ccw(buf),
        TransformSpec::Rotate180 => rotate_180(buf),

        TransformSpec::RotateArbitrary { degrees, mode } => match mode {
            RotateMode::RotSprite => rotate_rotsprite(buf, *degrees),
            RotateMode::Bilinear => rotate_bilinear(buf, degrees.to_radians()),
        },

        TransformSpec::FlipHorizontal => flip_horizontal(buf, mask),
        TransformSpec::FlipVertical => flip_vertical(buf, mask),

        TransformSpec::SkewX { factor } => skew_x(buf, *factor),
        TransformSpec::SkewY { factor } => skew_y(buf, *factor),

        TransformSpec::Perspective { corners } => perspective(buf, corners),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Rgba;

    fn red() -> Rgba {
        Rgba::opaque(255, 0, 0)
    }

    fn make_buf() -> PixelBuffer {
        let mut b = PixelBuffer::new(4, 4).unwrap();
        b.set_pixel(0, 0, red());
        b
    }

    #[test]
    fn dispatch_translate() {
        let buf = make_buf();
        let out = apply_transform(&TransformSpec::Translate { dx: 1, dy: 1 }, &buf, None).unwrap();
        assert_eq!(out.pixel(1, 1), Some(red()));
        assert_eq!(out.pixel(0, 0), Some(Rgba::transparent()));
    }

    #[test]
    fn dispatch_scale_nearest() {
        let buf = make_buf();
        let out = apply_transform(
            &TransformSpec::Scale {
                new_width: 8,
                new_height: 8,
                mode: ScaleMode::NearestNeighbor,
            },
            &buf,
            None,
        )
        .unwrap();
        assert_eq!(out.width(), 8);
        assert_eq!(out.height(), 8);
    }

    #[test]
    fn dispatch_scale_integer_multiple() {
        let buf = make_buf();
        let out = apply_transform(
            &TransformSpec::Scale {
                new_width: 8,
                new_height: 8,
                mode: ScaleMode::IntegerMultiple(2),
            },
            &buf,
            None,
        )
        .unwrap();
        assert_eq!(out.width(), 8);
        assert_eq!(out.height(), 8);
    }

    #[test]
    fn dispatch_rotate_90_cw() {
        let buf = make_buf();
        let out = apply_transform(&TransformSpec::Rotate90Cw, &buf, None).unwrap();
        assert_eq!(out.width(), 4);
        assert_eq!(out.height(), 4);
    }

    #[test]
    fn dispatch_flip_horizontal() {
        let buf = make_buf();
        let out = apply_transform(&TransformSpec::FlipHorizontal, &buf, None).unwrap();
        // Red was at (0,0); after H-flip it should be at (3,0).
        assert_eq!(out.pixel(3, 0), Some(red()));
    }

    #[test]
    fn dispatch_flip_vertical() {
        let buf = make_buf();
        let out = apply_transform(&TransformSpec::FlipVertical, &buf, None).unwrap();
        // Red was at (0,0); after V-flip it should be at (0,3).
        assert_eq!(out.pixel(0, 3), Some(red()));
    }

    #[test]
    fn dispatch_skew_x_zero() {
        let buf = make_buf();
        let out = apply_transform(&TransformSpec::SkewX { factor: 0.0 }, &buf, None).unwrap();
        assert_eq!(out.pixel(0, 0), Some(red()));
    }

    #[test]
    fn dispatch_rotsprite_0_is_identity() {
        let buf = make_buf();
        let out = apply_transform(
            &TransformSpec::RotateArbitrary {
                degrees: 0.0,
                mode: RotateMode::RotSprite,
            },
            &buf,
            None,
        )
        .unwrap();
        assert_eq!(out, buf);
    }

    #[test]
    fn dispatch_perspective_identity() {
        let buf = make_buf();
        let corners = [(0.0f32, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)];
        let out = apply_transform(&TransformSpec::Perspective { corners }, &buf, None).unwrap();
        assert_eq!(out.width(), 4);
        assert_eq!(out.height(), 4);
    }
}
