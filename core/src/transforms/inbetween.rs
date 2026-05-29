//! Procedural raster inbetweening (frame interpolation).
//!
//! Adapted from `OpenToonz` `toonz/sources/common/tvrender/tinbetween.cpp`
//! under BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.
//!
//! Toonz's `getAverage` and `getWeightedAverage` reject outlier samples
//! whose squared error from the mean exceeds `range * variance`; the
//! canonical `range` is 2.5σ. We apply that idea per pixel per channel:
//! sample a 3×3 neighbourhood from both frames, lerp each sample by `t`,
//! and average the survivors. The variance rejection keeps a sharp edge
//! moving cleanly instead of smearing into a grey halo, which a plain
//! cross-fade would do at pixel-art resolution.
//!
//! Two seams are deliberate. First, the byte-level [`interpolate_frames`]
//! takes tightly-packed RGBA8 `Vec<u8>` slices with explicit dimensions
//! and is infallible — it never panics on a short buffer, returning a
//! blank frame of the expected size instead. Second, [`tween`] wraps it
//! over [`PixelBuffer`] and is where a model-based backend (RIFE/FILM)
//! would slot in behind the same signature later, returning the same
//! `Vec<PixelBuffer>`; that is a deferred follow-up, not built here.

use thiserror::Error;

use crate::canvas::buffer::{PixelBuffer, RGBA_BYTES_PER_PIXEL_U32};

/// Default variance-rejection multiplier — matches Toonz's 2.5σ rule.
pub const DEFAULT_VARIANCE_RANGE: f32 = 2.5;

/// Errors raised by [`tween`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InbetweenError {
    /// The two source frames have different dimensions; interpolation
    /// needs a shared pixel grid.
    #[error("frame size mismatch: a is {a_width}×{a_height}, b is {b_width}×{b_height}")]
    SizeMismatch {
        /// Width of frame `a`.
        a_width: u32,
        /// Height of frame `a`.
        a_height: u32,
        /// Width of frame `b`.
        b_width: u32,
        /// Height of frame `b`.
        b_height: u32,
    },

    /// One of the source frames has zero width or height.
    #[error("source frame is empty (0×0)")]
    EmptyFrame,

    /// A count of zero was requested — there is nothing to generate.
    #[error("inbetween count must be at least 1")]
    ZeroCount,

    /// Forwarded pixel-buffer allocation error from rebuilding an output
    /// [`PixelBuffer`].
    #[error("pixel buffer error: {0}")]
    Buffer(#[from] crate::canvas::error::Error),
}

