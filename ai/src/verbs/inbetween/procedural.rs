//! Variance-rejected weighted averaging for raster inbetweening.
//!
//! Adapted from `OpenToonz` `toonz/sources/common/tvrender/tinbetween.cpp`
//! under BSD-3-Clause. See `THIRD_PARTY_NOTICES.md`.
//!
//! Toonz's `getAverage` (lines 21-54) and `getWeightedAverage` (lines
//! 56-98) reject outlier samples whose squared error from the mean
//! exceeds `range * variance`; the canonical `range` is 2.5σ. We apply
//! that idea per pixel per channel by sampling a 3×3 neighbourhood
//! from both frames, lerping each sample by `t`, and averaging the
//! survivors.

/// Default variance-rejection multiplier — matches Toonz's 2.5σ rule.
#[cfg(test)]
pub(super) const DEFAULT_VARIANCE_RANGE: f32 = 2.5;

/// Interpolates between two RGBA8 frames at parameter `t` using
/// variance-rejected weighted averaging of a 3×3 neighbourhood per
/// pixel.
///
/// Both input buffers must be tightly packed RGBA8 with exactly
/// `width * height * 4` bytes. `t` is the interpolation parameter in
/// `[0.0, 1.0]`: `t == 0` returns `frame_a` byte-for-byte; `t == 1`
/// returns `frame_b`. Edges of the 3×3 window are clamped at the
/// buffer boundary.
///
/// `variance_range` is the rejection multiplier; values at or below
/// zero disable rejection (every sample is accepted) so the output
/// degrades to the plain neighbourhood mean.
///
/// # Panics
///
/// Panics in debug builds if either buffer's length disagrees with
/// the expected byte count. Release builds silently fall back to the
/// shorter slice; the verb's `validate` is expected to catch this.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub(super) fn interpolate_frames(
    frame_a: &[u8],
    frame_b: &[u8],
    width: u32,
    height: u32,
    t: f32,
    variance_range: f32,
) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let pixels = w.saturating_mul(h);
    let expected = pixels.saturating_mul(4);
    debug_assert_eq!(frame_a.len(), expected, "frame_a length mismatch");
    debug_assert_eq!(frame_b.len(), expected, "frame_b length mismatch");

    let mut out = vec![0u8; expected];

    // Fast path: t at an endpoint copies the corresponding buffer
    // byte-for-byte. The 3×3 average would otherwise smear an
    // unchanged frame through rounding error.
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
                let v = sample_variance_rejected(
                    frame_a,
                    frame_b,
                    w,
                    h,
                    x,
                    y,
                    channel,
                    t,
                    variance_range,
                );
                let idx = (y * w + x) * 4 + channel;
                out[idx] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Returns the variance-rejected mean of the lerp samples drawn from
/// the 3×3 neighbourhood around `(x, y)` on `channel`.
#[allow(
    clippy::too_many_arguments,
    clippy::similar_names,
    clippy::many_single_char_names
)]
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
    // contribute fewer samples after clamping. Bound the per-sample
    // count to `u16` so we can convert losslessly to `f32`.
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

    // `variance_range * variance == 0` means every sample equals the
    // mean — the lerp produced uniform values, so the mean is the
    // answer.
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
    if accepted > 0 {
        accum / f32::from(accepted)
    } else {
        mean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn t05_solid_buffers_same_color_returns_same() {
        // 2x2 buffer of mid-grey opaque pixels.
        let a: Vec<u8> = std::iter::repeat_n([100u8, 100, 100, 255], 4)
            .flatten()
            .collect();
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
        // A 2x2 buffer means every pixel lives at a corner; the 3×3
        // window collapses to a 2×2 sample set after clamping. The
        // function must produce a sensible result without reading out
        // of bounds — the value is the variance-rejected mean of the
        // four corner channel values.
        let a: Vec<u8> = vec![
            10, 20, 30, 255, // (0,0)
            40, 50, 60, 255, // (1,0)
            70, 80, 90, 255, // (0,1)
            100, 110, 120, 255, // (1,1)
        ];
        let b = a.clone();
        let out = interpolate_frames(&a, &b, 2, 2, 0.5, 2.5);
        // Exactly 16 RGBA bytes returned.
        assert_eq!(out.len(), a.len());
        // Alpha is identical across all corners, so the mean is 255.
        for chunk in out.chunks_exact(4) {
            assert_eq!(chunk[3], 255, "alpha must round to 255");
        }
        // No panic, no out-of-bounds — assert that every channel
        // landed inside the input value range (the mean of bounded
        // samples cannot exceed the samples themselves).
        for chunk in out.chunks_exact(4) {
            assert!(chunk[0] >= 10 && chunk[0] <= 100);
            assert!(chunk[1] >= 20 && chunk[1] <= 110);
            assert!(chunk[2] >= 30 && chunk[2] <= 120);
        }
    }

    #[test]
    fn midpoint_lerp_of_uniform_solids_is_arithmetic_mean() {
        // A solid black frame against a solid white frame at t=0.5 has
        // all 3×3 samples equal to 127.5, which rounds to 128.
        let a: Vec<u8> = std::iter::repeat_n([0u8, 0, 0, 255], 4).flatten().collect();
        let b: Vec<u8> = std::iter::repeat_n([255u8, 255, 255, 255], 4)
            .flatten()
            .collect();
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
}
