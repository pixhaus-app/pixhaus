//! Destructive layer-consolidation primitives.
//!
//! Merge down, merge selected, and flatten visible all collapse a set
//! of layers into one. They must produce the same pixels the live
//! composite already shows, so this module is a thin, named wrapper
//! over [`composite_layers`] — it never re-implements the blend math.
//! Keeping the merge here, beside the compositor, gives the operation
//! its own name at the call site and its own test, while the single
//! source of truth for blending stays in [`mod@composite`].

use super::buffer::PixelBuffer;
use super::composite::{LayerInput, composite_layers};
use super::error::Result;

/// Composites `above` onto `below` and returns the merged buffer.
///
/// `below` is the backdrop (the layer that stays); `above` is the
/// source folded into it with its own blend mode and opacity. The two
/// inputs composite back-to-front through [`composite_layers`], so the
/// merged result is byte-identical to what the live compositor shows
/// for the same pair. This is the "merge down" primitive: collapse the
/// active layer onto the one directly beneath it.
///
/// # Errors
///
/// Returns [`Error::DimensionMismatch`] when either input's dimensions
/// disagree with `(width, height)`, forwarded from [`composite_layers`].
///
/// [`Error::DimensionMismatch`]: super::error::Error::DimensionMismatch
pub fn merge_pair(below: &LayerInput<'_>, above: &LayerInput<'_>, width: u32, height: u32) -> Result<PixelBuffer> {
    composite_layers(width, height, &[*below, *above])
}

/// Composites a back-to-front stack of layers into one buffer and returns it.
///
/// `inputs[0]` is the bottom of the stack; each later layer composites on top
/// with its own blend mode and opacity. This is the "flatten visible" primitive:
/// collapse every visible raster layer into the bottom-most one. Like
/// [`merge_pair`], it is a thin, named wrapper over [`composite_layers`] so the
/// intent reads at the call site and flatten gets its own test — the blend math
/// stays in [`mod@composite`]. The result is byte-identical to what the live
/// compositor shows for the same inputs.
///
/// # Errors
///
/// Returns [`Error::DimensionMismatch`] when any input's dimensions disagree
/// with `(width, height)`, forwarded from [`composite_layers`].
///
/// [`Error::DimensionMismatch`]: super::error::Error::DimensionMismatch
pub fn flatten(inputs: &[LayerInput<'_>], width: u32, height: u32) -> Result<PixelBuffer> {
    composite_layers(width, height, inputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{BlendMode, Rgba};

    fn solid(width: u32, height: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(width, height, color).expect("solid buffer")
    }

    fn input(buffer: &PixelBuffer, mode: BlendMode, opacity: u8) -> LayerInput<'_> {
        LayerInput {
            buffer,
            mode,
            opacity,
            visible: true,
            clip_below: false,
        }
    }

    #[test]
    fn merge_pair_opaque_top_replaces_bottom() {
        let green = solid(4, 4, Rgba::opaque(0, 255, 0));
        let red = solid(4, 4, Rgba::opaque(255, 0, 0));
        let below = input(&green, BlendMode::Normal, 255);
        let above = input(&red, BlendMode::Normal, 255);

        let merged = merge_pair(&below, &above, 4, 4).expect("merge");

        // Opaque red over green: every pixel is the top layer.
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(merged.pixel(x, y), Some(Rgba::opaque(255, 0, 0)), "({x},{y})");
            }
        }
    }

    #[test]
    fn merge_pair_semi_transparent_top_blends() {
        // Red at half alpha over opaque green: a blended pixel, not a
        // straight copy, so this exercises the blend path rather than a
        // replace.
        let green = solid(2, 2, Rgba::opaque(0, 255, 0));
        let red = solid(2, 2, Rgba::new(255, 0, 0, 128));
        let below = input(&green, BlendMode::Normal, 255);
        let above = input(&red, BlendMode::Normal, 255);

        let merged = merge_pair(&below, &above, 2, 2).expect("merge");
        let pixel = merged.pixel(0, 0).expect("pixel");

        // Some red mixes in and some green survives; neither channel is
        // fully wiped, and the result stays opaque.
        assert!(pixel.r > 0, "red contributes");
        assert!(pixel.g > 0, "green survives under a semi-transparent top");
        assert_eq!(pixel.a, 255, "compositing over an opaque backdrop stays opaque");
    }

    #[test]
    fn merge_pair_reuses_composite_layers_exactly() {
        // The reuse guarantee: merge_pair must produce byte-for-byte the
        // same buffer as compositing the same two inputs directly through
        // composite_layers. If this drifts, the merge has forked the
        // blend math, which is the one thing this module must never do.
        let green = solid(8, 8, Rgba::opaque(0, 200, 40));
        let red = solid(8, 8, Rgba::new(220, 30, 0, 160));
        let below = input(&green, BlendMode::Normal, 255);
        let above = input(&red, BlendMode::Multiply, 200);

        let merged = merge_pair(&below, &above, 8, 8).expect("merge");
        let reference = composite_layers(8, 8, &[below, above]).expect("reference composite");

        assert_eq!(merged, reference, "merge_pair must equal a direct composite of the same inputs");
    }

    #[test]
    fn flatten_stacks_inputs_back_to_front() {
        // Three opaque layers; the topmost (last) wins where opaque, so the
        // flattened result is the top color everywhere.
        let blue = solid(4, 4, Rgba::opaque(0, 0, 255));
        let green = solid(4, 4, Rgba::opaque(0, 255, 0));
        let red = solid(4, 4, Rgba::opaque(255, 0, 0));
        let inputs = [
            input(&blue, BlendMode::Normal, 255),
            input(&green, BlendMode::Normal, 255),
            input(&red, BlendMode::Normal, 255),
        ];

        let flat = flatten(&inputs, 4, 4).expect("flatten");

        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(flat.pixel(x, y), Some(Rgba::opaque(255, 0, 0)), "({x},{y})");
            }
        }
    }

    #[test]
    fn flatten_reuses_composite_layers_exactly() {
        // The reuse guarantee for the variable-length stack: flatten must equal
        // a direct composite of the same inputs, byte for byte.
        let blue = solid(8, 8, Rgba::opaque(10, 20, 200));
        let green = solid(8, 8, Rgba::new(0, 220, 40, 170));
        let red = solid(8, 8, Rgba::new(220, 30, 0, 120));
        let inputs = [
            input(&blue, BlendMode::Normal, 255),
            input(&green, BlendMode::Multiply, 220),
            input(&red, BlendMode::Screen, 200),
        ];

        let flat = flatten(&inputs, 8, 8).expect("flatten");
        let reference = composite_layers(8, 8, &inputs).expect("reference composite");

        assert_eq!(flat, reference, "flatten must equal a direct composite of the same inputs");
    }

    #[test]
    fn merge_pair_rejects_dimension_mismatch() {
        let green = solid(4, 4, Rgba::opaque(0, 255, 0));
        let red = solid(2, 2, Rgba::opaque(255, 0, 0));
        let below = input(&green, BlendMode::Normal, 255);
        let above = input(&red, BlendMode::Normal, 255);

        // A top layer that does not match the target dimensions surfaces
        // the compositor's error rather than silently mis-aligning.
        assert!(merge_pair(&below, &above, 4, 4).is_err());
    }
}
