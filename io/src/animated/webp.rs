//! Animated WebP encoder (stream S11).
//!
//! Encodes a sequence of composited RGBA frames into a single animated WebP
//! file. Two quality modes are available:
//!
//! - **Lossless** (default for pixel art) — exact colour reproduction, uses
//!   the VP8L lossless codec. Slightly larger than lossy but no colour
//!   quantization.
//! - **Lossy** — VP8 codec with configurable quality (`0`–`100`). Smaller
//!   files but introduces block artifacts; not recommended for pixel art.
//!
//! The `webp` crate wraps Google's `libwebp` C library (bundled at build
//! time via `libwebp-sys`; no system install required).

use webp::{AnimEncoder, AnimFrame, WebPConfig};

use pixhaus_core::canvas::PixelBuffer;

use crate::error::{Error, Result};

// ── Public types ──────────────────────────────────────────────────────────────

/// Options for animated WebP export.
#[derive(Clone, Debug)]
pub struct WebPOptions {
    /// Use the VP8L lossless codec. Recommended for pixel art.
    ///
    /// When `true` the `quality` field is ignored.
    pub lossless: bool,
    /// Encoding quality for lossy mode, in the range `[0.0, 100.0]`.
    ///
    /// Higher values produce larger files with fewer artefacts. Ignored when
    /// `lossless` is `true`. Default is `90.0`.
    pub quality: f32,
    /// Compression method, `0`–`6`. Higher values are slower but produce
    /// smaller files. Default is `4` (the libwebp default).
    pub method: i32,
    /// Number of times the animation loops. `0` means loop indefinitely.
    pub loop_count: i32,
}

impl Default for WebPOptions {
    fn default() -> Self {
        Self {
            lossless: true,
            quality: 90.0,
            method: 4,
            loop_count: 0,
        }
    }
}

// ── Encoder ───────────────────────────────────────────────────────────────────

/// Encode `frames` as an animated WebP, returning the encoded bytes.
///
/// `frames` is a slice of `(PixelBuffer, duration_ms)` pairs. All buffers must
/// share the same dimensions. Timestamps fed to libwebp are the cumulative
/// wall-clock position of each frame in milliseconds.
///
/// # Errors
///
/// - [`Error::NoAnimationFrames`] if `frames` is empty.
/// - [`Error::AnimFrameSizeMismatch`] if any buffer differs in size.
/// - [`Error::WebPEncode`] if `libwebp` reports an error.
pub fn encode_webp(frames: &[(PixelBuffer, u32)], options: &WebPOptions) -> Result<Vec<u8>> {
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

    let mut config = WebPConfig::new()
        .map_err(|e| Error::WebPEncode(format!("failed to create WebPConfig: {e:?}")))?;

    config.lossless = i32::from(options.lossless);
    config.quality = options.quality;
    config.method = options.method;

    // Pixel data vecs must outlive the AnimFrame slices.
    let rgba_bufs: Vec<Vec<u8>> = frames
        .iter()
        .map(|(buf, _)| buffer_to_rgba_vec(buf))
        .collect();

    let mut encoder = AnimEncoder::new(width, height, &config);
    encoder.set_loop_count(options.loop_count);

    let mut timestamp_ms: i32 = 0;
    for ((_, duration_ms), rgba) in frames.iter().zip(rgba_bufs.iter()) {
        let frame = AnimFrame::from_rgba(rgba, width, height, timestamp_ms);
        encoder.add_frame(frame);
        timestamp_ms = timestamp_ms.saturating_add(i32::try_from(*duration_ms).unwrap_or(i32::MAX));
    }

    let data = encoder
        .try_encode()
        .map_err(|e| Error::WebPEncode(format!("encoding failed: {e:?}")))?;

    Ok(data.to_vec())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pixhaus_core::canvas::PixelBuffer;
    use pixhaus_core::project::Rgba;

    fn solid_buf(w: u32, h: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(w, h, color).unwrap()
    }

    #[test]
    fn rejects_empty_frames() {
        assert!(matches!(
            encode_webp(&[], &WebPOptions::default()),
            Err(Error::NoAnimationFrames)
        ));
    }

    #[test]
    fn rejects_mismatched_sizes() {
        let frames = vec![
            (solid_buf(4, 4, Rgba::opaque(255, 0, 0)), 100u32),
            (solid_buf(8, 8, Rgba::opaque(0, 255, 0)), 100u32),
        ];
        assert!(matches!(
            encode_webp(&frames, &WebPOptions::default()),
            Err(Error::AnimFrameSizeMismatch { index: 1, .. })
        ));
    }

    #[test]
    fn single_frame_lossless_produces_riff_header() {
        let frames = vec![(solid_buf(8, 8, Rgba::opaque(100, 150, 200)), 100u32)];
        let bytes = encode_webp(
            &frames,
            &WebPOptions {
                lossless: true,
                ..Default::default()
            },
        )
        .unwrap();
        // Animated WebP files start with "RIFF".
        assert_eq!(&bytes[..4], b"RIFF");
    }

    #[test]
    fn multi_frame_lossless_produces_riff_header() {
        let frames: Vec<(PixelBuffer, u32)> = vec![
            (solid_buf(4, 4, Rgba::opaque(255, 0, 0)), 100),
            (solid_buf(4, 4, Rgba::opaque(0, 255, 0)), 150),
            (solid_buf(4, 4, Rgba::opaque(0, 0, 255)), 200),
        ];
        let bytes = encode_webp(&frames, &WebPOptions::default()).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
    }

    #[test]
    fn lossy_mode_produces_riff_header() {
        let frames = vec![(solid_buf(8, 8, Rgba::opaque(50, 100, 150)), 100u32)];
        let opts = WebPOptions {
            lossless: false,
            quality: 75.0,
            ..Default::default()
        };
        let bytes = encode_webp(&frames, &opts).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
    }
}
