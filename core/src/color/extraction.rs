//! Palette extraction from raster images.
//!
//! Used by the B10.3 approval flow: when the user approves a
//! [`crate::project::SheetVariant`] as canonical, the editor extracts the
//! sheet's palette and writes it to
//! [`crate::project::SheetVariant::extracted_palette`] so subsequent AI
//! verbs can constrain generation to the same palette as the anchor.
//!
//! The algorithm is the literal reading of the brief:
//!
//! > The embedded reference sheet's `extracted_palette` runs (eyedropper extraction
//! > across the sheet image, deduplicated, ordered by frequency).
//!
//! - Decode the bytes through the `image` crate.
//! - Convert to RGBA8.
//! - Skip pixels with alpha below [`MIN_OPAQUE_ALPHA`] (background canvas
//!   shouldn't dominate the palette).
//! - Optionally quantize each channel to [`DEFAULT_QUANTIZE_BITS`] so
//!   anti-aliasing fringes collapse onto their dominant tone.
//! - Count occurrences and sort descending by count. Ties break by
//!   insertion order (stable sort) so the first time a color is seen wins.
//! - Truncate to [`DEFAULT_MAX_COLORS`].

use std::collections::HashMap;

use image::GenericImageView;

use crate::project::color::Rgba;
use crate::project::palette::PaletteEntry;

/// Default cap on the number of swatches the extractor returns.
///
/// 64 is the floor most pixel-art palettes hit and the ceiling most
/// hand-curated ones stop at; matches the size of common reference sets
/// (PICO-8, Endesga 64).
pub const DEFAULT_MAX_COLORS: usize = 64;

/// Default per-channel bit depth used for quantisation.
///
/// 5 bits collapses 256 levels per channel down to 32, which folds JPEG
/// noise and dithering fringe colors into their dominant tone while
/// leaving room for distinct ramps. The output bytes are upscaled back
/// to the nearest 8-bit value so callers see standard `Rgba` swatches.
pub const DEFAULT_QUANTIZE_BITS: u8 = 5;

/// Minimum alpha (out of 255) for a pixel to count toward the palette.
///
/// Anything below this is treated as background. Most reference sheets
/// have an alpha-cleared border and any soft AA edge falls under this
/// threshold; the rest of the image is opaque.
pub const MIN_OPAQUE_ALPHA: u8 = 128;

/// Tunable knobs for [`extract_palette_from_image_bytes`] and
/// [`extract_palette_from_rgba8`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtractionOptions {
    /// Maximum number of swatches to return.
    pub max_colors: usize,
    /// Bits per channel kept during quantisation. `8` disables it.
    /// Values are clamped to `1..=8` at use — `0` previously collapsed
    /// every swatch to black, which was never the intended behaviour.
    pub quantize_bits: u8,
    /// Minimum alpha to count a pixel.
    pub min_opaque_alpha: u8,
}

impl Default for ExtractionOptions {
    fn default() -> Self {
        Self {
            max_colors: DEFAULT_MAX_COLORS,
            quantize_bits: DEFAULT_QUANTIZE_BITS,
            min_opaque_alpha: MIN_OPAQUE_ALPHA,
        }
    }
}

/// Errors returned by the extractor.
#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    /// The bytes did not decode as a recognised image format.
    #[error("failed to decode image: {0}")]
    Decode(#[from] image::ImageError),
}

/// Decodes `bytes` through the `image` crate and returns the extracted
/// palette ordered by descending frequency.
///
/// Accepts any format `image` knows (PNG, JPEG, WebP, ...). Use
/// [`extract_palette_from_rgba8`] when the bytes are already decoded.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when `image::load_from_memory`
/// rejects the bytes.
pub fn extract_palette_from_image_bytes(bytes: &[u8], options: ExtractionOptions) -> Result<Vec<PaletteEntry>, ExtractionError> {
    let img = image::load_from_memory(bytes)?;
    let (w, h) = img.dimensions();
    let rgba = img.into_rgba8();
    Ok(extract_palette_from_rgba8(&rgba, w, h, options))
}

