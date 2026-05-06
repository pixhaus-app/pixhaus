//! Perspective (projective) warp.
//!
//! Computes a projective homography from the four corners of the source
//! image to four caller-supplied destination corners, then applies the
//! **inverse** mapping with bilinear interpolation to fill each output
//! pixel.
//!
//! **This operation is documented as non-pixel-perfect.** Bilinear
//! interpolation introduces sub-pixel blending that is incompatible
//! with the strict palette discipline of pixel art. Use it for
//! perspective correction of reference images or for prototype layout
//! work; not as a final asset operation.
//!
//! # Corner convention
//!
//! `corners` is interpreted as `[top-left, top-right, bottom-right,
//! bottom-left]` in destination canvas coordinates (pixels, origin
//! at top-left, Y increasing downward). The source corners are always
//! the four corners of the buffer at full resolution.
//!
//! # Homography
//!
//! The function solves for a 3×3 projective transform H such that
//! each source corner maps to the corresponding destination corner,
//! then inverts H. For each output pixel (dx, dy) the inverse H⁻¹
//! produces a fractional source coordinate that is sampled with
//! bilinear interpolation.
//!
//! The solver uses Gaussian elimination with partial pivoting on a
//! dense 8×8 system — accurate enough for editorial precision at
//! sprite resolutions.

use crate::canvas::buffer::PixelBuffer;

use super::error::{Error, Result};
use super::rotate::bilinear_sample;

/// Warps `buf` so that its four corners map to `dst_corners`.
///
/// `dst_corners` = `[top-left, top-right, bottom-right, bottom-left]`
/// in destination pixel coordinates.
///
/// The output has the same dimensions as `buf`. Pixels that warp
/// outside the source bounds become transparent.
///
/// # Errors
///
/// - [`Error::EmptyBuffer`] if `buf` is 0×0.
/// - [`Error::InvalidCornerCount`] if `dst_corners.len() != 4`.
#[allow(clippy::cast_precision_loss)]
pub fn perspective(buf: &PixelBuffer, dst_corners: &[(f32, f32)]) -> Result<PixelBuffer> {
    if buf.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    if dst_corners.len() != 4 {
        return Err(Error::InvalidCornerCount(dst_corners.len()));
    }

    let w = buf.width() as f32;
    let h = buf.height() as f32;

    // Source corners: top-left, top-right, bottom-right, bottom-left.
    let src_corners: [(f32, f32); 4] = [
        (0.0, 0.0),
        (w - 1.0, 0.0),
        (w - 1.0, h - 1.0),
        (0.0, h - 1.0),
    ];

    // Compute forward homography H: src → dst.
    let h_fwd = solve_homography(&src_corners, dst_corners)?;

    // Invert H to get H⁻¹: dst → src.
    let h_inv = invert_3x3(h_fwd)?;

    // For each output pixel apply H⁻¹ to find the source coordinate.
    let out_w = buf.width();
    let out_h = buf.height();
    let mut out = PixelBuffer::new(out_w, out_h)?;

    for dy in 0..out_h {
        for dx in 0..out_w {
            let (sx, sy) = apply_homography(&h_inv, dx as f32, dy as f32);
            out.set_pixel(dx, dy, bilinear_sample(buf, sx, sy));
        }
    }

    Ok(out)
}

// ──────────────────────────────────────────────────────────────────────────────
// Homography solver
// ──────────────────────────────────────────────────────────────────────────────

/// A 3×3 homography stored in row-major order.
type H3x3 = [[f64; 3]; 3];

/// Solves for the projective homography H such that, for each i,
/// `src[i]` maps to `dst[i]`. Returns H as a 3×3 row-major matrix.
///
/// The 8 degrees of freedom of the homography (h22 is fixed to 1) are
/// found by solving the 8×8 DLT system via Gaussian elimination with
/// partial pivoting.
fn solve_homography(src: &[(f32, f32); 4], dst: &[(f32, f32)]) -> Result<H3x3> {
    // Build the 8×9 augmented matrix [A | b].
    // For each correspondence (xi,yi) → (ui,vi):
    //   xi*h00 + yi*h01 + h02                       - xi*ui*h20 - yi*ui*h21 = ui
    //              xi*h10 + yi*h11 + h12 - xi*vi*h20 - yi*vi*h21 = vi
    //
    // Unknowns: [h00, h01, h02, h10, h11, h12, h20, h21]
    let mut a = [[0f64; 9]; 8];

    for (i, (&(xi, yi), &(ui, vi))) in src.iter().zip(dst.iter()).enumerate() {
        let xi = f64::from(xi);
        let yi = f64::from(yi);
        let ui = f64::from(ui);
        let vi = f64::from(vi);
        let row0 = i * 2;
        let row1 = i * 2 + 1;

        a[row0][0] = xi;
        a[row0][1] = yi;
        a[row0][2] = 1.0;
        a[row0][6] = -xi * ui;
        a[row0][7] = -yi * ui;
        a[row0][8] = ui;

        a[row1][3] = xi;
        a[row1][4] = yi;
        a[row1][5] = 1.0;
        a[row1][6] = -xi * vi;
        a[row1][7] = -yi * vi;
        a[row1][8] = vi;
    }

    // Solve via Gaussian elimination with partial pivoting.
    let h_vec = gauss_elim_8x8(&mut a)?;

    Ok([
        [h_vec[0], h_vec[1], h_vec[2]],
        [h_vec[3], h_vec[4], h_vec[5]],
        [h_vec[6], h_vec[7], 1.0],
    ])
}

