//! Pure CPU compositing: flatten a sprite's visible layers into one RGBA8 buffer.
//!
//! This is the source of truth the renderer uploads as a texture (bible 16.5): the
//! GPU texture is a cache, this result is authoritative. Layers composite
//! bottom-to-top with straight-alpha source-over and per-layer opacity.
//!
//! PERF: composites the whole sprite. Generate sprites are small, so a full pass is
//! fine; dirty-region recompositing is the documented follow-up for large canvases
//! (bible 23.2).

use thiserror::Error;

use crate::document::{Document, Layer};
use crate::ids::{FrameId, SpriteId};
use crate::pixel::{PixelBuffer, Rgba};

/// Why a sprite could not be composited.
#[derive(Debug, Error)]
pub enum CompositeError {
    /// The sprite id is not present in the document.
    #[error("sprite {0:?} not found")]
    SpriteNotFound(SpriteId),
    /// The frame id is not present in the sprite.
    #[error("frame {0:?} not found")]
    FrameNotFound(FrameId),
    /// A layer references a pixel buffer that is not in the store.
    #[error("layer references a missing pixel buffer")]
    MissingBuffer,
    /// A layer's buffer dimensions do not match the sprite's.
    #[error("layer buffer size does not match sprite size")]
    SizeMismatch,
}

/// Composites the active sprite, or `None` if there is no active sprite or it fails.
pub fn composite_active(doc: &Document) -> Option<PixelBuffer> {
    let id = doc.active_sprite()?;
    composite_sprite(doc, id).ok()
}

/// Flattens the active frame of `sprite` into a fresh tightly-packed RGBA8 buffer.
///
/// # Errors
/// Returns [`CompositeError`] if the sprite or its active frame is absent, a layer's
/// buffer is missing, or a layer's buffer size does not match the sprite.
pub fn composite_sprite(doc: &Document, sprite: SpriteId) -> Result<PixelBuffer, CompositeError> {
    let sprite = doc.sprite(sprite).ok_or(CompositeError::SpriteNotFound(sprite))?;
    let frame = sprite.active_frame().ok_or(CompositeError::FrameNotFound(sprite.active_frame))?;
    composite_layers(doc, sprite.width, sprite.height, &frame.layers)
}

/// Flattens a specific frame of `sprite` into a fresh tightly-packed RGBA8 buffer.
///
/// # Errors
/// Returns [`CompositeError`] if the sprite or `frame` is absent, a layer's buffer is
/// missing, or a layer's buffer size does not match the sprite.
pub fn composite_frame(doc: &Document, sprite: SpriteId, frame: FrameId) -> Result<PixelBuffer, CompositeError> {
    let sprite = doc.sprite(sprite).ok_or(CompositeError::SpriteNotFound(sprite))?;
    let frame = sprite.frame(frame).ok_or(CompositeError::FrameNotFound(frame))?;
    composite_layers(doc, sprite.width, sprite.height, &frame.layers)
}

/// Flattens a layer stack over a transparent backdrop at the given size.
fn composite_layers(doc: &Document, width: u32, height: u32, layers: &[Layer]) -> Result<PixelBuffer, CompositeError> {
    let row_bytes = width as usize * 4;
    let mut out = vec![0u8; row_bytes * height as usize];

    for layer in layers {
        if !layer.visible || layer.opacity <= 0.0 {
            continue;
        }
        let buffer = doc.buffers().get(layer.buffer).ok_or(CompositeError::MissingBuffer)?;
        if buffer.width() != width || buffer.height() != height {
            return Err(CompositeError::SizeMismatch);
        }
        composite_layer(&mut out, row_bytes, buffer, layer.opacity.clamp(0.0, 1.0));
    }

    PixelBuffer::from_rgba8(width, height, width * 4, out).map_err(|_| CompositeError::SizeMismatch)
}

/// Blends one layer's pixels over `out` (tightly packed at `row_bytes` per row).
fn composite_layer(out: &mut [u8], row_bytes: usize, src: &PixelBuffer, opacity: f32) {
    let src_bytes = src.as_bytes();
    let src_stride = src.stride() as usize;
    for (y, row) in out.chunks_exact_mut(row_bytes).enumerate() {
        let src_row = y * src_stride;
        for (x, dst) in row.chunks_exact_mut(4).enumerate() {
            let offset = src_row + x * 4;
            let Some(s) = src_bytes.get(offset..offset + 4) else {
                continue;
            };
            let source = Rgba {
                r: s[0],
                g: s[1],
                b: s[2],
                a: s[3],
            };
            let below = Rgba {
                r: dst[0],
                g: dst[1],
                b: dst[2],
                a: dst[3],
            };
            let blended = blend_normal(source, below, opacity);
            dst[0] = blended.r;
            dst[1] = blended.g;
            dst[2] = blended.b;
            dst[3] = blended.a;
        }
    }
}