/// Returns the palette extracted from a tightly-packed RGBA8 buffer.
///
/// `bytes.len()` must be `width * height * 4`; rows mismatched against
/// that constraint return an empty palette rather than panicking.
#[must_use]
pub fn extract_palette_from_rgba8(bytes: &[u8], width: u32, height: u32, options: ExtractionOptions) -> Vec<PaletteEntry> {
    let expected = (width as usize).saturating_mul(height as usize).saturating_mul(4);
    if bytes.len() != expected || expected == 0 || options.max_colors == 0 {
        return Vec::new();
    }

    let quantize_bits = options.quantize_bits.clamp(1, 8);
    let min_alpha = options.min_opaque_alpha;

    // Insertion order so ties break by "first seen".
    let mut counts: HashMap<[u8; 4], (u64, u32)> = HashMap::new();
    let mut order: u32 = 0;

    for chunk in bytes.chunks_exact(4) {
        let a = chunk[3];
        if a < min_alpha {
            continue;
        }
        let key = quantize_pixel(chunk[0], chunk[1], chunk[2], 255, quantize_bits);
        let entry = counts.entry(key).or_insert_with(|| {
            let o = order;
            order = order.saturating_add(1);
            (0, o)
        });
        entry.0 += 1;
    }

    let mut sorted: Vec<([u8; 4], u64, u32)> = counts.into_iter().map(|(k, (c, o))| (k, c, o)).collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)));
    sorted.truncate(options.max_colors);

    sorted
        .into_iter()
        .map(|(k, _, _)| PaletteEntry::new(Rgba::new(k[0], k[1], k[2], k[3])))
        .collect()
}

