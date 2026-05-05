//! GIF encoder for animated pixel-art export (stream S11).
//!
//! Three palette modes are available, matching the brief:
//! - [`PaletteMode::ExistingPalette`] — use the project palette directly (≤ 256
//!   entries). Best round-trip fidelity for indexed-mode sprites.
//! - [`PaletteMode::GlobalQuantize`] — run `NeuQuant` across every pixel of every
//!   frame to build one shared 256-colour palette. All frames look consistent;
//!   uniform colour budget.
//! - [`PaletteMode::PerFrameQuantize`] — quantize each frame independently to 256
//!   colours. Maximum per-frame quality; palettes may shift between frames.
//!
//! Dithering (see [`super::dither`]) is applied before quantization in the
//! `ExistingPalette` and `GlobalQuantize` modes; in `PerFrameQuantize` mode the
//! `gif` crate's `NeuQuant` runs after any dithering pass.

use std::borrow::Cow;
use std::io::Write;

use gif::{Encoder, Frame, Repeat};
use pixhaus_core::canvas::PixelBuffer;
use pixhaus_core::project::Palette;

use super::dither::{self, DitherMode};
use crate::error::{Error, Result};

// ── Public types ──────────────────────────────────────────────────────────────

/// Which colour palette strategy to use when encoding the GIF.
#[derive(Clone, Debug, Default)]
pub enum PaletteMode {
    /// Use the project's existing palette (must have ≤ 256 entries).
    ///
    /// Ideal for indexed-mode sprites where the palette is already the
    /// intended 256-colour set.
    ExistingPalette,

    /// Run `NeuQuant` across all frames combined to build one 256-colour global
    /// palette shared by every frame.
    #[default]
    GlobalQuantize,

    /// Quantize each frame independently to up to 256 colours. Each frame
    /// carries its own local colour table.
    PerFrameQuantize,
}

/// How many times the GIF animation loops.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum LoopCount {
    /// Loop indefinitely (GIF extension `NETSCAPE 2.0` with count = 0).
    #[default]
    Infinite,
    /// Loop exactly `n` times. `0` is re-mapped to [`LoopCount::Infinite`] by
    /// the encoder.
    Count(u16),
}