/// Gaussian elimination with partial pivoting on the augmented 8×9 matrix.
/// Returns the 8-element solution vector.
#[allow(clippy::needless_range_loop)]
fn gauss_elim_8x8(a: &mut [[f64; 9]; 8]) -> Result<[f64; 8]> {
    const N: usize = 8;

    for col in 0..N {
        // Partial pivot: find the row with the largest absolute value in this column.
        let pivot_row = (col..N)
            .max_by(|&r1, &r2| {
                a[r1][col]
                    .abs()
                    .partial_cmp(&a[r2][col].abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(col);

        a.swap(col, pivot_row);

        let pivot = a[col][col];
        // Singular or near-singular system — degenerate corner configuration.
        if pivot.abs() < 1e-10 {
            return Err(Error::EmptyBuffer); // reuse closest error; add a Singular variant if needed
        }

        // Eliminate below.
        for row in (col + 1)..N {
            let factor = a[row][col] / pivot;
            for k in col..=N {
                a[row][k] -= factor * a[col][k];
            }
        }
    }

    // Back-substitution.
    let mut x = [0f64; N];
    for row in (0..N).rev() {
        x[row] = a[row][N];
        for k in (row + 1)..N {
            x[row] -= a[row][k] * x[k];
        }
        x[row] /= a[row][row];
    }

    Ok(x)
}

/// Computes the inverse of a 3×3 matrix using the adjugate method.
/// Returns the inverse scaled so that M[2][2] == 1.
fn invert_3x3(m: H3x3) -> Result<H3x3> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);

    if det.abs() < 1e-10 {
        return Err(Error::EmptyBuffer); // degenerate, same as singular homography
    }

    let inv_det = 1.0 / det;
    let inv = [
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * inv_det,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * inv_det,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * inv_det,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * inv_det,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * inv_det,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * inv_det,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * inv_det,
        ],
    ];

    Ok(inv)
}

/// Applies the homography `h` to point `(x, y)` and returns the
/// resulting `(x', y')` in the target coordinate system.
#[allow(clippy::cast_possible_truncation)]
fn apply_homography(h: &H3x3, x: f32, y: f32) -> (f32, f32) {
    let xf = f64::from(x);
    let yf = f64::from(y);
    let w = h[2][0] * xf + h[2][1] * yf + h[2][2];
    let ox = (h[0][0] * xf + h[0][1] * yf + h[0][2]) / w;
    let oy = (h[1][0] * xf + h[1][1] * yf + h[1][2]) / w;
    (ox as f32, oy as f32)
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

    #[test]
    fn identity_corners_is_near_identity() {
        // When the destination corners match the source corners exactly,
        // the warp should reproduce the source image (modulo bilinear rounding).
        let mut buf = PixelBuffer::new(8, 8).unwrap();
        buf.set_pixel(4, 4, red());

        let w = 7.0f32;
        let h = 7.0f32;
        let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
        let out = perspective(&buf, &corners).unwrap();

        assert_eq!(out.width(), 8);
        assert_eq!(out.height(), 8);
        // Centre pixel should survive near-identity warp.
        assert_eq!(out.pixel(4, 4), Some(red()));
    }

    #[test]
    fn wrong_corner_count_errors() {
        let buf = PixelBuffer::new(4, 4).unwrap();
        let three: &[(f32, f32)] = &[(0.0, 0.0), (3.0, 0.0), (3.0, 3.0)];
        assert!(matches!(
            perspective(&buf, three),
            Err(Error::InvalidCornerCount(3))
        ));
    }

    #[test]
    fn empty_buffer_errors() {
        let buf = PixelBuffer::empty();
        let corners = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        assert_eq!(perspective(&buf, &corners), Err(Error::EmptyBuffer));
    }

    #[test]
    fn preserves_dimensions() {
        let buf = PixelBuffer::new(16, 12).unwrap();
        let corners = [(1.0, 1.0), (14.0, 0.0), (15.0, 11.0), (0.0, 10.0)];
        let out = perspective(&buf, &corners).unwrap();
        assert_eq!(out.width(), 16);
        assert_eq!(out.height(), 12);
    }

    #[test]
    fn gauss_elim_solves_identity_case() {
        // Build the DLT system for an identity homography.
        // Source and destination corners are the same (4×4 image).
        let src: [(f32, f32); 4] = [(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)];
        let dst = src;
        let h = solve_homography(&src, &dst).unwrap();
        // H should be near the identity scaled so h22 = 1.
        assert!((h[0][0] - 1.0).abs() < 1e-6, "h00 = {}", h[0][0]);
        assert!((h[1][1] - 1.0).abs() < 1e-6, "h11 = {}", h[1][1]);
        assert!((h[2][2] - 1.0).abs() < 1e-6, "h22 = {}", h[2][2]);
        assert!(h[0][1].abs() < 1e-6, "h01 = {}", h[0][1]);
        assert!(h[1][0].abs() < 1e-6, "h10 = {}", h[1][0]);
    }
}