/// Straight-alpha source-over of `src` (scaled by `opacity`) over `dst`.
fn blend_normal(src: Rgba, dst: Rgba, opacity: f32) -> Rgba {
    let src_a = (f32::from(src.a) / 255.0) * opacity;
    if src_a <= 0.0 {
        return dst;
    }
    let dst_a = f32::from(dst.a) / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return Rgba::TRANSPARENT;
    }
    let mix = |s: u8, d: u8| -> u8 {
        let value = (f32::from(s) * src_a + f32::from(d) * dst_a * (1.0 - src_a)) / out_a;
        to_u8(value)
    };
    Rgba {
        r: mix(src.r, dst.r),
        g: mix(src.g, dst.g),
        b: mix(src.b, dst.b),
        a: to_u8(out_a * 255.0),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u8(value: f32) -> u8 {
    // `value` is clamped into 0.0..=255.0 here, so the round-then-cast neither
    // truncates a meaningful value nor loses a sign.
    value.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::Command;
    use crate::commands::ApplyGeneratedAsset;
    use crate::pixel::BlendMode;

    // Builds a 1x1 document with one opaque sprite of the given colour via the
    // public command path, returning the document and its sprite id.
    fn doc_with_pixel(rgba: [u8; 4]) -> Document {
        let mut doc = Document::new();
        let mut cmd = ApplyGeneratedAsset::new("s".to_owned(), 1, 1, 4, rgba.to_vec());
        cmd.apply(&mut doc).unwrap();
        doc
    }

    #[test]
    fn single_opaque_layer_is_identity() {
        let doc = doc_with_pixel([12, 34, 56, 255]);
        let id = doc.active_sprite().unwrap();
        let out = composite_sprite(&doc, id).unwrap();
        assert_eq!(out.pixel(0, 0), Some(Rgba::new(12, 34, 56, 255)));
    }

    #[test]
    fn invisible_layer_is_skipped() {
        let mut doc = doc_with_pixel([255, 0, 0, 255]);
        let id = doc.active_sprite().unwrap();
        // Hide the only layer; the composite should be transparent.
        if let Some(frame) = doc.sprites.iter_mut().find(|s| s.id == id).and_then(|s| s.active_frame_mut()) {
            frame.layers[0].visible = false;
        }
        let out = composite_sprite(&doc, id).unwrap();
        assert_eq!(out.pixel(0, 0), Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn half_opacity_over_opaque_blends_to_midpoint() {
        // Bottom opaque black, top opaque white at 50% -> ~mid grey.
        let mut doc = Document::new();
        let mut bottom = ApplyGeneratedAsset::new("b".to_owned(), 1, 1, 4, vec![0, 0, 0, 255]);
        bottom.apply(&mut doc).unwrap();
        let id = doc.active_sprite().unwrap();
        // Add a white top layer into the same sprite's active frame, at half opacity.
        let white = doc.buffers.insert(PixelBuffer::from_rgba8(1, 1, 4, vec![255, 255, 255, 255]).unwrap());
        if let Some(frame) = doc.sprites.iter_mut().find(|s| s.id == id).and_then(|s| s.active_frame_mut()) {
            let layer_id = frame.mint_layer_id();
            frame.layers.push(crate::Layer {
                id: layer_id,
                name: "top".to_owned(),
                visible: true,
                opacity: 0.5,
                blend: BlendMode::Normal,
                buffer: white,
            });
        }
        let out = composite_sprite(&doc, id).unwrap();
        let px = out.pixel(0, 0).unwrap();
        assert!((i32::from(px.r) - 128).abs() <= 2, "got {}", px.r);
        assert_eq!(px.a, 255);
    }

    proptest! {
        // A fully-opaque sprite composites to exactly its own bytes: source-over a
        // transparent backdrop returns the source. Holds across dimensions and colours.
        #[test]
        fn opaque_uniform_sprite_composites_to_itself(
            w in 1u32..=16,
            h in 1u32..=16,
            r in 0u8..=255,
            g in 0u8..=255,
            b in 0u8..=255,
        ) {
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            for _ in 0..(w * h) {
                rgba.extend_from_slice(&[r, g, b, 255]);
            }
            let mut doc = Document::new();
            let mut cmd = ApplyGeneratedAsset::new("p".to_owned(), w, h, w * 4, rgba.clone());
            cmd.apply(&mut doc).unwrap();
            let id = doc.active_sprite().unwrap();
            let out = composite_sprite(&doc, id).unwrap();
            prop_assert_eq!(out.as_bytes(), rgba.as_slice());
        }
    }
}