/// Options for GIF export.
#[derive(Clone, Debug, Default)]
pub struct GifOptions {
    /// Palette selection strategy.
    pub palette_mode: PaletteMode,
    /// Dithering algorithm applied before quantization.
    pub dither: DitherMode,
    /// Loop count for the animation.
    pub repeat: LoopCount,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Convert a [`LoopCount`] to the [`gif::Repeat`] type.
fn to_gif_repeat(lc: LoopCount) -> Repeat {
    match lc {
        LoopCount::Infinite | LoopCount::Count(0) => Repeat::Infinite,
        LoopCount::Count(n) => Repeat::Finite(n),
    }
}

/// Build a flat RGB (3-bytes-per-entry) colour table from a core palette.
fn palette_to_rgb_table(palette: &Palette) -> Vec<u8> {
    let mut table = Vec::with_capacity(palette.colors.len() * 3);
    for entry in &palette.colors {
        table.push(entry.color.r);
        table.push(entry.color.g);
        table.push(entry.color.b);
    }
    table
}

/// Build a flat RGB colour table from the `NeuQuant` colour map.
///
/// `color_quant::NeuQuant::color_map_rgba()` returns groups of 4 bytes
/// (RGBA). The GIF colour table needs RGB triples.
fn neuquant_to_rgb_table(rgba_map: &[u8]) -> Vec<u8> {
    rgba_map
        .chunks_exact(4)
        .flat_map(|c| [c[0], c[1], c[2]])
        .collect()
}

/// Quantize `rgba_pixels` against `palette` (RGBA4 entries) and return a
/// packed Vec of palette indices (one byte per pixel).
fn quantize_to_palette(
    rgba_pixels: &[u8],
    palette: &[[u8; 4]],
    width: u32,
    height: u32,
    dither: DitherMode,
) -> Vec<u8> {
    let mut pixels = rgba_pixels.to_owned();
    dither::apply(dither, &mut pixels, width, height, palette);

    pixels
        .chunks_exact(4)
        .map(|p| {
            // Transparent pixels map to palette index 0 by convention; the
            // encoder marks index 0 as the transparent colour if any frame has
            // transparent pixels.
            if p[3] == 0 {
                return 0u8;
            }
            let rgba4 = [p[0], p[1], p[2], p[3]];
            // Linear scan — palette ≤ 256, so this is at most 256 compares.
            palette
                .iter()
                .enumerate()
                .min_by_key(|&(_, &c)| {
                    let sq = |a: u8, b: u8| {
                        let d = u32::from(a).abs_diff(u32::from(b));
                        d * d
                    };
                    sq(rgba4[0], c[0]) + sq(rgba4[1], c[1]) + sq(rgba4[2], c[2])
                })
                .map_or(0, |(i, _)| u8::try_from(i).unwrap_or(0))
        })
        .collect()
}

/// Extract raw RGBA bytes from a [`PixelBuffer`] into a flat `Vec<u8>`.
fn buffer_to_rgba_vec(buf: &PixelBuffer) -> Vec<u8> {
    let w = buf.width() as usize;
    let h = buf.height() as usize;
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..buf.height() {
        if let Some(row) = buf.row(y) {
            out.extend_from_slice(row);
        } else {
            out.extend(std::iter::repeat_n(0u8, w * 4));
        }
    }
    out
}

/// Convert `u32` canvas dimensions to `u16`, returning an error if either
/// dimension exceeds the GIF format's 65535 px limit.
fn gif_dims(width: u32, height: u32) -> Result<(u16, u16)> {
    let w16 = u16::try_from(width).map_err(|_| {
        Error::Io(std::io::Error::other(
            "canvas exceeds GIF max width (65535 px)",
        ))
    })?;
    let h16 = u16::try_from(height).map_err(|_| {
        Error::Io(std::io::Error::other(
            "canvas exceeds GIF max height (65535 px)",
        ))
    })?;
    Ok((w16, h16))
}

/// Convert frame duration (milliseconds) to GIF delay units (1/100 second).
///
/// GIF delay must be ≥ 1; durations shorter than 10 ms are rounded up.
fn ms_to_centisec(ms: u32) -> u16 {
    let cs = ms.saturating_add(9) / 10;
    u16::try_from(cs.max(1)).unwrap_or(u16::MAX)
}

// ── Public encoder ─────────────────────────────────────────────────────────────

/// Encode `frames` as an animated GIF into `writer`.
///
/// `frames` is a slice of `(PixelBuffer, duration_ms)` pairs. All buffers must
/// have the same dimensions.
///
/// # Errors
///
/// - [`Error::NoAnimationFrames`] if `frames` is empty.
/// - [`Error::AnimFrameSizeMismatch`] if frames differ in size.
/// - [`Error::PaletteExceedsGifMax`] if `PaletteMode::ExistingPalette` is chosen
///   but the palette has more than 256 entries.
/// - [`Error::GifEncode`] if the `gif` crate fails.
pub fn encode_gif<W: Write>(
    frames: &[(PixelBuffer, u32)],
    options: &GifOptions,
    existing_palette: Option<&Palette>,
    writer: W,
) -> Result<()> {
    if frames.is_empty() {
        return Err(Error::NoAnimationFrames);
    }

    let (first_buf, _) = &frames[0];
    let width = first_buf.width();
    let height = first_buf.height();

    for (i, (buf, _)) in frames.iter().enumerate().skip(1) {
        if buf.width() != width || buf.height() != height {
            return Err(Error::AnimFrameSizeMismatch {
                index: i,
                expected_w: width,
                expected_h: height,
                actual_w: buf.width(),
                actual_h: buf.height(),
            });
        }
    }

    match &options.palette_mode {
        PaletteMode::ExistingPalette => {
            encode_existing_palette(frames, options, existing_palette, width, height, writer)
        }
        PaletteMode::GlobalQuantize => {
            encode_global_quantize(frames, options, width, height, writer)
        }
        PaletteMode::PerFrameQuantize => encode_per_frame(frames, options, width, height, writer),
    }
}

// ── ExistingPalette mode ──────────────────────────────────────────────────────

fn encode_existing_palette<W: Write>(
    frames: &[(PixelBuffer, u32)],
    options: &GifOptions,
    existing_palette: Option<&Palette>,
    width: u32,
    height: u32,
    writer: W,
) -> Result<()> {
    let (w16, h16) = gif_dims(width, height)?;
    let palette = existing_palette.ok_or_else(|| {
        Error::InvalidPalette("ExistingPalette mode requires a project palette".into())
    })?;

    if palette.colors.len() > 256 {
        return Err(Error::PaletteExceedsGifMax {
            count: palette.colors.len(),
        });
    }

    let rgb_table = palette_to_rgb_table(palette);
    let rgba4: Vec<[u8; 4]> = palette
        .colors
        .iter()
        .map(|e| [e.color.r, e.color.g, e.color.b, e.color.a])
        .collect();

    let mut encoder = Encoder::new(writer, w16, h16, &rgb_table)?;
    encoder.set_repeat(to_gif_repeat(options.repeat))?;

    for (buf, duration_ms) in frames {
        let rgba = buffer_to_rgba_vec(buf);
        let indexed = quantize_to_palette(&rgba, &rgba4, width, height, options.dither);

        let has_transparent = rgba.chunks_exact(4).any(|p| p[3] == 0);
        let transparent = has_transparent.then_some(0u8);

        let frame = Frame {
            delay: ms_to_centisec(*duration_ms),
            width: w16,
            height: h16,
            buffer: Cow::Owned(indexed),
            transparent,
            palette: None, // use global colour table
            ..Frame::default()
        };
        encoder.write_frame(&frame)?;
    }

    Ok(())
}

// ── GlobalQuantize mode ───────────────────────────────────────────────────────

fn encode_global_quantize<W: Write>(
    frames: &[(PixelBuffer, u32)],
    options: &GifOptions,
    width: u32,
    height: u32,
    writer: W,
) -> Result<()> {
    let (w16, h16) = gif_dims(width, height)?;
    // Collect all RGBA pixels to build the global palette.
    let all_pixels: Vec<u8> = frames
        .iter()
        .flat_map(|(buf, _)| buffer_to_rgba_vec(buf))
        .collect();

    // NeuQuant: sample_fac 10 is a good balance of speed vs. quality.
    let nq = color_quant::NeuQuant::new(10, 256, &all_pixels);
    let rgb_table = neuquant_to_rgb_table(&nq.color_map_rgba());

    // Build RGBA4 palette for dither + quantize.
    let rgba4: Vec<[u8; 4]> = nq
        .color_map_rgba()
        .chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect();

    let mut encoder = Encoder::new(writer, w16, h16, &rgb_table)?;
    encoder.set_repeat(to_gif_repeat(options.repeat))?;

    for (buf, duration_ms) in frames {
        let rgba = buffer_to_rgba_vec(buf);
        let indexed = quantize_to_palette(&rgba, &rgba4, width, height, options.dither);

        let has_transparent = rgba.chunks_exact(4).any(|p| p[3] == 0);
        let transparent = has_transparent.then_some(0u8);

        let frame = Frame {
            delay: ms_to_centisec(*duration_ms),
            width: w16,
            height: h16,
            buffer: Cow::Owned(indexed),
            transparent,
            palette: None,
            ..Frame::default()
        };
        encoder.write_frame(&frame)?;
    }

    Ok(())
}

// ── PerFrameQuantize mode ─────────────────────────────────────────────────────

fn encode_per_frame<W: Write>(
    frames: &[(PixelBuffer, u32)],
    options: &GifOptions,
    width: u32,
    height: u32,
    writer: W,
) -> Result<()> {
    let (w16, h16) = gif_dims(width, height)?;
    // No global palette — each frame carries its own local table.
    let mut encoder = Encoder::new(writer, w16, h16, &[])?;
    encoder.set_repeat(to_gif_repeat(options.repeat))?;

    for (buf, duration_ms) in frames {
        let mut rgba = buffer_to_rgba_vec(buf);

        // Dithering in per-frame mode: build a per-frame palette via NeuQuant
        // first, dither against it, then let from_rgba_speed re-quantize.
        if options.dither != DitherMode::Off {
            let nq = color_quant::NeuQuant::new(10, 256, &rgba);
            let rgba4: Vec<[u8; 4]> = nq
                .color_map_rgba()
                .chunks_exact(4)
                .map(|c| [c[0], c[1], c[2], c[3]])
                .collect();
            dither::apply(options.dither, &mut rgba, width, height, &rgba4);
        }

        // gif::Frame::from_rgba_speed quantizes and embeds local palette.
        let mut frame = Frame::from_rgba_speed(w16, h16, &mut rgba, 10);
        frame.delay = ms_to_centisec(*duration_ms);

        encoder.write_frame(&frame)?;
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::canvas::PixelBuffer;
    use pixhaus_core::project::id::PaletteId;
    use pixhaus_core::project::{Palette, Rgba};

    fn solid_buf(w: u32, h: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(w, h, color).unwrap()
    }

    fn two_color_palette() -> Palette {
        Palette::from_colors(
            PaletteId::new(1),
            "test",
            vec![Rgba::opaque(0, 0, 0), Rgba::opaque(255, 255, 255)],
        )
    }

    fn encode_to_bytes(
        frames: &[(PixelBuffer, u32)],
        opts: &GifOptions,
        palette: Option<&Palette>,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        encode_gif(frames, opts, palette, &mut out).unwrap();
        out
    }

    #[test]
    fn rejects_empty_frames() {
        let opts = GifOptions::default();
        let mut out = Vec::new();
        assert!(matches!(
            encode_gif(&[], &opts, None, &mut out),
            Err(Error::NoAnimationFrames)
        ));
    }

    #[test]
    fn rejects_mismatched_frame_sizes() {
        let frames = vec![
            (solid_buf(4, 4, Rgba::opaque(255, 0, 0)), 100u32),
            (solid_buf(8, 8, Rgba::opaque(0, 255, 0)), 100u32),
        ];
        let opts = GifOptions::default();
        let mut out = Vec::new();
        assert!(matches!(
            encode_gif(&frames, &opts, None, &mut out),
            Err(Error::AnimFrameSizeMismatch { index: 1, .. })
        ));
    }

    #[test]
    fn single_frame_global_quantize_produces_valid_gif() {
        let frames = vec![(solid_buf(8, 8, Rgba::opaque(255, 0, 0)), 100u32)];
        let opts = GifOptions {
            palette_mode: PaletteMode::GlobalQuantize,
            ..Default::default()
        };
        let bytes = encode_to_bytes(&frames, &opts, None);
        // GIF89a magic bytes.
        assert_eq!(&bytes[..6], b"GIF89a");
    }

    #[test]
    fn multi_frame_per_frame_produces_valid_gif() {
        let frames = vec![
            (solid_buf(4, 4, Rgba::opaque(255, 0, 0)), 100u32),
            (solid_buf(4, 4, Rgba::opaque(0, 255, 0)), 100u32),
            (solid_buf(4, 4, Rgba::opaque(0, 0, 255)), 100u32),
        ];
        let opts = GifOptions {
            palette_mode: PaletteMode::PerFrameQuantize,
            ..Default::default()
        };
        let bytes = encode_to_bytes(&frames, &opts, None);
        assert_eq!(&bytes[..6], b"GIF89a");
    }

    #[test]
    fn existing_palette_mode_encodes_correctly() {
        let pal = two_color_palette();
        let frames = vec![(solid_buf(4, 4, Rgba::opaque(0, 0, 0)), 200u32)];
        let opts = GifOptions {
            palette_mode: PaletteMode::ExistingPalette,
            ..Default::default()
        };
        let bytes = encode_to_bytes(&frames, &opts, Some(&pal));
        assert_eq!(&bytes[..6], b"GIF89a");
    }

    #[test]
    fn existing_palette_too_large_returns_error() {
        let colors: Vec<Rgba> = (0u16..=256)
            .map(|i| Rgba::opaque((i % 256) as u8, 0, 0))
            .collect();
        let pal = Palette::from_colors(PaletteId::new(1), "big", colors);
        let frames = vec![(solid_buf(2, 2, Rgba::opaque(0, 0, 0)), 100u32)];
        let opts = GifOptions {
            palette_mode: PaletteMode::ExistingPalette,
            ..Default::default()
        };
        let mut out = Vec::new();
        assert!(matches!(
            encode_gif(&frames, &opts, Some(&pal), &mut out),
            Err(Error::PaletteExceedsGifMax { .. })
        ));
    }

    #[test]
    fn ms_to_centisec_rounds_up_below_10ms() {
        assert_eq!(ms_to_centisec(1), 1);
        assert_eq!(ms_to_centisec(9), 1);
        assert_eq!(ms_to_centisec(10), 1);
        assert_eq!(ms_to_centisec(100), 10);
        assert_eq!(ms_to_centisec(150), 15);
    }

    #[test]
    fn loop_count_infinite_maps_correctly() {
        assert!(matches!(
            to_gif_repeat(LoopCount::Infinite),
            Repeat::Infinite
        ));
        assert!(matches!(
            to_gif_repeat(LoopCount::Count(0)),
            Repeat::Infinite
        ));
    }

    #[test]
    fn loop_count_finite_maps_correctly() {
        assert!(matches!(
            to_gif_repeat(LoopCount::Count(3)),
            Repeat::Finite(3)
        ));
    }

    #[test]
    fn global_quantize_with_floyd_steinberg_produces_valid_gif() {
        let frames: Vec<(PixelBuffer, u32)> = (0..4)
            .map(|_| (solid_buf(8, 8, Rgba::opaque(128, 64, 192)), 100u32))
            .collect();
        let opts = GifOptions {
            palette_mode: PaletteMode::GlobalQuantize,
            dither: DitherMode::FloydSteinberg,
            ..Default::default()
        };
        let bytes = encode_to_bytes(&frames, &opts, None);
        assert_eq!(&bytes[..6], b"GIF89a");
    }
}
