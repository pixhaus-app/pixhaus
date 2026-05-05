//! Dithering algorithms for palette-reduced export (GIF, indexed WebP).
//!
//! Two modes are available:
//! - [`DitherMode::FloydSteinberg`] — error diffusion; best quality, but
//!   introduces temporal noise between frames when applied independently.
//! - [`DitherMode::Bayer8x8`] — ordered (threshold) dithering; produces a
//!   regular cross-hatch pattern that is stable across frames and compresses
//!   better in GIF than error diffusion.
//!
//! Both modes operate on a flat RGBA pixel slice (4 bytes per pixel). Dithering
//! mutates the slice in-place before the caller passes it to the quantizer.

use pixhaus_core::project::Rgba;

// ── Public types ──────────────────────────────────────────────────────────────

/// Dithering algorithm applied before palette quantization.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum DitherMode {
    /// No dithering — each pixel maps to its nearest palette colour.
    #[default]
    Off,
    /// Floyd-Steinberg error diffusion. Highest visual quality; temporal noise
    /// between frames when each frame is quantized independently.
    FloydSteinberg,
    /// Ordered 8 × 8 Bayer threshold matrix. Stable pattern across frames;
    /// slightly lower quality than Floyd-Steinberg but friendlier to GIF LZW
    /// compression and less prone to frame flicker.
    Bayer8x8,
}

// ── Bayer 8 × 8 threshold matrix ─────────────────────────────────────────────