/// Quantises one pixel to `bits` per channel, then upscales back to
/// 8 bits so the output is still a normal sRGB byte triple.
///
/// `bits` must be `1..=8`; `8` is a no-op identity. Lower values fold
/// nearby colors together by clearing the low bits.
fn quantize_pixel(r: u8, g: u8, b: u8, a: u8, bits: u8) -> [u8; 4] {
    if bits >= 8 {
        return [r, g, b, a];
    }
    let mask: u8 = if bits == 0 {
        0
    } else {
        // `bits` is in 1..=7 here, so the shift is valid for u8.
        0xFFu8.wrapping_shl(u32::from(8 - bits))
    };
    [r & mask, g & mask, b & mask, a]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba8(width: u32, height: u32, pixels: &[(u8, u8, u8, u8)]) -> Vec<u8> {
        assert_eq!(pixels.len(), (width * height) as usize, "fixture pixel count must match dimensions");
        let mut out = Vec::with_capacity(pixels.len() * 4);
        for &(r, g, b, a) in pixels {
            out.extend_from_slice(&[r, g, b, a]);
        }
        out
    }

    #[test]
    fn empty_buffer_returns_empty_palette() {
        let out = extract_palette_from_rgba8(&[], 0, 0, ExtractionOptions::default());
        assert!(out.is_empty());
    }

    #[test]
    fn buffer_size_mismatch_returns_empty_palette() {
        let bytes = vec![0u8; 7];
        let out = extract_palette_from_rgba8(&bytes, 2, 1, ExtractionOptions::default());
        assert!(out.is_empty());
    }

    #[test]
    fn most_frequent_color_first() {
        // Three reds, two greens, one blue → red, green, blue.
        let pixels = [
            (255, 0, 0, 255),
            (255, 0, 0, 255),
            (255, 0, 0, 255),
            (0, 255, 0, 255),
            (0, 255, 0, 255),
            (0, 0, 255, 255),
        ];
        let bytes = rgba8(6, 1, &pixels);
        let opts = ExtractionOptions {
            quantize_bits: 8,
            ..Default::default()
        };
        let pal = extract_palette_from_rgba8(&bytes, 6, 1, opts);
        let colors: Vec<Rgba> = pal.iter().map(|e| e.color).collect();
        assert_eq!(colors.len(), 3);
        assert_eq!(colors[0], Rgba::opaque(255, 0, 0));
        assert_eq!(colors[1], Rgba::opaque(0, 255, 0));
        assert_eq!(colors[2], Rgba::opaque(0, 0, 255));
    }

    #[test]
    fn transparent_pixels_are_skipped() {
        let pixels = [
            (255, 0, 0, 255),
            (0, 0, 0, 0),  // fully transparent
            (0, 0, 0, 16), // below threshold
            (0, 255, 0, 255),
        ];
        let bytes = rgba8(4, 1, &pixels);
        let opts = ExtractionOptions {
            quantize_bits: 8,
            ..Default::default()
        };
        let pal = extract_palette_from_rgba8(&bytes, 4, 1, opts);
        let colors: Vec<Rgba> = pal.iter().map(|e| e.color).collect();
        assert_eq!(colors.len(), 2, "only fully-opaque pixels should count");
        assert!(colors.contains(&Rgba::opaque(255, 0, 0)));
        assert!(colors.contains(&Rgba::opaque(0, 255, 0)));
    }

    #[test]
    fn extracted_palette_is_deduplicated() {
        let pixels = [(10, 20, 30, 255); 100];
        let bytes = rgba8(10, 10, &pixels);
        let opts = ExtractionOptions {
            quantize_bits: 8,
            ..Default::default()
        };
        let pal = extract_palette_from_rgba8(&bytes, 10, 10, opts);
        assert_eq!(pal.len(), 1);
        assert_eq!(pal[0].color, Rgba::opaque(10, 20, 30));
    }

    #[test]
    fn quantization_collapses_nearby_colors() {
        // Two reds that differ only in the low bits should collapse with
        // 5-bit quantisation but stay distinct at 8 bits.
        let pixels = [(255, 0, 0, 255), (252, 1, 2, 255)];
        let bytes = rgba8(2, 1, &pixels);

        let no_quant = extract_palette_from_rgba8(
            &bytes,
            2,
            1,
            ExtractionOptions {
                quantize_bits: 8,
                ..Default::default()
            },
        );
        assert_eq!(no_quant.len(), 2, "no quantisation: two distinct colors");

        let quant = extract_palette_from_rgba8(
            &bytes,
            2,
            1,
            ExtractionOptions {
                quantize_bits: 5,
                ..Default::default()
            },
        );
        assert_eq!(quant.len(), 1, "5-bit quantisation collapses near-reds");
    }

    #[test]
    fn quantize_bits_zero_does_not_collapse_to_black() {
        // `quantize_bits: 0` previously masked every channel to 0 and
        // produced an all-black palette. The clamp to 1..=8 means it
        // behaves the same as `quantize_bits: 1` now — coarse but not
        // catastrophic.
        let bytes = rgba8(2, 1, &[(200, 50, 50, 255), (255, 240, 240, 255)]);
        let pal = extract_palette_from_rgba8(
            &bytes,
            2,
            1,
            ExtractionOptions {
                quantize_bits: 0,
                ..Default::default()
            },
        );
        assert!(!pal.is_empty(), "must not produce empty palette");
        assert!(
            pal.iter().any(|p| p.color.r > 0 || p.color.g > 0 || p.color.b > 0),
            "must not collapse every swatch to black"
        );
    }

    #[test]
    fn max_colors_caps_output() {
        let mut pixels = Vec::with_capacity(16);
        for i in 0..16u8 {
            pixels.push((i * 16, 0, 0, 255));
        }
        let bytes = rgba8(16, 1, &pixels);
        let opts = ExtractionOptions {
            max_colors: 4,
            quantize_bits: 8,
            ..Default::default()
        };
        let pal = extract_palette_from_rgba8(&bytes, 16, 1, opts);
        assert_eq!(pal.len(), 4);
    }

    #[test]
    fn ties_break_by_first_seen() {
        // Three distinct colors, each appearing exactly once. Sort must
        // be stable: order in the output equals first-seen order in the
        // buffer.
        let pixels = [(10, 0, 0, 255), (0, 10, 0, 255), (0, 0, 10, 255)];
        let bytes = rgba8(3, 1, &pixels);
        let opts = ExtractionOptions {
            quantize_bits: 8,
            ..Default::default()
        };
        let pal = extract_palette_from_rgba8(&bytes, 3, 1, opts);
        let colors: Vec<Rgba> = pal.iter().map(|e| e.color).collect();
        assert_eq!(colors, vec![Rgba::opaque(10, 0, 0), Rgba::opaque(0, 10, 0), Rgba::opaque(0, 0, 10),]);
    }

    #[test]
    fn extract_from_png_bytes_decodes_and_counts() {
        use image::{ImageBuffer, ImageFormat, RgbaImage};
        use std::io::Cursor;

        // 2×1 image: one red, one green.
        let mut img: RgbaImage = ImageBuffer::new(2, 1);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, image::Rgba([0, 255, 0, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png).unwrap();

        let pal = extract_palette_from_image_bytes(
            &buf,
            ExtractionOptions {
                quantize_bits: 8,
                ..Default::default()
            },
        )
        .unwrap();
        let colors: Vec<Rgba> = pal.iter().map(|e| e.color).collect();
        assert!(colors.contains(&Rgba::opaque(255, 0, 0)));
        assert!(colors.contains(&Rgba::opaque(0, 255, 0)));
    }

    #[test]
    fn extract_from_png_bytes_rejects_garbage() {
        let err = extract_palette_from_image_bytes(b"not a png", ExtractionOptions::default());
        assert!(err.is_err());
    }
}