/// Interpolates between two RGBA8 frames at parameter `t` using
/// variance-rejected weighted averaging of a 3×3 neighbourhood per pixel.
///
/// Both input buffers must be tightly packed RGBA8 with exactly
/// `width * height * 4` bytes. `t` is the interpolation parameter in
/// `[0.0, 1.0]`: `t == 0` returns `frame_a` byte-for-byte; `t == 1`
/// returns `frame_b`. Edges of the 3×3 window are clamped at the buffer
/// boundary.
///
/// `variance_range` is the rejection multiplier; values at or below zero
/// disable rejection (every sample is accepted) so the output degrades to
/// the plain neighbourhood mean.
///
/// # Length handling
///
/// If either buffer is shorter than the expected byte count
/// (`width * height * 4`), this returns an all-zero frame of the expected
/// size rather than indexing out of bounds. The caller is expected to pass
/// matching, tightly-packed buffers, so the guard is a safety net, not a
/// routine path. The function is infallible and returns the bytes directly.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
pub fn interpolate_frames(frame_a: &[u8], frame_b: &[u8], width: u32, height: u32, t: f32, variance_range: f32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let pixels = w.saturating_mul(h);
    let expected = pixels.saturating_mul(4);

    // Safety net: a short buffer would index out of bounds in the main
    // loop. Return a blank frame of the expected size instead.
    if frame_a.len() < expected || frame_b.len() < expected {
        return vec![0u8; expected];
    }

    let mut out = vec![0u8; expected];

    // Fast path: t at an endpoint copies the corresponding buffer
    // byte-for-byte. The 3×3 average would otherwise smear an unchanged
    // frame through rounding error.
    if t <= 0.0 {
        let n = frame_a.len().min(expected);
        out[..n].copy_from_slice(&frame_a[..n]);
        return out;
    }
    if t >= 1.0 {
        let n = frame_b.len().min(expected);
        out[..n].copy_from_slice(&frame_b[..n]);
        return out;
    }

    for y in 0..h {
        for x in 0..w {
            for channel in 0..4 {
                let v = sample_variance_rejected(frame_a, frame_b, w, h, x, y, channel, t, variance_range);
                let idx = (y * w + x) * 4 + channel;
                out[idx] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Returns the variance-rejected mean of the lerp samples drawn from the
/// 3×3 neighbourhood around `(x, y)` on `channel`.
#[allow(clippy::too_many_arguments, clippy::similar_names, clippy::many_single_char_names)]
fn sample_variance_rejected(
    frame_a: &[u8],
    frame_b: &[u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    channel: usize,
    t: f32,
    variance_range: f32,
) -> f32 {
    // Capacity 9 covers the full 3×3 neighbourhood; corners and edges
    // contribute fewer samples after clamping. Bound the per-sample count
    // to `u16` so we can convert losslessly to `f32`.
    let mut samples: [f32; 9] = [0.0; 9];
    let mut n: u16 = 0;

    let y_lo = y.saturating_sub(1);
    let y_hi = (y + 1).min(height - 1);
    let x_lo = x.saturating_sub(1);
    let x_hi = (x + 1).min(width - 1);

    for ny in y_lo..=y_hi {
        for nx in x_lo..=x_hi {
            let idx = (ny * width + nx) * 4 + channel;
            let va = f32::from(frame_a[idx]);
            let vb = f32::from(frame_b[idx]);
            samples[usize::from(n)] = va * (1.0 - t) + vb * t;
            n += 1;
        }
    }

    if n == 0 {
        // Unreachable for any non-empty image, but defend the divide.
        return 0.0;
    }
    if n == 1 {
        return samples[0];
    }

    let slice = &samples[..usize::from(n)];
    let inv_n = 1.0 / f32::from(n);
    let mean: f32 = slice.iter().sum::<f32>() * inv_n;
    let variance: f32 = slice
        .iter()
        .map(|v| {
            let d = v - mean;
            d * d
        })
        .sum::<f32>()
        * inv_n;

    // `variance_range * variance == 0` means every sample equals the mean —
    // the lerp produced uniform values, so the mean is the answer.
    let cutoff = variance_range * variance;
    if cutoff <= 0.0 {
        return mean;
    }

    let mut accum = 0.0f32;
    let mut accepted: u16 = 0;
    for &s in slice {
        let d = s - mean;
        if d * d <= cutoff {
            accum += s;
            accepted += 1;
        }
    }
    if accepted > 0 { accum / f32::from(accepted) } else { mean }
}

/// Generates `count` evenly spaced inbetweens between `a` and `b`.
///
/// The returned buffers are the interior frames only: for `count == 1` the
/// single output sits at `t = 0.5`; for `count == 3` they fall at
/// `t = 0.25, 0.5, 0.75`. The endpoints `a` and `b` are not included — the
/// caller already has them. Each output keeps the source dimensions and is
/// tightly packed.
///
/// This is CPU-bound: the shell runs it under `spawn_blocking`, bounded by
/// the frame region rather than the whole canvas.
///
/// A model-based backend tween (RIFE/FILM) would slot in behind this same
/// signature later as an async variant; the procedural path ships now.
///
/// # Errors
///
/// - [`InbetweenError::EmptyFrame`] if either frame is `0×0`.
/// - [`InbetweenError::SizeMismatch`] if the two frames differ in size.
/// - [`InbetweenError::ZeroCount`] if `count == 0`.
/// - [`InbetweenError::Buffer`] if rebuilding an output buffer fails.
#[allow(clippy::cast_precision_loss)]
pub fn tween(a: &PixelBuffer, b: &PixelBuffer, count: usize) -> Result<Vec<PixelBuffer>, InbetweenError> {
    tween_with_variance(a, b, count, DEFAULT_VARIANCE_RANGE)
}

/// [`tween`] with an explicit variance-rejection multiplier. Pass
/// [`DEFAULT_VARIANCE_RANGE`] for the canonical 2.5σ behaviour, or `<= 0`
/// to fall back to a plain neighbourhood mean.
///
/// # Errors
///
/// Same as [`tween`].
#[allow(clippy::cast_precision_loss)]
pub fn tween_with_variance(a: &PixelBuffer, b: &PixelBuffer, count: usize, variance_range: f32) -> Result<Vec<PixelBuffer>, InbetweenError> {
    if a.is_empty() || b.is_empty() {
        return Err(InbetweenError::EmptyFrame);
    }
    if a.width() != b.width() || a.height() != b.height() {
        return Err(InbetweenError::SizeMismatch {
            a_width: a.width(),
            a_height: a.height(),
            b_width: b.width(),
            b_height: b.height(),
        });
    }
    if count == 0 {
        return Err(InbetweenError::ZeroCount);
    }

    let width = a.width();
    let height = a.height();
    let bytes_a = packed_bytes(a);
    let bytes_b = packed_bytes(b);
    let stride = width.saturating_mul(RGBA_BYTES_PER_PIXEL_U32);

    let steps = (count + 1) as f32;
    (1..=count)
        .map(|i| {
            let t = i as f32 / steps;
            let bytes = interpolate_frames(&bytes_a, &bytes_b, width, height, t, variance_range);
            PixelBuffer::from_raw(width, height, stride, bytes).map_err(InbetweenError::from)
        })
        .collect()
}

/// Returns the buffer's pixels as a tightly-packed RGBA8 `Vec<u8>`
/// (`width * height * 4` bytes), stripping any SIMD row padding so the
/// byte-level interpolator can index by `(y * width + x) * 4`.
fn packed_bytes(buf: &PixelBuffer) -> Vec<u8> {
    let width = buf.width() as usize;
    let height = buf.height() as usize;
    let mut out = Vec::with_capacity(width * height * 4);
    for y in 0..buf.height() {
        if let Some(row) = buf.row(y) {
            out.extend_from_slice(row);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Rgba;
    use proptest::prelude::*;

    #[test]
    fn t0_returns_frame_a_byte_for_byte() {
        let a = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let b = vec![0, 0, 255, 255, 0, 255, 0, 255];
        let out = interpolate_frames(&a, &b, 2, 1, 0.0, 2.5);
        assert_eq!(out, a);
    }

    #[test]
    fn t1_returns_frame_b_byte_for_byte() {
        let a = vec![255, 0, 0, 255, 0, 255, 0, 255];
        let b = vec![0, 0, 255, 255, 0, 255, 0, 255];
        let out = interpolate_frames(&a, &b, 2, 1, 1.0, 2.5);
        assert_eq!(out, b);
    }

    #[test]
    fn short_buffer_returns_blank_frame_without_panicking() {
        // Buffers shorter than width*height*4 must not index out of bounds;
        // we return an all-zero frame of the expected size in both debug and
        // release. t is mid-range so the main loop runs.
        let a = vec![0u8; 4]; // claims 2x2 (16 bytes) but only has 4
        let b = vec![0u8; 4];
        let out = interpolate_frames(&a, &b, 2, 2, 0.5, 2.5);
        assert_eq!(out, vec![0u8; 16]);
    }

    #[test]
    fn t05_solid_buffers_same_color_returns_same() {
        // 2x2 buffer of mid-grey opaque pixels.
        let a: Vec<u8> = std::iter::repeat_n([100u8, 100, 100, 255], 4).flatten().collect();
        let b = a.clone();
        let out = interpolate_frames(&a, &b, 2, 2, 0.5, 2.5);
        assert_eq!(out, a);
    }

    #[test]
    fn deterministic() {
        let a = vec![10, 20, 30, 255, 40, 50, 60, 255];
        let b = vec![60, 50, 40, 255, 30, 20, 10, 255];
        let r1 = interpolate_frames(&a, &b, 2, 1, 0.5, 2.5);
        let r2 = interpolate_frames(&a, &b, 2, 1, 0.5, 2.5);
        assert_eq!(r1, r2);
    }

    #[test]
    fn clamps_at_edges() {
        // A 2x2 buffer means every pixel lives at a corner; the 3×3 window
        // collapses to a 2×2 sample set after clamping. The function must
        // produce a sensible result without reading out of bounds.
        let a: Vec<u8> = vec![
            10, 20, 30, 255, // (0,0)
            40, 50, 60, 255, // (1,0)
            70, 80, 90, 255, // (0,1)
            100, 110, 120, 255, // (1,1)
        ];
        let b = a.clone();
        let out = interpolate_frames(&a, &b, 2, 2, 0.5, 2.5);
        assert_eq!(out.len(), a.len());
        for chunk in out.chunks_exact(4) {
            assert_eq!(chunk[3], 255, "alpha must round to 255");
        }
        for chunk in out.chunks_exact(4) {
            assert!(chunk[0] >= 10 && chunk[0] <= 100);
            assert!(chunk[1] >= 20 && chunk[1] <= 110);
            assert!(chunk[2] >= 30 && chunk[2] <= 120);
        }
    }

    #[test]
    fn midpoint_lerp_of_uniform_solids_is_arithmetic_mean() {
        // A solid black frame against a solid white frame at t=0.5 has all 3×3
        // samples equal to 127.5, which rounds to 128.
        let a: Vec<u8> = std::iter::repeat_n([0u8, 0, 0, 255], 4).flatten().collect();
        let b: Vec<u8> = std::iter::repeat_n([255u8, 255, 255, 255], 4).flatten().collect();
        let out = interpolate_frames(&a, &b, 2, 2, 0.5, 2.5);
        for chunk in out.chunks_exact(4) {
            assert_eq!(chunk[0], 128);
            assert_eq!(chunk[1], 128);
            assert_eq!(chunk[2], 128);
            assert_eq!(chunk[3], 255);
        }
    }

    #[test]
    fn default_variance_range_matches_toonz_2_5_sigma() {
        assert!((DEFAULT_VARIANCE_RANGE - 2.5).abs() < f32::EPSILON);
    }

    fn solid(width: u32, height: u32, color: Rgba) -> PixelBuffer {
        PixelBuffer::filled(width, height, color).expect("solid buffer")
    }

    #[test]
    fn tween_generates_count_evenly_spaced_frames() {
        let a = solid(2, 2, Rgba::opaque(0, 0, 0));
        let b = solid(2, 2, Rgba::opaque(255, 255, 255));
        let frames = tween(&a, &b, 3).expect("tween");
        assert_eq!(frames.len(), 3);
        // t = 0.25, 0.5, 0.75 over uniform solids → 64, 128, 191.
        let expected = [64u8, 128, 191];
        for (frame, &exp) in frames.iter().zip(expected.iter()) {
            assert_eq!(frame.width(), 2);
            assert_eq!(frame.height(), 2);
            let px = frame.pixel(0, 0).expect("pixel");
            assert_eq!(px.r, exp, "channel value at this step");
            assert_eq!(px.a, 255);
        }
    }

    #[test]
    fn tween_single_inbetween_is_midpoint() {
        let a = solid(3, 3, Rgba::opaque(0, 0, 0));
        let b = solid(3, 3, Rgba::opaque(200, 100, 50));
        let frames = tween(&a, &b, 1).expect("tween");
        assert_eq!(frames.len(), 1);
        let px = frames[0].pixel(1, 1).expect("pixel");
        // Uniform solids → arithmetic mean at t=0.5: 100, 50, 25.
        assert_eq!((px.r, px.g, px.b), (100, 50, 25));
    }

    #[test]
    fn tween_rejects_size_mismatch() {
        let a = solid(2, 2, Rgba::opaque(0, 0, 0));
        let b = solid(3, 2, Rgba::opaque(0, 0, 0));
        let err = tween(&a, &b, 1).unwrap_err();
        assert_eq!(
            err,
            InbetweenError::SizeMismatch {
                a_width: 2,
                a_height: 2,
                b_width: 3,
                b_height: 2,
            }
        );
    }

    #[test]
    fn tween_rejects_empty_frame() {
        let a = PixelBuffer::empty();
        let b = solid(2, 2, Rgba::opaque(0, 0, 0));
        assert_eq!(tween(&a, &b, 1).unwrap_err(), InbetweenError::EmptyFrame);
    }

    #[test]
    fn tween_rejects_zero_count() {
        let a = solid(2, 2, Rgba::opaque(0, 0, 0));
        let b = solid(2, 2, Rgba::opaque(255, 255, 255));
        assert_eq!(tween(&a, &b, 0).unwrap_err(), InbetweenError::ZeroCount);
    }

    #[test]
    fn tween_handles_padded_stride() {
        // A buffer carrying SIMD padding must pack down correctly so the
        // byte-level interpolator indexes the right neighbours. With a == b the
        // variance is zero, so each output pixel is the plain mean of its 3×3
        // (here 1×2 after edge clamping) neighbourhood. Reading the padding
        // bytes as pixels would skew these means, so a correct strip is what
        // makes the assertion hold.
        let packed: Vec<u8> = vec![
            10, 20, 30, 255, // (0,0)
            40, 50, 60, 255, // (1,0)
        ];
        let mut padded = vec![0u8; 24]; // stride 24 for width 2 (min 8)
        padded[0..8].copy_from_slice(&packed);
        let a = PixelBuffer::from_raw(2, 1, 24, padded).expect("padded buffer");
        let b = a.clone();
        let frames = tween(&a, &b, 1).expect("tween");
        // Both pixels see the same two-pixel neighbourhood: mean of 10 and 40,
        // 20 and 50, 30 and 60.
        assert_eq!(frames[0].pixel(0, 0), Some(Rgba::new(25, 35, 45, 255)));
        assert_eq!(frames[0].pixel(1, 0), Some(Rgba::new(25, 35, 45, 255)));
    }

    proptest! {
        // Each interpolated channel must land within the min/max of the two
        // source frames' values for that channel — the variance-rejected mean
        // of lerped samples cannot escape the convex hull of its inputs, and
        // every lerp sample lies on the segment between an `a` and a `b` value.
        #[test]
        fn interpolated_channels_stay_within_source_bounds(
            a in proptest::collection::vec(any::<u8>(), 16),
            b in proptest::collection::vec(any::<u8>(), 16),
            t_milli in 1u16..=999,
        ) {
            let t = f32::from(t_milli) / 1000.0;
            let out = interpolate_frames(&a, &b, 2, 2, t, DEFAULT_VARIANCE_RANGE);
            prop_assert_eq!(out.len(), 16);
            for channel in 0..4 {
                // The 3×3 neighbourhood of a 2×2 grid is the whole grid, so the
                // bounds are the per-channel min/max across all four pixels of
                // both frames.
                let mut lo = u8::MAX;
                let mut hi = u8::MIN;
                for pixel in 0..4 {
                    let idx = pixel * 4 + channel;
                    lo = lo.min(a[idx]).min(b[idx]);
                    hi = hi.max(a[idx]).max(b[idx]);
                }
                for pixel in 0..4 {
                    let idx = pixel * 4 + channel;
                    prop_assert!(out[idx] >= lo, "channel {} pixel {}: {} < lo {}", channel, pixel, out[idx], lo);
                    prop_assert!(out[idx] <= hi, "channel {} pixel {}: {} > hi {}", channel, pixel, out[idx], hi);
                }
            }
        }
    }
}