/// Bayer 8 × 8 ordered dithering matrix (values 0–63).
///
/// Normalised to 0.0–1.0 by dividing by 64 at the use site.
#[rustfmt::skip]
const BAYER_8X8: [[u8; 8]; 8] = [
    [ 0, 32,  8, 40,  2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44,  4, 36, 14, 46,  6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [ 3, 35, 11, 43,  1, 33,  9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47,  7, 39, 13, 45,  5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

// ── Palette helpers ───────────────────────────────────────────────────────────

/// Find the index of the nearest colour in `palette` to `pixel` (RGB
/// Euclidean distance; alpha is ignored).
fn nearest_index(pixel: [u8; 4], palette: &[[u8; 4]]) -> usize {
    let mut best = 0usize;
    let mut best_dist = u32::MAX;
    for (i, &p) in palette.iter().enumerate() {
        let dr = (i32::from(pixel[0]) - i32::from(p[0])).pow(2) as u32;
        let dg = (i32::from(pixel[1]) - i32::from(p[1])).pow(2) as u32;
        let db = (i32::from(pixel[2]) - i32::from(p[2])).pow(2) as u32;
        let dist = dr + dg + db;
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}

// ── Floyd-Steinberg ───────────────────────────────────────────────────────────

/// Apply Floyd-Steinberg error-diffusion dithering.
///
/// Each pixel is replaced with its nearest palette colour; the quantization
/// error is propagated to the right (7/16), lower-left (3/16), lower (5/16),
/// and lower-right (1/16) neighbours.
///
/// Alpha values are preserved unchanged. Fully-transparent pixels (alpha = 0)
/// are skipped — they do not contribute error to neighbours.
pub fn floyd_steinberg(pixels: &mut [u8], width: u32, height: u32, palette: &[[u8; 4]]) {
    let w = width as usize;
    let h = height as usize;

    // Work with signed i32 to hold accumulated errors.
    let mut buf: Vec<[i32; 3]> = pixels
        .chunks_exact(4)
        .map(|p| [i32::from(p[0]), i32::from(p[1]), i32::from(p[2])])
        .collect();

    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            // Skip fully transparent pixels.
            if pixels[idx * 4 + 3] == 0 {
                continue;
            }

            let orig = buf[idx];
            let clamped = [
                u8::try_from(orig[0].clamp(0, 255)).unwrap_or(0),
                u8::try_from(orig[1].clamp(0, 255)).unwrap_or(0),
                u8::try_from(orig[2].clamp(0, 255)).unwrap_or(0),
                pixels[idx * 4 + 3],
            ];

            let best = nearest_index(clamped, palette);
            let nearest = palette[best];

            // Write quantized colour back.
            pixels[idx * 4] = nearest[0];
            pixels[idx * 4 + 1] = nearest[1];
            pixels[idx * 4 + 2] = nearest[2];

            let err = [
                i32::from(clamped[0]) - i32::from(nearest[0]),
                i32::from(clamped[1]) - i32::from(nearest[1]),
                i32::from(clamped[2]) - i32::from(nearest[2]),
            ];

            // Distribute error with Floyd-Steinberg weights:
            // right 7/16, lower-left 3/16, lower 5/16, lower-right 1/16.
            let neighbours: [Option<(usize, i32)>; 4] = [
                (x + 1 < w).then_some((y * w + x + 1, 7)),
                (y + 1 < h && x > 0).then_some(((y + 1) * w + x - 1, 3)),
                (y + 1 < h).then_some(((y + 1) * w + x, 5)),
                (y + 1 < h && x + 1 < w).then_some(((y + 1) * w + x + 1, 1)),
            ];

            for (ni, weight) in neighbours.into_iter().flatten() {
                for c in 0..3 {
                    buf[ni][c] += err[c] * weight / 16;
                }
            }
        }
    }
}

// ── Bayer 8 × 8 ───────────────────────────────────────────────────────────────

/// Apply ordered Bayer 8 × 8 dithering.
///
/// Each pixel's RGB channels are perturbed by a threshold value drawn from the
/// Bayer matrix before finding the nearest palette colour. The threshold is
/// in the range `[−31, +32]` channel units, centred at zero.
///
/// Fully-transparent pixels (alpha = 0) are left unchanged.
pub fn bayer8x8(pixels: &mut [u8], width: u32, height: u32, palette: &[[u8; 4]]) {
    let w = width as usize;
    let h = height as usize;

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            if pixels[idx + 3] == 0 {
                continue;
            }

            // Threshold in range [0, 63]; map to [-31, +32] (centre at 0).
            let t = i32::from(BAYER_8X8[y % 8][x % 8]) - 31;

            let adjusted = [
                u8::try_from((i32::from(pixels[idx]) + t).clamp(0, 255)).unwrap_or(0),
                u8::try_from((i32::from(pixels[idx + 1]) + t).clamp(0, 255)).unwrap_or(0),
                u8::try_from((i32::from(pixels[idx + 2]) + t).clamp(0, 255)).unwrap_or(0),
                pixels[idx + 3],
            ];

            let best = nearest_index(adjusted, palette);
            let c = palette[best];
            pixels[idx] = c[0];
            pixels[idx + 1] = c[1];
            pixels[idx + 2] = c[2];
        }
    }
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Apply the requested dithering mode in-place.
///
/// When `mode` is [`DitherMode::Off`] the function is a no-op.
/// When `palette` is empty and mode is not `Off`, the function is also a no-op
/// (there are no colours to snap to; quantization is meaningless).
pub fn apply(mode: DitherMode, pixels: &mut [u8], width: u32, height: u32, palette: &[[u8; 4]]) {
    if palette.is_empty() {
        return;
    }
    match mode {
        DitherMode::Off => {}
        DitherMode::FloydSteinberg => floyd_steinberg(pixels, width, height, palette),
        DitherMode::Bayer8x8 => bayer8x8(pixels, width, height, palette),
    }
}

// ── Rgba → [u8; 4] helper ──────────────────────────────────────────────────────

/// Convert a slice of [`Rgba`] palette entries to the `[[u8; 4]]` form the
/// dithering functions expect.
pub fn palette_to_rgba4(palette: &[Rgba]) -> Vec<[u8; 4]> {
    palette.iter().map(|c| [c.r, c.g, c.b, c.a]).collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn two_color_palette() -> Vec<[u8; 4]> {
        vec![[0, 0, 0, 255], [255, 255, 255, 255]]
    }

    #[test]
    fn nearest_black_and_white() {
        let pal = two_color_palette();
        assert_eq!(nearest_index([10, 10, 10, 255], &pal), 0);
        assert_eq!(nearest_index([200, 200, 200, 255], &pal), 1);
    }

    #[test]
    fn floyd_steinberg_maps_to_palette() {
        let pal = two_color_palette();
        // A 2×1 grey image: [128,128,128,255, 128,128,128,255]
        let mut pixels = vec![128u8, 128, 128, 255, 128, 128, 128, 255];
        floyd_steinberg(&mut pixels, 2, 1, &pal);
        // Each output pixel must be one of the two palette colours.
        assert!(pixels[0] == 0 || pixels[0] == 255);
        assert!(pixels[4] == 0 || pixels[4] == 255);
    }

    #[test]
    fn bayer_maps_to_palette() {
        let pal = two_color_palette();
        let mut pixels = vec![128u8, 128, 128, 255];
        bayer8x8(&mut pixels, 1, 1, &pal);
        assert!(pixels[0] == 0 || pixels[0] == 255);
    }

    #[test]
    fn transparent_pixel_skipped_by_fs() {
        let pal = two_color_palette();
        let mut pixels = vec![128u8, 128, 128, 0]; // alpha = 0
        let original = pixels.clone();
        floyd_steinberg(&mut pixels, 1, 1, &pal);
        assert_eq!(pixels, original, "transparent pixel must not be modified");
    }

    #[test]
    fn transparent_pixel_skipped_by_bayer() {
        let pal = two_color_palette();
        let mut pixels = vec![128u8, 128, 128, 0];
        let original = pixels.clone();
        bayer8x8(&mut pixels, 1, 1, &pal);
        assert_eq!(pixels, original);
    }

    #[test]
    fn apply_off_is_noop() {
        let pal = two_color_palette();
        let mut pixels = vec![128u8, 128, 128, 255];
        let original = pixels.clone();
        apply(DitherMode::Off, &mut pixels, 1, 1, &pal);
        assert_eq!(pixels, original);
    }

    #[test]
    fn apply_empty_palette_is_noop() {
        let mut pixels = vec![128u8, 128, 128, 255];
        let original = pixels.clone();
        apply(DitherMode::FloydSteinberg, &mut pixels, 1, 1, &[]);
        assert_eq!(pixels, original);
    }
}
